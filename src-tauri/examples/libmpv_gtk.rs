//! Experimental Linux-only libmpv renderer for a GTK `GLArea`.
//!
//! Run with:
//! `cargo run --example libmpv_gtk -- /absolute/path/to/video [--smoke-seconds 5]`

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("The libmpv GTK experiment is Linux-only.");
}

#[cfg(target_os = "linux")]
mod linux {
    use gtk::{glib, prelude::*};
    use libloading::Library;
    use std::{
        cell::{Cell, RefCell},
        env,
        ffi::{c_char, c_int, c_void, CStr, CString},
        path::{Path, PathBuf},
        ptr,
        rc::Rc,
        time::Duration,
    };

    const MPV_RENDER_PARAM_INVALID: c_int = 0;
    const MPV_RENDER_PARAM_API_TYPE: c_int = 1;
    const MPV_RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
    const MPV_RENDER_PARAM_OPENGL_FBO: c_int = 3;
    const MPV_RENDER_PARAM_FLIP_Y: c_int = 4;
    const MPV_RENDER_UPDATE_FRAME: u64 = 1;

    fn configure_numeric_locale() -> Result<(), String> {
        let locale = b"C\0";
        let configured = unsafe {
            nix::libc::setlocale(
                nix::libc::LC_NUMERIC,
                locale.as_ptr().cast::<nix::libc::c_char>(),
            )
        };
        if configured.is_null() {
            Err("Could not configure LC_NUMERIC=C for libmpv.".to_owned())
        } else {
            Ok(())
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
        terminate_destroy: MpvTerminateDestroy,
        error_string: MpvErrorString,
        wait_event: MpvWaitEvent,
        render_create: MpvRenderCreate,
        render_update: MpvRenderUpdate,
        render: MpvRender,
        render_free: MpvRenderFree,
    }

    impl MpvApi {
        fn load() -> Result<Self, String> {
            let library = load_library(&["libmpv.so.2", "libmpv.so"])?;
            // SAFETY: symbol signatures mirror mpv/client.h and mpv/render.h. The
            // library is retained in `MpvApi` for at least as long as every pointer.
            unsafe {
                Ok(Self {
                    create: load_symbol(&library, b"mpv_create\0")?,
                    initialize: load_symbol(&library, b"mpv_initialize\0")?,
                    set_option_string: load_symbol(&library, b"mpv_set_option_string\0")?,
                    command: load_symbol(&library, b"mpv_command\0")?,
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
            // SAFETY: these are the standard EGL/GLX resolver signatures. Each
            // backing library remains owned by this structure.
            let egl_get_proc_address = unsafe {
                egl.as_ref()
                    .and_then(|library| load_symbol(library, b"eglGetProcAddress\0").ok())
            };
            // SAFETY: same lifetime and signature reasoning as EGL above.
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
        // SAFETY: libmpv passes back the stable `GlResolver` pointer supplied when
        // creating the render context, and the context outlives the resolver.
        let resolver = unsafe { &*(context.cast::<GlResolver>()) };
        if let Some(resolve) = resolver.egl_get_proc_address {
            // SAFETY: `name` is the NUL-terminated GL symbol supplied by libmpv.
            let value = unsafe { resolve(name) };
            if !value.is_null() {
                return value.cast_mut();
            }
        }
        if let Some(resolve) = resolver.glx_get_proc_address {
            // SAFETY: same contract as the EGL resolver call above.
            return unsafe { resolve(name) }.cast_mut();
        }
        ptr::null_mut()
    }

    struct Player {
        api: MpvApi,
        handle: *mut MpvHandle,
        render_context: *mut MpvRenderContext,
        _resolver: Box<GlResolver>,
    }

    impl Player {
        fn create(path: &Path) -> Result<Self, String> {
            let api = MpvApi::load()?;
            let resolver = Box::new(GlResolver::load()?);
            // SAFETY: all calls use signatures loaded from the retained libmpv
            // library, validated pointers, and NUL-terminated strings.
            unsafe {
                let handle = (api.create)();
                if handle.is_null() {
                    return Err("mpv_create returned no client handle.".to_owned());
                }
                let mut player = Self {
                    api,
                    handle,
                    render_context: ptr::null_mut(),
                    _resolver: resolver,
                };
                for (name, value) in [
                    ("terminal", "no"),
                    ("audio-display", "no"),
                    ("hwdec", "auto-safe"),
                    ("vo", "libmpv"),
                ] {
                    player.set_option(name, value)?;
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
            let name = CString::new(name).map_err(|_| "Invalid option name.".to_owned())?;
            let value = CString::new(value).map_err(|_| "Invalid option value.".to_owned())?;
            // SAFETY: the client handle is live and strings remain valid for this call.
            let result =
                unsafe { (self.api.set_option_string)(self.handle, name.as_ptr(), value.as_ptr()) };
            (result >= 0)
                .then_some(())
                .ok_or_else(|| self.api.error(result, "mpv_set_option_string"))
        }

        unsafe fn create_render_context(&mut self) -> Result<(), String> {
            let mut init = MpvOpenGlInitParams {
                get_proc_address: resolve_gl,
                get_proc_address_context: self._resolver.as_mut() as *mut GlResolver as *mut c_void,
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
            // SAFETY: GLArea has made its context current before this method is called;
            // params and resolver remain valid throughout render-context creation.
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
                .ok_or_else(|| "The experiment requires a UTF-8 media path.".to_owned())?;
            let command = CString::new("loadfile").expect("static command");
            let path = CString::new(path).map_err(|_| "Media path contains NUL.".to_owned())?;
            let args = [command.as_ptr(), path.as_ptr(), ptr::null()];
            // SAFETY: argument pointers remain live and are terminated by NULL.
            let result = unsafe { (self.api.command)(self.handle, args.as_ptr()) };
            (result >= 0)
                .then_some(())
                .ok_or_else(|| self.api.error(result, "mpv_command(loadfile)"))
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
            // SAFETY: GTK invokes this while the GLArea context and default FBO are
            // current on the same thread used to create the libmpv render context.
            let result = unsafe {
                self.drain_events();
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

        unsafe fn drain_events(&self) {
            loop {
                // SAFETY: the client handle remains live, a zero timeout never blocks,
                // and mpv owns the returned event until the next API call.
                let event = unsafe { (self.api.wait_event)(self.handle, 0.0) };
                if event.is_null() {
                    return;
                }
                // MPV_EVENT_NONE is zero and marks the drained queue.
                if unsafe { (*event).event_id } == 0 {
                    return;
                }
            }
        }
    }

    impl Drop for Player {
        fn drop(&mut self) {
            // SAFETY: GTK unrealize makes the GL context current before dropping the
            // render context. libmpv requires render teardown before client teardown.
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

    pub fn run() -> Result<(), String> {
        configure_numeric_locale()?;
        let (path, smoke_seconds) = parse_args()?;
        gtk::init().map_err(|error| format!("Could not initialize GTK: {error}"))?;

        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("Nice Downloader libmpv GTK experiment");
        window.set_default_size(960, 540);
        let gl_area = gtk::GLArea::new();
        gl_area.set_required_version(3, 2);
        gl_area.set_has_depth_buffer(false);
        gl_area.set_has_stencil_buffer(false);
        gl_area.set_auto_render(false);
        window.add(&gl_area);

        let player = Rc::new(RefCell::new(None::<Player>));
        let failure = Rc::new(RefCell::new(None::<String>));
        let rendered_frames = Rc::new(Cell::new(0_u64));

        let realize_player = Rc::clone(&player);
        let realize_failure = Rc::clone(&failure);
        let realize_path = path.clone();
        gl_area.connect_realize(move |area| {
            area.make_current();
            if let Some(error) = area.error() {
                *realize_failure.borrow_mut() = Some(format!("GTK GLArea failed: {error}"));
                gtk::main_quit();
                return;
            }
            match Player::create(&realize_path) {
                Ok(created) => *realize_player.borrow_mut() = Some(created),
                Err(error) => {
                    *realize_failure.borrow_mut() = Some(error);
                    gtk::main_quit();
                }
            }
        });

        let render_player = Rc::clone(&player);
        let render_failure = Rc::clone(&failure);
        let render_count = Rc::clone(&rendered_frames);
        gl_area.connect_render(move |area, _| {
            let scale = area.scale_factor().max(1);
            let result = render_player.borrow_mut().as_mut().map(|player| {
                player.render(
                    area.allocated_width().saturating_mul(scale),
                    area.allocated_height().saturating_mul(scale),
                )
            });
            match result {
                Some(Ok(true)) => render_count.set(render_count.get().saturating_add(1)),
                Some(Ok(false)) => {}
                Some(Err(error)) => {
                    *render_failure.borrow_mut() = Some(error);
                    gtk::main_quit();
                }
                None => {}
            }
            glib::Propagation::Stop
        });

        let unrealize_player = Rc::clone(&player);
        gl_area.connect_unrealize(move |area| {
            area.make_current();
            unrealize_player.borrow_mut().take();
        });

        let render_area = gl_area.downgrade();
        glib::timeout_add_local(Duration::from_millis(16), move || {
            let Some(area) = render_area.upgrade() else {
                return glib::ControlFlow::Break;
            };
            area.queue_render();
            glib::ControlFlow::Continue
        });

        window.connect_destroy(|_| gtk::main_quit());
        window.show_all();
        if let Some(seconds) = smoke_seconds {
            let smoke_window = window.downgrade();
            glib::timeout_add_local_once(Duration::from_secs(seconds), move || {
                if let Some(window) = smoke_window.upgrade() {
                    window.close();
                }
            });
        }
        gtk::main();

        if let Some(error) = failure.borrow_mut().take() {
            return Err(error);
        }
        let frames = rendered_frames.get();
        if frames == 0 {
            return Err("libmpv initialized, but GTK rendered no frames.".to_owned());
        }
        let gdk_backend = gtk::gdk::Display::default()
            .map(|display| display.type_().name().to_owned())
            .unwrap_or_else(|| "unknown".to_owned());
        println!(
            "LIBMPV_GTK_RESULT frames={frames} session={} display={} gdk_backend={} media={}",
            env::var("XDG_SESSION_TYPE").unwrap_or_else(|_| "unknown".to_owned()),
            env::var("WAYLAND_DISPLAY")
                .or_else(|_| env::var("DISPLAY"))
                .unwrap_or_else(|_| "unknown".to_owned()),
            gdk_backend,
            path.display(),
        );
        Ok(())
    }

    fn parse_args() -> Result<(PathBuf, Option<u64>), String> {
        let mut args = env::args_os().skip(1);
        let path = args.next().map(PathBuf::from).ok_or_else(|| {
            "Usage: cargo run --example libmpv_gtk -- /absolute/video [--smoke-seconds N]"
                .to_owned()
        })?;
        if !path.is_absolute() || !path.is_file() {
            return Err("Choose an existing absolute regular media file.".to_owned());
        }
        let remaining = args.collect::<Vec<_>>();
        let smoke_seconds = match remaining.as_slice() {
            [] => None,
            [flag, seconds] if flag == "--smoke-seconds" => Some(
                seconds
                    .to_string_lossy()
                    .parse::<u64>()
                    .ok()
                    .filter(|seconds| (1..=60).contains(seconds))
                    .ok_or_else(|| "Smoke duration must be between 1 and 60 seconds.".to_owned())?,
            ),
            _ => return Err("Expected only --smoke-seconds N after the media path.".to_owned()),
        };
        Ok((path, smoke_seconds))
    }

    fn load_library(names: &[&str]) -> Result<Library, String> {
        names
            .iter()
            .find_map(|name| {
                // SAFETY: the experiment loads only fixed system sonames and retains
                // the returned handle for every loaded symbol's complete lifetime.
                unsafe { Library::new(name).ok() }
            })
            .ok_or_else(|| format!("Could not load any of: {}", names.join(", ")))
    }

    fn load_optional_library(names: &[&str]) -> Option<Library> {
        names.iter().find_map(|name| {
            // SAFETY: fixed OpenGL system sonames; handle lifetime is retained.
            unsafe { Library::new(name).ok() }
        })
    }

    unsafe fn load_symbol<T: Copy>(library: &Library, name: &[u8]) -> Result<T, String> {
        // SAFETY: caller supplies the exact C signature for this symbol.
        unsafe { library.get::<T>(name) }
            .map(|symbol| *symbol)
            .map_err(|error| {
                format!(
                    "Could not load {}: {error}",
                    String::from_utf8_lossy(name).trim_end_matches('\0')
                )
            })
    }
}

#[cfg(target_os = "linux")]
fn main() {
    if let Err(error) = linux::run() {
        eprintln!("LIBMPV_GTK_ERROR {error}");
        std::process::exit(1);
    }
}
