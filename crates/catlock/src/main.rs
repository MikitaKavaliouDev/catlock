//! catlock - user-facing binary: agent (tray/notify), settings (Slint), CLI.
//!
//! TODO(M3/M4): agent, settings window, D-Bus client.

use clap::{Parser, Subcommand};
use tracing::info;

#[derive(Parser)]
#[command(name = "catlock", version, about = "Keyboard lock for Linux")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Show daemon status.
    Status,
    /// Toggle the lock.
    Toggle,
    /// Open the settings window.
    Settings,
    /// Run the tray agent.
    Agent,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    match cli.cmd {
        Some(Cmd::Status) => info!("status: not implemented yet"),
        Some(Cmd::Toggle) => info!("toggle: not implemented yet"),
        Some(Cmd::Settings) => info!("settings: not implemented yet"),
        Some(Cmd::Agent) => info!("agent: not implemented yet"),
        None => info!("catlock: use --help"),
    }
}