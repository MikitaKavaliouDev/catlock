use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub shortcuts: Shortcuts,
    pub behaviour: Behaviour,
    pub notify: Notify,
    pub devices: Devices,
    pub config_version: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Shortcuts {
    pub lock: String,
    pub unlock: String,
    pub hold_to_unlock_ms: u64,
    pub allow_unmodified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Behaviour {
    pub auto_unlock_after_s: u64,
    pub unlock_on_power_key: bool,
    pub lock_on_start: bool,
    pub idle_inhibit_while_locked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Notify {
    pub desktop: bool,
    pub tray: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Devices {
    pub exclude: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shortcuts: Shortcuts::default(),
            behaviour: Behaviour::default(),
            notify: Notify::default(),
            devices: Devices::default(),
            config_version: 1,
        }
    }
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            lock: "ctrl+alt+shift+k".into(),
            unlock: "ctrl+alt+shift+k".into(),
            hold_to_unlock_ms: 1200,
            allow_unmodified: false,
        }
    }
}

impl Default for Behaviour {
    fn default() -> Self {
        Self {
            auto_unlock_after_s: 120,
            unlock_on_power_key: true,
            lock_on_start: false,
            idle_inhibit_while_locked: true,
        }
    }
}

impl Default for Notify {
    fn default() -> Self {
        Self {
            desktop: true,
            tray: true,
        }
    }
}

impl Default for Devices {
    fn default() -> Self {
        Self {
            exclude: vec!["*-power-button".into(), "video-bus".into()],
        }
    }
}

impl Config {
    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }

    pub fn from_toml(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips() {
        let c = Config::default();
        let s = c.to_toml().unwrap();
        let c2 = Config::from_toml(&s).unwrap();
        assert_eq!(c2.shortcuts.lock, "ctrl+alt+shift+k");
        assert_eq!(c2.behaviour.auto_unlock_after_s, 120);
        assert_eq!(c2.config_version, 1);
    }
}