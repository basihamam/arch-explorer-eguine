use std::cell::RefCell;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use ahash::AHashMap;

use sctk::reexports::calloop::LoopHandle;
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::globals::GlobalList;
use sctk::reexports::client::protocol::wl_output::WlOutput;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use sctk::compositor::{CompositorHandler, CompositorState};
use sctk::output::{OutputHandler, OutputState};
use sctk::registry::{ProvidesRegistryState, RegistryState};
use sctk::seat::pointer::ThemedPointer;
use sctk::seat::SeatState;
use sctk::shell::xdg::window::{Window, WindowConfigure, WindowHandler};
use sctk::shell::xdg::XdgShell;
use sctk::shell::WaylandSurface;
use sctk::shm::slot::SlotPool;
use sctk::shm::{Shm, ShmHandler};
use sctk::subcompositor::SubcompositorState;

use sctk::data_device_manager::data_device::DataDeviceHandler;
use sctk::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use sctk::data_device_manager::data_source::{DataSourceHandler, DragSource};
use sctk::data_device_manager::{DataDeviceManagerState, WritePipe};
use sctk::reexports::client::protocol::wl_data_device::WlDataDevice;
use sctk::reexports::client::protocol::wl_data_device_manager::DndAction;
use sctk::reexports::client::protocol::wl_data_source::WlDataSource;
use sctk::reexports::client::protocol::wl_shm::Format;

use crate::platform_impl::wayland::event_loop::sink::EventSink;
use crate::platform_impl::wayland::output::MonitorHandle;
use crate::platform_impl::wayland::seat::{
    PointerConstraintsState, RelativePointerState, TextInputState, WinitPointerData,
    WinitPointerDataExt, WinitSeatState,
};
use crate::platform_impl::wayland::types::kwin_blur::KWinBlurManager;
use crate::platform_impl::wayland::types::wp_fractional_scaling::FractionalScalingManager;
use crate::platform_impl::wayland::types::wp_viewporter::ViewporterState;
use crate::platform_impl::wayland::types::xdg_activation::XdgActivationState;
use crate::platform_impl::wayland::window::{WindowRequests, WindowState};
use crate::platform_impl::wayland::{WaylandError, WindowId};
use crate::platform_impl::OsError;

/// Winit's Wayland state.
pub struct WinitState {
    /// The WlRegistry.
    pub registry_state: RegistryState,

    /// The state of the WlOutput handling.
    pub output_state: OutputState,

    /// The compositor state which is used to create new windows and regions.
    pub compositor_state: Arc<CompositorState>,

    /// The state of the subcompositor.
    pub subcompositor_state: Option<Arc<SubcompositorState>>,

    /// The seat state responsible for all sorts of input.
    pub seat_state: SeatState,

    /// The shm for software buffers, such as cursors.
    pub shm: Shm,

    /// The pool where custom cursors are allocated.
    pub custom_cursor_pool: Arc<Mutex<SlotPool>>,

    /// The XDG shell that is used for windows.
    pub xdg_shell: XdgShell,

    /// The currently present windows.
    pub windows: RefCell<AHashMap<WindowId, Arc<Mutex<WindowState>>>>,

    /// The requests from the `Window` to EventLoop, such as close operations and redraw requests.
    pub window_requests: RefCell<AHashMap<WindowId, Arc<WindowRequests>>>,

    /// The events that were generated directly from the window.
    pub window_events_sink: Arc<Mutex<EventSink>>,

    /// The update for the `windows` coming from the compositor.
    pub window_compositor_updates: Vec<WindowCompositorUpdate>,

    /// Currently handled seats.
    pub seats: AHashMap<ObjectId, WinitSeatState>,

    /// Currently present cursor surfaces.
    pub pointer_surfaces: AHashMap<ObjectId, Arc<ThemedPointer<WinitPointerData>>>,

    /// The state of the text input on the client.
    pub text_input_state: Option<TextInputState>,

    /// Observed monitors.
    pub monitors: Arc<Mutex<Vec<MonitorHandle>>>,

    /// Sink to accumulate window events from the compositor, which is latter dispatched in
    /// event loop run.
    pub events_sink: EventSink,

    /// Xdg activation.
    pub xdg_activation: Option<XdgActivationState>,

    /// Relative pointer.
    pub relative_pointer: Option<RelativePointerState>,

    /// Pointer constraints to handle pointer locking and confining.
    pub pointer_constraints: Option<Arc<PointerConstraintsState>>,

    /// Viewporter state on the given window.
    pub viewporter_state: Option<ViewporterState>,

    /// Fractional scaling manager.
    pub fractional_scaling_manager: Option<FractionalScalingManager>,

    /// KWin blur manager.
    pub kwin_blur_manager: Option<KWinBlurManager>,

    /// Data device manager for Wayland Drag & Drop support.
    pub data_device_manager_state: DataDeviceManagerState,

    /// Active drag offer during DnD.
    pub current_drag_offer: Option<(WindowId, DragOffer)>,

    /// Ping handle to wake up the event loop from background tasks.
    pub awakener: Option<calloop::ping::Ping>,

    /// Queue handle to create wayland objects like drag sources.
    pub queue_handle: QueueHandle<Self>,

    /// Outgoing drag sources active on Wayland.
    pub outgoing_drag_sources: Vec<(DragSource, Vec<String>, Option<WlSurface>, Option<SlotPool>)>,

    /// Loop handle to re-register event sources, such as keyboard repeat.
    pub loop_handle: LoopHandle<'static, Self>,

    /// Whether we have dispatched events to the user thus we want to
    /// send `AboutToWait` and normally wakeup the user.
    pub dispatched_events: bool,
}

pub struct DragRequest {
    pub paths: Vec<String>,
    pub preview: Option<(u32, u32, Vec<u8>)>,
}

pub static OUTGOING_DRAG_REQUEST: Mutex<Option<DragRequest>> = Mutex::new(None);
pub static GLOBAL_AWAKENER: Mutex<Option<calloop::ping::Ping>> = Mutex::new(None);

pub fn request_wayland_drag(paths: Vec<String>, preview: Option<(u32, u32, Vec<u8>)>) {
    *OUTGOING_DRAG_REQUEST.lock().unwrap() = Some(DragRequest { paths, preview });
    if let Some(ref aw) = *GLOBAL_AWAKENER.lock().unwrap() {
        aw.ping();
    }
}

impl WinitState {
    pub fn new(
        globals: &GlobalList,
        queue_handle: &QueueHandle<Self>,
        loop_handle: LoopHandle<'static, WinitState>,
    ) -> Result<Self, OsError> {
        let registry_state = RegistryState::new(globals);
        let compositor_state =
            CompositorState::bind(globals, queue_handle).map_err(WaylandError::Bind)?;
        let subcompositor_state = match SubcompositorState::bind(
            compositor_state.wl_compositor().clone(),
            globals,
            queue_handle,
        ) {
            Ok(c) => Some(c),
            Err(e) => {
                tracing::warn!("Subcompositor protocol not available, ignoring CSD: {e:?}");
                None
            },
        };

        let output_state = OutputState::new(globals, queue_handle);
        let monitors = output_state.outputs().map(MonitorHandle::new).collect();

        let seat_state = SeatState::new(globals, queue_handle);

        let data_device_manager_state =
            DataDeviceManagerState::bind(globals, queue_handle)
                .expect("wl_data_device_manager is required for DnD");

        let mut seats = AHashMap::default();
        for seat in seat_state.seats() {
            let mut seat_st = WinitSeatState::new();
            seat_st.data_device = Some(data_device_manager_state.get_data_device(queue_handle, &seat));
            seats.insert(seat.id(), seat_st);
        }

        let (viewporter_state, fractional_scaling_manager) =
            if let Ok(fsm) = FractionalScalingManager::new(globals, queue_handle) {
                (ViewporterState::new(globals, queue_handle).ok(), Some(fsm))
            } else {
                (None, None)
            };

        let shm = Shm::bind(globals, queue_handle).map_err(WaylandError::Bind)?;
        let custom_cursor_pool = Arc::new(Mutex::new(SlotPool::new(2, &shm).unwrap()));

        Ok(Self {
            registry_state,
            compositor_state: Arc::new(compositor_state),
            subcompositor_state: subcompositor_state.map(Arc::new),
            output_state,
            seat_state,
            shm,
            custom_cursor_pool,

            data_device_manager_state,
            current_drag_offer: None,
            awakener: None,
            queue_handle: queue_handle.clone(),
            outgoing_drag_sources: Vec::new(),
            xdg_shell: XdgShell::bind(globals, queue_handle).map_err(WaylandError::Bind)?,
            xdg_activation: XdgActivationState::bind(globals, queue_handle).ok(),

            windows: Default::default(),
            window_requests: Default::default(),
            window_compositor_updates: Vec::new(),
            window_events_sink: Default::default(),
            viewporter_state,
            fractional_scaling_manager,
            kwin_blur_manager: KWinBlurManager::new(globals, queue_handle).ok(),

            seats,
            text_input_state: TextInputState::new(globals, queue_handle).ok(),

            relative_pointer: RelativePointerState::new(globals, queue_handle).ok(),
            pointer_constraints: PointerConstraintsState::new(globals, queue_handle)
                .map(Arc::new)
                .ok(),
            pointer_surfaces: Default::default(),

            monitors: Arc::new(Mutex::new(monitors)),
            events_sink: EventSink::new(),
            loop_handle,
            // Make it true by default.
            dispatched_events: true,
        })
    }

    pub fn scale_factor_changed(
        &mut self,
        surface: &WlSurface,
        scale_factor: f64,
        is_legacy: bool,
    ) {
        // Check if the cursor surface.
        let window_id = super::make_wid(surface);

        if let Some(window) = self.windows.get_mut().get(&window_id) {
            // Don't update the scaling factor, when legacy method is used.
            if is_legacy && self.fractional_scaling_manager.is_some() {
                return;
            }

            // The scale factor change is for the window.
            let pos = if let Some(pos) = self
                .window_compositor_updates
                .iter()
                .position(|update| update.window_id == window_id)
            {
                pos
            } else {
                self.window_compositor_updates.push(WindowCompositorUpdate::new(window_id));
                self.window_compositor_updates.len() - 1
            };

            // Update the scale factor right away.
            window.lock().unwrap().set_scale_factor(scale_factor);
            self.window_compositor_updates[pos].scale_changed = true;
        } else if let Some(pointer) = self.pointer_surfaces.get(&surface.id()) {
            // Get the window, where the pointer resides right now.
            let focused_window = match pointer.pointer().winit_data().focused_window() {
                Some(focused_window) => focused_window,
                None => return,
            };

            if let Some(window_state) = self.windows.get_mut().get(&focused_window) {
                window_state.lock().unwrap().reload_cursor_style()
            }
        }
    }

    pub fn queue_close(updates: &mut Vec<WindowCompositorUpdate>, window_id: WindowId) {
        let pos = if let Some(pos) = updates.iter().position(|update| update.window_id == window_id)
        {
            pos
        } else {
            updates.push(WindowCompositorUpdate::new(window_id));
            updates.len() - 1
        };

        updates[pos].close_window = true;
    }
}

impl ShmHandler for WinitState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl WindowHandler for WinitState {
    fn request_close(&mut self, _: &Connection, _: &QueueHandle<Self>, window: &Window) {
        let window_id = super::make_wid(window.wl_surface());
        Self::queue_close(&mut self.window_compositor_updates, window_id);
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        let window_id = super::make_wid(window.wl_surface());

        let pos = if let Some(pos) =
            self.window_compositor_updates.iter().position(|update| update.window_id == window_id)
        {
            pos
        } else {
            self.window_compositor_updates.push(WindowCompositorUpdate::new(window_id));
            self.window_compositor_updates.len() - 1
        };

        // Populate the configure to the window.
        self.window_compositor_updates[pos].resized |= self
            .windows
            .get_mut()
            .get_mut(&window_id)
            .expect("got configure for dead window.")
            .lock()
            .unwrap()
            .configure(configure, &self.shm, &self.subcompositor_state);

        // NOTE: configure demands wl_surface::commit, however winit doesn't commit on behalf of the
        // users, since it can break a lot of things, thus it'll ask users to redraw instead.
        self.window_requests
            .get_mut()
            .get(&window_id)
            .unwrap()
            .redraw_requested
            .store(true, Ordering::Relaxed);

        // Manually mark that we've got an event, since configure may not generate a resize.
        self.dispatched_events = true;
    }
}

impl OutputHandler for WinitState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, output: WlOutput) {
        self.monitors.lock().unwrap().push(MonitorHandle::new(output));
    }

    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, updated: WlOutput) {
        let mut monitors = self.monitors.lock().unwrap();
        let updated = MonitorHandle::new(updated);
        if let Some(pos) = monitors.iter().position(|output| output == &updated) {
            monitors[pos] = updated
        } else {
            monitors.push(updated)
        }
    }

    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, removed: WlOutput) {
        let mut monitors = self.monitors.lock().unwrap();
        let removed = MonitorHandle::new(removed);
        if let Some(pos) = monitors.iter().position(|output| output == &removed) {
            monitors.remove(pos);
        }
    }
}

impl CompositorHandler for WinitState {
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: wayland_client::protocol::wl_output::Transform,
    ) {
        // TODO(kchibisov) we need to expose it somehow in winit.
    }

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &WlOutput,
    ) {
    }

    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &WlSurface,
        scale_factor: i32,
    ) {
        self.scale_factor_changed(surface, scale_factor as f64, true)
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &WlSurface, _: u32) {
        let window_id = super::make_wid(surface);
        let window = match self.windows.get_mut().get(&window_id) {
            Some(window) => window,
            None => return,
        };

        // In case we have a redraw requested we must indicate the wake up.
        if self
            .window_requests
            .get_mut()
            .get(&window_id)
            .unwrap()
            .redraw_requested
            .load(Ordering::Relaxed)
        {
            self.dispatched_events = true;
        }

        window.lock().unwrap().frame_callback_received();
    }
}

impl ProvidesRegistryState for WinitState {
    sctk::registry_handlers![OutputState, SeatState];

    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
}

// The window update coming from the compositor.
#[derive(Debug, Clone, Copy)]
pub struct WindowCompositorUpdate {
    /// The id of the window this updates belongs to.
    pub window_id: WindowId,

    /// New window size.
    pub resized: bool,

    /// New scale factor.
    pub scale_changed: bool,

    /// Close the window.
    pub close_window: bool,
}

impl WindowCompositorUpdate {
    fn new(window_id: WindowId) -> Self {
        Self { window_id, resized: false, scale_changed: false, close_window: false }
    }
}

sctk::delegate_subcompositor!(WinitState);
sctk::delegate_compositor!(WinitState);
sctk::delegate_output!(WinitState);
sctk::delegate_registry!(WinitState);
sctk::delegate_shm!(WinitState);
sctk::delegate_xdg_shell!(WinitState);
sctk::delegate_xdg_window!(WinitState);
sctk::delegate_data_device!(WinitState);

impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _offer: &mut DragOffer,
        _actions: DndAction,
    ) {}

    fn selected_action(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _offer: &mut DragOffer,
        _actions: DndAction,
    ) {}
}

impl DataSourceHandler for WinitState {
    fn accept_mime(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _source: &WlDataSource,
        _mime: Option<String>,
    ) {}

    fn send_request(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &WlDataSource,
        mime: String,
        fd: WritePipe,
    ) {
        if let Some((_, paths, _, _)) = self.outgoing_drag_sources.iter().find(|item| item.0.inner() == source) {
            use std::io::Write;
            use std::os::fd::FromRawFd;
            use std::os::fd::IntoRawFd;
            let mut file = unsafe { std::fs::File::from_raw_fd(fd.into_raw_fd()) };
            if mime == "text/uri-list" {
                let uris: Vec<String> = paths.iter().map(|p| {
                    let clean = p.replace('\\', "/");
                    let parts: Vec<String> = clean.split('/').map(|seg| {
                        let mut enc = String::new();
                        for b in seg.bytes() {
                            if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' || b == b'/' {
                                enc.push(b as char);
                            } else {
                                enc.push_str(&format!("%{:02X}", b));
                            }
                        }
                        enc
                    }).collect();
                    format!("file://{}\r\n", parts.join("/"))
                }).collect();
                let _ = file.write_all(uris.concat().as_bytes());
            } else if mime == "x-special/gnome-copied-files" {
                let uris: Vec<String> = paths.iter().map(|p| format!("file://{}", p)).collect();
                let body = format!("copy\n{}", uris.join("\n"));
                let _ = file.write_all(body.as_bytes());
            } else {
                let _ = file.write_all(paths.join("\n").as_bytes());
            }
            let _ = file.flush();
        }
    }

    fn cancelled(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &WlDataSource,
    ) {
        self.outgoing_drag_sources.retain(|item| item.0.inner() != source);
    }

    fn dnd_dropped(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _source: &WlDataSource,
    ) {}

    fn dnd_finished(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        source: &WlDataSource,
    ) {
        self.outgoing_drag_sources.retain(|item| item.0.inner() != source);
    }

    fn action(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _source: &WlDataSource,
        _action: DndAction,
    ) {}
}

impl WinitState {
    pub fn process_outgoing_drag_requests(&mut self) {
        let req = match OUTGOING_DRAG_REQUEST.lock().unwrap().take() {
            Some(r) => r,
            None => return,
        };

        let paths = req.paths;
        if paths.is_empty() { return; }

        let mut target_seat_and_serial = None;
        for seat_state in self.seats.values() {
            if let Some(ref data_device) = seat_state.data_device {
                if let Some(ref pointer) = seat_state.pointer {
                    let serial = pointer.pointer().winit_data().latest_button_serial();
                    if serial != 0 {
                        target_seat_and_serial = Some((data_device, serial));
                        break;
                    }
                }
            }
        }

        let (data_device, serial) = match target_seat_and_serial {
            Some(s) => s,
            None => return,
        };

        let surface = {
            let windows = self.windows.borrow();
            match windows.values().next() {
                Some(w) => w.lock().unwrap().window.wl_surface().clone(),
                None => return,
            }
        };

        // Strictly advertise Copy action to prevent target file managers from moving/deleting files
        let drag_source = self.data_device_manager_state.create_drag_and_drop_source(
            &self.queue_handle,
            vec![
                "text/uri-list".to_string(),
                "x-special/gnome-copied-files".to_string(),
                "text/plain;charset=utf-8".to_string(),
                "text/plain".to_string(),
            ],
            DndAction::Copy,
        );

        // Render custom file thumbnail or icon preview surface on Wayland
        let icon_surface = self.compositor_state.create_surface(&self.queue_handle);
        let (icon_surf_opt, pool_opt) = if let Some((w, h, bgra)) = req.preview {
            let width = (w as usize).clamp(16, 256);
            let height = (h as usize).clamp(16, 256);
            let stride = width * 4;
            if let Ok(mut pool) = SlotPool::new(width * height * 4, &self.shm) {
                if let Ok((buffer, canvas)) = pool.create_buffer(width as i32, height as i32, stride as i32, Format::Argb8888) {
                    if bgra.len() == width * height * 4 {
                        canvas.copy_from_slice(&bgra);
                    }
                    buffer.attach_to(&icon_surface).ok();
                    icon_surface.damage(0, 0, width as i32, height as i32);
                    icon_surface.commit();
                    (Some(icon_surface), Some(pool))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        } else {
            // Default pill card fallback
            let width = 160;
            let height = 36;
            let stride = width * 4;
            if let Ok(mut pool) = SlotPool::new(width * height * 4, &self.shm) {
                if let Ok((buffer, canvas)) = pool.create_buffer(width as i32, height as i32, stride as i32, Format::Argb8888) {
                    for (i, chunk) in canvas.chunks_exact_mut(4).enumerate() {
                        let px = (i % width) as i32;
                        let py = (i / width) as i32;
                        let is_border = px < 2 || px >= width as i32 - 2 || py < 2 || py >= height as i32 - 2;
                        if is_border {
                            chunk[0] = 255; chunk[1] = 140; chunk[2] = 160; chunk[3] = 255;
                        } else {
                            chunk[0] = 240; chunk[1] = 85; chunk[2] = 120; chunk[3] = 210;
                        }
                    }
                    buffer.attach_to(&icon_surface).ok();
                    icon_surface.damage(0, 0, width as i32, height as i32);
                    icon_surface.commit();
                    (Some(icon_surface), Some(pool))
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            }
        };

        if let Some(ref icon_surf) = icon_surf_opt {
            drag_source.start_drag(data_device, &surface, Some(icon_surf), serial);
        } else {
            drag_source.start_drag(data_device, &surface, None, serial);
        }

        self.outgoing_drag_sources.push((drag_source, paths, icon_surf_opt, pool_opt));
    }
}

fn pick_dnd_mime_type(offer: &DragOffer) -> Option<String> {
    offer.with_mime_types(|mimes| {
        for target in &["text/uri-list", "x-special/gnome-copied-files", "text/plain;charset=utf-8", "text/plain"] {
            if let Some(m) = mimes.iter().find(|m| m.as_str() == *target) {
                return Some(m.clone());
            }
        }
        mimes.first().cloned()
    })
}

impl DataDeviceHandler for WinitState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        data_device: &WlDataDevice,
        _x: f64,
        _y: f64,
        surface: &WlSurface,
    ) {
        let window_id = super::make_wid(surface);
        if let Some(seat) = self.seats.values().find(|s| s.data_device.as_ref().map(|d| d.inner()) == Some(data_device)) {
            if let Some(offer) = seat.data_device.as_ref().and_then(|d| d.data().drag_offer()) {
                if let Some(mime) = pick_dnd_mime_type(&offer) {
                    offer.accept_mime_type(offer.serial, Some(mime));
                }
                offer.set_actions(DndAction::Copy, DndAction::Copy);
                self.current_drag_offer = Some((window_id, offer.clone()));
            }
        }
        self.events_sink.push_window_event(crate::event::WindowEvent::HoveredFile(std::path::PathBuf::new()), window_id);
    }

    fn leave(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _data_device: &WlDataDevice) {
        if let Some((window_id, _)) = self.current_drag_offer.take() {
            self.events_sink.push_window_event(crate::event::WindowEvent::HoveredFileCancelled, window_id);
        }
    }

    fn motion(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &WlDataDevice,
        _x: f64,
        _y: f64,
    ) {
        if let Some((_, offer)) = &self.current_drag_offer {
            if let Some(mime) = pick_dnd_mime_type(offer) {
                offer.accept_mime_type(offer.serial, Some(mime));
            }
            offer.set_actions(DndAction::Copy, DndAction::Copy);
        }
    }

    fn selection(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &WlDataDevice,
    ) {}

    fn drop_performed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _data_device: &WlDataDevice,
    ) {
        if let Some((window_id, offer)) = self.current_drag_offer.take() {
            let mime = pick_dnd_mime_type(&offer).unwrap_or_else(|| "text/uri-list".to_string());
            offer.accept_mime_type(offer.serial, Some(mime.clone()));
            offer.set_actions(DndAction::Copy, DndAction::Copy);
            if let Ok(mut read_pipe) = offer.receive(mime) {
                let sink = self.window_events_sink.clone();
                let awakener = self.awakener.clone();
                std::thread::spawn(move || {
                    use std::io::Read;
                    let mut buffer = String::new();
                    let read_ok = read_pipe.read_to_string(&mut buffer).is_ok();
                    offer.finish();
                    offer.destroy();

                    let mut found_any = false;
                    if read_ok {
                        for line in buffer.lines() {
                            let line = line.trim();
                            if line.is_empty() || line.starts_with('#') || line == "copy" || line == "cut" {
                                continue;
                            }
                            let decoded_opt = if let Some(path_str) = line.strip_prefix("file://") {
                                Some(decode_uri_component(path_str))
                            } else if line.starts_with('/') {
                                Some(line.to_string())
                            } else {
                                None
                            };

                            if let Some(decoded) = decoded_opt {
                                if !decoded.is_empty() {
                                    let p = std::path::PathBuf::from(decoded);
                                    sink.lock().unwrap().push_window_event(crate::event::WindowEvent::DroppedFile(p), window_id);
                                    found_any = true;
                                }
                            }
                        }
                    }

                    if !found_any {
                        sink.lock().unwrap().push_window_event(crate::event::WindowEvent::HoveredFileCancelled, window_id);
                    }

                    if let Some(aw) = awakener {
                        aw.ping();
                    }
                });
            } else {
                offer.finish();
                offer.destroy();
                self.events_sink.push_window_event(crate::event::WindowEvent::HoveredFileCancelled, window_id);
            }
        }
    }
}

fn decode_uri_component(s: &str) -> String {
    let mut bytes = Vec::new();
    let mut chars = s.bytes();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let h1 = chars.next();
            let h2 = chars.next();
            if let (Some(h1), Some(h2)) = (h1, h2) {
                if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&[h1, h2]).unwrap_or("0"), 16) {
                    bytes.push(val);
                    continue;
                }
            }
        }
        bytes.push(b);
    }
    String::from_utf8_lossy(&bytes).to_string()
}
