use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub voice: String,
    /// Voice list filter, e.g. "ko-KR". Empty = all languages.
    pub language: String,
    pub rate: f32,
    pub pitch: f32,
    pub volume: f32,
    /// Main output (virtual cable). None = system default device.
    pub output_device: Option<String>,
    pub monitor_enabled: bool,
    /// Monitor output (your speakers). None = system default device.
    pub monitor_device: Option<String>,
    pub monitor_volume: f32,
    pub always_on_top: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            voice: "ko-KR-SunHiNeural".into(),
            language: "ko-KR".into(),
            rate: 1.0,
            pitch: 0.0,
            volume: 1.0,
            output_device: None,
            monitor_enabled: true,
            monitor_device: None,
            monitor_volume: 0.6,
            always_on_top: false,
        }
    }
}

fn path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tts-for-window")
        .join("config.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let p = path();
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(p, json).map_err(|e| e.to_string())
    }
}
