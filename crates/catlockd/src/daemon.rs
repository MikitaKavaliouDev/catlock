//! Daemon core loop (M1a+): evdev read loop with lock/unlock, deadman, hold
//! (optional), built-in Esc panic unlock, VT watchdog, and re-lock cooldown.

use std::io::ErrorKind;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};

use catlock_core::{Config, Matcher};
use catlock_input::Keyboard;
use evdev::EventSummary;
use tracing::{debug, info, warn};

const KEY_POWER: u16 = 116; // KEY_POWER
const KEY_ESC: u16 = 1; // KEY_ESC
const ESC_PANIC_MS: u64 = 2000;
const RELOCK_COOLDOWN_MS: u64 = 1500;

pub struct Daemon {
    config: Config,
    keyboards: Vec<Keyboard>,
    matcher: Matcher,
    locked: bool,
    deadman_deadline: Option<Instant>,
    hold_deadline: Option<Instant>,
    hold_active: bool,
    esc_deadline: Option<Instant>,
    lock_blocked_until: Option<Instant>,
    initial_vt: Option<String>,
    debug_keys: bool,
    dry_run: bool,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Daemon {
    pub fn new(
        config: Config,
        debug_keys: bool,
        dry_run: bool,
        stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        let keyboards = catlock_input::discover();
        info!(count = keyboards.len(), "discovered keyboards");
        for kb in &keyboards {
            info!(path = %kb.path.display(), name = %kb.name, "keyboard");
        }
        let lock = catlock_core::Chord::parse(&config.shortcuts.lock).expect("valid lock chord");
        let unlock =
            catlock_core::Chord::parse(&config.shortcuts.unlock).expect("valid unlock chord");
        let matcher = Matcher::new(lock, unlock);
        let initial_vt = read_active_vt();
        info!(?initial_vt, "starting VT");
        Self {
            config,
            keyboards,
            matcher,
            locked: false,
            deadman_deadline: None,
            hold_deadline: None,
            hold_active: false,
            esc_deadline: None,
            lock_blocked_until: None,
            initial_vt,
            debug_keys,
            dry_run,
            stop,
        }
    }

    pub fn run(&mut self) {
        for kb in &mut self.keyboards {
            if let Err(e) = kb.device.set_nonblocking(true) {
                warn!(path = %kb.path.display(), error = %e, "set_nonblocking failed");
            }
        }

        info!(
            lock = %self.config.shortcuts.lock,
            unlock = %self.config.shortcuts.unlock,
            hold_ms = self.config.shortcuts.hold_to_unlock_ms,
            deadman_s = self.config.behaviour.auto_unlock_after_s,
            dry_run = self.dry_run,
            "daemon running"
        );

        while !self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            // Poll (raw fds to avoid borrow/lifetime issues).
            let mut pollfds: Vec<nix::libc::pollfd> = self
                .keyboards
                .iter()
                .map(|kb| nix::libc::pollfd {
                    fd: kb.device.as_raw_fd(),
                    events: nix::libc::POLLIN,
                    revents: 0,
                })
                .collect();
            unsafe {
                nix::libc::poll(pollfds.as_mut_ptr(), pollfds.len() as _, 50);
            }

            // Read ready devices into a buffer first.
            let mut collected: Vec<(u16, i32)> = Vec::new();
            let mut read_errors: Vec<(usize, String)> = Vec::new();
            for (i, kb) in self.keyboards.iter_mut().enumerate() {
                let ready = pollfds
                    .get(i)
                    .map(|p| p.revents & nix::libc::POLLIN != 0)
                    .unwrap_or(false);
                if !ready {
                    continue;
                }
                match kb.device.fetch_events() {
                    Ok(events) => {
                        for ev in events {
                            if let EventSummary::Key(_, code, value) = ev.destructure() {
                                collected.push((code.0, value));
                            }
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                    Err(e) => {
                        read_errors.push((i, e.to_string()));
                    }
                }
            }
            for (i, e) in read_errors {
                warn!(idx = i, error = %e, "read error");
            }

            let mut force_unlock = false;
            let mut actions: Vec<catlock_core::Action> = Vec::new();
            for (code, value) in collected {
                if self.debug_keys {
                    debug!(code, value, "key");
                }
                if code == KEY_POWER && value == 1 && self.locked {
                    force_unlock = true;
                    continue;
                }
                // Built-in panic: hold Esc for 2s while locked.
                if code == KEY_ESC {
                    if self.locked {
                        if value == 1 {
                            self.esc_deadline = Some(Instant::now() + Duration::from_millis(ESC_PANIC_MS));
                            info!("Esc panic timer started");
                        } else if value == 0 {
                            if self.esc_deadline.take().is_some() {
                                info!("Esc panic timer cancelled");
                            }
                        }
                    }
                    continue;
                }
                let a = self.matcher.on_key(code, value);
                if a != catlock_core::Action::None {
                    actions.push(a);
                }
            }
            if force_unlock {
                info!("power key -> force unlock");
                self.unlock("power");
            }

            for a in actions {
                match a {
                    catlock_core::Action::LockPressed => {
                        let now = Instant::now();
                        if self.locked {
                            continue;
                        }
                        if let Some(until) = self.lock_blocked_until {
                            if now < until {
                                debug!("lock blocked by cooldown");
                                continue;
                            }
                        }
                        self.lock();
                    }
                    catlock_core::Action::UnlockPressed => {
                        if !self.locked {
                            continue;
                        }
                        if self.config.shortcuts.hold_to_unlock_ms == 0 {
                            self.unlock("chord");
                        } else if !self.hold_active {
                            self.hold_active = true;
                            self.hold_deadline = Some(
                                Instant::now()
                                    + Duration::from_millis(self.config.shortcuts.hold_to_unlock_ms),
                            );
                            info!("unlock hold started");
                        }
                    }
                    catlock_core::Action::UnlockReleased => {
                        if self.hold_active {
                            self.hold_active = false;
                            self.hold_deadline = None;
                            info!("unlock hold cancelled");
                        }
                    }
                    catlock_core::Action::None => {}
                }
            }

            // Timers.
            if self.locked {
                if self.config.shortcuts.hold_to_unlock_ms > 0 && self.hold_active {
                    if let Some(dl) = self.hold_deadline {
                        if Instant::now() >= dl {
                            info!("hold complete -> unlock");
                            self.unlock("hold");
                        }
                    }
                }
                if let Some(dl) = self.esc_deadline {
                    if Instant::now() >= dl {
                        info!("Esc panic -> unlock");
                        self.unlock("esc-panic");
                    }
                }
                if let Some(dl) = self.deadman_deadline {
                    if Instant::now() >= dl {
                        info!("deadman expired -> unlock");
                        self.unlock("deadman");
                    }
                }
            }

            // VT watchdog: unlock if the active VT changed.
            if self.locked {
                if let Some(current) = read_active_vt() {
                    if let Some(initial) = &self.initial_vt {
                        if current != *initial {
                            info!(from = %initial, to = %current, "VT changed -> unlock");
                            self.unlock("vt-change");
                        }
                    }
                }
            }
        }

        if self.locked {
            self.unlock("shutdown");
        }
        info!("daemon stopped");
    }

    fn lock(&mut self) {
        if self.dry_run {
            info!("WOULD LOCK (dry-run: keyboard stays live)");
        } else {
            info!("LOCK");
            for kb in &mut self.keyboards {
                if let Err(e) = kb.device.grab() {
                    warn!(path = %kb.path.display(), error = %e, "grab failed");
                }
            }
        }
        self.locked = true;
        self.matcher.set_locked(true);
        self.deadman_deadline = if self.config.behaviour.auto_unlock_after_s > 0 {
            Some(Instant::now() + Duration::from_secs(self.config.behaviour.auto_unlock_after_s))
        } else {
            None
        };
        self.hold_active = false;
        self.hold_deadline = None;
        self.esc_deadline = None;
    }

    fn unlock(&mut self, reason: &str) {
        if self.dry_run {
            info!(reason, "WOULD UNLOCK (dry-run)");
        } else {
            info!(reason, "UNLOCK");
            for kb in &mut self.keyboards {
                let _ = kb.device.ungrab();
            }
        }
        self.locked = false;
        self.matcher.set_locked(false);
        self.matcher.reset();
        self.deadman_deadline = None;
        self.hold_active = false;
        self.hold_deadline = None;
        self.esc_deadline = None;
        // Cooldown: ignore lock chord briefly after an (auto) unlock.
        self.lock_blocked_until = Some(Instant::now() + Duration::from_millis(RELOCK_COOLDOWN_MS));
    }
}

pub fn run_daemon(
    config: Config,
    debug_keys: bool,
    dry_run: bool,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let mut d = Daemon::new(config, debug_keys, dry_run, stop);
    d.run();
}

fn read_active_vt() -> Option<String> {
    std::fs::read_to_string("/sys/class/tty/tty0/active")
        .ok()
        .map(|s| s.trim().to_string())
}
