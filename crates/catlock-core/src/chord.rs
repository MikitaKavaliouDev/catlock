//! Chord parsing and matching.
//!
//! A chord is a set of modifiers plus exactly one non-modifier key, e.g.
//! `ctrl+alt+shift+k`. Matching happens on physical evdev key codes, so it is
//! layout-independent; the GUI resolves a display label separately.

use std::collections::BTreeSet;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Modifier {
    Ctrl,
    Alt,
    Shift,
    Super,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Chord {
    pub modifiers: BTreeSet<Modifier>,
    pub key: u16, // evdev KEY_* code
}

impl Chord {
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut modifiers = BTreeSet::new();
        let mut key: Option<u16> = None;
        for raw in s.split('+') {
            let part = raw.trim().to_lowercase();
            if part.is_empty() {
                return Err("empty token".into());
            }
            match part.as_str() {
                "ctrl" | "control" => {
                    modifiers.insert(Modifier::Ctrl);
                }
                "alt" => {
                    modifiers.insert(Modifier::Alt);
                }
                "shift" => {
                    modifiers.insert(Modifier::Shift);
                }
                "super" | "meta" | "win" | "cmd" | "super_l" => {
                    modifiers.insert(Modifier::Super);
                }
                other => {
                    if key.is_some() {
                        return Err("multiple non-modifier keys".into());
                    }
                    let code = name_to_code(other)
                        .ok_or_else(|| format!("unknown key: {other}"))?;
                    key = Some(code);
                }
            }
        }
        let key = key.ok_or_else(|| "missing key".to_string())?;
        Ok(Chord { modifiers, key })
    }

    pub fn has_modifier(&self) -> bool {
        !self.modifiers.is_empty()
    }
}

impl fmt::Display for Chord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts: Vec<&str> = Vec::new();
        if self.modifiers.contains(&Modifier::Ctrl) {
            parts.push("ctrl");
        }
        if self.modifiers.contains(&Modifier::Alt) {
            parts.push("alt");
        }
        if self.modifiers.contains(&Modifier::Shift) {
            parts.push("shift");
        }
        if self.modifiers.contains(&Modifier::Super) {
            parts.push("super");
        }
        parts.push(code_to_name(self.key));
        write!(f, "{}", parts.join("+"))
    }
}

fn name_to_code(name: &str) -> Option<u16> {
    // evdev KEY_* numeric codes (linux/input-event-codes.h).
    // Letters a-z: KEY_A=30 .. KEY_Z=54 (contiguous in qwerty order? No:
    // a=30,b=48,c=46,d=32,...). Use explicit table to be safe.
    match name {
        // letters
        "a" => Some(30),
        "b" => Some(48),
        "c" => Some(46),
        "d" => Some(32),
        "e" => Some(18),
        "f" => Some(33),
        "g" => Some(34),
        "h" => Some(35),
        "i" => Some(23),
        "j" => Some(36),
        "k" => Some(37),
        "l" => Some(38),
        "m" => Some(50),
        "n" => Some(49),
        "o" => Some(24),
        "p" => Some(25),
        "q" => Some(16),
        "r" => Some(19),
        "s" => Some(31),
        "t" => Some(20),
        "u" => Some(22),
        "v" => Some(47),
        "w" => Some(17),
        "x" => Some(45),
        "y" => Some(21),
        "z" => Some(44),
        // digits
        "0" | "kp0" => Some(11),
        "1" => Some(2),
        "2" => Some(3),
        "3" => Some(4),
        "4" => Some(5),
        "5" => Some(6),
        "6" => Some(7),
        "7" => Some(8),
        "8" => Some(9),
        "9" => Some(10),
        // function keys
        "f1" => Some(59),
        "f2" => Some(60),
        "f3" => Some(61),
        "f4" => Some(62),
        "f5" => Some(63),
        "f6" => Some(64),
        "f7" => Some(65),
        "f8" => Some(66),
        "f9" => Some(67),
        "f10" => Some(68),
        "f11" => Some(87),
        "f12" => Some(88),
        // common
        "space" => Some(57),
        "enter" | "return" => Some(28),
        "tab" => Some(15),
        "esc" | "escape" => Some(1),
        "backspace" => Some(14),
        "delete" | "del" => Some(111),
        "insert" | "ins" => Some(110),
        "home" => Some(102),
        "end" => Some(107),
        "pageup" | "pgup" => Some(104),
        "pagedown" | "pgdn" => Some(109),
        "up" => Some(103),
        "down" => Some(108),
        "left" => Some(105),
        "right" => Some(106),
        _ => None,
    }
}

fn code_to_name(code: u16) -> &'static str {
    // inverse of the small table above (best-effort).
    match code {
        30 => "a",
        48 => "b",
        46 => "c",
        32 => "d",
        18 => "e",
        33 => "f",
        34 => "g",
        35 => "h",
        23 => "i",
        36 => "j",
        37 => "k",
        38 => "l",
        50 => "m",
        49 => "n",
        24 => "o",
        25 => "p",
        16 => "q",
        19 => "r",
        31 => "s",
        20 => "t",
        22 => "u",
        47 => "v",
        17 => "w",
        45 => "x",
        21 => "y",
        44 => "z",
        57 => "space",
        28 => "enter",
        1 => "esc",
        _ => "?",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_default_chord() {
        let c = Chord::parse("ctrl+alt+shift+k").unwrap();
        assert!(c.modifiers.contains(&Modifier::Ctrl));
        assert!(c.modifiers.contains(&Modifier::Alt));
        assert!(c.modifiers.contains(&Modifier::Shift));
        assert_eq!(c.key, 37);
        assert!(c.has_modifier());
    }

    #[test]
    fn rejects_empty_and_multi_key() {
        assert!(Chord::parse("").is_err());
        assert!(Chord::parse("ctrl+a+b").is_err());
        assert!(Chord::parse("ctrl").is_err());
    }

    #[test]
    fn roundtrips_display() {
        let c = Chord::parse("ctrl+alt+shift+k").unwrap();
        assert_eq!(c.to_string(), "ctrl+alt+shift+k");
    }
}