//! catlockd - system daemon.
//!
//! TODO(M1): evdev read loop, chord matcher, state machine, D-Bus service,
//! logind idle inhibitor, deadman timer. This entrypoint only proves the
//! workspace builds end to end.

use tracing::info;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    info!("catlockd starting (skeleton)");
    // TODO(M1): run the daemon.
}