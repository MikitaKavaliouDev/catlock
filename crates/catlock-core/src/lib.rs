//! Shared types, config, chord matcher and state machine for catlock.
//!
//! This crate has no I/O: no evdev, no D-Bus, no filesystem. Everything here
//! is pure and unit-testable.

pub mod chord;
pub mod config;
pub mod matcher;
pub mod state;

pub use chord::Chord;
pub use config::Config;
pub use matcher::{Action, Matcher};
pub use state::State;