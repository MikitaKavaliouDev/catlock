//! Daemon core loop for M1a.
//!
//! Synchronous, single-threaded poll loop. Grabs on lock, ungrabs on unlock.
//! No D-Bus, no udev hotplug, no logind yet (M1b/M1c).

use std::io::ErrorKind;
use std::os::unix::io::AsRawFd;
use std::time::{Duration, Instant};

use catlock_core::{Config, Matcher};
use catlock_input::Keyboard;
use evdev::EventSummary;
// poll via nix::libc::poll
use tracing::{info, warn};

const KEY_POWER: u16 = 116; // KEY_POWER

pub struct Daemon {
    config: Config,
    keyboards: Vec<Keyboard>,
    matcher: Matcher,
    locked: bool,
    deadman_deadline: Option<Instant>,
    hold_deadline: Option<Instant>,
    hold_active: bool,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Daemon {
    pub fn new(config: Config, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) -> Self {
        let keyboards = catlock_input::discover();
        info!(count = keyboards.len(), "discovered keyboards");
        for kb in &keyboards {
            info!(path = %kb.path.display(), name = %kb.name, "keyboard");
        }
        let lock = catlock_core::Chord::parse(&config.shortcuts.lock).expect("valid lock chord");
        let unlock = catlock_core::Chord::parse(&config.shortcuts.unlock).expect("valid unlock chord");
        let matcher = Matcher::new(lock, unlock);
        Self {
            config,
            keyboards,
            matcher,
            locked: false,
            deadman_deadline: None,
            hold_deadline: None,
            hold_active: false,
            stop,
        }
    }

    pub fn run(&mut self) {
        // Non-blocking reads on every device.
        for kb in &mut self.keyboards {
            if let Err(e) = kb.device.set_nonblocking(true) {
                warn!(path = %kb.path.display(), error = %e, "set_nonblocking failed");
            }
        }

        info!("daemon running. Lock chord: {}", self.config.shortcuts.lock);

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

            // Read ready devices into a buffer first to avoid borrow conflicts.
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
                if code == KEY_POWER && value == 1 && self.locked {
                    force_unlock = true;
                    continue;
                }
                let a = self.matcher.on_key(code, value);
                if a != catlock_core::Action::None {
                    actions.push(a);
                }
            }
            if force_unlock {
                info!("power key -> force unlock");
                self.unlock();
            }

            for a in actions {
                match a {
                    catlock_core::Action::LockPressed => {
                        if !self.locked {
                            self.lock();
                        }
                    }
                    catlock_core::Action::UnlockPressed => {
                        if self.locked && !self.hold_active {
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

            // Check timers.
            if self.locked {
                if self.hold_active {
                    if let Some(dl) = self.hold_deadline {
                        if Instant::now() >= dl {
                            info!("hold complete -> unlock");
                            self.unlock();
                        }
                    }
                }
                if let Some(dl) = self.deadman_deadline {
                    if Instant::now() >= dl {
                        info!("deadman expired -> unlock");
                        self.unlock();
                    }
                }
            }
        }

        // Graceful shutdown: ensure ungrabbed.
        if self.locked {
            self.unlock();
        }
        info!("daemon stopped");
    }

    fn lock(&mut self) {
        info!("LOCK");
        for kb in &mut self.keyboards {
            if let Err(e) = kb.device.grab() {
                warn!(path = %kb.path.display(), error = %e, "grab failed");
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
    }

    fn unlock(&mut self) {
        info!("UNLOCK");
        for kb in &mut self.keyboards {
            let _ = kb.device.ungrab();
        }
        self.locked = false;
        self.matcher.set_locked(false);
        self.matcher.reset();
        self.deadman_deadline = None;
        self.hold_active = false;
        self.hold_deadline = None;
    }
}

pub fn run_daemon(config: Config, stop: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    let mut d = Daemon::new(config, stop);
    d.run();
}
