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
    /// Участники, по одному на строку (как в поле ввода).
    pub text: String,
    /// Фраза в окне результата.
    pub prompt: String,
    pub remove_after: bool,
    pub confetti: bool,
    pub spin_secs: f32,
    pub view: View,
    pub history: Vec<String>,
    /// Проверять обновления при запуске (NIMVORO).
    pub check_updates: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            text: "Анна\nБорис\nВиктория\nГлеб\nДарья\nЕгор\nЖанна\nЗахар".into(),
            prompt: String::new(),
            remove_after: false,
            confetti: true,
            spin_secs: 5.0,
            view: View::Wheel,
            history: Vec::new(),
            check_updates: true,
        }
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("Random Wheel").join("config.json"))
}

impl Config {
    pub fn load() -> Self {
        let mut cfg: Config = path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
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
