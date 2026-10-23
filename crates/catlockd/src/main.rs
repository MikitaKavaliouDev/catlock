//! catlockd - system daemon (M1a).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context;
use catlock_core::Config;
use tracing::info;

mod daemon;

fn load_config() -> Config {
    for path in ["/etc/catlock/config.toml", "./catlock.toml"] {
        if let Ok(s) = std::fs::read_to_string(path) {
            if let Ok(c) = Config::from_toml(&s) {
                info!(path, "loaded config");
                return c;
            }
        }
    }
    info!("using default config");
    Config::default()
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Tiny dev-only arg parsing.
    let mut deadman_override: Option<u64> = None;
    let mut hold_override: Option<u64> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--deadman" => {
                if let Some(v) = args.next() {
                    deadman_override = v.parse().ok();
                }
            }
            "--hold" => {
                if let Some(v) = args.next() {
                    hold_override = v.parse().ok();
                }
            }
            _ => {}
        }
    }

    let mut config = load_config();
    if let Some(d) = deadman_override {
        config.behaviour.auto_unlock_after_s = d;
    }
    if let Some(h) = hold_override {
        config.shortcuts.hold_to_unlock_ms = h;
    }

    let stop = Arc::new(AtomicBool::new(false));
    let stop_cb = stop.clone();
    ctrlc::set_handler(move || {
        stop_cb.store(true, Ordering::Relaxed);
    })
    .context("set ctrlc handler")?;

    info!("catlockd starting (M1a)");
    daemon::run_daemon(config, stop);
    Ok(())
}