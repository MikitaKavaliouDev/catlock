//! Lock state machine. Time is not tracked here; the daemon owns the clock and
//! the deadman timer. This module only knows about transitions.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Unlocked,
    Locked,
}

impl State {
    pub fn is_locked(self) -> bool {
        matches!(self, State::Locked)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    LockRequested,
    UnlockRequested,
    DeadmanExpired,
    PowerKey,
    SessionEnded,
}

impl State {
    pub fn transition(self, ev: Event) -> Self {
        use Event::*;
        use State::*;
        match (self, ev) {
            (Unlocked, LockRequested) => Locked,
            (Locked, UnlockRequested) => Unlocked,
            (Locked, DeadmanExpired) => Unlocked,
            (Locked, PowerKey) => Unlocked,
            (Locked, SessionEnded) => Unlocked,
            (s, _) => s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_unlock_cycle() {
        let s = State::Unlocked;
        let s = s.transition(Event::LockRequested);
        assert_eq!(s, State::Locked);
        let s = s.transition(Event::UnlockRequested);
        assert_eq!(s, State::Unlocked);
    }

    #[test]
    fn deadman_and_power_unlock() {
        let s = State::Locked;
        assert_eq!(s.transition(Event::DeadmanExpired), State::Unlocked);
        let s = State::Locked;
        assert_eq!(s.transition(Event::PowerKey), State::Unlocked);
    }

    #[test]
    fn session_end_unlocks() {
        assert_eq!(
            State::Locked.transition(Event::SessionEnded),
            State::Unlocked
        );
    }
}