//! High-level SDK for writing WASM guest modules for Wyrd Shell.

#[cfg(target_arch = "wasm32")]
use anyhow::anyhow;
use anyhow::Result;
use serde_json::Value;

pub trait WasmModule {
    fn init(&mut self, config: Value) -> Result<()>;
    fn on_event(&mut self, _widget_id: &str, _event: &str) {}
    fn on_topic(&mut self, _topic: &str, _value: &Value) {}
    fn on_dbus_signal(&mut self, _interface: &str, _member: &str, _payload: &Value) {}
    fn tick(&mut self) {}
    fn shutdown(&mut self) {}
}

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "wyrd_host")]
extern "C" {
    fn host_send_update(
        widget_ptr: *const u8,
        widget_len: usize,
        payload_ptr: *const u8,
        payload_len: usize,
    ) -> u32;

    fn host_request_surface(
        action_ptr: *const u8,
        action_len: usize,
        params_ptr: *const u8,
        params_len: usize,
    ) -> u32;

    fn host_publish(
        topic_ptr: *const u8,
        topic_len: usize,
        val_ptr: *const u8,
        val_len: usize,
    ) -> u32;

    fn host_request_capability(
        cap_ptr: *const u8,
        cap_len: usize,
        action_ptr: *const u8,
        action_len: usize,
        params_ptr: *const u8,
        params_len: usize,
    ) -> u32;

    fn host_focus_request(popup_ptr: *const u8, popup_len: usize) -> u32;

    fn host_log(level: u32, msg_ptr: *const u8, msg_len: usize);

    fn host_now_ms() -> u64;
    fn host_local_offset_seconds() -> i32;

    fn host_read_file(
        path_ptr: *const u8,
        path_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    #[allow(dead_code)]
    fn host_exec_command(
        cmd_ptr: *const u8,
        cmd_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_exec_process(
        prog_ptr: *const u8,
        prog_len: usize,
        args_ptr: *const u8,
        args_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_dbus_call(
        bus_ptr: *const u8,
        bus_len: usize,
        service_ptr: *const u8,
        service_len: usize,
        path_ptr: *const u8,
        path_len: usize,
        iface_ptr: *const u8,
        iface_len: usize,
        member_ptr: *const u8,
        member_len: usize,
        args_ptr: *const u8,
        args_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_dbus_subscribe(
        bus_ptr: *const u8,
        bus_len: usize,
        service_ptr: *const u8,
        service_len: usize,
        path_ptr: *const u8,
        path_len: usize,
        iface_ptr: *const u8,
        iface_len: usize,
        member_ptr: *const u8,
        member_len: usize,
    ) -> i32;

    fn host_file_exists(path_ptr: *const u8, path_len: usize) -> i32;

    fn host_list_dir(
        path_ptr: *const u8,
        path_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_unix_socket_request(
        path_ptr: *const u8,
        path_len: usize,
        payload_ptr: *const u8,
        payload_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_unix_socket_subscribe(socket_ptr: *const u8, socket_len: usize) -> i32;

    fn host_get_env(
        key_ptr: *const u8,
        key_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_resolve_icon(
        name_ptr: *const u8,
        name_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;

    fn host_read_dir(
        path_ptr: *const u8,
        path_len: usize,
        out_ptr: *mut u8,
        out_max_len: usize,
    ) -> i32;
}

#[allow(unused_variables)]
pub fn read_file(path: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = vec![0u8; 65536];
        // SAFETY: `path` string slice and `buf` vector are valid, non-null guest memory regions
        // allocated in WASM linear memory. The host reads up to `path.len()` and writes up to `buf.len()`.
        let n = unsafe { host_read_file(path.as_ptr(), path.len(), buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            String::from_utf8(buf[..n as usize].to_vec()).ok()
        } else {
            None
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(path).ok()
    }
}

#[allow(unused_variables)]
pub fn read_dir(path: &str) -> Option<Vec<String>> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = vec![0u8; 65536];
        // SAFETY: `path` slice and `buf` output buffer are valid, non-null guest linear memory allocations.
        // The host validates memory bounds before copying directory listing JSON into `buf`.
        let n = unsafe { host_read_dir(path.as_ptr(), path.len(), buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            let s = String::from_utf8_lossy(&buf[..n as usize]);
            serde_json::from_str(&s).ok()
        } else {
            None
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let rd = std::fs::read_dir(path).ok()?;
        Some(
            rd.flatten()
                .filter_map(|e| e.file_name().into_string().ok())
                .collect(),
        )
    }
}

/// Invokes a D-Bus method call natively via the host bridge.
#[allow(unused_variables)]
pub fn dbus_call(
    bus: &str,
    service: &str,
    path: &str,
    interface: &str,
    member: &str,
    args: &Value,
) -> Option<Value> {
    #[cfg(target_arch = "wasm32")]
    {
        let args_json = serde_json::to_string(args).unwrap_or_else(|_| "[]".to_string());
        let mut buf = vec![0u8; 65536];
        // SAFETY: All string parameters and the response buffer point to valid, initialized memory ranges
        // inside the module's linear memory. The host checks limits and safely bounds writes to `buf.len()`.
        let n = unsafe {
            host_dbus_call(
                bus.as_ptr(),
                bus.len(),
                service.as_ptr(),
                service.len(),
                path.as_ptr(),
                path.len(),
                interface.as_ptr(),
                interface.len(),
                member.as_ptr(),
                member.len(),
                args_json.as_ptr(),
                args_json.len(),
                buf.as_mut_ptr(),
                buf.len(),
            )
        };
        if n > 0 {
            let json_str = String::from_utf8(buf[..n as usize].to_vec()).ok()?;
            serde_json::from_str(&json_str).ok()
        } else {
            None
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

/// Subscribes to a D-Bus signal via the host bridge.
#[allow(unused_variables)]
pub fn dbus_subscribe(bus: &str, service: &str, path: &str, interface: &str, member: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: All pointer and length pairs represent valid UTF-8 string slices within WASM memory.
        let res = unsafe {
            host_dbus_subscribe(
                bus.as_ptr(),
                bus.len(),
                service.as_ptr(),
                service.len(),
                path.as_ptr(),
                path.len(),
                interface.as_ptr(),
                interface.len(),
                member.as_ptr(),
                member.len(),
            )
        };
        res == 0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

/// Checks if a file exists without spawning any subprocess.
#[allow(unused_variables)]
pub fn file_exists(path: &str) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: `path` string pointer and length point to valid guest memory.
        let res = unsafe { host_file_exists(path.as_ptr(), path.len()) };
        res == 1
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::path::Path::new(path).exists()
    }
}

/// Lists directory contents natively without spawning `find` or `ls`.
#[allow(unused_variables)]
pub fn list_dir(path: &str) -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = vec![0u8; 131072];
        // SAFETY: `path` string pointer and `buf` destination are valid allocations in guest WASM memory.
        let n = unsafe { host_list_dir(path.as_ptr(), path.len(), buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            if let Ok(json_str) = String::from_utf8(buf[..n as usize].to_vec()) {
                serde_json::from_str(&json_str).unwrap_or_default()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut res = Vec::new();
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                res.push(entry.path().to_string_lossy().to_string());
            }
        }
        res
    }
}

/// Executes an external binary directly with argv array (never through shell).
#[allow(unused_variables)]
pub fn exec_process(prog: &str, args: &[&str]) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let args_json =
            serde_json::to_string(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string());
        let mut buf = vec![0u8; 65536];
        // SAFETY: `prog`, `args_json`, and `buf` slices reside within valid guest WASM memory.
        // The host verifies the capability manifest before executing and bounds output writes to `buf.len()`.
        let n = unsafe {
            host_exec_process(
                prog.as_ptr(),
                prog.len(),
                args_json.as_ptr(),
                args_json.len(),
                buf.as_mut_ptr(),
                buf.len(),
            )
        };
        if n > 0 {
            String::from_utf8(buf[..n as usize].to_vec()).ok()
        } else {
            None
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let out = std::process::Command::new(prog).args(args).output().ok()?;
        String::from_utf8(out.stdout).ok()
    }
}

/// Spawns an external binary directly and asynchronously in the background (never waits or blocks).
#[allow(unused_variables)]
pub fn spawn_process(prog: &str, args: &[&str]) -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let args_json =
            serde_json::to_string(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>())
                .unwrap_or_else(|_| "[]".to_string());
        // SAFETY: `prog` and `args_json` are valid string slices in guest memory; null pointer passed for output buffer since output is discarded.
        let n = unsafe {
            host_exec_process(
                prog.as_ptr(),
                prog.len(),
                args_json.as_ptr(),
                args_json.len(),
                std::ptr::null_mut(),
                0,
            )
        };
        n >= 0
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::process::Command::new(prog)
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
    }
}

/// Reads an environment variable from the host environment.
#[allow(unused_variables)]
pub fn get_env(key: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = vec![0u8; 4096];
        // SAFETY: `key` and `buf` are valid guest allocations. The host enforces environment capabilities and bounds writes.
        let n = unsafe { host_get_env(key.as_ptr(), key.len(), buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            String::from_utf8(buf[..n as usize].to_vec()).ok()
        } else {
            None
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var(key).ok()
    }
}

/// Resolves a FreeDesktop icon name or application class to an absolute SVG/PNG file path.
#[allow(unused_variables)]
pub fn resolve_icon(name: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = vec![0u8; 1024];
        // SAFETY: `name` and destination `buf` are valid guest memory slices.
        let n =
            unsafe { host_resolve_icon(name.as_ptr(), name.len(), buf.as_mut_ptr(), buf.len()) };
        if n > 0 {
            String::from_utf8(buf[..n as usize].to_vec()).ok()
        } else {
            None
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        None
    }
}

/// Resolves standard XDG application directories according to the Freedesktop XDG Base Directory specification.
pub fn get_application_dirs() -> Vec<String> {
    let mut dirs = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1. User applications from $XDG_DATA_HOME/applications or $HOME/.local/share/applications
    if let Some(data_home) = get_env("XDG_DATA_HOME").filter(|s| !s.trim().is_empty()) {
        let p = format!("{}/applications", data_home.trim_end_matches('/'));
        if seen.insert(p.clone()) {
            dirs.push(p);
        }
    } else if let Some(home) = get_env("HOME").filter(|s| !s.trim().is_empty()) {
        let p = format!("{}/.local/share/applications", home.trim_end_matches('/'));
        if seen.insert(p.clone()) {
            dirs.push(p);
        }
    }

    // 2. System and Flatpak directories from $XDG_DATA_DIRS
    if let Some(data_dirs) = get_env("XDG_DATA_DIRS").filter(|s| !s.trim().is_empty()) {
        for entry in data_dirs
            .split(':')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
        {
            let p = if entry.ends_with("/applications") {
                entry.to_string()
            } else {
                format!("{}/applications", entry.trim_end_matches('/'))
            };
            if seen.insert(p.clone()) {
                dirs.push(p);
            }
        }
    } else {
        for fallback in &["/usr/local/share/applications", "/usr/share/applications"] {
            let p = fallback.to_string();
            if seen.insert(p.clone()) {
                dirs.push(p);
            }
        }
    }

    dirs
}

#[allow(unused_variables)]
pub fn exec_command(cmd: &str) -> Option<String> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }
    let prog = tokens[0];
    let args = &tokens[1..];
    exec_process(prog, args)
}

#[allow(unused_variables)]
pub fn send_update(widget_id: &str, payload: &Value) {
    #[cfg(target_arch = "wasm32")]
    {
        let payload_json = serde_json::to_string(payload).unwrap_or_else(|_| "{}".to_string());
        // SAFETY: `widget_id` and `payload_json` represent valid UTF-8 string slices within WASM linear memory.
        unsafe {
            host_send_update(
                widget_id.as_ptr(),
                widget_id.len(),
                payload_json.as_ptr(),
                payload_json.len(),
            );
        }
    }
}

#[allow(unused_variables)]
pub fn request_surface(action: &str, params: &Value) {
    #[cfg(target_arch = "wasm32")]
    {
        let params_json = serde_json::to_string(params).unwrap_or_else(|_| "{}".to_string());
        // SAFETY: `action` and `params_json` slices point to valid initialized guest memory.
        unsafe {
            host_request_surface(
                action.as_ptr(),
                action.len(),
                params_json.as_ptr(),
                params_json.len(),
            );
        }
    }
}

#[allow(unused_variables)]
pub fn publish(topic: &str, value: &Value) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        let val_json = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
        // SAFETY: String pointers and lengths represent valid initialized guest memory.
        let res = unsafe {
            host_publish(
                topic.as_ptr(),
                topic.len(),
                val_json.as_ptr(),
                val_json.len(),
            )
        };
        if res == 0 {
            Ok(())
        } else if res == 403 {
            Err(anyhow!(
                "Permission Denied: publish to topic '{}' not allowed in manifest",
                topic
            ))
        } else {
            Err(anyhow!(
                "publish to topic '{}' failed with code {}",
                topic,
                res
            ))
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(())
    }
}

#[allow(unused_variables)]
pub fn request_capability(capability: &str, action: &str, params: &Value) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        let params_json = serde_json::to_string(params).unwrap_or_else(|_| "{}".to_string());
        // SAFETY: All string slices point to valid initialized guest memory regions.
        let res = unsafe {
            host_request_capability(
                capability.as_ptr(),
                capability.len(),
                action.as_ptr(),
                action.len(),
                params_json.as_ptr(),
                params_json.len(),
            )
        };
        if res == 0 {
            Ok(())
        } else if res == 403 {
            Err(anyhow!(
                "Permission Denied: capability '{}' not granted in manifest",
                capability
            ))
        } else {
            Err(anyhow!(
                "capability '{}' request failed with code {}",
                capability,
                res
            ))
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Ok(())
    }
}

#[allow(unused_variables)]
pub fn request_focus(popup_id: &str) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: `popup_id` points to a valid guest string slice.
    unsafe {
        host_focus_request(popup_id.as_ptr(), popup_id.len());
    }
}

#[allow(unused_variables)]
pub fn log_info(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: `msg` points to a valid UTF-8 string slice in guest memory.
    unsafe {
        host_log(2, msg.as_ptr(), msg.len());
    }
}

#[allow(unused_variables)]
pub fn log_warn(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: `msg` points to a valid UTF-8 string slice in guest memory.
    unsafe {
        host_log(3, msg.as_ptr(), msg.len());
    }
}

#[allow(unused_variables)]
pub fn log_error(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: `msg` points to a valid UTF-8 string slice in guest memory.
    unsafe {
        host_log(4, msg.as_ptr(), msg.len());
    }
}

pub fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: `host_now_ms` is a stateless host function returning a primitive 64-bit integer timestamp.
    unsafe {
        host_now_ms()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

pub fn local_offset_seconds() -> i32 {
    #[cfg(target_arch = "wasm32")]
    // SAFETY: `host_local_offset_seconds` is a stateless host function returning a primitive 32-bit offset.
    unsafe {
        host_local_offset_seconds()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

pub fn unix_socket_request(socket_path: &str, payload: &str) -> Result<String, i32> {
    #[cfg(target_arch = "wasm32")]
    {
        let mut buf = vec![0u8; 65536];
        // SAFETY: `socket_path`, `payload`, and `buf` point to valid guest memory ranges. The host checks bounds.
        let res = unsafe {
            host_unix_socket_request(
                socket_path.as_ptr(),
                socket_path.len(),
                payload.as_ptr(),
                payload.len(),
                buf.as_mut_ptr(),
                buf.len(),
            )
        };
        if res >= 0 {
            let s = String::from_utf8_lossy(&buf[..res as usize]).to_string();
            Ok(s)
        } else {
            Err(res)
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (socket_path, payload);
        Err(-404)
    }
}

/// Register a persistent event subscription on a Unix socket daemon.
///
/// The shell opens a background connection to `socket_name`, sends
/// `{"cmd":"subscribe"}`, and delivers every `{"event":"..."}` line
/// from the daemon as `on_event(socket_name, "changed")` calls on this module.
///
/// Call once from `init()`. Returns `Ok(())` if the subscription was registered,
/// `Err(code)` on permission denial or connection failure.
#[allow(unused_variables)]
pub fn unix_socket_subscribe(socket_name: &str) -> Result<(), i32> {
    #[cfg(target_arch = "wasm32")]
    {
        // SAFETY: `socket_name` points to a valid UTF-8 string slice within guest linear memory.
        let res = unsafe { host_unix_socket_subscribe(socket_name.as_ptr(), socket_name.len()) };
        if res == 0 {
            Ok(())
        } else {
            Err(res)
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(-404)
    }
}

const AUDIO_SOCKET_NAME: &str = "wyrd-audio.sock";
const NOTIFICATIONS_SOCKET_NAME: &str = "wyrd-notifications.sock";
const TRAY_SOCKET_NAME: &str = "wyrd-tray.sock";

fn audio_socket_cmd(
    cmd: &str,
    target: Option<&str>,
    value: Option<f64>,
    mute: Option<bool>,
    name: Option<&str>,
) -> Result<Value, i32> {
    let mut payload = serde_json::json!({ "cmd": cmd });
    if let Some(target) = target {
        payload["target"] = serde_json::Value::String(target.to_string());
    }
    if let Some(value) = value {
        payload["value"] = serde_json::Value::Number(
            serde_json::Number::from_f64(value).unwrap_or(serde_json::Number::from(0)),
        );
    }
    if let Some(mute) = mute {
        payload["mute"] = serde_json::Value::Bool(mute);
    }
    if let Some(name) = name {
        payload["name"] = serde_json::Value::String(name.to_string());
    }

    let payload_json = payload.to_string();
    let response = unix_socket_request(AUDIO_SOCKET_NAME, &payload_json)?;
    serde_json::from_str::<Value>(&response).map_err(|_| -500)
}

pub fn audio_get_status() -> Option<Value> {
    audio_socket_cmd("status", None, None, None, None).ok()
}

pub fn audio_subscribe() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let socket = AUDIO_SOCKET_NAME.as_bytes();
        // SAFETY: `socket` points to a valid static byte slice in guest linear memory.
        unsafe { host_unix_socket_subscribe(socket.as_ptr(), socket.len()) == 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

pub fn notifications_subscribe() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        let socket = NOTIFICATIONS_SOCKET_NAME.as_bytes();
        // SAFETY: `socket` points to a valid static byte slice in guest linear memory.
        unsafe { host_unix_socket_subscribe(socket.as_ptr(), socket.len()) == 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

pub fn windows_subscribe() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        const WINDOW_SOCKET_NAME: &str = "wyrd-windows.sock";
        let socket = WINDOW_SOCKET_NAME.as_bytes();
        // SAFETY: `socket` points to a valid static byte slice in guest linear memory.
        unsafe { host_unix_socket_subscribe(socket.as_ptr(), socket.len()) == 0 }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

pub fn audio_set_volume(target: &str, value: f64) -> bool {
    match audio_socket_cmd("set_volume", Some(target), Some(value), None, None) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

pub fn audio_set_mute(target: &str, mute: bool) -> bool {
    match audio_socket_cmd("set_mute", Some(target), None, Some(mute), None) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

pub fn audio_toggle_mute(target: &str) -> bool {
    match audio_socket_cmd("toggle_mute", Some(target), None, None, None) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

pub fn audio_set_default(target: &str, name: &str) -> bool {
    let daemon_target = if target.contains("source")
        || target.contains("mic")
        || target == "@DEFAULT_AUDIO_SOURCE@"
    {
        "source"
    } else {
        "sink"
    };
    match audio_socket_cmd("set_default", Some(daemon_target), None, None, Some(name)) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

fn notifications_socket_cmd(
    cmd: &str,
    id: Option<u32>,
    reason: Option<u32>,
    dnd: Option<bool>,
) -> Result<Value, i32> {
    let mut payload = serde_json::json!({ "cmd": cmd });
    if let Some(id) = id {
        payload["id"] = serde_json::Value::Number(serde_json::Number::from(id));
    }
    if let Some(reason) = reason {
        payload["reason"] = serde_json::Value::Number(serde_json::Number::from(reason));
    }
    if let Some(dnd) = dnd {
        payload["dnd"] = serde_json::Value::Bool(dnd);
    }

    let payload_json = payload.to_string();
    let response = unix_socket_request(NOTIFICATIONS_SOCKET_NAME, &payload_json)?;
    serde_json::from_str::<Value>(&response).map_err(|_| -500)
}

pub fn notifications_get_status() -> Option<Value> {
    notifications_socket_cmd("status", None, None, None).ok()
}

pub fn notifications_list() -> Option<Value> {
    notifications_socket_cmd("list", None, None, None).ok()
}

pub fn notifications_dismiss(id: u32, reason: u32) -> bool {
    match notifications_socket_cmd("dismiss", Some(id), Some(reason), None) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

pub fn notifications_clear() -> bool {
    match notifications_socket_cmd("clear", None, None, None) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

pub fn notifications_set_dnd(enabled: bool) -> bool {
    match notifications_socket_cmd("dnd", None, None, Some(enabled)) {
        Ok(resp) => resp.get("status").and_then(|v| v.as_str()) == Some("ok"),
        Err(_) => false,
    }
}

fn tray_socket_cmd(cmd: &str, payload: Option<Value>) -> Result<Value, i32> {
    let request = if let Some(value) = payload {
        serde_json::json!({ "cmd": cmd, "payload": value })
    } else {
        serde_json::json!({ "cmd": cmd })
    };

    let response = unix_socket_request(TRAY_SOCKET_NAME, &request.to_string())?;
    serde_json::from_str::<Value>(&response).map_err(|_| -500)
}

pub fn tray_get_status() -> Option<Value> {
    tray_socket_cmd("status", None).ok()
}

/// Macro for exporting a WASM module instance and FFI entry points.
#[macro_export]
macro_rules! export_wasm_module {
    ($module_ty:ident) => {
        static mut MODULE_INSTANCE: Option<$module_ty> = None;

        #[no_mangle]
        pub extern "C" fn wyrd_alloc(size: u32) -> *mut u8 {
            let mut buf = Vec::with_capacity(size as usize);
            let ptr = buf.as_mut_ptr();
            std::mem::forget(buf);
            ptr
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn wyrd_dealloc(ptr: *mut u8, size: u32) {
            if !ptr.is_null() && size > 0 {
                // SAFETY: Reclaims memory previously allocated by `wyrd_alloc` via Vec::with_capacity.
                // The caller must provide the pointer returned by `wyrd_alloc` and its original capacity `size`.
                unsafe {
                    let _ = Vec::from_raw_parts(ptr, 0, size as usize);
                }
            }
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn wyrd_init(config_ptr: *const u8, config_len: u32) -> u32 {
            let config_json: serde_json::Value = if !config_ptr.is_null() && config_len > 0 {
                // SAFETY: The host runtime guarantees `config_ptr` points to `config_len` bytes of valid guest memory.
                let slice = unsafe { std::slice::from_raw_parts(config_ptr, config_len as usize) };
                serde_json::from_slice(slice).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            let mut instance = $module_ty::default();
            match $crate::wasm::WasmModule::init(&mut instance, config_json) {
                Ok(()) => {
                    // SAFETY: Single-threaded WASM execution environment guarantees exclusive access during module lifecycle callbacks.
                    unsafe {
                        MODULE_INSTANCE = Some(instance);
                    }
                    0
                }
                Err(e) => {
                    $crate::wasm::log_error(&format!("Module init failed: {:#}", e));
                    1
                }
            }
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn wyrd_on_event(
            widget_ptr: *const u8,
            widget_len: u32,
            event_ptr: *const u8,
            event_len: u32,
        ) {
            // SAFETY: In single-threaded WASM, mutable access to MODULE_INSTANCE is synchronized across host calls.
            // The host ensures `widget_ptr` and `event_ptr` point to readable memory of length `widget_len` and `event_len`.
            unsafe {
                if let Some(ref mut instance) = MODULE_INSTANCE {
                    let widget_slice = std::slice::from_raw_parts(widget_ptr, widget_len as usize);
                    let event_slice = std::slice::from_raw_parts(event_ptr, event_len as usize);
                    let widget_id = std::str::from_utf8(widget_slice).unwrap_or("");
                    let event = std::str::from_utf8(event_slice).unwrap_or("");
                    $crate::wasm::WasmModule::on_event(instance, widget_id, event);
                }
            }
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn wyrd_on_topic(
            topic_ptr: *const u8,
            topic_len: u32,
            val_ptr: *const u8,
            val_len: u32,
        ) {
            // SAFETY: Single-threaded WASM execution ensures exclusive access to MODULE_INSTANCE.
            // The host guarantees `topic_ptr` and `val_ptr` point to readable guest memory for their respective lengths.
            unsafe {
                if let Some(ref mut instance) = MODULE_INSTANCE {
                    let topic_slice = std::slice::from_raw_parts(topic_ptr, topic_len as usize);
                    let val_slice = std::slice::from_raw_parts(val_ptr, val_len as usize);
                    let topic = std::str::from_utf8(topic_slice).unwrap_or("");
                    let val: serde_json::Value =
                        serde_json::from_slice(val_slice).unwrap_or(serde_json::Value::Null);
                    $crate::wasm::WasmModule::on_topic(instance, topic, &val);
                }
            }
        }

        #[no_mangle]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn wyrd_on_dbus_signal(
            iface_ptr: *const u8,
            iface_len: u32,
            member_ptr: *const u8,
            member_len: u32,
            body_ptr: *const u8,
            body_len: u32,
        ) {
            // SAFETY: Single-threaded WASM execution ensures exclusive access to MODULE_INSTANCE.
            // The host runtime guarantees pointer and length validity for interface, member, and payload body slices.
            unsafe {
                if let Some(ref mut instance) = MODULE_INSTANCE {
                    let iface_slice = std::slice::from_raw_parts(iface_ptr, iface_len as usize);
                    let member_slice = std::slice::from_raw_parts(member_ptr, member_len as usize);
                    let body_slice = std::slice::from_raw_parts(body_ptr, body_len as usize);
                    let iface = std::str::from_utf8(iface_slice).unwrap_or("");
                    let member = std::str::from_utf8(member_slice).unwrap_or("");
                    let payload: serde_json::Value =
                        serde_json::from_slice(body_slice).unwrap_or(serde_json::Value::Null);
                    $crate::wasm::WasmModule::on_dbus_signal(instance, iface, member, &payload);
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn wyrd_on_tick() {
            // SAFETY: In single-threaded WASM, module tick operates with exclusive access to MODULE_INSTANCE.
            unsafe {
                if let Some(ref mut instance) = MODULE_INSTANCE {
                    $crate::wasm::WasmModule::tick(instance);
                }
            }
        }

        #[no_mangle]
        pub extern "C" fn wyrd_shutdown() {
            // SAFETY: Exclusive single-threaded access to take ownership and cleanly drop MODULE_INSTANCE.
            unsafe {
                if let Some(mut instance) = MODULE_INSTANCE.take() {
                    $crate::wasm::WasmModule::shutdown(&mut instance);
                }
            }
        }
    };
}
