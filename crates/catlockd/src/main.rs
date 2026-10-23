//! catlockd - system daemon (M1a+).

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
    // Tiny dev-only arg parsing.
    let mut deadman_override: Option<u64> = None;
    let mut hold_override: Option<u64> = None;
    let mut debug_keys = false;
    let mut log_file: Option<String> = None;
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
            "--debug-keys" => {
                debug_keys = true;
            }
            "--log-file" => {
                if let Some(v) = args.next() {
                    log_file = Some(v);
                }
            }
            _ => {}
        }
    }

    // Logging: stderr always; file appender if requested (survives session death).
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    if let Some(path) = &log_file {
        let appender = tracing_appender::rolling::never(
            std::path::Path::new(path).parent().unwrap_or(std::path::Path::new(".")),
            std::path::Path::new(path)
                .file_name()
                .unwrap_or(std::ffi::OsStr::new("catlockd.log")),
        );
        let (nb, _guard) = tracing_appender::non_blocking(appender);
        // Keep guard alive for the process lifetime.
        Box::leak(Box::new(_guard));
        tracing_subscriber::fmt()
            .with_env_filter(env_filter.clone())
            .with_writer(nb)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
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

    info!("catlockd starting (M1a+)");
    daemon::run_daemon(config, debug_keys, stop);
    Ok(())
}