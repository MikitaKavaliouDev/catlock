//! M0 spike: grab every keyboard-like evdev device for N seconds and report.
//!
//! Safety: the grab is released when this process exits for ANY reason,
//! including kill -9, because EVIOCGRAB is tied to the open fd. The default
//! hold time is 10 seconds so a forgotten run can never brick the session.

use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result};
use evdev::{Device, EventSummary, KeyCode};
use tracing::{info, warn};

fn is_keyboard(dev: &Device) -> bool {
    // Heuristic: real keyboards expose a spread of alphanumeric keys plus
    // space and enter. Power buttons / lid switches / video buses do not.
    dev.supported_keys().map_or(false, |keys| {
        keys.contains(KeyCode::KEY_A)
            && keys.contains(KeyCode::KEY_Z)
            && keys.contains(KeyCode::KEY_SPACE)
            && keys.contains(KeyCode::KEY_ENTER)
    })
}

fn parse_seconds() -> u64 {
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--seconds" || a == "-s" {
            if let Some(v) = args.next() {
                if let Ok(n) = v.parse() {
                    return n;
                }
            }
        } else if a == "--list" {
            return 0; // handled separately
        }
    }
    10
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let list_only = std::env::args().any(|a| a == "--list");
    let seconds = parse_seconds();

    // Discover keyboard-like devices.
    let mut found: Vec<(std::path::PathBuf, Device)> = Vec::new();
    for entry in std::fs::read_dir("/dev/input").context("read /dev/input")? {
        let entry = entry?;
        let path = entry.path();
        let name = path.to_string_lossy();
        if !name.contains("event") {
            continue;
        }
        let dev = match Device::open(&path) {
            Ok(d) => d,
            Err(e) => {
                warn!(path = %path.display(), error = %e, "cannot open");
                continue;
            }
        };
        if is_keyboard(&dev) {
            found.push((path, dev));
        }
    }

    if found.is_empty() {
        anyhow::bail!("no keyboard-like evdev devices found (are you in the `input` group or root?)");
    }

    info!(count = found.len(), "discovered keyboard-like devices");
    for (path, dev) in &found {
        info!(path = %path.display(), name = %dev.name().unwrap_or("?"), "device");
    }

    if list_only {
        return Ok(());
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_timer = stop.clone();
    let timer = thread::spawn(move || {
        thread::sleep(Duration::from_secs(seconds));
        stop_timer.store(true, Ordering::Relaxed);
    });

    info!(seconds, "GRABBED - type now. Auto-release in {seconds}s. (kill -9 also releases.)");

    let mut handles = Vec::new();
    for (path, mut dev) in found {
        let stop = stop.clone();
        let handle = thread::spawn(move || {
            if let Err(e) = dev.set_nonblocking(true) {
                warn!(path = %path.display(), error = %e, "set_nonblocking failed");
                return;
            }
            if let Err(e) = dev.grab() {
                warn!(path = %path.display(), error = %e, "GRAB FAILED (need input group / root?)");
                return;
            }
            info!(path = %path.display(), "grabbed");
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                match dev.fetch_events() {
                    Ok(events) => {
                        for ev in events {
                            if let EventSummary::Key(_, key, value) = ev.destructure() {
                                // value: 0=up, 1=down, 2=repeat
                                info!(path = %path.display(), key = ?key, value, "KEY");
                            }
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => {
                        warn!(path = %path.display(), error = %e, "read error");
                        break;
                    }
                }
            }
            let _ = dev.ungrab();
            info!(path = %path.display(), "ungrabbed");
        });
        handles.push(handle);
    }

    for h in handles {
        let _ = h.join();
    }
    let _ = timer.join();

    info!("released all grabs - keyboard should be live again");
    Ok(())
}
