//! WASM System Metrics Module.

use anyhow::Result;
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, log_info, publish, read_file, resolve_icon, send_update, WasmModule,
};

#[derive(Default)]
pub struct SystemModule {
    prev_total: u64,
    prev_idle: u64,
    popup_open: bool,
    last_text: String,
}

impl WasmModule for SystemModule {
    fn init(&mut self, _config: Value) -> Result<()> {
        log_info("System metrics WASM module initialized");
        if let Some((t, i)) = self.read_cpu_stat() {
            self.prev_total = t;
            self.prev_idle = i;
        }
        self.update_metrics();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if widget_id == "system" || widget_id == "popup:system" {
            if event == "open" {
                self.popup_open = true;
                self.update_metrics();
                return;
            } else if event == "close" {
                self.popup_open = false;
                return;
            }
        }
        let is_sys = widget_id == "cpu"
            || widget_id == "ram"
            || widget_id == "memory"
            || widget_id == "system"
            || widget_id.contains("cpu")
            || widget_id.contains("ram")
            || widget_id.contains("mem")
            || widget_id.contains("sys");
        if is_sys && (event == "open" || event == "click") {
            if event == "open" {
                self.popup_open = true;
            }
            self.update_metrics();
        }
    }

    fn tick(&mut self) {
        self.update_metrics();
    }
}

impl SystemModule {
    fn update_metrics(&mut self) {
        let mut cpu = 0.0f32;
        if let Some((cur_total, cur_idle)) = self.read_cpu_stat() {
            let total_delta = cur_total.saturating_sub(self.prev_total);
            let idle_delta = cur_idle.saturating_sub(self.prev_idle);
            self.prev_total = cur_total;
            self.prev_idle = cur_idle;

            if total_delta > 0 {
                cpu = (total_delta.saturating_sub(idle_delta) as f32 * 100.0 / total_delta as f32)
                    .clamp(0.0, 100.0);
            }
        }

        let (mem_pct, used_gb, total_gb) = self.read_mem_details().unwrap_or((0.0, 0.0, 0.0));

        let full_text = format!("CPU {:.0}% • RAM {:.0}%", cpu, mem_pct);
        if full_text != self.last_text {
            self.last_text = full_text.clone();
            let icon_path = resolve_icon("utilities-system-monitor");
            let sys_payload = json!({
                "text": full_text,
                "icon": "utilities-system-monitor",
                "icon_path": icon_path,
                "path": icon_path,
                "cpu": format!("{:.0}", cpu),
                "cpu_pct": cpu,
                "mem": format!("{:.0}", mem_pct),
                "memory": format!("{:.0}", mem_pct),
                "ram": format!("{:.0}", mem_pct),
                "ram_pct": mem_pct,
                "used_gb": used_gb,
                "total_gb": total_gb,
                "ram_used_gb": used_gb,
                "ram_total_gb": total_gb,
            });

            send_update("system", &sys_payload);

            let _ = publish(
                "system.metrics",
                &json!({
                    "cpu": cpu,
                    "memory": mem_pct,
                    "used_gb": used_gb,
                    "total_gb": total_gb
                }),
            );
        }

        if self.popup_open {
            self.send_system_popup_with_metrics(cpu, mem_pct, used_gb, total_gb);
        }
    }

    fn send_system_popup_with_metrics(&self, cpu: f32, mem_pct: f32, used_gb: f32, total_gb: f32) {
        let mut children = Vec::new();

        // 1. Side-by-side Progress Rings Card for CPU & RAM
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-around", "padding": [12.0, 14.0, 12.0, 14.0] },
            "children": [
                {
                    "type": "container",
                    "layout": { "mode": "flex_col", "align": "center", "gap": 6.0 },
                    "children": [
                        {
                            "type": "ring",
                            "value": cpu,
                            "max": 100.0,
                            "stroke_width": 6.0,
                            "text": format!("{:.0}%", cpu),
                            "layout": { "fixed_width": 64.0, "fixed_height": 64.0 }
                        },
                        {
                            "type": "text",
                            "text": "CPU Usage",
                            "style": "clean_accent",
                        }
                    ]
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_col", "align": "center", "gap": 6.0 },
                    "children": [
                        {
                            "type": "ring",
                            "value": mem_pct,
                            "max": 100.0,
                            "stroke_width": 6.0,
                            "text": format!("{:.0}%", mem_pct),
                            "layout": { "fixed_width": 64.0, "fixed_height": 64.0 }
                        },
                        {
                            "type": "text",
                            "text": "RAM Usage",
                            "style": "clean_accent",
                        }
                    ]
                }
            ]
        }));

        // 2. Metrics Details Card
        children.push(json!({
            "type": "container",
            "style": "card",
            "layout": { "mode": "flex_col", "gap": 6.0, "padding": [8.0, 12.0, 8.0, 12.0] },
            "children": [
                {
                    "type": "text",
                    "text": format!("Processor Load: {:.1}%", cpu),
                    "style": "clean_item",
                },
                {
                    "type": "text",
                    "text": format!("Memory: {:.2} GB / {:.2} GB ({:.0}%)", used_gb, total_gb, mem_pct),
                    "style": "clean_item",
                }
            ]
        }));

        let payload = json!({
            "children": children,
            "cpu": cpu,
            "cpu_pct": cpu,
            "memory": mem_pct,
            "ram": mem_pct,
            "ram_pct": mem_pct,
            "used_gb": used_gb,
            "total_gb": total_gb,
            "ram_used_gb": used_gb,
            "ram_total_gb": total_gb,
        });

        send_update("popup:cpu", &payload);
        send_update("popup:ram", &payload);
        send_update("popup:memory", &payload);
        send_update("popup:system", &payload);
    }

    fn read_cpu_stat(&self) -> Option<(u64, u64)> {
        let content = read_file("/proc/stat")?;
        let first_line = content.lines().next()?;
        if !first_line.starts_with("cpu ") {
            return None;
        }

        let mut parts = first_line.split_whitespace().skip(1);
        let user: u64 = parts.next()?.parse().ok()?;
        let nice: u64 = parts.next()?.parse().ok()?;
        let system: u64 = parts.next()?.parse().ok()?;
        let idle: u64 = parts.next()?.parse().ok()?;
        let iowait: u64 = parts.next()?.parse().ok()?;
        let irq: u64 = parts.next()?.parse().ok()?;
        let softirq: u64 = parts.next()?.parse().ok()?;
        let steal: u64 = parts.next()?.parse().ok()?;

        let total = user + nice + system + idle + iowait + irq + softirq + steal;
        let idle_total = idle + iowait;

        Some((total, idle_total))
    }

    fn read_mem_details(&self) -> Option<(f32, f32, f32)> {
        let content = read_file("/proc/meminfo")?;
        let mut total_kb = 0u64;
        let mut avail_kb = 0u64;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = line.split_whitespace().nth(1)?.parse().ok()?;
            } else if line.starts_with("MemAvailable:") {
                avail_kb = line.split_whitespace().nth(1)?.parse().ok()?;
            }
        }

        if total_kb == 0 {
            return None;
        }

        let used_kb = total_kb.saturating_sub(avail_kb);
        let pct = (used_kb as f32 * 100.0 / total_kb as f32).clamp(0.0, 100.0);
        let total_gb = total_kb as f32 / (1024.0 * 1024.0);
        let used_gb = used_kb as f32 / (1024.0 * 1024.0);

        Some((pct, used_gb, total_gb))
    }
}

export_wasm_module!(SystemModule);
