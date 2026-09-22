//! WASM Dynamic Color Module (Material You / M3).

use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use wyrd_module_sdk::{export_wasm_module, log_info, publish, read_file, WasmModule};

#[derive(Default)]
pub struct DynamicColorModule {
    last_accent: Option<String>,
    last_palette: Option<HashMap<String, String>>,
    ticks: u32,
}

#[derive(Deserialize)]
struct WallpaperState {
    #[serde(default = "default_accent")]
    accent: String,
    #[serde(default)]
    palette: Option<HashMap<String, String>>,
}

fn default_accent() -> String {
    "#c72548".to_string()
}

impl WasmModule for DynamicColorModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Dynamic Color WASM module (Material You M3) initialized");
        self.check_and_publish();
        Ok(())
    }

    fn on_topic(&mut self, topic: &str, _value: &Value) {
        if topic == "theme.request" || topic == "wallpaper.change" {
            self.check_and_publish();
        }
    }

    fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        // Event-driven via theme.request and wallpaper.change. Periodic fallback every 5s (10 * 500ms).
        if self.ticks.is_multiple_of(10) {
            self.check_and_publish();
        }
    }
}

impl DynamicColorModule {
    fn check_and_publish(&mut self) {
        let state = self.read_wallpaper_state();
        let accent = state
            .as_ref()
            .map(|s| s.accent.clone())
            .unwrap_or_else(|| "#c72548".to_string());

        let mut palette = state.and_then(|s| s.palette).unwrap_or_default();
        if !palette.contains_key("accent") {
            palette.insert("accent".to_string(), accent.clone());
        }
        if !palette.contains_key("primary") {
            palette.insert("primary".to_string(), accent.clone());
        }

        let palette_changed = match &self.last_palette {
            Some(prev) => prev != &palette,
            None => true,
        };

        let accent_changed = match &self.last_accent {
            Some(prev) => prev != &accent,
            None => true,
        };

        if palette_changed || accent_changed {
            self.last_palette = Some(palette.clone());
            self.last_accent = Some(accent.clone());

            let palette_val = serde_json::to_value(&palette).unwrap_or_else(|_| json!({}));
            let _ = publish("theme.palette", &palette_val);
            let _ = publish("theme.accent", &json!({ "accent": accent }));
        }
    }

    fn read_wallpaper_state(&self) -> Option<WallpaperState> {
        if let Some(content) = read_file("wallpaper.toml") {
            if let Ok(state) = toml::from_str::<WallpaperState>(&content) {
                return Some(state);
            }
        }

        if let Some(content) = read_file("theme.toml") {
            return toml::from_str::<WallpaperState>(&content).ok();
        }
        None
    }
}

export_wasm_module!(DynamicColorModule);
