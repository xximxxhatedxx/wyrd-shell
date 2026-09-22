//! WASM Launcher Module.

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashSet;
use wyrd_module_sdk::{
    export_wasm_module, get_application_dirs, get_env, list_dir, log_info, publish, read_file,
    request_surface, resolve_icon, send_update, spawn_process, WasmModule,
};

#[derive(Debug, Clone)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub exec: String,
    pub comment: String,
    pub icon: String,
    pub terminal: bool,
    pub categories: Vec<String>,
}

#[derive(Default)]
pub struct LauncherModule {
    apps: Vec<AppInfo>,
    query: String,
    selected: usize,
    initialized: bool,
    config: Value,
    max_items: usize,
}

impl WasmModule for LauncherModule {
    fn init(&mut self, config: Value) -> Result<()> {
        log_info("Launcher WASM module initialized with in-memory application index");
        let configured_limit = config
            .get("max_items")
            .or_else(|| config.get("max_results"))
            .or_else(|| config.get("limit"))
            .or_else(|| config.get("count"))
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
            })
            .map(|v| v as usize);

        let env_limit = get_env("WYRD_LAUNCHER_MAX_ITEMS").and_then(|s| s.parse::<usize>().ok());

        let height_derived_limit = config
            .get("height")
            .or_else(|| config.get("max_height"))
            .and_then(|v| v.as_f64())
            .map(|h| {
                let available = (h - 60.0).max(37.0);
                (available / 37.0).floor() as usize
            });

        self.max_items = configured_limit
            .or(env_limit)
            .or(height_derived_limit)
            .unwrap_or(8);

        self.config = config;
        self.load_apps();
        self.initialized = true;
        self.send_launcher_popup();
        Ok(())
    }

    fn on_event(&mut self, widget_id: &str, event: &str) {
        log_info(&format!("Launcher event: {} on {}", event, widget_id));

        if widget_id == "launcher" || widget_id == "appmenu" || widget_id == "launcher_btn" {
            if event == "open" || event == "click" {
                self.query.clear();
                self.selected = 0;
                self.send_launcher_popup();
            }
            return;
        }

        if widget_id.starts_with("app_launch_") {
            let app_id = widget_id.trim_start_matches("app_launch_");
            if let Some(app) = self.apps.iter().find(|a| a.id == app_id) {
                let _ = launch_app(&app.exec, app.terminal);
                let _ = publish(
                    "launcher.launched",
                    &json!({ "id": app.id, "exec": app.exec }),
                );
                request_surface("close", &json!({ "id": "launcher", "name": "launcher" }));
                self.query.clear();
                self.selected = 0;
            }
            return;
        }

        if widget_id == "launcher_search" {
            if event == "submit" || event == "enter" {
                let matches = self.filtered_apps();
                if let Some(target_app) = matches.get(self.selected).or_else(|| matches.first()) {
                    let _ = launch_app(&target_app.exec, target_app.terminal);
                    let _ = publish(
                        "launcher.launched",
                        &json!({ "id": target_app.id, "exec": target_app.exec }),
                    );
                    request_surface("close", &json!({ "id": "launcher", "name": "launcher" }));
                    self.query.clear();
                    self.selected = 0;
                }
            } else if event == "down" {
                let matches_len = self.filtered_apps().len();
                if matches_len > 0 {
                    self.selected = (self.selected + 1) % matches_len;
                    self.send_launcher_popup();
                }
            } else if event == "up" {
                let matches_len = self.filtered_apps().len();
                if matches_len > 0 {
                    self.selected = if self.selected == 0 {
                        matches_len - 1
                    } else {
                        self.selected - 1
                    };
                    self.send_launcher_popup();
                }
            } else {
                self.query = event.to_string();
                self.selected = 0;
                self.send_launcher_popup();
            }
        }
    }

    fn tick(&mut self) {
        if !self.initialized {
            self.load_apps();
            self.initialized = true;
        }
    }
}

fn launch_app(command: &str, terminal: bool) -> bool {
    let argv = match shell_words::split(command) {
        Ok(a) if !a.is_empty() => a,
        _ => return false,
    };
    if terminal {
        if let Some(term) = get_env("TERMINAL") {
            let mut term_args = vec!["-e"];
            term_args.extend(argv.iter().map(String::as_str));
            if spawn_process(&term, &term_args) {
                return true;
            }
        }
        for term in &[
            "foot",
            "kitty",
            "alacritty",
            "wezterm",
            "ghostty",
            "gnome-terminal",
            "konsole",
            "xfce4-terminal",
            "xterm",
        ] {
            let mut term_args = vec!["-e"];
            term_args.extend(argv.iter().map(String::as_str));
            if spawn_process(term, &term_args) {
                return true;
            }
        }
        false
    } else {
        let (program, args) = argv.split_first().unwrap();
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        spawn_process(program, &arg_refs)
    }
}

impl LauncherModule {
    fn load_apps(&mut self) {
        let mut apps = Vec::new();
        let mut seen_ids = HashSet::new();

        let mut search_dirs = get_application_dirs();

        // Dynamically include any directories passed via config or environment
        if let Some(extra_dirs) = self
            .config
            .get("application_dirs")
            .and_then(|v| v.as_array())
        {
            for d in extra_dirs.iter().filter_map(|v| v.as_str()) {
                if !search_dirs.iter().any(|existing| existing == d) {
                    search_dirs.push(d.to_string());
                }
            }
        }
        if let Some(home) = self.config.get("home").and_then(|v| v.as_str()) {
            let user_apps = format!("{}/.local/share/applications", home.trim_end_matches('/'));
            if !search_dirs.iter().any(|existing| existing == &user_apps) {
                search_dirs.insert(0, user_apps);
            }
        }
        if let Some(xdg_data_dirs) = self.config.get("xdg_data_dirs").and_then(|v| v.as_str()) {
            for entry in xdg_data_dirs
                .split(':')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
            {
                let p = if entry.ends_with("/applications") {
                    entry.to_string()
                } else {
                    format!("{}/applications", entry.trim_end_matches('/'))
                };
                if !search_dirs.iter().any(|existing| existing == &p) {
                    search_dirs.push(p);
                }
            }
        }

        for dir in &search_dirs {
            let files = list_dir(dir);
            for path_str in files {
                if !path_str.ends_with(".desktop") {
                    continue;
                }
                let file_stem = path_str
                    .rsplit('/')
                    .next()
                    .unwrap_or("")
                    .trim_end_matches(".desktop");
                if file_stem.is_empty() || !seen_ids.insert(file_stem.to_string()) {
                    continue;
                }

                if let Some(content) = read_file(&path_str) {
                    if let Some(app) = parse_desktop_content(&content, file_stem) {
                        apps.push(app);
                    }
                }
            }
        }

        apps.sort_by_key(|a| a.name.to_lowercase());
        self.apps = apps;
    }

    fn filtered_apps(&self) -> Vec<&AppInfo> {
        let q = self.query.trim().to_lowercase();
        let limit = self.max_items;
        if q.is_empty() {
            return self.apps.iter().take(limit).collect();
        }
        self.apps
            .iter()
            .filter(|app| {
                app.name.to_lowercase().contains(&q)
                    || app.comment.to_lowercase().contains(&q)
                    || app.id.to_lowercase().contains(&q)
                    || app.exec.to_lowercase().contains(&q)
            })
            .take(limit)
            .collect()
    }

    fn send_launcher_popup(&self) {
        let mut children = Vec::new();

        let search_icon = resolve_icon("system-search");
        let mut search_header_children = Vec::new();
        if let Some(ref icon) = search_icon {
            search_header_children.push(json!({
                "type": if icon.ends_with(".svg") { "svg" } else { "image" },
                "path": icon,
                "layout": { "width": 18.0, "height": 18.0 }
            }));
        }
        search_header_children.push(json!({
            "type": "text_input",
            "id": "launcher_search",
            "text": self.query,
            "placeholder": "Type to search applications...",
            "focused": true,
            "style": "launcher_input",
            "layout": { "height": 30.0, "weight": 1.0 }
        }));

        children.push(json!({
            "type": "container",
            "style": "launcher_search",
            "layout": { "mode": "flex_row", "gap": 8.0, "align": "center", "padding": [8.0, 10.0, 8.0, 10.0] },
            "children": search_header_children
        }));

        // 2. Applications List
        let matches = self.filtered_apps();
        let mut app_buttons = Vec::new();

        if matches.is_empty() {
            app_buttons.push(json!({
                "type": "text",
                "text": "No applications found",
                "style": "launcher_empty",
                "layout": { "padding": [12.0, 8.0, 12.0, 8.0], "justify": "center" }
            }));
        } else {
            for (idx, app) in matches.iter().enumerate() {
                let resolved_icon = resolve_icon(&app.icon)
                    .or_else(|| resolve_icon(&app.id))
                    .or_else(|| {
                        let prog = app.exec.split_whitespace().next().unwrap_or("");
                        let name = std::path::Path::new(prog).file_name()?.to_str()?;
                        resolve_icon(name)
                    })
                    .or_else(|| resolve_icon("applications-other"))
                    .or_else(|| resolve_icon("application-x-executable"));

                let item_style = if idx == self.selected {
                    "launcher_item_active"
                } else {
                    "launcher_item"
                };

                let mut row_children = Vec::new();
                if let Some(ref icon_path) = resolved_icon {
                    row_children.push(json!({
                        "type": if icon_path.ends_with(".svg") { "svg" } else { "image" },
                        "path": icon_path,
                        "layout": { "width": 20.0, "height": 20.0 }
                    }));
                } else {
                    row_children.push(json!({
                        "type": "text",
                        "text": "•",
                    }));
                }

                row_children.push(json!({
                    "type": "text",
                    "text": &app.name,
                }));

                app_buttons.push(json!({
                    "type": "button",
                    "id": format!("app_launch_{}", app.id),
                    "style": item_style,
                    "on_click": format!("event:launcher:app_launch_{}:click", app.id),
                    "layout": {
                        "mode": "flex_row",
                        "gap": 10.0,
                        "height": 34.0,
                        "justify": "start",
                        "align": "center",
                        "padding": [4.0, 10.0, 4.0, 10.0]
                    },
                    "children": row_children
                }));
            }
        }

        children.push(json!({
            "type": "container",
            "style": "launcher_list",
            "layout": {
                "mode": "flex_col",
                "gap": 3.0,
                "align": "stretch",
                "padding": [7.0, 7.0, 7.0, 7.0],
                "scroll_y": true,
                "max_height": 380.0
            },
            "children": app_buttons
        }));

        let payload = json!({
            "children": children
        });

        send_update("popup:launcher", &payload);
        send_update("popup:appmenu", &payload);
        send_update("popup_root", &payload);
    }
}

fn clean_exec_line(raw: &str) -> String {
    let tokens = shell_words::split(raw)
        .unwrap_or_else(|_| raw.split_whitespace().map(|s| s.to_string()).collect());
    let mut filtered = Vec::new();
    for token in tokens {
        if token.starts_with('%') && token.len() == 2 {
            continue;
        }
        if token == "@@" || token.starts_with("@@") {
            continue;
        }
        filtered.push(token);
    }
    if filtered.last().map(|s| s.as_str()) == Some("--") {
        filtered.pop();
    }
    shell_words::join(&filtered)
}

fn parse_desktop_content(content: &str, id: &str) -> Option<AppInfo> {
    let mut in_desktop_entry = false;
    let mut name = None;
    let mut exec = None;
    let mut comment = None;
    let mut icon = None;
    let mut no_display = false;
    let mut terminal = false;
    let mut categories = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_desktop_entry || line.starts_with('#') || !line.contains('=') {
            continue;
        }

        let mut parts = line.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next()?.trim();

        match key {
            "Name" if name.is_none() => name = Some(value.to_string()),
            k if k.starts_with("Name[") && name.is_none() => name = Some(value.to_string()),
            "Exec" if exec.is_none() => {
                exec = Some(clean_exec_line(value));
            }
            "Comment" if comment.is_none() => comment = Some(value.to_string()),
            "Icon" if icon.is_none() => icon = Some(value.to_string()),
            "NoDisplay" => no_display = value.eq_ignore_ascii_case("true"),
            "Terminal" => terminal = value.eq_ignore_ascii_case("true"),
            "Categories" => {
                categories = value
                    .split(';')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
            _ => {}
        }
    }

    if no_display || name.is_none() || exec.is_none() {
        return None;
    }

    Some(AppInfo {
        id: id.to_string(),
        name: name.unwrap(),
        exec: exec.unwrap(),
        comment: comment.unwrap_or_default(),
        icon: icon.unwrap_or_default(),
        terminal,
        categories,
    })
}

export_wasm_module!(LauncherModule);
