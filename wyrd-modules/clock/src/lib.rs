use anyhow::Result;
use chrono::{DateTime, Datelike};
use serde_json::{json, Value};
use wyrd_module_sdk::{
    export_wasm_module, local_offset_seconds, log_info, now_ms, publish, send_update, WasmModule,
};

pub struct ClockModule {
    format: String,
    month_offset: i32,
    calendar_open: bool,
    last_text: String,
}

impl Default for ClockModule {
    fn default() -> Self {
        Self {
            format: "%H:%M".to_string(),
            month_offset: 0,
            calendar_open: false,
            last_text: String::new(),
        }
    }
}

impl WasmModule for ClockModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("Clock WASM module initialized");
        if let Some(fmt) = config.get("format").and_then(|v| v.as_str()) {
            self.format = fmt.to_string();
        }
        self.render();
        Ok(())
    }

    fn tick(&mut self) {
        self.render();
        if self.calendar_open {
            self.send_calendar_popup();
        }
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        if widget_id == "calendar" || widget_id == "popup:calendar" {
            if event == "open" {
                self.calendar_open = true;
                self.send_calendar_popup();
                return;
            } else if event == "close" {
                self.calendar_open = false;
                return;
            }
        }

        if widget_id == "cal_prev" || widget_id == "popup:cal-prev" || widget_id.contains("prev") {
            self.month_offset -= 1;
            self.send_calendar_popup();
        } else if widget_id == "cal_next"
            || widget_id == "popup:cal-next"
            || widget_id.contains("next")
        {
            self.month_offset += 1;
            self.send_calendar_popup();
        } else if widget_id == "cal_today"
            || widget_id == "popup:cal-today"
            || widget_id.contains("today")
        {
            self.month_offset = 0;
            self.send_calendar_popup();
        }
    }
}

impl ClockModule {
    fn render(&mut self) {
        let offset_ms = (local_offset_seconds() as i64) * 1000;
        let ms = (now_ms() as i64) + offset_ms;
        let dt = DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::UNIX_EPOCH);

        let now = dt.format(&self.format).to_string();
        let date_str = dt.format("%a, %d %b").to_string();
        let full_text = format!("{} • {}", now, date_str);
        if full_text == self.last_text {
            return;
        }
        self.last_text = full_text.clone();

        let payload = json!({
            "text": full_text,
            "time": now,
            "raw_time": now,
            "date": date_str,
            "year": dt.year(),
            "month": dt.month(),
            "day": dt.day(),
            "month_name": dt.format("%B").to_string(),
            "day_name": dt.format("%A").to_string(),
            "tooltip": dt.format("%Y-%m-%d %A").to_string(),
        });
        send_update("clock", &payload);
        let _ = publish("clock.tick", &json!({ "time": now }));
    }

    fn send_calendar_popup(&self) {
        let offset_ms = (local_offset_seconds() as i64) * 1000;
        let ms = (now_ms() as i64) + offset_ms;
        let dt = DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::UNIX_EPOCH);
        let today_naive = dt.date_naive();

        let mut target_year = today_naive.year();
        let mut target_month = today_naive.month() as i32 + self.month_offset;
        while target_month < 1 {
            target_month += 12;
            target_year -= 1;
        }
        while target_month > 12 {
            target_month -= 12;
            target_year += 1;
        }
        let target_month = target_month as u32;
        let first_day =
            chrono::NaiveDate::from_ymd_opt(target_year, target_month, 1).unwrap_or(today_naive);

        let month_name = first_day.format("%B %Y").to_string();
        let now_time = dt.format("%H:%M:%S").to_string();
        let today_full = dt.format("%A, %d %B %Y").to_string();

        let mut children = Vec::new();

        children.push(json!({
            "type": "container",
            "style": "cal_banner",
            "layout": { "mode": "flex_col", "gap": 2.0, "align": "center", "justify": "center", "padding": [10.0, 14.0, 10.0, 14.0] },
            "children": [
                {
                    "type": "text",
                    "text": now_time,
                    "style": "cal_banner_time",
                    "layout": { "align": "center", "justify": "center" }
                },
                {
                    "type": "text",
                    "text": today_full,
                    "style": "cal_banner_date",
                    "layout": { "align": "center", "justify": "center" }
                }
            ]
        }));

        children.push(json!({
            "type": "container",
            "style": "cal_nav",
            "layout": { "mode": "flex_row", "align": "center", "justify": "space-between", "padding": [4.0, 6.0, 4.0, 12.0] },
            "children": [
                {
                    "type": "text",
                    "text": month_name,
                    "style": "cal_month",
                    "layout": { "align": "center" }
                },
                {
                    "type": "container",
                    "layout": { "mode": "flex_row", "align": "center", "gap": 6.0 },
                    "children": [
                        {
                            "type": "button",
                            "id": "cal_prev",
                            "text": "‹",
                            "style": "chip",
                            "on_click": "event:clock:cal_prev:click",
                            "layout": { "fixed_width": 30.0, "height": 28.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "cal_today",
                            "text": "Today",
                            "style": if self.month_offset == 0 { "chip_accent" } else { "chip" },
                            "on_click": "event:clock:cal_today:click",
                            "layout": { "fixed_width": 58.0, "height": 28.0, "justify": "center", "align": "center" }
                        },
                        {
                            "type": "button",
                            "id": "cal_next",
                            "text": "›",
                            "style": "chip",
                            "on_click": "event:clock:cal_next:click",
                            "layout": { "fixed_width": 30.0, "height": 28.0, "justify": "center", "align": "center" }
                        }
                    ]
                }
            ]
        }));

        let day_names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
        let mut header_cells = Vec::new();
        for (idx, day) in day_names.iter().enumerate() {
            let is_weekend = idx >= 5;
            header_cells.push(json!({
                "type": "text",
                "text": *day,
                "style": if is_weekend { "cal_header_weekend" } else { "cal_header_work" },
                "layout": { "weight": 1.0, "height": 22.0, "justify": "center", "align": "center" }
            }));
        }

        let header_row = json!({
            "type": "container",
            "layout": { "mode": "flex_row", "gap": 4.0, "align": "center", "padding": [2.0, 0.0, 6.0, 0.0] },
            "children": header_cells
        });

        let mut days_cells = Vec::new();

        let first_weekday = first_day.weekday().num_days_from_monday();
        if first_weekday > 0 {
            let prev_start = first_day - chrono::Duration::days(first_weekday as i64);
            for i in 0..first_weekday {
                let d = prev_start + chrono::Duration::days(i as i64);
                days_cells.push(json!({
                    "type": "button",
                    "id": format!("prev_day_{}", d.day()),
                    "text": format!("{}", d.day()),
                    "style": "cal_day_dim",
                    "layout": { "weight": 1.0, "height": 32.0, "justify": "center", "align": "center" }
                }));
            }
        }

        let mut cur_day = first_day;
        let mut last_cur_day = first_day;
        while cur_day.month() == target_month {
            last_cur_day = cur_day;
            let is_today = cur_day == today_naive && self.month_offset == 0;
            let day_num = cur_day.day();

            days_cells.push(json!({
                "type": "button",
                "id": format!("day_{}", day_num),
                "text": format!("{}", day_num),
                "style": if is_today { "cal_day_today" } else { "cal_day" },
                "layout": { "weight": 1.0, "height": 32.0, "justify": "center", "align": "center" }
            }));

            if let Some(next) = cur_day.succ_opt() {
                cur_day = next;
            } else {
                break;
            }
        }

        let mut next_trailing = last_cur_day + chrono::Duration::days(1);
        while days_cells.len() < 42 {
            days_cells.push(json!({
                "type": "button",
                "id": format!("next_day_{}", next_trailing.day()),
                "text": format!("{}", next_trailing.day()),
                "style": "cal_day_dim",
                "layout": { "weight": 1.0, "height": 32.0, "justify": "center", "align": "center" }
            }));
            next_trailing += chrono::Duration::days(1);
        }

        let mut card_children = Vec::new();
        card_children.push(header_row);

        for chunk in days_cells.chunks(7) {
            card_children.push(json!({
                "type": "container",
                "layout": { "mode": "flex_row", "gap": 4.0, "align": "center", "padding": [2.0, 0.0, 2.0, 0.0] },
                "children": chunk.to_vec()
            }));
        }

        children.push(json!({
            "type": "container",
            "style": "cal_grid",
            "layout": { "mode": "flex_col", "align": "stretch", "gap": 4.0, "padding": [10.0, 10.0, 10.0, 10.0] },
            "children": card_children
        }));

        let payload = json!({
            "children": children,
            "time": dt.format(&self.format).to_string(),
            "date": dt.format("%a, %d %b").to_string(),
            "year": dt.year(),
            "month": dt.month(),
            "day": dt.day(),
            "month_name": dt.format("%B").to_string(),
            "day_name": dt.format("%A").to_string(),
        });

        send_update("popup:calendar", &payload);
        send_update("popup:clock", &payload);
    }
}

export_wasm_module!(ClockModule);
