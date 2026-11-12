use crate::theme::ThemeMode;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    List,
    Wheel,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// Participants, one per line (as in the input field).
    pub text: String,
    /// Phrase shown in the result dialog.
    pub prompt: String,
    pub remove_after: bool,
    pub confetti: bool,
    pub spin_secs: f32,
    pub view: View,
    pub history: Vec<String>,
    /// Check for updates on startup (NIMVORO).
    pub check_updates: bool,
    /// Interface language code (`en`, `ru`, `uk`); `None` until first resolved.
    pub language: Option<String>,
    pub theme: ThemeMode,
    /// True when no config file existed yet (never saved).
    #[serde(skip)]
    pub fresh: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            text: String::new(),
            prompt: String::new(),
            remove_after: false,
            confetti: true,
            spin_secs: 5.0,
            view: View::Wheel,
            history: Vec::new(),
            check_updates: true,
            language: None,
            theme: ThemeMode::System,
            fresh: false,
        }
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Random Wheel").join("config.json"))
}

impl Config {
    pub fn load() -> Self {
        let text = path().and_then(|p| std::fs::read_to_string(p).ok());
        let mut cfg: Config =
            text.as_deref().and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok()).unwrap_or_default();
        cfg.fresh = text.is_none();
        cfg.spin_secs = cfg.spin_secs.clamp(2.0, 12.0);
        cfg
    }

    pub fn save(&self) {
        let Some(p) = path() else { return };
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, json);
        }
    }
}
