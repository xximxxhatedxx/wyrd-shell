use anyhow::{anyhow, Result};
use libpulse_binding as pulse;
use pulse::callbacks::ListResult;
use pulse::context::subscribe::{Facility, InterestMaskSet, Operation};
use pulse::context::{Context, FlagSet};
use pulse::mainloop::standard::{IterateResult, Mainloop};
use pulse::volume::{ChannelVolumes, Volume};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub description: String,
    pub volume: u8,
    pub muted: bool,
    pub is_default: bool,
    #[serde(default = "default_channels")]
    pub channels: u8,
}

fn default_channels() -> u8 {
    2
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AudioStatus {
    pub volume: u8,
    pub muted: bool,
    pub active_sink_name: String,
    pub active_sink_desc: String,
    pub mic_volume: u8,
    pub mic_muted: bool,
    pub active_source_name: String,
    pub active_source_desc: String,
    pub sinks: Vec<AudioDeviceInfo>,
    pub sources: Vec<AudioDeviceInfo>,
}

pub enum AudioCmd {
    SetSinkVolume { sink: String, vol_pct: f64 },
    SetSinkMute { sink: String, mute: bool },
    ToggleSinkMute { sink: String },
    SetSourceVolume { source: String, vol_pct: f64 },
    SetSourceMute { source: String, mute: bool },
    ToggleSourceMute { source: String },
    SetDefaultSink(String),
    SetDefaultSource(String),
}

#[derive(Clone)]
pub struct AudioService {
    status: Arc<parking_lot::RwLock<AudioStatus>>,
    cmd_tx: std::sync::mpsc::Sender<AudioCmd>,
    broadcast_tx: broadcast::Sender<String>,
}

impl AudioService {
    pub fn new() -> Result<Self> {
        let status = Arc::new(parking_lot::RwLock::new(AudioStatus::default()));
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel::<AudioCmd>();
        let (broadcast_tx, _) = broadcast::channel::<String>(128);

        let status_clone = status.clone();
        let broadcast_tx_clone = broadcast_tx.clone();
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<()>>();

        std::thread::Builder::new()
            .name("wyrd-audio".to_string())
            .spawn(move || {
                run_audio_thread(status_clone, cmd_rx, broadcast_tx_clone, init_tx);
            })?;

        init_rx.recv_timeout(Duration::from_secs(10))
            .map_err(|_| anyhow!("Timeout connecting to PulseAudio/PipeWire"))??;

        Ok(Self {
            status,
            cmd_tx,
            broadcast_tx,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.broadcast_tx.subscribe()
    }

    pub fn get_status(&self) -> AudioStatus {
        self.status.read().clone()
    }

    pub fn set_sink_volume(&self, sink_name: &str, vol_pct: f64) -> bool {
        let target = if sink_name.is_empty() || sink_name == "@DEFAULT_AUDIO_SINK@" || sink_name == "@DEFAULT_SINK@" {
            self.status.read().active_sink_name.clone()
        } else {
            sink_name.to_string()
        };
        self.cmd_tx.send(AudioCmd::SetSinkVolume { sink: target, vol_pct }).is_ok()
    }

    pub fn set_sink_mute(&self, sink_name: &str, mute: bool) -> bool {
        let target = if sink_name.is_empty() || sink_name == "@DEFAULT_AUDIO_SINK@" || sink_name == "@DEFAULT_SINK@" {
            self.status.read().active_sink_name.clone()
        } else {
            sink_name.to_string()
        };
        self.cmd_tx.send(AudioCmd::SetSinkMute { sink: target, mute }).is_ok()
    }

    pub fn toggle_sink_mute(&self, sink_name: &str) -> bool {
        let target = if sink_name.is_empty() || sink_name == "@DEFAULT_AUDIO_SINK@" || sink_name == "@DEFAULT_SINK@" {
            self.status.read().active_sink_name.clone()
        } else {
            sink_name.to_string()
        };
        self.cmd_tx.send(AudioCmd::ToggleSinkMute { sink: target }).is_ok()
    }

    pub fn set_source_volume(&self, source_name: &str, vol_pct: f64) -> bool {
        let target = if source_name.is_empty() || source_name == "@DEFAULT_AUDIO_SOURCE@" || source_name == "@DEFAULT_SOURCE@" {
            self.status.read().active_source_name.clone()
        } else {
            source_name.to_string()
        };
        self.cmd_tx.send(AudioCmd::SetSourceVolume { source: target, vol_pct }).is_ok()
    }

    pub fn set_source_mute(&self, source_name: &str, mute: bool) -> bool {
        let target = if source_name.is_empty() || source_name == "@DEFAULT_AUDIO_SOURCE@" || source_name == "@DEFAULT_SOURCE@" {
            self.status.read().active_source_name.clone()
        } else {
            source_name.to_string()
        };
        self.cmd_tx.send(AudioCmd::SetSourceMute { source: target, mute }).is_ok()
    }

    pub fn toggle_source_mute(&self, source_name: &str) -> bool {
        let target = if source_name.is_empty() || source_name == "@DEFAULT_AUDIO_SOURCE@" || source_name == "@DEFAULT_SOURCE@" {
            self.status.read().active_source_name.clone()
        } else {
            source_name.to_string()
        };
        self.cmd_tx.send(AudioCmd::ToggleSourceMute { source: target }).is_ok()
    }

    pub fn set_default_sink(&self, sink_name: &str) -> bool {
        self.cmd_tx.send(AudioCmd::SetDefaultSink(sink_name.to_string())).is_ok()
    }

    pub fn set_default_source(&self, source_name: &str) -> bool {
        self.cmd_tx.send(AudioCmd::SetDefaultSource(source_name.to_string())).is_ok()
    }
}

#[derive(Default)]
struct UserAudioOverrides {
    default_sink: Option<(String, std::time::Instant)>,
    default_source: Option<(String, std::time::Instant)>,
    sink_volume: Option<(u8, std::time::Instant)>,
    source_volume: Option<(u8, std::time::Instant)>,
    sink_muted: Option<(bool, std::time::Instant)>,
    source_muted: Option<(bool, std::time::Instant)>,
}

struct PendingQuery {
    server_done: bool,
    sinks_done: bool,
    sources_done: bool,
    def_sink: String,
    def_source: String,
    sinks: Vec<AudioDeviceInfo>,
    sources: Vec<AudioDeviceInfo>,
}

fn start_query(
    context: &mut Context,
    status: &Arc<parking_lot::RwLock<AudioStatus>>,
    broadcast_tx: &broadcast::Sender<String>,
    query_in_flight: &Arc<std::sync::atomic::AtomicBool>,
    query_rerun: &Arc<std::sync::atomic::AtomicBool>,
    init_done: &Arc<std::sync::atomic::AtomicBool>,
    overrides: &Arc<parking_lot::Mutex<UserAudioOverrides>>,
) {
    if query_in_flight.swap(true, std::sync::atomic::Ordering::SeqCst) {
        query_rerun.store(true, std::sync::atomic::Ordering::SeqCst);
        return;
    }

    let pending = Arc::new(parking_lot::Mutex::new(PendingQuery {
        server_done: false,
        sinks_done: false,
        sources_done: false,
        def_sink: String::new(),
        def_source: String::new(),
        sinks: Vec::new(),
        sources: Vec::new(),
    }));

    let check_and_commit = {
        let pending = pending.clone();
        let status = status.clone();
        let bcast_tx = broadcast_tx.clone();
        let in_flight = query_in_flight.clone();
        let init_done = init_done.clone();
        let overrides = overrides.clone();

        Arc::new(move || {
            let mut p = pending.lock();
            if p.server_done && p.sinks_done && p.sources_done {
                let mut guard = status.write();
                let ov = overrides.lock();

                let def_sink = ov.default_sink.as_ref()
                    .and_then(|(s, t)| if t.elapsed() < Duration::from_millis(1000) { Some(s.clone()) } else { None })
                    .unwrap_or_else(|| p.def_sink.clone());

                let def_source = ov.default_source.as_ref()
                    .and_then(|(s, t)| if t.elapsed() < Duration::from_millis(1000) { Some(s.clone()) } else { None })
                    .unwrap_or_else(|| p.def_source.clone());

                guard.active_sink_name = def_sink;
                guard.active_source_name = def_source;

                for sink in &mut p.sinks {
                    sink.is_default = sink.name == guard.active_sink_name;
                    if sink.is_default {
                        if let Some((vol, t)) = ov.sink_volume {
                            if t.elapsed() < Duration::from_millis(600) {
                                sink.volume = vol;
                            }
                        }
                        if let Some((m, t)) = ov.sink_muted {
                            if t.elapsed() < Duration::from_millis(600) {
                                sink.muted = m;
                            }
                        }
                    }
                }

                for source in &mut p.sources {
                    source.is_default = source.name == guard.active_source_name;
                    if source.is_default {
                        if let Some((vol, t)) = ov.source_volume {
                            if t.elapsed() < Duration::from_millis(600) {
                                source.volume = vol;
                            }
                        }
                        if let Some((m, t)) = ov.source_muted {
                            if t.elapsed() < Duration::from_millis(600) {
                                source.muted = m;
                            }
                        }
                    }
                }

                if !p.sinks.is_empty() && !p.sinks.iter().any(|s| s.is_default) {
                    p.sinks[0].is_default = true;
                    guard.active_sink_name = p.sinks[0].name.clone();
                }
                if !p.sources.is_empty() && !p.sources.iter().any(|s| s.is_default) {
                    p.sources[0].is_default = true;
                    guard.active_source_name = p.sources[0].name.clone();
                }

                if let Some(def_sink_dev) = p.sinks.iter().find(|s| s.is_default) {
                    guard.volume = def_sink_dev.volume;
                    guard.muted = def_sink_dev.muted;
                    guard.active_sink_desc = def_sink_dev.description.clone();
                }

                if let Some(def_source_dev) = p.sources.iter().find(|s| s.is_default) {
                    guard.mic_volume = def_source_dev.volume;
                    guard.mic_muted = def_source_dev.muted;
                    guard.active_source_desc = def_source_dev.description.clone();
                }

                guard.sinks = std::mem::take(&mut p.sinks);
                guard.sources = std::mem::take(&mut p.sources);
                drop(ov);
                drop(guard);

                in_flight.store(false, std::sync::atomic::Ordering::SeqCst);
                init_done.store(true, std::sync::atomic::Ordering::SeqCst);

                let _ = bcast_tx.send("pipewire:changed".to_string());
            }
        })
    };

    let introspect = context.introspect();

    let p_server = pending.clone();
    let commit_server = check_and_commit.clone();
    introspect.get_server_info(move |info| {
        let mut p = p_server.lock();
        p.def_sink = info.default_sink_name.as_ref().map(|s| s.to_string()).unwrap_or_default();
        p.def_source = info.default_source_name.as_ref().map(|s| s.to_string()).unwrap_or_default();
        p.server_done = true;
        drop(p);
        commit_server();
    });

    let p_sinks = pending.clone();
    let commit_sinks = check_and_commit.clone();
    introspect.get_sink_info_list(move |list| {
        match list {
            ListResult::Item(item) => {
                let name = item.name.as_ref().map(|s| s.to_string()).unwrap_or_default();
                let desc = item.description.as_ref().map(|s| s.to_string()).unwrap_or_default();
                let vol_avg = item.volume.avg().0 as f64 / Volume::NORMAL.0 as f64 * 100.0;
                let vol_pct = vol_avg.round().clamp(0.0, 150.0) as u8;
                let muted = item.mute;
                let channels = item.volume.len();

                let mut p = p_sinks.lock();
                p.sinks.push(AudioDeviceInfo {
                    name,
                    description: desc,
                    volume: vol_pct,
                    muted,
                    is_default: false,
                    channels,
                });
            }
            ListResult::End | ListResult::Error => {
                let mut p = p_sinks.lock();
                p.sinks_done = true;
                drop(p);
                commit_sinks();
            }
        }
    });

    let p_sources = pending.clone();
    let commit_sources = check_and_commit.clone();
    introspect.get_source_info_list(move |list| {
        match list {
            ListResult::Item(item) => {
                let name = item.name.as_ref().map(|s| s.to_string()).unwrap_or_default();
                if !name.ends_with(".monitor") {
                    let desc = item.description.as_ref().map(|s| s.to_string()).unwrap_or_default();
                    let vol_avg = item.volume.avg().0 as f64 / Volume::NORMAL.0 as f64 * 100.0;
                    let vol_pct = vol_avg.round().clamp(0.0, 150.0) as u8;
                    let muted = item.mute;
                    let channels = item.volume.len();

                    let mut p = p_sources.lock();
                    p.sources.push(AudioDeviceInfo {
                        name,
                        description: desc,
                        volume: vol_pct,
                        muted,
                        is_default: false,
                        channels,
                    });
                }
            }
            ListResult::End | ListResult::Error => {
                let mut p = p_sources.lock();
                p.sources_done = true;
                drop(p);
                commit_sources();
            }
        }
    });
}

fn run_audio_thread(
    status: Arc<parking_lot::RwLock<AudioStatus>>,
    cmd_rx: std::sync::mpsc::Receiver<AudioCmd>,
    broadcast_tx: broadcast::Sender<String>,
    init_tx: std::sync::mpsc::Sender<Result<()>>,
) {
    let mut mainloop = match Mainloop::new() {
        Some(ml) => ml,
        None => {
            let _ = init_tx.send(Err(anyhow!("Failed to create pulse Mainloop")));
            return;
        }
    };

    let mut context = match Context::new(&mainloop, "wyrd-shell") {
        Some(ctx) => ctx,
        None => {
            let _ = init_tx.send(Err(anyhow!("Failed to create pulse Context")));
            return;
        }
    };

    if let Err(e) = context.connect(None, FlagSet::NOFLAGS, None) {
        let _ = init_tx.send(Err(anyhow!("Failed to connect to pulse: {:?}", e)));
        return;
    }

    let start = std::time::Instant::now();
    let mut ready = false;
    while start.elapsed() < Duration::from_secs(3) {
        match mainloop.iterate(false) {
            IterateResult::Success(_) => {}
            IterateResult::Err(e) => {
                let _ = init_tx.send(Err(anyhow!("Mainloop error: {:?}", e)));
                return;
            }
            IterateResult::Quit(_) => return,
        }
        let state = context.get_state();
        if state == pulse::context::State::Ready {
            ready = true;
            break;
        }
        if state == pulse::context::State::Failed || state == pulse::context::State::Terminated {
            let _ = init_tx.send(Err(anyhow!("PulseAudio state failed: {:?}", state)));
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    if !ready {
        let _ = init_tx.send(Err(anyhow!("PulseAudio connection timed out")));
        return;
    }

    let refresh_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let refresh_flag_cb = refresh_flag.clone();

    context.set_subscribe_callback(Some(Box::new(move |facility, op, _idx| {
        if let Some(fac) = facility {
            match fac {
                Facility::Sink | Facility::Source | Facility::Server
                    if (op == Some(Operation::New) || op == Some(Operation::Changed) || op == Some(Operation::Removed)) => {
                        refresh_flag_cb.store(true, std::sync::atomic::Ordering::SeqCst);
                    }
                _ => {}
            }
        }
    })));

    let interest = InterestMaskSet::SINK | InterestMaskSet::SOURCE | InterestMaskSet::SERVER;
    context.subscribe(interest, |_| {});

    let query_in_flight = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let query_rerun = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let init_done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let overrides = Arc::new(parking_lot::Mutex::new(UserAudioOverrides::default()));

    start_query(
        &mut context,
        &status,
        &broadcast_tx,
        &query_in_flight,
        &query_rerun,
        &init_done,
        &overrides,
    );

    let init_start = std::time::Instant::now();
    while !init_done.load(std::sync::atomic::Ordering::SeqCst) && init_start.elapsed() < Duration::from_secs(2) {
        match mainloop.iterate(false) {
            IterateResult::Success(_) => {}
            IterateResult::Err(_) | IterateResult::Quit(_) => break,
        }
        std::thread::sleep(Duration::from_millis(5));
    }

    if !init_done.load(std::sync::atomic::Ordering::SeqCst) {
        let _ = init_tx.send(Err(anyhow!("PulseAudio initial state query timed out")));
        return;
    }

    let _ = init_tx.send(Ok(()));

    while let IterateResult::Success(_) = mainloop.iterate(false) {

        while let Ok(cmd) = cmd_rx.try_recv() {
            match cmd {
                AudioCmd::SetSinkVolume { sink, vol_pct } => {
                    let target = if sink.is_empty() || sink == "@DEFAULT_AUDIO_SINK@" || sink == "@DEFAULT_SINK@" {
                        status.read().active_sink_name.clone()
                    } else {
                        sink
                    };
                    let v = vol_pct.clamp(0.0, 150.0).round() as u8;
                    overrides.lock().sink_volume = Some((v, std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.volume = v;
                        if v > 0 && guard.muted {
                            guard.muted = false;
                        }
                        for s in &mut guard.sinks {
                            if s.name == target {
                                s.volume = v;
                                if v > 0 && s.muted {
                                    s.muted = false;
                                }
                            }
                        }
                    }
                    let _ = broadcast_tx.send("pipewire:changed".to_string());
                    let channels = status.read().sinks.iter()
                        .find(|s| s.name == target)
                        .map(|s| s.channels)
                        .unwrap_or(2)
                        .max(1);
                    let vol_val = ((vol_pct.clamp(0.0, 150.0) / 100.0) * Volume::NORMAL.0 as f64).round() as u32;
                    let mut cv = ChannelVolumes::default();
                    cv.set(channels, Volume(vol_val));
                    let mut introspect = context.introspect();
                    let _ = introspect.set_sink_volume_by_name(&target, &cv, None);
                }
                AudioCmd::SetSinkMute { sink, mute } => {
                    let target = if sink.is_empty() || sink == "@DEFAULT_AUDIO_SINK@" || sink == "@DEFAULT_SINK@" {
                        status.read().active_sink_name.clone()
                    } else {
                        sink
                    };
                    overrides.lock().sink_muted = Some((mute, std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.muted = mute;
                        for s in &mut guard.sinks {
                            if s.name == target {
                                s.muted = mute;
                            }
                        }
                    }
                    let _ = broadcast_tx.send("pipewire:changed".to_string());
                    let mut introspect = context.introspect();
                    let _ = introspect.set_sink_mute_by_name(&target, mute, None);
                }
                AudioCmd::ToggleSinkMute { sink } => {
                    let target = if sink.is_empty() || sink == "@DEFAULT_AUDIO_SINK@" || sink == "@DEFAULT_SINK@" {
                        status.read().active_sink_name.clone()
                    } else {
                        sink
                    };
                    let is_muted = status.read().sinks.iter()
                        .find(|s| s.name == target)
                        .map(|s| s.muted)
                        .unwrap_or_else(|| status.read().muted);
                    let new_mute = !is_muted;
                    overrides.lock().sink_muted = Some((new_mute, std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.muted = new_mute;
                        for s in &mut guard.sinks {
                            if s.name == target {
                                s.muted = new_mute;
                            }
                        }
                    }
                    let _ = broadcast_tx.send("pipewire:changed".to_string());
                    let mut introspect = context.introspect();
                    let _ = introspect.set_sink_mute_by_name(&target, new_mute, None);
                }
                AudioCmd::SetSourceVolume { source, vol_pct } => {
                    let target = if source.is_empty() || source == "@DEFAULT_AUDIO_SOURCE@" || source == "@DEFAULT_SOURCE@" {
                        status.read().active_source_name.clone()
                    } else {
                        source
                    };
                    let v = vol_pct.clamp(0.0, 150.0).round() as u8;
                    overrides.lock().source_volume = Some((v, std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.mic_volume = v;
                        if v > 0 && guard.mic_muted {
                            guard.mic_muted = false;
                        }
                        for s in &mut guard.sources {
                            if s.name == target {
                                s.volume = v;
                                if v > 0 && s.muted {
                                    s.muted = false;
                                }
                            }
                        }
                    }
                    let channels = status.read().sources.iter()
                        .find(|s| s.name == target)
                        .map(|s| s.channels)
                        .unwrap_or(2)
                        .max(1);
                    let vol_val = ((vol_pct.clamp(0.0, 150.0) / 100.0) * Volume::NORMAL.0 as f64).round() as u32;
                    let mut cv = ChannelVolumes::default();
                    cv.set(channels, Volume(vol_val));
                    let mut introspect = context.introspect();
                    let _ = introspect.set_source_volume_by_name(&target, &cv, None);
                }
                AudioCmd::SetSourceMute { source, mute } => {
                    let target = if source.is_empty() || source == "@DEFAULT_AUDIO_SOURCE@" || source == "@DEFAULT_SOURCE@" {
                        status.read().active_source_name.clone()
                    } else {
                        source
                    };
                    overrides.lock().source_muted = Some((mute, std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.mic_muted = mute;
                        for s in &mut guard.sources {
                            if s.name == target {
                                s.muted = mute;
                            }
                        }
                    }
                    let _ = broadcast_tx.send("pipewire:changed".to_string());
                    let mut introspect = context.introspect();
                    let _ = introspect.set_source_mute_by_name(&target, mute, None);
                }
                AudioCmd::ToggleSourceMute { source } => {
                    let target = if source.is_empty() || source == "@DEFAULT_AUDIO_SOURCE@" || source == "@DEFAULT_SOURCE@" {
                        status.read().active_source_name.clone()
                    } else {
                        source
                    };
                    let is_muted = status.read().sources.iter()
                        .find(|s| s.name == target)
                        .map(|s| s.muted)
                        .unwrap_or_else(|| status.read().mic_muted);
                    let new_mute = !is_muted;
                    overrides.lock().source_muted = Some((new_mute, std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.mic_muted = new_mute;
                        for s in &mut guard.sources {
                            if s.name == target {
                                s.muted = new_mute;
                            }
                        }
                    }
                    let mut introspect = context.introspect();
                    let _ = introspect.set_source_mute_by_name(&target, new_mute, None);
                }
                AudioCmd::SetDefaultSink(sink) => {
                    overrides.lock().default_sink = Some((sink.clone(), std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.active_sink_name = sink.clone();
                        let mut desc = None;
                        for s in &mut guard.sinks {
                            s.is_default = s.name == sink;
                            if s.is_default {
                                desc = Some(s.description.clone());
                            }
                        }
                        if let Some(d) = desc {
                            guard.active_sink_desc = d;
                        }
                    }
                    let _ = context.set_default_sink(&sink, |_| {});
                }
                AudioCmd::SetDefaultSource(source) => {
                    overrides.lock().default_source = Some((source.clone(), std::time::Instant::now()));
                    {
                        let mut guard = status.write();
                        guard.active_source_name = source.clone();
                        let mut desc = None;
                        for s in &mut guard.sources {
                            s.is_default = s.name == source;
                            if s.is_default {
                                desc = Some(s.description.clone());
                            }
                        }
                        if let Some(d) = desc {
                            guard.active_source_desc = d;
                        }
                    }
                    let _ = context.set_default_source(&source, |_| {});
                }
            }
        }

        if refresh_flag.swap(false, std::sync::atomic::Ordering::SeqCst)
            || query_rerun.swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            start_query(
                &mut context,
                &status,
                &broadcast_tx,
                &query_in_flight,
                &query_rerun,
                &init_done,
                &overrides,
            );
        }

        std::thread::sleep(Duration::from_millis(15));
    }
}
