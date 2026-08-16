use crate::{
    database::Database,
    errors::AppError,
    models::{
        EmbeddedMpvBounds, EmbeddedMpvControlRequest, EmbeddedMpvSession, EmbeddedMpvStatus,
        LibraryMediaKind,
    },
};
use tauri::WebviewWindow;

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use crate::models::{EmbeddedMpvControl, SaveLibraryPlaybackRequest};
    use gtk::{glib, prelude::*};
    use libloading::Library;
    use std::{
        cell::RefCell,
        collections::VecDeque,
        ffi::{c_char, c_int, c_void, CStr, CString},
        path::{Path, PathBuf},
        ptr,
        rc::Rc,
        sync::{mpsc, Arc, Mutex},
        time::{Duration, Instant},
    };

    const MPV_FORMAT_FLAG: c_int = 3;
    const MPV_FORMAT_DOUBLE: c_int = 5;
    const MPV_EVENT_END_FILE: c_int = 7;
    const MPV_RENDER_PARAM_INVALID: c_int = 0;
    const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
    const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
    const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
    const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
    const MPV_RENDER_UPDATE_FRAME: u64 = 1;
    const ACTION_TIMEOUT: Duration = Duration::from_secs(4);

    #[derive(Clone)]
    pub struct EmbeddedMpvManager {
        database: Database,
        shared: Arc<Shared>,
    }

    struct Shared {
        status: Mutex<EmbeddedMpvStatus>,
        actions: Mutex<VecDeque<Action>>,
    }

    struct OpenRequest {
        path: PathBuf,
        session: EmbeddedMpvSession,
        bounds: EmbeddedMpvBounds,
    }

    enum Action {
        Open(
            OpenRequest,
            mpsc::Sender<Result<EmbeddedMpvSession, AppError>>,
        ),
        Bounds(EmbeddedMpvBounds),
        Control(
            EmbeddedMpvControlRequest,
            mpsc::Sender<Result<(), AppError>>,
        ),
        Close,
    }

    impl EmbeddedMpvManager {
        pub fn new(database: Database) -> Self {
            Self {
                database,
                shared: Arc::new(Shared {
                    status: Mutex::new(EmbeddedMpvStatus {
                        available: false,
                        active: None,
                        detail: "Embedded libmpv is initializing.".to_owned(),
                        last_error: None,
                    }),
                    actions: Mutex::new(VecDeque::new()),
                }),
            }
        }

        pub fn install(&self, window: &WebviewWindow) -> Result<(), AppError> {
            let shared = Arc::clone(&self.shared);
            let database = self.database.clone();
            window
                .with_webview(move |platform| {
                    if let Err(error) = install_overlay(platform.inner(), shared.clone(), database)
                    {
                        set_unavailable(&shared, error);
                    }
                })
                .map_err(|error| {
                    AppError::internal("could not install embedded libmpv surface", error)
                })
        }

        pub fn status(&self) -> EmbeddedMpvStatus {
            self.shared.status.lock().map_or_else(
                |_| EmbeddedMpvStatus {
                    available: false,
                    active: None,
                    detail: "Embedded libmpv state is unavailable.".to_owned(),
                    last_error: Some("The embedded player state lock was poisoned.".to_owned()),
                },
                |status| status.clone(),
            )
        }

        pub fn open(
            &self,
            item_id: &str,
            start_at_seconds: Option<f64>,
            bounds: EmbeddedMpvBounds,
        ) -> Result<EmbeddedMpvSession, AppError> {
            validate_bounds(bounds)?;
            if !self.status().available {
                return Err(AppError::new(
                    "embedded_mpv_unavailable",
                    self.status().detail,
                ));
            }
            let item = self.database.get_library_item(item_id)?;
            if item.missing {
                return Err(AppError::new(
                    "library_file_not_found",
                    "This video is no longer available at its indexed location.",
                ));
            }
            if !matches!(
                item.media_kind,
                Some(LibraryMediaKind::Video | LibraryMediaKind::Both)
            ) {
                return Err(AppError::new(
                    "embedded_mpv_video_required",
                    "Embedded libmpv playback is available only for cataloged videos.",
                ));
            }
            let path = PathBuf::from(&item.path);
            if !path.is_absolute() || !path.is_file() {
                return Err(AppError::new(
                    "library_file_not_found",
                    "This video is no longer available at its indexed location.",
                ));
            }
            if start_at_seconds.is_some_and(|value| !value.is_finite() || value < 0.0) {
                return Err(AppError::new(
                    "invalid_embedded_mpv_start",
                    "The requested embedded playback timestamp is invalid.",
                ));
            }
            let duration = item.duration_seconds.unwrap_or(0.0).max(0.0);
            let saved = self.database.get_library_playback(item_id)?;
            let resumed_at = start_at_seconds
                .map(|value| {
                    if duration > 0.0 {
                        value.min(duration)
                    } else {
                        value
                    }
                })
                .or_else(|| {
                    saved
                        .filter(|state| !state.completed && state.position_seconds >= 5.0)
                        .map(|state| state.position_seconds)
                })
                .unwrap_or(0.0);
            let session = EmbeddedMpvSession {
                item_id: item.id,
                title: item.title,
                resumed_at_seconds: resumed_at,
                position_seconds: resumed_at,
                duration_seconds: duration,
                paused: false,
            };
            let (sender, receiver) = mpsc::channel();
            self.enqueue(Action::Open(
                OpenRequest {
                    path,
                    session,
                    bounds,
                },
                sender,
            ))?;
            receiver.recv_timeout(ACTION_TIMEOUT).map_err(|_| {
                AppError::new(
                    "embedded_mpv_start_timeout",
                    "Embedded libmpv did not start in time. The existing playback options remain available.",
                )
            })?
        }

        pub fn update_bounds(&self, bounds: EmbeddedMpvBounds) -> Result<(), AppError> {
            validate_bounds(bounds)?;
            self.enqueue(Action::Bounds(bounds))
        }

        pub fn control(&self, request: EmbeddedMpvControlRequest) -> Result<(), AppError> {
            validate_control(request)?;
            let (sender, receiver) = mpsc::channel();
            self.enqueue(Action::Control(request, sender))?;
            receiver.recv_timeout(ACTION_TIMEOUT).map_err(|_| {
                AppError::new(
                    "embedded_mpv_control_timeout",
                    "Embedded libmpv did not respond to the playback control.",
                )
            })?
        }

        pub fn close(&self) {
            let _ = self.enqueue(Action::Close);
        }

        fn enqueue(&self, action: Action) -> Result<(), AppError> {
            self.shared
                .actions
                .lock()
                .map_err(|_| {
                    AppError::new(
                        "embedded_mpv_unavailable",
                        "Embedded libmpv action queue is unavailable.",
                    )
                })?
                .push_back(action);
            Ok(())
        }
    }

    fn validate_bounds(bounds: EmbeddedMpvBounds) -> Result<(), AppError> {
        let values = [
            bounds.x,
            bounds.y,
            bounds.width,
            bounds.height,
            bounds.device_pixel_ratio,
        ];
        if values.iter().any(|value| !value.is_finite())
            || bounds.width < 0.0
            || bounds.height < 0.0
            || bounds.width > 32_768.0
            || bounds.height > 32_768.0
            || !(-32_768.0..=32_768.0).contains(&bounds.x)
            || !(-32_768.0..=32_768.0).contains(&bounds.y)
            || !(0.25..=8.0).contains(&bounds.device_pixel_ratio)
        {
            return Err(AppError::new(
                "invalid_embedded_mpv_bounds",
                "The embedded playback surface bounds are invalid.",
            ));
        }
        Ok(())
    }

    fn validate_control(request: EmbeddedMpvControlRequest) -> Result<(), AppError> {
        let valid = match request.action {
            EmbeddedMpvControl::Play | EmbeddedMpvControl::Pause => request.value.is_none(),
            EmbeddedMpvControl::SeekRelative => request
                .value
                .is_some_and(|value| value.is_finite() && value.abs() <= 3_600.0),
            EmbeddedMpvControl::SeekAbsolute => request
                .value
                .is_some_and(|value| value.is_finite() && value >= 0.0),
            EmbeddedMpvControl::SetRate => request
                .value
                .is_some_and(|value| value.is_finite() && (0.25..=4.0).contains(&value)),
            EmbeddedMpvControl::SetMuted => {
                matches!(request.value, Some(0.0 | 1.0))
            }
        };
        if valid {
            Ok(())
        } else {
            Err(AppError::new(
                "invalid_embedded_mpv_control",
                "The embedded playback control is invalid.",
            ))
        }
    }

    fn set_unavailable(shared: &Arc<Shared>, detail: String) {
        if let Ok(mut status) = shared.status.lock() {
            status.available = false;
            status.active = None;
            status.detail = detail.clone();
            status.last_error = Some(detail);
        }
    }

    fn install_overlay(
        webview: impl IsA<gtk::Widget>,
        shared: Arc<Shared>,
        database: Database,
    ) -> Result<(), String> {
        MpvApi::probe()?;
        let widget = webview.upcast_ref::<gtk::Widget>();
        let parent = widget
            .parent()
            .and_then(|parent| parent.downcast::<gtk::Box>().ok())
            .ok_or_else(|| "The Tauri WebView is not inside its expected GTK box.".to_owned())?;
        let position = parent
            .children()
            .iter()
            .position(|child| child == widget)
            .unwrap_or(0) as i32;
        parent.remove(widget);

        let overlay = gtk::Overlay::new();
        let fixed = gtk::Fixed::new();
        fixed.set_halign(gtk::Align::Fill);
        fixed.set_valign(gtk::Align::Fill);
        let area = gtk::GLArea::new();
        area.set_required_version(3, 2);
        area.set_has_depth_buffer(false);
        area.set_has_stencil_buffer(false);
        area.set_auto_render(false);
        area.set_visible(false);
        fixed.put(&area, 0, 0);
        overlay.add(widget);
        overlay.add_overlay(&fixed);
        overlay.set_overlay_pass_through(&fixed, true);
        parent.pack_start(&overlay, true, true, 0);
        parent.reorder_child(&overlay, position);
        overlay.show();
        fixed.show();

        let runtime = Rc::new(RefCell::new(OverlayRuntime {
            shared: Arc::clone(&shared),
            database,
            player: None,
            bounds: None,
            last_status_update: Instant::now(),
        }));

        let render_runtime = Rc::clone(&runtime);
        area.connect_render(move |area, _| {
            let scale = area.scale_factor().max(1);
            let result = render_runtime.borrow_mut().player.as_mut().map(|player| {
                player.render(
                    area.allocated_width().saturating_mul(scale),
                    area.allocated_height().saturating_mul(scale),
                )
            });
            if let Some(Err(error)) = result {
                render_runtime.borrow_mut().finish(area, false, Some(error));
            }
            glib::Propagation::Stop
        });

        let unrealize_runtime = Rc::clone(&runtime);
        area.connect_unrealize(move |area| {
            area.make_current();
            unrealize_runtime.borrow_mut().finish(area, false, None);
        });

        let tick_runtime = Rc::clone(&runtime);
        let tick_area = area.downgrade();
        let tick_fixed = fixed.downgrade();
        glib::timeout_add_local(Duration::from_millis(16), move || {
            let (Some(area), Some(fixed)) = (tick_area.upgrade(), tick_fixed.upgrade()) else {
                return glib::ControlFlow::Break;
            };
            let mut runtime = tick_runtime.borrow_mut();
            runtime.process_actions(&area, &fixed);
            runtime.tick(&area);
            glib::ControlFlow::Continue
        });

        if let Ok(mut status) = shared.status.lock() {
            status.available = true;
            status.detail = "Experimental libmpv rendering is available inside the Watch workspace. No compatibility copy is created.".to_owned();
            status.last_error = None;
        }
        Ok(())
    }

    struct OverlayRuntime {
        shared: Arc<Shared>,
        database: Database,
        player: Option<Player>,
        bounds: Option<EmbeddedMpvBounds>,
        last_status_update: Instant,
    }

    impl OverlayRuntime {
        fn process_actions(&mut self, area: &gtk::GLArea, fixed: &gtk::Fixed) {
            let actions = self
                .shared
                .actions
                .lock()
                .map(|mut actions| actions.drain(..).collect::<Vec<_>>())
                .unwrap_or_default();
            for action in actions {
                match action {
                    Action::Open(request, sender) => {
                        let result = self.open(area, fixed, request);
                        let _ = sender.send(result);
                    }
                    Action::Bounds(bounds) => {
                        self.bounds = Some(bounds);
                        apply_bounds(fixed, area, bounds, self.player.is_some());
                    }
                    Action::Control(request, sender) => {
                        let result = self.control(request);
                        let _ = sender.send(result);
                    }
                    Action::Close => {
                        self.finish(area, false, None);
                    }
                }
            }
        }

        fn open(
            &mut self,
            area: &gtk::GLArea,
            fixed: &gtk::Fixed,
            request: OpenRequest,
        ) -> Result<EmbeddedMpvSession, AppError> {
            self.finish(area, false, None);
            self.bounds = Some(request.bounds);
            apply_bounds(fixed, area, request.bounds, true);
            area.realize();
            area.make_current();
            if let Some(error) = area.error() {
                area.hide();
                return Err(AppError::new(
                    "embedded_mpv_gl_unavailable",
                    format!("The GTK OpenGL surface could not start: {error}"),
                ));
            }
            let player = Player::create(&request.path, request.session.resumed_at_seconds)
                .map_err(|error| {
                    area.hide();
                    if let Ok(mut status) = self.shared.status.lock() {
                        status.last_error = Some(error.clone());
                    }
                    AppError::new("embedded_mpv_start_failed", error)
                })?;
            self.player = Some(player);
            if let Ok(mut status) = self.shared.status.lock() {
                status.active = Some(request.session.clone());
                status.last_error = None;
                status.detail =
                    "Playing through embedded libmpv without a compatibility copy.".to_owned();
            }
            area.queue_render();
            Ok(request.session)
        }

        fn control(&mut self, request: EmbeddedMpvControlRequest) -> Result<(), AppError> {
            let player = self.player.as_mut().ok_or_else(|| {
                AppError::new(
                    "embedded_mpv_not_active",
                    "No embedded libmpv playback is active.",
                )
            })?;
            let value = request.value.unwrap_or_default();
            let result = match request.action {
                EmbeddedMpvControl::Play => player.command(&["set", "pause", "no"]),
                EmbeddedMpvControl::Pause => player.command(&["set", "pause", "yes"]),
                EmbeddedMpvControl::SeekRelative => {
                    player.command(&["seek", &format!("{value:.3}"), "relative+exact"])
                }
                EmbeddedMpvControl::SeekAbsolute => {
                    player.command(&["seek", &format!("{value:.3}"), "absolute+exact"])
                }
                EmbeddedMpvControl::SetRate => {
                    player.command(&["set", "speed", &format!("{value:.3}")])
                }
                EmbeddedMpvControl::SetMuted => {
                    player.command(&["set", "mute", if value >= 0.5 { "yes" } else { "no" }])
                }
            };
            result.map_err(|error| AppError::new("embedded_mpv_control_failed", error))
        }

        fn tick(&mut self, area: &gtk::GLArea) {
            let ended = self
                .player
                .as_ref()
                .is_some_and(|player| player.drain_events());
            if ended {
                self.finish(area, true, None);
                return;
            }
            if self.player.is_none() {
                return;
            }
            area.queue_render();
            if self.last_status_update.elapsed() < Duration::from_millis(200) {
                return;
            }
            self.last_status_update = Instant::now();
            let Some(player) = self.player.as_ref() else {
                return;
            };
            let position = player.get_double("time-pos").unwrap_or(0.0).max(0.0);
            let duration = player.get_double("duration").unwrap_or(0.0).max(0.0);
            let paused = player.get_flag("pause").unwrap_or(false);
            if let Ok(mut status) = self.shared.status.lock() {
                if let Some(active) = status.active.as_mut() {
                    active.position_seconds = position;
                    if duration > 0.0 {
                        active.duration_seconds = duration;
                    }
                    active.paused = paused;
                }
            }
        }

        fn finish(&mut self, area: &gtk::GLArea, ended: bool, error: Option<String>) {
            let Some(player) = self.player.as_ref() else {
                area.hide();
                return;
            };
            let position = player.get_double("time-pos").unwrap_or(0.0).max(0.0);
            let duration = player.get_double("duration").unwrap_or(0.0).max(0.0);
            let active = self
                .shared
                .status
                .lock()
                .ok()
                .and_then(|status| status.active.clone());
            area.make_current();
            self.player.take();
            area.hide();
            if let Some(active) = active {
                let effective_duration = if duration > 0.0 {
                    duration
                } else {
                    active.duration_seconds
                };
                let completed = ended
                    && effective_duration > 0.0
                    && (position >= effective_duration - 10.0
                        || position / effective_duration >= 0.95);
                if effective_duration > 0.0 {
                    if let Err(save_error) =
                        self.database
                            .save_library_playback(SaveLibraryPlaybackRequest {
                                item_id: active.item_id,
                                position_seconds: position.min(effective_duration),
                                duration_seconds: effective_duration,
                                completed,
                            })
                    {
                        tracing::warn!(%save_error, "could not persist embedded libmpv progress");
                    }
                }
            }
            if let Ok(mut status) = self.shared.status.lock() {
                status.active = None;
                status.last_error = error.clone();
                status.detail = error.unwrap_or_else(|| {
                    "Experimental embedded libmpv is ready. No compatibility copy is required."
                        .to_owned()
                });
            }
        }
    }

    fn apply_bounds(
        fixed: &gtk::Fixed,
        area: &gtk::GLArea,
        bounds: EmbeddedMpvBounds,
        active: bool,
    ) {
        let gtk_scale = area.scale_factor().max(1) as f64;
        let factor = bounds.device_pixel_ratio / gtk_scale;
        let x = (bounds.x * factor).round() as i32;
        let y = (bounds.y * factor).round() as i32;
        let width = (bounds.width * factor).round().max(1.0) as i32;
        let height = (bounds.height * factor).round().max(1.0) as i32;
        fixed.move_(area, x, y);
        area.set_size_request(width, height);
        area.set_visible(active && bounds.visible && bounds.width >= 2.0 && bounds.height >= 2.0);
        if area.is_visible() {
            area.queue_render();
        }
    }

    #[repr(C)]
    struct MpvHandle {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct MpvRenderContext {
        _private: [u8; 0],
    }

    #[repr(C)]
    struct MpvEvent {
        event_id: c_int,
        error: c_int,
        reply_userdata: u64,
        data: *mut c_void,
    }

    #[repr(C)]
    struct MpvRenderParam {
        kind: c_int,
        data: *mut c_void,
    }

    type GetProcAddress =
        unsafe extern "C" fn(context: *mut c_void, name: *const c_char) -> *mut c_void;

    #[repr(C)]
    struct MpvOpenGlInitParams {
        get_proc_address: GetProcAddress,
        get_proc_address_context: *mut c_void,
    }

    #[repr(C)]
    struct MpvOpenGlFbo {
        fbo: c_int,
        width: c_int,
        height: c_int,
        internal_format: c_int,
    }

    type MpvCreate = unsafe extern "C" fn() -> *mut MpvHandle;
    type MpvInitialize = unsafe extern "C" fn(*mut MpvHandle) -> c_int;
    type MpvSetOptionString =
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, *const c_char) -> c_int;
    type MpvCommand = unsafe extern "C" fn(*mut MpvHandle, *const *const c_char) -> c_int;
    type MpvGetProperty =
        unsafe extern "C" fn(*mut MpvHandle, *const c_char, c_int, *mut c_void) -> c_int;
    type MpvTerminateDestroy = unsafe extern "C" fn(*mut MpvHandle);
    type MpvErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
    type MpvWaitEvent = unsafe extern "C" fn(*mut MpvHandle, f64) -> *const MpvEvent;
    type MpvRenderCreate = unsafe extern "C" fn(
        *mut *mut MpvRenderContext,
        *mut MpvHandle,
        *mut MpvRenderParam,
    ) -> c_int;
    type MpvRenderUpdate = unsafe extern "C" fn(*mut MpvRenderContext) -> u64;
    type MpvRender = unsafe extern "C" fn(*mut MpvRenderContext, *mut MpvRenderParam) -> c_int;
    type MpvRenderFree = unsafe extern "C" fn(*mut MpvRenderContext);
    type GlGetProcAddress = unsafe extern "C" fn(*const c_char) -> *const c_void;

    struct MpvApi {
        _library: Library,
        create: MpvCreate,
        initialize: MpvInitialize,
        set_option_string: MpvSetOptionString,
        command: MpvCommand,
        get_property: MpvGetProperty,
        terminate_destroy: MpvTerminateDestroy,
        error_string: MpvErrorString,
        wait_event: MpvWaitEvent,
        render_create: MpvRenderCreate,
        render_update: MpvRenderUpdate,
        render: MpvRender,
        render_free: MpvRenderFree,
    }

    impl MpvApi {
        fn probe() -> Result<(), String> {
            Self::load().map(drop)
        }

        fn load() -> Result<Self, String> {
            let library = load_library(&["libmpv.so.2", "libmpv.so"])?;
            // SAFETY: signatures mirror mpv/client.h and mpv/render.h, and the
            // library stays owned for the lifetime of every loaded pointer.
            unsafe {
                Ok(Self {
                    create: load_symbol(&library, b"mpv_create\0")?,
                    initialize: load_symbol(&library, b"mpv_initialize\0")?,
                    set_option_string: load_symbol(&library, b"mpv_set_option_string\0")?,
                    command: load_symbol(&library, b"mpv_command\0")?,
                    get_property: load_symbol(&library, b"mpv_get_property\0")?,
                    terminate_destroy: load_symbol(&library, b"mpv_terminate_destroy\0")?,
                    error_string: load_symbol(&library, b"mpv_error_string\0")?,
                    wait_event: load_symbol(&library, b"mpv_wait_event\0")?,
                    render_create: load_symbol(&library, b"mpv_render_context_create\0")?,
                    render_update: load_symbol(&library, b"mpv_render_context_update\0")?,
                    render: load_symbol(&library, b"mpv_render_context_render\0")?,
                    render_free: load_symbol(&library, b"mpv_render_context_free\0")?,
                    _library: library,
                })
            }
        }

        fn error(&self, code: c_int, operation: &str) -> String {
            // SAFETY: mpv_error_string returns a process-lifetime static string.
            let detail = unsafe {
                let value = (self.error_string)(code);
                if value.is_null() {
                    format!("mpv error {code}")
                } else {
                    CStr::from_ptr(value).to_string_lossy().into_owned()
                }
            };
            format!("{operation}: {detail}")
        }
    }

    struct GlResolver {
        _egl: Option<Library>,
        _gl: Option<Library>,
        egl_get_proc_address: Option<GlGetProcAddress>,
        glx_get_proc_address: Option<GlGetProcAddress>,
    }

    impl GlResolver {
        fn load() -> Result<Self, String> {
            let egl = load_optional_library(&["libEGL.so.1", "libEGL.so"]);
            let gl = load_optional_library(&["libGL.so.1", "libGL.so"]);
            // SAFETY: standard EGL/GLX resolver signatures; libraries are retained.
            let egl_get_proc_address = unsafe {
                egl.as_ref()
                    .and_then(|library| load_symbol(library, b"eglGetProcAddress\0").ok())
            };
            // SAFETY: same contract as the EGL resolver above.
            let glx_get_proc_address = unsafe {
                gl.as_ref()
                    .and_then(|library| load_symbol(library, b"glXGetProcAddress\0").ok())
            };
            if egl_get_proc_address.is_none() && glx_get_proc_address.is_none() {
                return Err("Neither EGL nor GLX exposes an OpenGL symbol resolver.".to_owned());
            }
            Ok(Self {
                _egl: egl,
                _gl: gl,
                egl_get_proc_address,
                glx_get_proc_address,
            })
        }
    }

    unsafe extern "C" fn resolve_gl(context: *mut c_void, name: *const c_char) -> *mut c_void {
        if context.is_null() || name.is_null() {
            return ptr::null_mut();
        }
        // SAFETY: libmpv returns the stable resolver pointer supplied at creation.
        let resolver = unsafe { &*(context.cast::<GlResolver>()) };
        if let Some(resolve) = resolver.egl_get_proc_address {
            // SAFETY: name is a NUL-terminated GL symbol supplied by libmpv.
            let value = unsafe { resolve(name) };
            if !value.is_null() {
                return value.cast_mut();
            }
        }
        if let Some(resolve) = resolver.glx_get_proc_address {
            // SAFETY: same contract as EGL above.
            return unsafe { resolve(name) }.cast_mut();
        }
        ptr::null_mut()
    }

    struct Player {
        api: MpvApi,
        handle: *mut MpvHandle,
        render_context: *mut MpvRenderContext,
        resolver: Box<GlResolver>,
    }

    impl Player {
        fn create(path: &Path, start_at: f64) -> Result<Self, String> {
            let api = MpvApi::load()?;
            let resolver = Box::new(GlResolver::load()?);
            // SAFETY: calls use retained library pointers and NUL-terminated strings.
            unsafe {
                let handle = (api.create)();
                if handle.is_null() {
                    return Err("mpv_create returned no client handle.".to_owned());
                }
                let mut player = Self {
                    api,
                    handle,
                    render_context: ptr::null_mut(),
                    resolver,
                };
                for (name, value) in [
                    ("terminal", "no"),
                    ("audio-display", "no"),
                    ("hwdec", "auto-safe"),
                    ("vo", "libmpv"),
                    ("ytdl", "no"),
                    ("input-default-bindings", "no"),
                ] {
                    player.set_option(name, value)?;
                }
                if start_at > 0.0 {
                    player.set_option("start", &format!("{start_at:.3}"))?;
                }
                let result = (player.api.initialize)(player.handle);
                if result < 0 {
                    return Err(player.api.error(result, "mpv_initialize"));
                }
                player.create_render_context()?;
                player.load_file(path)?;
                Ok(player)
            }
        }

        unsafe fn set_option(&mut self, name: &str, value: &str) -> Result<(), String> {
            let name = CString::new(name).map_err(|_| "Invalid mpv option name.".to_owned())?;
            let value = CString::new(value).map_err(|_| "Invalid mpv option value.".to_owned())?;
            // SAFETY: handle is live and strings remain valid for this call.
            let result =
                unsafe { (self.api.set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
            (result >= 0)
                .then_some(())
                .ok_or_else(|| self.api.error(result, "mpv_set_option_string"))
        }

        unsafe fn create_render_context(&mut self) -> Result<(), String> {
            let mut init = MpvOpenGlInitParams {
                get_proc_address: resolve_gl,
                get_proc_address_context: self.resolver.as_mut() as *mut GlResolver as *mut c_void,
            };
            let api_type = b"opengl\0";
            let mut params = [
                MpvRenderParam {
                    kind: MPV_RENDER_PARAM_API_TYPE,
                    data: api_type.as_ptr().cast_mut().cast(),
                },
                MpvRenderParam {
                    kind: MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                    data: (&mut init as *mut MpvOpenGlInitParams).cast(),
                },
                MpvRenderParam {
                    kind: MPV_RENDER_PARAM_INVALID,
                    data: ptr::null_mut(),
                },
            ];
            // SAFETY: GLArea context is current and parameters live for the call.
            let result = unsafe {
                (self.api.render_create)(&mut self.render_context, self.handle, params.as_mut_ptr())
            };
            (result >= 0 && !self.render_context.is_null())
                .then_some(())
                .ok_or_else(|| self.api.error(result, "mpv_render_context_create"))
        }

        unsafe fn load_file(&self, path: &Path) -> Result<(), String> {
            let path = path
                .to_str()
                .ok_or_else(|| "Embedded libmpv requires a UTF-8 media path.".to_owned())?;
            let command = CString::new("loadfile").expect("static command");
            let path = CString::new(path).map_err(|_| "Media path contains NUL.".to_owned())?;
            let args = [command.as_ptr(), path.as_ptr(), ptr::null()];
            // SAFETY: argument pointers remain live and terminate with NULL.
            let result = unsafe { (self.api.command)(self.handle, args.as_ptr()) };
            (result >= 0)
                .then_some(())
                .ok_or_else(|| self.api.error(result, "mpv_command(loadfile)"))
        }

        fn command(&self, values: &[&str]) -> Result<(), String> {
            let strings = values
                .iter()
                .map(|value| CString::new(*value).map_err(|_| "mpv command contains NUL."))
                .collect::<Result<Vec<_>, _>>()?;
            let mut pointers = strings
                .iter()
                .map(|value| value.as_ptr())
                .collect::<Vec<_>>();
            pointers.push(ptr::null());
            // SAFETY: handle is live and argument pointers remain live for the call.
            let result = unsafe { (self.api.command)(self.handle, pointers.as_ptr()) };
            (result >= 0)
                .then_some(())
                .ok_or_else(|| self.api.error(result, "mpv_command"))
        }

        fn get_double(&self, name: &str) -> Option<f64> {
            let name = CString::new(name).ok()?;
            let mut value = 0.0_f64;
            // SAFETY: destination matches MPV_FORMAT_DOUBLE and lives for the call.
            let result = unsafe {
                (self.api.get_property)(
                    self.handle,
                    name.as_ptr(),
                    MPV_FORMAT_DOUBLE,
                    (&mut value as *mut f64).cast(),
                )
            };
            (result >= 0 && value.is_finite()).then_some(value)
        }

        fn get_flag(&self, name: &str) -> Option<bool> {
            let name = CString::new(name).ok()?;
            let mut value = 0_i32;
            // SAFETY: destination matches MPV_FORMAT_FLAG and lives for the call.
            let result = unsafe {
                (self.api.get_property)(
                    self.handle,
                    name.as_ptr(),
                    MPV_FORMAT_FLAG,
                    (&mut value as *mut i32).cast(),
                )
            };
            (result >= 0).then_some(value != 0)
        }

        fn drain_events(&self) -> bool {
            loop {
                // SAFETY: zero timeout never blocks and mpv owns the returned event.
                let event = unsafe { (self.api.wait_event)(self.handle, 0.0) };
                if event.is_null() {
                    return false;
                }
                // SAFETY: pointer remains valid until the next mpv API call.
                let event_id = unsafe { (*event).event_id };
                if event_id == 0 {
                    return false;
                }
                if event_id == MPV_EVENT_END_FILE {
                    return true;
                }
            }
        }

        fn render(&mut self, width: c_int, height: c_int) -> Result<bool, String> {
            if width <= 0 || height <= 0 {
                return Ok(false);
            }
            let mut fbo = MpvOpenGlFbo {
                fbo: 0,
                width,
                height,
                internal_format: 0,
            };
            let mut flip_y: c_int = 1;
            let mut params = [
                MpvRenderParam {
                    kind: MPV_RENDER_PARAM_OPENGL_FBO,
                    data: (&mut fbo as *mut MpvOpenGlFbo).cast(),
                },
                MpvRenderParam {
                    kind: MPV_RENDER_PARAM_FLIP_Y,
                    data: (&mut flip_y as *mut c_int).cast(),
                },
                MpvRenderParam {
                    kind: MPV_RENDER_PARAM_INVALID,
                    data: ptr::null_mut(),
                },
            ];
            // SAFETY: GTK invokes this with GLArea's context and FBO current.
            let result = unsafe {
                let update = (self.api.render_update)(self.render_context);
                if update & MPV_RENDER_UPDATE_FRAME == 0 {
                    return Ok(false);
                }
                (self.api.render)(self.render_context, params.as_mut_ptr())
            };
            (result >= 0)
                .then_some(true)
                .ok_or_else(|| self.api.error(result, "mpv_render_context_render"))
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            // SAFETY: caller makes GLArea context current before dropping; render
            // context must be released before the client handle.
            unsafe {
                if !self.render_context.is_null() {
                    (self.api.render_free)(self.render_context);
                    self.render_context = ptr::null_mut();
                }
                if !self.handle.is_null() {
                    (self.api.terminate_destroy)(self.handle);
                    self.handle = ptr::null_mut();
                }
            }
        }
    }

    fn load_library(names: &[&str]) -> Result<Library, String> {
        names
            .iter()
            .find_map(|name| {
                // SAFETY: fixed system sonames; handle is retained by the owner.
                unsafe { Library::new(name).ok() }
            })
            .ok_or_else(|| {
                format!(
                    "Embedded libmpv requires {}. Install the mpv/libmpv runtime; existing WebView and system-player options remain available.",
                    names.join(" or ")
                )
            })
    }

    fn load_optional_library(names: &[&str]) -> Option<Library> {
        names.iter().find_map(|name| {
            // SAFETY: fixed OpenGL system sonames; owner retains the handle.
            unsafe { Library::new(name).ok() }
        })
    }

    unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
        // SAFETY: caller supplies the exact C signature for the requested symbol.
        unsafe { library.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|error| {
                format!(
                    "Could not load {}: {error}",
                    String::from_utf8_lossy(name).trim_end_matches('\0')
                )
            })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn bounds() -> EmbeddedMpvBounds {
            EmbeddedMpvBounds {
                x: 10.0,
                y: 20.0,
                width: 1280.0,
                height: 720.0,
                device_pixel_ratio: 1.5,
                visible: true,
            }
        }

        #[test]
        fn validates_bounded_finite_surface_geometry() {
            assert!(validate_bounds(bounds()).is_ok());
            assert_eq!(
                validate_bounds(EmbeddedMpvBounds {
                    width: f64::NAN,
                    ..bounds()
                })
                .expect_err("non-finite bounds")
                .code,
                "invalid_embedded_mpv_bounds"
            );
            assert_eq!(
                validate_bounds(EmbeddedMpvBounds {
                    device_pixel_ratio: 9.0,
                    ..bounds()
                })
                .expect_err("unbounded scale")
                .code,
                "invalid_embedded_mpv_bounds"
            );
        }

        #[test]
        fn validates_typed_control_ranges() {
            assert!(validate_control(EmbeddedMpvControlRequest {
                action: EmbeddedMpvControl::SeekRelative,
                value: Some(-10.0),
            })
            .is_ok());
            assert!(validate_control(EmbeddedMpvControlRequest {
                action: EmbeddedMpvControl::SetRate,
                value: Some(2.0),
            })
            .is_ok());
            assert_eq!(
                validate_control(EmbeddedMpvControlRequest {
                    action: EmbeddedMpvControl::SetRate,
                    value: Some(8.0),
                })
                .expect_err("rate must be bounded")
                .code,
                "invalid_embedded_mpv_control"
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod linux {
    use super::*;

    #[derive(Clone)]
    pub struct EmbeddedMpvManager;

    impl EmbeddedMpvManager {
        pub fn new(_database: Database) -> Self {
            Self
        }

        pub fn install(&self, _window: &WebviewWindow) -> Result<(), AppError> {
            Ok(())
        }

        pub fn status(&self) -> EmbeddedMpvStatus {
            EmbeddedMpvStatus {
                available: false,
                active: None,
                detail: "Experimental embedded libmpv is currently Linux-only.".to_owned(),
                last_error: None,
            }
        }

        pub fn open(
            &self,
            _item_id: &str,
            _start_at_seconds: Option<f64>,
            _bounds: EmbeddedMpvBounds,
        ) -> Result<EmbeddedMpvSession, AppError> {
            Err(AppError::new(
                "embedded_mpv_unavailable",
                "Experimental embedded libmpv is currently Linux-only.",
            ))
        }

        pub fn update_bounds(&self, _bounds: EmbeddedMpvBounds) -> Result<(), AppError> {
            Ok(())
        }

        pub fn control(&self, _request: EmbeddedMpvControlRequest) -> Result<(), AppError> {
            Err(AppError::new(
                "embedded_mpv_unavailable",
                "Experimental embedded libmpv is currently Linux-only.",
            ))
        }

        pub fn close(&self) {}
    }
}

pub use linux::EmbeddedMpvManager;
