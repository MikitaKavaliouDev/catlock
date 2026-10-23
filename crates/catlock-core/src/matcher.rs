//! Chord matcher over a stream of evdev key events.
//!
//! Pure: no I/O, no timers. The daemon feeds it events and reacts to the
//! returned [`Action`]s. Hold-to-unlock timing lives in the daemon.
//!
//! The matcher is state-aware: while unlocked only the lock chord can fire,
//! while locked only the unlock chord can. This keeps a single chord from
//! firing both actions and makes "lock instant, unlock requires hold" work.

use std::collections::{BTreeSet, HashSet};

use crate::chord::{Chord, Modifier};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    LockPressed,
    UnlockPressed,
    UnlockReleased,
}

#[derive(Debug, Clone)]
pub struct Matcher {
    lock: Chord,
    unlock: Chord,
    locked: bool,
    mods: HashSet<Modifier>,
    pressed: HashSet<u16>,
    unlock_held: bool,
}

impl Matcher {
    pub fn new(lock: Chord, unlock: Chord) -> Self {
        Self {
            lock,
            unlock,
            locked: false,
            mods: HashSet::new(),
            pressed: HashSet::new(),
            unlock_held: false,
        }
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        self.unlock_held = false;
        self.pressed.clear();
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Feed one evdev key event. `value`: 0=up, 1=down, 2=repeat.
    pub fn on_key(&mut self, code: u16, value: i32) -> Action {
        if let Some(m) = modifier_of(code) {
            match value {
                1 => {
                    self.mods.insert(m);
                }
                0 => {
                    self.mods.remove(&m);
                }
                _ => {}
            }
            return Action::None;
        }

        match value {
            1 => {
                self.pressed.insert(code);
            }
            0 => {
                self.pressed.remove(&code);
                if self.locked && self.unlock.key == code && self.unlock_held {
                    self.unlock_held = false;
                    return Action::UnlockReleased;
                }
                return Action::None;
            }
            _ => return Action::None, // repeat
        }

        if !self.locked {
            // Lock chord: fires instantly when its non-modifier key goes down
            // with all required modifiers held.
            if code == self.lock.key && subset(&self.lock.modifiers, &self.mods) {
                self.pressed.remove(&code);
                return Action::LockPressed;
            }
        } else if code == self.unlock.key && subset(&self.unlock.modifiers, &self.mods) {
            self.unlock_held = true;
            return Action::UnlockPressed;
        }

        Action::None
    }

    pub fn reset(&mut self) {
        self.mods.clear();
        self.pressed.clear();
        self.unlock_held = false;
    }
}

fn subset(wanted: &BTreeSet<Modifier>, have: &HashSet<Modifier>) -> bool {
    wanted.iter().all(|m| have.contains(m))
}

fn modifier_of(code: u16) -> Option<Modifier> {
    match code {
        29 | 97 => Some(Modifier::Ctrl),
        42 | 54 => Some(Modifier::Shift),
        56 | 100 => Some(Modifier::Alt),
        125 | 126 => Some(Modifier::Super),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chord::Chord;

    fn chord(s: &str) -> Chord {
        Chord::parse(s).unwrap()
    }

    #[test]
    fn lock_fires_while_unlocked() {
        let mut m = Matcher::new(chord("ctrl+alt+shift+k"), chord("ctrl+alt+shift+k"));
        m.on_key(29, 1);
        m.on_key(56, 1);
        m.on_key(42, 1);
        assert_eq!(m.on_key(37, 1), Action::LockPressed);
    }

    #[test]
    fn unlock_requires_hold_while_locked() {
        let mut m = Matcher::new(chord("ctrl+alt+shift+k"), chord("ctrl+alt+shift+k"));
        m.set_locked(true);
        m.on_key(29, 1);
        m.on_key(56, 1);
        m.on_key(42, 1);
        assert_eq!(m.on_key(37, 1), Action::UnlockPressed);
        assert_eq!(m.on_key(37, 0), Action::UnlockReleased);
    }

    #[test]
    fn lock_ignored_while_locked() {
        let mut m = Matcher::new(chord("ctrl+alt+shift+k"), chord("ctrl+alt+shift+u"));
        m.set_locked(true);
        m.on_key(29, 1);
        m.on_key(56, 1);
        m.on_key(42, 1);
        // lock chord (k) should do nothing while locked
        assert_eq!(m.on_key(37, 1), Action::None);
    }

    #[test]
    fn repeat_does_not_refire() {
        let mut m = Matcher::new(chord("ctrl+alt+shift+k"), chord("ctrl+alt+shift+k"));
        m.on_key(29, 1);
        m.on_key(56, 1);
        m.on_key(42, 1);
        assert_eq!(m.on_key(37, 1), Action::LockPressed);
        assert_eq!(m.on_key(37, 2), Action::None);
    }

    #[test]
    fn reset_clears_state() {
        let mut m = Matcher::new(chord("ctrl+alt+shift+k"), chord("ctrl+alt+shift+k"));
        m.on_key(29, 1);
        m.reset();
        m.on_key(56, 1);
        m.on_key(42, 1);
        assert_eq!(m.on_key(37, 1), Action::None);
    }

    #[test]
    fn different_chords_work() {
        let mut m = Matcher::new(chord("ctrl+alt+shift+k"), chord("ctrl+alt+shift+u"));
        // lock
        m.on_key(29, 1);
        m.on_key(56, 1);
        m.on_key(42, 1);
        assert_eq!(m.on_key(37, 1), Action::LockPressed);
        // now locked
        m.set_locked(true);
        // unlock chord u=22
        m.on_key(29, 1);
        m.on_key(56, 1);
        m.on_key(42, 1);
        assert_eq!(m.on_key(22, 1), Action::UnlockPressed);
    }
}