use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    audio_get_status, audio_set_default, audio_set_mute, audio_set_volume, audio_subscribe,
    audio_toggle_mute, export_wasm_module, log_info, publish, resolve_icon, send_update,
    WasmModule,
};

#[derive(Debug, Clone, Default)]
struct AudioDev {
    id: usize,
    name: String,
    desc: String,
    is_default: bool,
    volume: u8,
    muted: bool,
}

#[derive(Default)]
pub struct AudioModule {
    volume: u8,
    muted: bool,
    mic_volume: u8,
    mic_muted: bool,
    active_sink: String,
    active_source: String,
    sinks: Vec<AudioDev>,
    sources: Vec<AudioDev>,
    initialized: bool,
}

impl WasmModule for AudioModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("Audio WASM module initialized with native PipeWire/Pulse integration");
        let _ = audio_subscribe();
        self.update_audio();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if widget_id == "pipewire:changed" {
            self.update_audio();
            self.emit_bar_updates();
            self.send_speaker_popup();
            self.send_microphone_popup();
            return;
        }
        if widget_id == "audio-sinks"
            || widget_id == "popup:audio-sinks"
            || widget_id == "audio_sinks"
        {
            self.send_audio_sinks_popup();
            return;
        }
        if widget_id == "audio-sources"
            || widget_id == "popup:audio-sources"
            || widget_id == "audio_sources"
        {
            self.send_audio_sources_popup();
            return;
        }

        let is_root_mic =
            widget_id == "microphone" || widget_id == "mic" || widget_id == "popup:microphone";
        let is_root_audio = widget_id == "audio"
            || widget_id == "volume"
            || widget_id == "speaker"
            || widget_id == "vol"
            || widget_id == "popup:audio";

        if widget_id == "audio_mute_toggle" || (is_root_audio && event == "middle_click") {
            self.muted = !self.muted;
            self.emit_bar_updates();
            self.send_speaker_popup();
            let _ = audio_toggle_mute("@DEFAULT_AUDIO_SINK@");
        } else if widget_id == "mic_mute_toggle" || (is_root_mic && event == "middle_click") {
            self.mic_muted = !self.mic_muted;
            self.emit_bar_updates();
            self.send_microphone_popup();
            let _ = audio_toggle_mute("@DEFAULT_AUDIO_SOURCE@");
        } else if widget_id == "audio_slider"
            || widget_id == "volume_slider"
            || widget_id.starts_with("qs_volume")
            || widget_id.contains("volume_slider")
            || widget_id.contains("audio_slider")
        {
            let parsed_vol = event
                .parse::<f64>()
                .ok()
                .or_else(|| {
                    event
                        .strip_prefix("change:")
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .or_else(|| {
                    event
                        .strip_prefix("slider:")
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .or_else(|| {
                    event
                        .strip_prefix("value:")
                        .and_then(|s| s.parse::<f64>().ok())
                });
            if let Some(vol) = parsed_vol {
                let v = vol.clamp(0.0, 100.0).round() as u8;
                self.volume = v;
                if v > 0 && self.muted {
                    self.muted = false;
                    let _ = audio_set_mute("@DEFAULT_AUDIO_SINK@", false);
                }
                self.emit_bar_updates();
                self.send_speaker_popup();
                let _ = audio_set_volume("@DEFAULT_AUDIO_SINK@", vol);
            }
        } else if widget_id == "mic_slider"
            || widget_id == "microphone_slider"
            || widget_id.starts_with("qs_mic")
            || widget_id.contains("mic_slider")
        {
            let parsed_vol = event
                .parse::<f64>()
                .ok()
                .or_else(|| {
                    event
                        .strip_prefix("change:")
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .or_else(|| {
                    event
                        .strip_prefix("slider:")
                        .and_then(|s| s.parse::<f64>().ok())
                })
                .or_else(|| {
                    event
                        .strip_prefix("value:")
                        .and_then(|s| s.parse::<f64>().ok())
                });
            if let Some(vol) = parsed_vol {
                let v = vol.clamp(0.0, 100.0).round() as u8;
                self.mic_volume = v;
                if v > 0 && self.mic_muted {
                    self.mic_muted = false;
                    let _ = audio_set_mute("@DEFAULT_AUDIO_SOURCE@", false);
                }
                self.emit_bar_updates();
                self.send_microphone_popup();
                let _ = audio_set_volume("@DEFAULT_AUDIO_SOURCE@", vol);
            }
        } else if widget_id.starts_with("vol_p") {
            let pct_str = widget_id.trim_start_matches("vol_p");
            if let Ok(pct) = pct_str.parse::<f64>() {
                let v = pct.clamp(0.0, 100.0).round() as u8;
                self.volume = v;
                if self.muted {
                    self.muted = false;
                    let _ = audio_set_mute("@DEFAULT_AUDIO_SINK@", false);
                }
                self.emit_bar_updates();
                self.send_speaker_popup();
                let _ = audio_set_volume("@DEFAULT_AUDIO_SINK@", pct);
            }
        } else if widget_id.starts_with("mic_p") {
            let pct_str = widget_id.trim_start_matches("mic_p");
            if let Ok(pct) = pct_str.parse::<f64>() {
                let v = pct.clamp(0.0, 100.0).round() as u8;
                self.mic_volume = v;
                if self.mic_muted {
                    self.mic_muted = false;
                    let _ = audio_set_mute("@DEFAULT_AUDIO_SOURCE@", false);
                }
                self.emit_bar_updates();
                self.send_microphone_popup();
                let _ = audio_set_volume("@DEFAULT_AUDIO_SOURCE@", pct);
            }
        } else if widget_id.starts_with("sink_dev_") {
            let idx_str = widget_id.trim_start_matches("sink_dev_");
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(sink) = self.sinks.iter().find(|s| s.id == idx) {
                    let target_name = sink.name.clone();
                    let target_desc = sink.desc.clone();
                    for s in &mut self.sinks {
                        s.is_default = s.id == idx;
                    }
                    self.active_sink = if !target_desc.is_empty() {
                        target_desc
                    } else {
                        target_name.clone()
                    };
                    self.send_speaker_popup();
                    self.send_audio_sinks_popup();
                    self.emit_bar_updates();

                    let _ = audio_set_default("@DEFAULT_AUDIO_SINK@", &target_name);
                }
            }
        } else if widget_id.starts_with("source_dev_") {
            let idx_str = widget_id.trim_start_matches("source_dev_");
            if let Ok(idx) = idx_str.parse::<usize>() {
                if let Some(source) = self.sources.iter().find(|s| s.id == idx) {
                    let target_name = source.name.clone();
                    let target_desc = source.desc.clone();
                    for s in &mut self.sources {
                        s.is_default = s.id == idx;
                    }
                    self.active_source = if !target_desc.is_empty() {
                        target_desc
                    } else {
                        target_name.clone()
                    };
                    self.send_microphone_popup();
                    self.send_audio_sources_popup();
                    self.emit_bar_updates();

                    let _ = audio_set_default("@DEFAULT_AUDIO_SOURCE@", &target_name);
                }
            }
        } else if is_root_mic && (event == "open" || event == "click") {
            self.send_microphone_popup();
        } else if is_root_audio && (event == "open" || event == "click") {
            self.send_speaker_popup();
        }
    }

    fn tick(&mut self) {
        if !self.initialized {
            self.update_audio();
        }
    }
}

impl AudioModule {
    fn emit_bar_updates(&self) {
        let icon_name = if self.muted || self.volume == 0 {
            "audio-volume-muted"
        } else if self.volume < 30 {
            "audio-volume-low"
        } else if self.volume < 70 {
            "audio-volume-medium"
        } else {
            "audio-volume-high"
        };
        let icon_path = resolve_icon(icon_name);

        let mic_name = if self.mic_muted {
            "microphone-sensitivity-muted"
        } else {
            "microphone-sensitivity-high"
        };
        let mic_path = resolve_icon(mic_name).or_else(|| resolve_icon("audio-input-microphone"));
        let vol_icon = if self.muted || self.volume == 0 {
            "󰝟"
        } else if self.volume < 30 {
            "󰕿"
        } else if self.volume < 70 {
            "󰖀"
        } else {
            "󰕾"
        };
        let vol_text = if self.muted {
            format!("{} Muted", vol_icon)
        } else {
            format!("{} {}%", vol_icon, self.volume)
        };

        let mic_icon = if self.mic_muted || self.mic_volume == 0 {
            "󰍭"
        } else {
            "󰍬"
        };
        let mic_text = if self.mic_muted {
            format!("{} Muted", mic_icon)
        } else {
            format!("{} {}%", mic_icon, self.mic_volume)
        };

        let audio_payload = json!({
            "text": vol_text,
            "icon_glyph": vol_icon,
            "volume": self.volume,
            "muted": self.muted,
            "is_muted": self.muted,
            "icon": icon_name,
            "icon_path": icon_path,
            "path": icon_path,
            "mic_volume": self.mic_volume,
            "mic_muted": self.mic_muted,
        });

        send_update("audio", &audio_payload);
        send_update("volume", &audio_payload);

        let mic_payload = json!({
            "text": mic_text,
            "icon_glyph": mic_icon,
            "volume": self.mic_volume,
            "muted": self.mic_muted,
            "is_muted": self.mic_muted,
            "icon": mic_name,
            "icon_path": mic_path,
            "path": mic_path,
        });
        send_update("microphone", &mic_payload);
        send_update("mic", &mic_payload);

        let _ = publish(
            "audio.volume",
            &json!({ "volume": self.volume, "muted": self.muted }),
        );
    }

    fn update_audio(&mut self) {
        let Some(status) = audio_get_status() else {
            return;
        };
        if status.get("active_sink_name").is_none() && status.get("sinks").is_none() {
            return;
        }
        self.initialized = true;

        self.volume = status.get("volume").and_then(|v| v.as_u64()).unwrap_or(50) as u8;
        self.muted = status
            .get("muted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.active_sink = status
            .get("active_sink_desc")
            .or_else(|| status.get("active_sink_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Default Output")
            .to_string();

        self.mic_volume = status
            .get("mic_volume")
            .and_then(|v| v.as_u64())
            .unwrap_or(100) as u8;
        self.mic_muted = status
            .get("mic_muted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.active_source = status
            .get("active_source_desc")
            .or_else(|| status.get("active_source_name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Default Input")
            .to_string();

        if let Some(sinks_arr) = status.get("sinks").and_then(|v| v.as_array()) {
            self.sinks = sinks_arr
                .iter()
                .enumerate()
                .map(|(i, s)| AudioDev {
                    id: i,
                    name: s
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    desc: s
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    is_default: s
                        .get("is_default")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    volume: s.get("volume").and_then(|v| v.as_u64()).unwrap_or(50) as u8,
                    muted: s.get("muted").and_then(|v| v.as_bool()).unwrap_or(false),
                })
                .collect();
        }

        if let Some(sources_arr) = status.get("sources").and_then(|v| v.as_array()) {
            self.sources = sources_arr
                .iter()
                .enumerate()
                .map(|(i, s)| AudioDev {
                    id: i,
                    name: s
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    desc: s
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    is_default: s
                        .get("is_default")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    volume: s.get("volume").and_then(|v| v.as_u64()).unwrap_or(100) as u8,
                    muted: s.get("muted").and_then(|v| v.as_bool()).unwrap_or(false),
                })
                .collect();
        }

        self.emit_bar_updates();
    }

    fn send_speaker_popup(&mut self) {
        let clean_sink = clean_device_name(&self.active_sink, 24);
        let vol_str = format!("{}%", self.volume);

        let mut children = Vec::new();

        children.push(json!({
            "type": "container",
            "style": "audio_banner",
            "layout": { "mode": "flex_row", "align": "center", "gap": 18.0 },
            "children": [
                {
                    "type": "ring",
                    "value": self.volume as f32,
                    "max": 100.0,
                    "stroke_width": 6.0,
                    "text": if self.muted { "MUTED".to_string() } else { vol_str.clone() },
                    "layout": { "fixed_width": 64.0, "fixed_height": 64.0 }
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_col", "gap": 4.0, "weight": 1.0, "padding": [0.0, 0.0, 0.0, 12.0] },
                    "children": [
                        {
                            "type": "text",
                            "text": "OUTPUT AUDIO",
                            "style": "audio_title",
                        },
                        {
                            "type": "text",
                            "text": clean_sink,
                            "style": "audio_device_name",
                        },
                        {
                            "type": "button",
                            "id": "audio_mute_toggle",
                            "text": if self.muted { "UNMUTE" } else { "MUTE" },
                            "style": if self.muted { "audio_mute_active" } else { "audio_mute_btn" },
                            "on_click": "event:audio:audio_mute_toggle:click",
                            "layout": { "height": 26.0, "justify": "center", "align": "center" }
                        }
                    ]
                }
            ]
        }));

        let p25_style = if !self.muted && self.volume == 25 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };
        let p50_style = if !self.muted && self.volume == 50 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };
        let p75_style = if !self.muted && self.volume == 75 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };
        let p100_style = if !self.muted && self.volume == 100 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };

        children.push(json!({
            "type": "container",
            "style": "audio_card",
            "layout": { "mode": "flex_col", "gap": 8.0 },
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "justify": "space-between", "align": "center" },
                    "children": [
                        {
                            "type": "text",
                            "text": "Output Volume",
                            "style": "audio_device_name",
                        },
                        {
                            "type": "text",
                            "text": &vol_str,
                            "style": "audio_title",
                        }
                    ]
                },
                {
                    "type": "slider",
                    "id": "audio_slider",
                    "value": self.volume as f32,
                    "min": 0.0,
                    "max": 100.0,
                    "on_change": "event:audio:audio_slider:{value}",
                    "layout": { "height": 24.0 },
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "justify": "space-between", "gap": 4.0 },
                    "children": [
                        {
                            "type": "button",
                            "id": "vol_p25",
                            "text": "25%",
                            "style": p25_style,
                            "on_click": "event:audio:vol_p25:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "vol_p50",
                            "text": "50%",
                            "style": p50_style,
                            "on_click": "event:audio:vol_p50:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "vol_p75",
                            "text": "75%",
                            "style": p75_style,
                            "on_click": "event:audio:vol_p75:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "vol_p100",
                            "text": "100%",
                            "style": p100_style,
                            "on_click": "event:audio:vol_p100:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        }
                    ]
                }
            ]
        }));

        let mut sink_buttons = Vec::new();
        sink_buttons.push(json!({
            "type": "text",
            "text": "OUTPUT DEVICES",
            "style": "audio_title",
        }));

        for sink in self.sinks.iter().take(5) {
            let icon = if sink.is_default { "✓" } else { " " };
            let style_name = if sink.is_default {
                "audio_sink_active"
            } else {
                "audio_sink_btn"
            };
            let clean_name = clean_device_name(
                if !sink.desc.is_empty() {
                    &sink.desc
                } else {
                    &sink.name
                },
                26,
            );

            sink_buttons.push(json!({
                "type": "button",
                "id": format!("sink_dev_{}", sink.id),
                "text": format!("{}  {}", icon, clean_name),
                "style": style_name,
                "on_click": format!("event:audio:sink_dev_{}:click", sink.id),
                "layout": { "height": 30.0, "justify": "start", "align": "center" }
            }));
        }

        if sink_buttons.len() <= 1 {
            sink_buttons.push(json!({
                "type": "text",
                "text": "No output devices found",
                "style": "muted",
            }));
        }

        children.push(json!({
            "type": "container",
            "style": "audio_card",
            "layout": { "mode": "flex_col", "gap": 4.0 },
            "children": sink_buttons
        }));

        let sinks_json: Vec<Value> = self
            .sinks
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "desc": s.desc,
                    "is_default": s.is_default,
                    "volume": s.volume,
                    "muted": s.muted,
                })
            })
            .collect();

        let payload = json!({
            "children": children,
            "volume": self.volume,
            "muted": self.muted,
            "is_muted": self.muted,
            "sink_name": self.active_sink,
            "sinks": sinks_json,
        });

        send_update("popup:audio", &payload);
    }

    fn send_microphone_popup(&mut self) {
        let clean_source = clean_device_name(&self.active_source, 24);
        let vol_str = format!("{}%", self.mic_volume);

        let mut children = Vec::new();

        children.push(json!({
            "type": "container",
            "style": "audio_banner",
            "layout": { "mode": "flex_row", "align": "center", "gap": 18.0 },
            "children": [
                {
                    "type": "ring",
                    "value": self.mic_volume as f32,
                    "max": 100.0,
                    "stroke_width": 6.0,
                    "text": if self.mic_muted { "MUTED".to_string() } else { vol_str.clone() },
                    "layout": { "fixed_width": 64.0, "fixed_height": 64.0 }
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_col", "gap": 4.0, "weight": 1.0, "padding": [0.0, 0.0, 0.0, 12.0] },
                    "children": [
                        {
                            "type": "text",
                            "text": "INPUT MICROPHONE",
                            "style": "audio_title",
                        },
                        {
                            "type": "text",
                            "text": clean_source,
                            "style": "audio_device_name",
                        },
                        {
                            "type": "button",
                            "id": "mic_mute_toggle",
                            "text": if self.mic_muted { "UNMUTE" } else { "MUTE" },
                            "style": if self.mic_muted { "audio_mute_active" } else { "audio_mute_btn" },
                            "on_click": "event:audio:mic_mute_toggle:click",
                            "layout": { "height": 26.0, "justify": "center", "align": "center" }
                        }
                    ]
                }
            ]
        }));

        let p25_style = if !self.mic_muted && self.mic_volume == 25 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };
        let p50_style = if !self.mic_muted && self.mic_volume == 50 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };
        let p75_style = if !self.mic_muted && self.mic_volume == 75 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };
        let p100_style = if !self.mic_muted && self.mic_volume == 100 {
            "audio_preset_active"
        } else {
            "audio_preset_btn"
        };

        children.push(json!({
            "type": "container",
            "style": "audio_card",
            "layout": { "mode": "flex_col", "gap": 8.0 },
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "justify": "space-between", "align": "center" },
                    "children": [
                        {
                            "type": "text",
                            "text": "Input Gain Level",
                            "style": "audio_device_name",
                        },
                        {
                            "type": "text",
                            "text": &vol_str,
                            "style": "audio_title",
                        }
                    ]
                },
                {
                    "type": "slider",
                    "id": "mic_slider",
                    "value": self.mic_volume as f32,
                    "min": 0.0,
                    "max": 100.0,
                    "on_change": "event:audio:mic_slider:{value}",
                    "layout": { "height": 24.0 },
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "justify": "space-between", "gap": 4.0 },
                    "children": [
                        {
                            "type": "button",
                            "id": "mic_p25",
                            "text": "25%",
                            "style": p25_style,
                            "on_click": "event:audio:mic_p25:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "mic_p50",
                            "text": "50%",
                            "style": p50_style,
                            "on_click": "event:audio:mic_p50:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "mic_p75",
                            "text": "75%",
                            "style": p75_style,
                            "on_click": "event:audio:mic_p75:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "mic_p100",
                            "text": "100%",
                            "style": p100_style,
                            "on_click": "event:audio:mic_p100:click",
                            "layout": { "height": 24.0, "weight": 1.0, "justify": "center", "align": "center" }
                        }
                    ]
                }
            ]
        }));

        let mut source_buttons = Vec::new();
        source_buttons.push(json!({
            "type": "text",
            "text": "INPUT DEVICES",
            "style": "audio_title",
        }));

        for source in self.sources.iter().take(5) {
            let icon = if source.is_default { "✓" } else { " " };
            let style_name = if source.is_default {
                "audio_source_active"
            } else {
                "audio_source_btn"
            };
            let clean_name = clean_device_name(
                if !source.desc.is_empty() {
                    &source.desc
                } else {
                    &source.name
                },
                26,
            );

            source_buttons.push(json!({
                "type": "button",
                "id": format!("source_dev_{}", source.id),
                "text": format!("{}  {}", icon, clean_name),
                "style": style_name,
                "on_click": format!("event:audio:source_dev_{}:click", source.id),
                "layout": { "height": 30.0, "justify": "start", "align": "center" }
            }));
        }

        if source_buttons.len() <= 1 {
            source_buttons.push(json!({
                "type": "text",
                "text": "No input devices found",
                "style": "muted",
            }));
        }

        children.push(json!({
            "type": "container",
            "style": "audio_card",
            "layout": { "mode": "flex_col", "gap": 4.0 },
            "children": source_buttons
        }));

        let sources_json: Vec<Value> = self
            .sources
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "desc": s.desc,
                    "is_default": s.is_default,
                    "volume": s.volume,
                    "muted": s.muted,
                })
            })
            .collect();

        let payload = json!({
            "children": children,
            "volume": self.mic_volume,
            "muted": self.mic_muted,
            "is_muted": self.mic_muted,
            "source_name": self.active_source,
            "sources": sources_json,
        });

        send_update("popup:microphone", &payload);
    }

    fn send_audio_sinks_popup(&mut self) {
        let mut dev_buttons = Vec::new();

        for sink in &self.sinks {
            let icon = if sink.is_default { "✓" } else { " " };
            let style_name = if sink.is_default {
                "audio_sink_active"
            } else {
                "audio_sink_btn"
            };
            let clean_name = clean_device_name(
                if !sink.desc.is_empty() {
                    &sink.desc
                } else {
                    &sink.name
                },
                28,
            );

            dev_buttons.push(json!({
                "type": "button",
                "id": format!("sink_dev_{}", sink.id),
                "text": format!("{}  {}", icon, clean_name),
                "style": style_name,
                "on_click": format!("event:audio:sink_dev_{}:click", sink.id),
                "layout": { "height": 32.0, "justify": "start", "align": "center", "padding": [2.0, 8.0, 2.0, 8.0] }
            }));
        }

        if dev_buttons.is_empty() {
            dev_buttons.push(json!({
                "type": "text",
                "text": "No output devices found",
                "style": "muted",
                "layout": { "padding": [8.0, 8.0, 8.0, 8.0] }
            }));
        }

        let mut children = Vec::new();
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [8.0, 12.0, 8.0, 12.0] },
            "children": [
                {
                    "type": "text",
                    "text": "OUTPUT DEVICES",
                    "style": "clean_accent",
                },
                {
                    "type": "button",
                    "id": "back_to_audio_btn",
                    "text": "‹ Audio",
                    "style": "audio_sink_btn",
                    "on_click": "popup:toggle audio",
                    "layout": { "height": 26.0, "justify": "center", "align": "center", "padding": [2.0, 8.0, 2.0, 8.0] }
                }
            ]
        }));
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 8.0, 8.0, 8.0] },
            "children": dev_buttons
        }));

        let sinks_json: Vec<Value> = self
            .sinks
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "desc": s.desc,
                    "is_default": s.is_default,
                    "volume": s.volume,
                })
            })
            .collect();

        let payload = json!({
            "children": children,
            "sinks": sinks_json,
        });

        send_update("popup:audio-sinks", &payload);
    }

    fn send_audio_sources_popup(&mut self) {
        let mut dev_buttons = Vec::new();

        for source in &self.sources {
            let icon = if source.is_default { "✓" } else { " " };
            let style_name = if source.is_default {
                "audio_source_active"
            } else {
                "audio_source_btn"
            };
            let clean_name = clean_device_name(
                if !source.desc.is_empty() {
                    &source.desc
                } else {
                    &source.name
                },
                28,
            );

            dev_buttons.push(json!({
                "type": "button",
                "id": format!("source_dev_{}", source.id),
                "text": format!("{}  {}", icon, clean_name),
                "style": style_name,
                "on_click": format!("event:audio:source_dev_{}:click", source.id),
                "layout": { "height": 32.0, "justify": "start", "align": "center", "padding": [2.0, 8.0, 2.0, 8.0] }
            }));
        }

        if dev_buttons.is_empty() {
            dev_buttons.push(json!({
                "type": "text",
                "text": "No input devices found",
                "style": "muted",
                "layout": { "padding": [8.0, 8.0, 8.0, 8.0] }
            }));
        }

        let mut children = Vec::new();
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [8.0, 12.0, 8.0, 12.0] },
            "children": [
                {
                    "type": "text",
                    "text": "INPUT DEVICES",
                    "style": "clean_accent",
                },
                {
                    "type": "button",
                    "id": "back_to_mic_btn",
                    "text": "‹ Mic",
                    "style": "chip",
                    "on_click": "popup:toggle microphone",
                    "layout": { "height": 26.0, "justify": "center", "align": "center", "padding": [2.0, 8.0, 2.0, 8.0] }
                }
            ]
        }));
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 4.0, "padding": [8.0, 8.0, 8.0, 8.0] },
            "children": dev_buttons
        }));

        let sources_json: Vec<Value> = self
            .sources
            .iter()
            .map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "desc": s.desc,
                    "is_default": s.is_default,
                    "volume": s.volume,
                })
            })
            .collect();

        let payload = json!({
            "children": children,
            "sources": sources_json,
        });

        send_update("popup:audio-sources", &payload);
    }
}

fn clean_device_name(name: &str, max_len: usize) -> String {
    let s = name
        .replace("Built-in Audio", "")
        .replace("Analog Stereo", "")
        .replace("Digital Stereo (HDMI)", "HDMI")
        .replace("Family 17h/19h HD Audio Controller", "HD Audio")
        .trim()
        .to_string();

    let s = if s.is_empty() { name.to_string() } else { s };

    if s.chars().count() > max_len {
        let short: String = s.chars().take(max_len - 2).collect();
        format!("{}..", short)
    } else {
        s
    }
}

export_wasm_module!(AudioModule);
