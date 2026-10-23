//! Linux input layer: device discovery, exclusive grab, hotplug monitoring.
//!
//! Everything here talks to `/dev/input` via the `evdev` crate. The rest of the
//! daemon depends on the [`InputBackend`] trait so a future port to another OS
//! only re-implements this crate.

use std::path::PathBuf;

use evdev::{Device, KeyCode};
use tracing::warn;

pub use backend::InputBackend;

/// A discovered keyboard-like input device.
pub struct Keyboard {
    pub path: PathBuf,
    pub name: String,
    pub device: Device,
}

/// Heuristic: a real keyboard exposes a-z, space and enter, and is NOT also a
/// pointer (mouse/touchpad). The MX Master 2S, for example, exposes keyboard
/// keys but must never be grabbed — grabbing it kills the mouse.
pub fn is_keyboard(dev: &Device) -> bool {
    let keys = match dev.supported_keys() {
        Some(k) => k,
        None => return false,
    };
    if !keys.contains(KeyCode::KEY_A)
        || !keys.contains(KeyCode::KEY_Z)
        || !keys.contains(KeyCode::KEY_SPACE)
        || !keys.contains(KeyCode::KEY_ENTER)
    {
        return false;
    }
    // Exclude pointer devices: they expose relative axes or mouse buttons.
    if let Some(rel) = dev.supported_relative_axes() {
        if rel.contains(evdev::RelativeAxisCode::REL_X)
            || rel.contains(evdev::RelativeAxisCode::REL_Y)
        {
            return false;
        }
    }
    if let Some(btns) = dev.supported_keys() {
        // BTN_LEFT (0x110) indicates a mouse/touchpad.
        if btns.contains(KeyCode::BTN_LEFT) {
            return false;
        }
    }
    true
}

/// Open and return every keyboard-like device currently present.
pub fn discover() -> Vec<Keyboard> {
    let mut out = Vec::new();
    let entries = match std::fs::read_dir("/dev/input") {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "cannot read /dev/input");
            return out;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.to_string_lossy().contains("event") {
            continue;
        }
        let dev = match Device::open(&path) {
            Ok(d) => d,
            Err(e) => {
                warn!(path = %path.display(), error = %e, "cannot open device");
                continue;
            }
        };
        if is_keyboard(&dev) {
            let name = dev.name().unwrap_or("unknown").to_string();
            out.push(Keyboard { path, name, device: dev });
        }
    }
    out
}

/// Grab a device exclusively. Returns Ok(()) on success.
pub fn grab(dev: &mut Device) -> std::io::Result<()> {
    dev.grab()
}

pub fn ungrab(dev: &mut Device) -> std::io::Result<()> {
    dev.ungrab()
}

pub mod backend {
    use std::io;

    /// Abstract input backend. The daemon programs against this trait.
    pub trait InputBackend {
        fn discover(&mut self) -> Vec<super::Keyboard>;
        fn grab(&mut self, dev: &mut evdev::Device) -> io::Result<()>;
        fn ungrab(&mut self, dev: &mut evdev::Device) -> io::Result<()>;
    }
}