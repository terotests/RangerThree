//! A headless GL context for tests and tools on Linux: EGL without a window
//! (EGL_MESA_platform_surfaceless, Mesa's llvmpipe in a container) drawing
//! into an offscreen framebuffer. libEGL is opened with dlopen, so the crate
//! links nothing. A real host (SDL3, Android, iOS) brings its own context
//! and only needs `gl::Gl::load` with its GetProcAddress.
#![allow(non_snake_case)]

use crate::gl::{self, Gl};
use std::ffi::{c_char, c_void, CString};

type P = *mut c_void;

#[cfg(target_os = "linux")]
#[link(name = "dl")]
extern "C" {
    fn dlopen(name: *const c_char, flags: i32) -> P;
    fn dlsym(lib: P, name: *const c_char) -> P;
}
#[cfg(not(target_os = "linux"))]
unsafe fn dlopen(_: *const c_char, _: i32) -> P {
    std::ptr::null_mut()
}
#[cfg(not(target_os = "linux"))]
unsafe fn dlsym(_: P, _: *const c_char) -> P {
    std::ptr::null_mut()
}

const PLATFORM_SURFACELESS_MESA: u32 = 0x31DD;
const NONE: i32 = 0x3038;
const RENDERABLE_TYPE: i32 = 0x3040;
const SURFACE_TYPE: i32 = 0x3033;
const PBUFFER_BIT: i32 = 0x0001;
const OPENGL_ES3_BIT: i32 = 0x0040;
const OPENGL_BIT: i32 = 0x0008;
const OPENGL_ES_API: u32 = 0x30A0;
const OPENGL_API: u32 = 0x30A2;
const CONTEXT_MAJOR_VERSION: i32 = 0x3098;
const CONTEXT_MINOR_VERSION: i32 = 0x30FB;
const CONTEXT_OPENGL_PROFILE_MASK: i32 = 0x30FD;
const CONTEXT_OPENGL_CORE_PROFILE_BIT: i32 = 0x0001;

struct Egl {
    GetPlatformDisplay: unsafe extern "C" fn(u32, P, *const isize) -> P,
    Initialize: unsafe extern "C" fn(P, *mut i32, *mut i32) -> u32,
    BindAPI: unsafe extern "C" fn(u32) -> u32,
    ChooseConfig: unsafe extern "C" fn(P, *const i32, *mut P, i32, *mut i32) -> u32,
    CreateContext: unsafe extern "C" fn(P, P, P, *const i32) -> P,
    MakeCurrent: unsafe extern "C" fn(P, P, P, P) -> u32,
    DestroyContext: unsafe extern "C" fn(P, P) -> u32,
    GetProcAddress: unsafe extern "C" fn(*const c_char) -> *const c_void,
    GetError: unsafe extern "C" fn() -> i32,
}

/// A current headless context with an RGBA8 + depth/stencil framebuffer of `w`×`h`.
pub struct Headless {
    egl: Egl,
    display: P,
    context: P,
    pub fb: u32,
    pub width: i32,
    pub height: i32,
    pub desktop: bool,
}

impl Headless {
    /// `desktop` false: OpenGL ES 3.0; true: OpenGL 3.3 core.
    pub fn new(w: i32, h: i32, desktop: bool) -> Result<Headless, String> {
        // SAFETY: symbols are looked up by their EGL names and cast to their EGL signatures.
        unsafe {
            let lib = dlopen(c"libEGL.so.1".as_ptr(), 2);
            if lib.is_null() {
                return Err("libEGL.so.1 not found (apt install libegl1 libegl-mesa0)".into());
            }
            let sym = |n: &str| -> Result<P, String> {
                let c = CString::new(n).unwrap();
                let p = dlsym(lib, c.as_ptr());
                if p.is_null() { Err(format!("{} missing in libEGL", n)) } else { Ok(p) }
            };
            macro_rules! load {
                ($n:literal) => {
                    std::mem::transmute(sym($n)?)
                };
            }
            let egl = Egl {
                GetPlatformDisplay: load!("eglGetPlatformDisplay"),
                Initialize: load!("eglInitialize"),
                BindAPI: load!("eglBindAPI"),
                ChooseConfig: load!("eglChooseConfig"),
                CreateContext: load!("eglCreateContext"),
                MakeCurrent: load!("eglMakeCurrent"),
                DestroyContext: load!("eglDestroyContext"),
                GetProcAddress: load!("eglGetProcAddress"),
                GetError: load!("eglGetError"),
            };
            let display = (egl.GetPlatformDisplay)(PLATFORM_SURFACELESS_MESA, std::ptr::null_mut(), std::ptr::null());
            let fail = |what: &str| format!("{} failed (EGL error 0x{:x})", what, (egl.GetError)());
            if display.is_null() {
                return Err(fail("eglGetPlatformDisplay(surfaceless)"));
            }
            let (mut major, mut minor) = (0, 0);
            if (egl.Initialize)(display, &mut major, &mut minor) == 0 {
                return Err(fail("eglInitialize"));
            }
            if (egl.BindAPI)(if desktop { OPENGL_API } else { OPENGL_ES_API }) == 0 {
                return Err(fail("eglBindAPI"));
            }
            let attrs = [SURFACE_TYPE, PBUFFER_BIT, RENDERABLE_TYPE, if desktop { OPENGL_BIT } else { OPENGL_ES3_BIT }, NONE];
            let mut config: P = std::ptr::null_mut();
            let mut count = 0;
            if (egl.ChooseConfig)(display, attrs.as_ptr(), &mut config, 1, &mut count) == 0 || count == 0 {
                return Err(fail("eglChooseConfig"));
            }
            let ctx_attrs: Vec<i32> = if desktop {
                vec![CONTEXT_MAJOR_VERSION, 3, CONTEXT_MINOR_VERSION, 3, CONTEXT_OPENGL_PROFILE_MASK, CONTEXT_OPENGL_CORE_PROFILE_BIT, NONE]
            } else {
                vec![CONTEXT_MAJOR_VERSION, 3, CONTEXT_MINOR_VERSION, 0, NONE]
            };
            let context = (egl.CreateContext)(display, config, std::ptr::null_mut(), ctx_attrs.as_ptr());
            if context.is_null() {
                return Err(fail("eglCreateContext"));
            }
            if (egl.MakeCurrent)(display, std::ptr::null_mut(), std::ptr::null_mut(), context) == 0 {
                return Err(fail("eglMakeCurrent"));
            }
            let mut me = Headless { egl, display, context, fb: 0, width: w, height: h, desktop };
            let g = me.load_gl()?;
            let (mut fb, mut rb) = (0, [0u32; 2]);
            (g.GenFramebuffers)(1, &mut fb);
            (g.BindFramebuffer)(gl::FRAMEBUFFER, fb);
            (g.GenRenderbuffers)(2, rb.as_mut_ptr());
            (g.BindRenderbuffer)(gl::RENDERBUFFER, rb[0]);
            (g.RenderbufferStorage)(gl::RENDERBUFFER, gl::RGBA8, w, h);
            (g.FramebufferRenderbuffer)(gl::FRAMEBUFFER, gl::COLOR_ATTACHMENT0, gl::RENDERBUFFER, rb[0]);
            (g.BindRenderbuffer)(gl::RENDERBUFFER, rb[1]);
            (g.RenderbufferStorage)(gl::RENDERBUFFER, gl::DEPTH24_STENCIL8, w, h);
            (g.FramebufferRenderbuffer)(gl::FRAMEBUFFER, gl::DEPTH_STENCIL_ATTACHMENT, gl::RENDERBUFFER, rb[1]);
            if (g.CheckFramebufferStatus)(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                return Err("offscreen framebuffer is incomplete".into());
            }
            me.fb = fb;
            Ok(me)
        }
    }

    /// GL's entry points for this context.
    pub fn load_gl(&self) -> Result<Gl, String> {
        let gpa = self.egl.GetProcAddress;
        Gl::load(&|name| {
            let c = CString::new(name).unwrap();
            // SAFETY: eglGetProcAddress takes a NUL-terminated name.
            unsafe { gpa(c.as_ptr()) }
        })
    }
}

impl Drop for Headless {
    fn drop(&mut self) {
        // SAFETY: the display and context were made by this value.
        unsafe {
            (self.egl.MakeCurrent)(self.display, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
            (self.egl.DestroyContext)(self.display, self.context);
        }
    }
}
