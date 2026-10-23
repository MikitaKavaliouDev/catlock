//! catlockd - system daemon (M1a+).

use std::ffi::OsStr;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Context;
use catlock_core::Config;
use tracing::{info, warn};
use tracing_subscriber::prelude::*;

mod daemon;

/// Manual (non-systemd) runs mirror their log here so it survives a session
/// death or a reboot. Under systemd stderr already goes to the journal.
const DEFAULT_LOG_FILE: &str = "/var/log/catlockd.log";

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

fn build_appender(path: &Path) -> anyhow::Result<tracing_appender::rolling::RollingFileAppender> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("catlockd.log");
    tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::NEVER)
        .filename_prefix(name)
        .build(dir)
        .with_context(|| format!("open log file {}", path.display()))
}

/// Resolve the log file: explicit flag wins, otherwise mirror to the
/// persistent default unless we are running under systemd (journald already
/// captures stderr there).
fn resolve_log_file(explicit: Option<&str>) -> anyhow::Result<Option<String>> {
    if let Some(path) = explicit {
        build_appender(Path::new(path))?; // fail loudly on an explicit bad path
        return Ok(Some(path.to_string()));
    }
    if std::env::var_os("JOURNAL_STREAM").is_some() {
        return Ok(None);
    }
    match build_appender(Path::new(DEFAULT_LOG_FILE)) {
        Ok(_) => Ok(Some(DEFAULT_LOG_FILE.to_string())),
        Err(_) => Ok(None), // not root: stderr only
    }
}

fn init_logging(log_file: Option<&str>) -> anyhow::Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let subscriber = tracing_subscriber::registry().with(env_filter).with(
        tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_target(true),
    );
    match log_file {
        Some(path) => {
            let appender = build_appender(Path::new(path))?;
            subscriber
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_writer(appender)
                        .with_ansi(false)
                        .with_target(true),
                )
                .init();
        }
        None => subscriber.init(),
    }
    Ok(())
}

/// The kernel SAK (Alt+SysRq+K) kills every session on the seat and can leave
/// a blank VT with a blinking cursor. It is the exact trap that bit us twice.
fn warn_about_sak() {
    let Ok(value) = std::fs::read_to_string("/proc/sys/kernel/sysrq") else {
        return;
    };
    let value = value.trim();
    if value == "0" {
        return;
    }
    warn!(
        sysrq = value,
        "Alt+SysRq+K (Secure Attention Key) is enabled and kills the whole session, \
         leaving a blank VT. Never press it while testing catlock. Emergency unlock \
         is: hold Esc for 2 s, or wait for the deadman."
    );
}

fn main() -> anyhow::Result<()> {
    // Tiny dev-only arg parsing.
    let mut deadman_override: Option<u64> = None;
    let mut hold_override: Option<u64> = None;
    let mut debug_keys = false;
    let mut dry_run = false;
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
            "--dry-run" => {
                dry_run = true;
            }
            "--log-file" => {
                if let Some(v) = args.next() {
                    log_file = Some(v);
                }
            }
            other => {
                eprintln!("unknown argument: {other}");
            }
        }
    }

    let resolved_log_file = resolve_log_file(log_file.as_deref())?;
    init_logging(resolved_log_file.as_deref())?;
    if let Some(path) = &resolved_log_file {
        info!(path, "logging to file (and stderr)");
    }

    warn_about_sak();

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
    daemon::run_daemon(config, debug_keys, dry_run, stop);
    Ok(())
}
