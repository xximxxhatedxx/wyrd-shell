use crate::compositor::CompositorIntegration;
use crate::interaction::{DragState, RecentSliderRelease, SliderDragState};
use crate::popup::PopupKey;
use crate::runtime::tooltip::HoverTooltipState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use wayland_client::backend::ObjectId;
use wyrd_engine::config::lua::LuaRuntime;
use wyrd_engine::modules::CoreMessage;
use wyrd_engine::render::context::RenderContext;
use wyrd_engine::render::damage::DamageTracker;
use wyrd_engine::wayland::backend::WaylandBackend;
use wyrd_engine::widgets::WidgetTree;
use wyrd_engine::BarState;

#[allow(dead_code)]
pub struct RuntimeState {
    pub backend: WaylandBackend,
    pub state: Arc<std::sync::Mutex<BarState>>,
    pub compositor: Arc<dyn CompositorIntegration>,
    pub lua: Arc<LuaRuntime>,
    pub surface_trees: HashMap<ObjectId, Arc<WidgetTree>>,
    pub open_popups: HashSet<PopupKey>,
    pub render_ctx: RenderContext,
    pub damage_trackers: HashMap<ObjectId, DamageTracker>,
    pub module_commands: HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
    pub hover_tooltip: Option<HoverTooltipState>,
    pub dragging_popup: Option<DragState>,
    pub active_slider: Option<SliderDragState>,
    pub recent_slider_release: Option<RecentSliderRelease>,
    pub output_signature: Vec<u32>,
}

#[allow(dead_code)]
impl RuntimeState {
    pub fn new(
        backend: WaylandBackend,
        state: Arc<std::sync::Mutex<BarState>>,
        compositor: Arc<dyn CompositorIntegration>,
        lua: Arc<LuaRuntime>,
        module_commands: HashMap<String, tokio::sync::mpsc::Sender<CoreMessage>>,
    ) -> Self {
        Self {
            backend,
            state,
            compositor,
            lua,
            surface_trees: HashMap::new(),
            open_popups: HashSet::new(),
            render_ctx: RenderContext::new(1.0),
            damage_trackers: HashMap::new(),
            module_commands,
            hover_tooltip: None,
            dragging_popup: None,
            active_slider: None,
            recent_slider_release: None,
            output_signature: Vec::new(),
        }
    }
}
