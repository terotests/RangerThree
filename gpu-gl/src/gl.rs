//! The OpenGL ES 3 / OpenGL 3.3 entry points the player calls, loaded from
//! the host's GetProcAddress (SDL_GL_GetProcAddress, eglGetProcAddress,
//! wglGetProcAddress...). Only what the command stream needs.
#![allow(non_snake_case, clippy::too_many_arguments)]

use std::ffi::c_void;

pub type GLenum = u32;
pub type GLuint = u32;
pub type GLint = i32;
pub type GLsizei = i32;
pub type GLfloat = f32;
pub type GLboolean = u8;
pub type GLbitfield = u32;
pub type GLsizeiptr = isize;
pub type GLintptr = isize;

pub const ARRAY_BUFFER: GLenum = 0x8892;
pub const ELEMENT_ARRAY_BUFFER: GLenum = 0x8893;
pub const STATIC_DRAW: GLenum = 0x88E4;
pub const DYNAMIC_DRAW: GLenum = 0x88E8;
pub const VERTEX_SHADER: GLenum = 0x8B31;
pub const FRAGMENT_SHADER: GLenum = 0x8B30;
pub const COMPILE_STATUS: GLenum = 0x8B81;
pub const LINK_STATUS: GLenum = 0x8B82;
pub const INFO_LOG_LENGTH: GLenum = 0x8B84;
pub const TEXTURE_2D: GLenum = 0x0DE1;
pub const TEXTURE_CUBE_MAP: GLenum = 0x8513;
pub const TEXTURE_CUBE_MAP_POSITIVE_X: GLenum = 0x8515;
pub const TEXTURE_MIN_FILTER: GLenum = 0x2801;
pub const TEXTURE_MAG_FILTER: GLenum = 0x2800;
pub const TEXTURE_WRAP_S: GLenum = 0x2802;
pub const TEXTURE_WRAP_T: GLenum = 0x2803;
pub const TEXTURE_COMPARE_MODE: GLenum = 0x884C;
pub const TEXTURE_COMPARE_FUNC: GLenum = 0x884D;
pub const COMPARE_REF_TO_TEXTURE: GLint = 0x884E;
pub const LEQUAL: GLint = 0x0203;
pub const UNPACK_ALIGNMENT: GLenum = 0x0CF5;
pub const PACK_ALIGNMENT: GLenum = 0x0D05;
pub const TEXTURE0: GLenum = 0x84C0;
pub const FRAMEBUFFER: GLenum = 0x8D40;
pub const RENDERBUFFER: GLenum = 0x8D41;
pub const COLOR_ATTACHMENT0: GLenum = 0x8CE0;
pub const DEPTH_ATTACHMENT: GLenum = 0x8D00;
pub const DEPTH_STENCIL_ATTACHMENT: GLenum = 0x821A;
pub const FRAMEBUFFER_COMPLETE: GLenum = 0x8CD5;
pub const RGBA8: GLenum = 0x8058;
pub const DEPTH24_STENCIL8: GLenum = 0x88F0;
pub const COLOR_BUFFER_BIT: GLbitfield = 0x4000;
pub const DEPTH_BUFFER_BIT: GLbitfield = 0x0100;
pub const BLEND: GLenum = 0x0BE2;
pub const DEPTH_TEST: GLenum = 0x0B71;
pub const CULL_FACE: GLenum = 0x0B44;
pub const POLYGON_OFFSET_FILL: GLenum = 0x8037;
pub const PROGRAM_POINT_SIZE: GLenum = 0x8642;
pub const FRONT: GLenum = 0x0404;
pub const BACK: GLenum = 0x0405;
pub const CW: GLenum = 0x0900;
pub const CCW: GLenum = 0x0901;
pub const FLOAT: GLenum = 0x1406;
pub const UNSIGNED_INT: GLenum = 0x1405;
pub const UNSIGNED_BYTE: GLenum = 0x1401;
pub const RGBA: GLenum = 0x1908;
pub const VERSION: GLenum = 0x1F02;
pub const RENDERER: GLenum = 0x1F01;

macro_rules! gl_api {
    ($($name:ident : fn($($arg:ty),*) $(-> $ret:ty)?;)*) => {
        /// GL's entry points, as loaded.
        pub struct Gl {
            $(pub $name: unsafe extern "C" fn($($arg),*) $(-> $ret)?,)*
        }
        impl Gl {
            /// Loads every entry point with `get` (the host's GetProcAddress, given the C name).
            pub fn load(get: &dyn Fn(&str) -> *const c_void) -> Result<Gl, String> {
                Ok(Gl {
                    $($name: {
                        let p = get(concat!("gl", stringify!($name)));
                        if p.is_null() {
                            return Err(format!("GL entry point gl{} is missing", stringify!($name)));
                        }
                        // SAFETY: the host returns the address of the GL function of this name and signature.
                        unsafe { std::mem::transmute::<*const c_void, unsafe extern "C" fn($($arg),*) $(-> $ret)?>(p) }
                    },)*
                })
            }
        }
    };
}

gl_api! {
    GenBuffers: fn(GLsizei, *mut GLuint);
    DeleteBuffers: fn(GLsizei, *const GLuint);
    BindBuffer: fn(GLenum, GLuint);
    BufferData: fn(GLenum, GLsizeiptr, *const c_void, GLenum);
    BufferSubData: fn(GLenum, GLintptr, GLsizeiptr, *const c_void);
    CreateShader: fn(GLenum) -> GLuint;
    DeleteShader: fn(GLuint);
    ShaderSource: fn(GLuint, GLsizei, *const *const u8, *const GLint);
    CompileShader: fn(GLuint);
    GetShaderiv: fn(GLuint, GLenum, *mut GLint);
    GetShaderInfoLog: fn(GLuint, GLsizei, *mut GLsizei, *mut u8);
    CreateProgram: fn() -> GLuint;
    DeleteProgram: fn(GLuint);
    AttachShader: fn(GLuint, GLuint);
    LinkProgram: fn(GLuint);
    GetProgramiv: fn(GLuint, GLenum, *mut GLint);
    GetProgramInfoLog: fn(GLuint, GLsizei, *mut GLsizei, *mut u8);
    GetUniformLocation: fn(GLuint, *const u8) -> GLint;
    UseProgram: fn(GLuint);
    GenTextures: fn(GLsizei, *mut GLuint);
    DeleteTextures: fn(GLsizei, *const GLuint);
    BindTexture: fn(GLenum, GLuint);
    TexImage2D: fn(GLenum, GLint, GLint, GLsizei, GLsizei, GLint, GLenum, GLenum, *const c_void);
    TexParameteri: fn(GLenum, GLenum, GLint);
    PixelStorei: fn(GLenum, GLint);
    GenerateMipmap: fn(GLenum);
    ActiveTexture: fn(GLenum);
    GenFramebuffers: fn(GLsizei, *mut GLuint);
    DeleteFramebuffers: fn(GLsizei, *const GLuint);
    BindFramebuffer: fn(GLenum, GLuint);
    FramebufferRenderbuffer: fn(GLenum, GLenum, GLenum, GLuint);
    CheckFramebufferStatus: fn(GLenum) -> GLenum;
    GenRenderbuffers: fn(GLsizei, *mut GLuint);
    BindRenderbuffer: fn(GLenum, GLuint);
    RenderbufferStorage: fn(GLenum, GLenum, GLsizei, GLsizei);
    Viewport: fn(GLint, GLint, GLsizei, GLsizei);
    ClearColor: fn(GLfloat, GLfloat, GLfloat, GLfloat);
    ClearDepthf: fn(GLfloat);
    Clear: fn(GLbitfield);
    DepthMask: fn(GLboolean);
    ColorMask: fn(GLboolean, GLboolean, GLboolean, GLboolean);
    Enable: fn(GLenum);
    Disable: fn(GLenum);
    BlendEquationSeparate: fn(GLenum, GLenum);
    BlendFuncSeparate: fn(GLenum, GLenum, GLenum, GLenum);
    BlendColor: fn(GLfloat, GLfloat, GLfloat, GLfloat);
    DepthFunc: fn(GLenum);
    CullFace: fn(GLenum);
    FrontFace: fn(GLenum);
    PolygonOffset: fn(GLfloat, GLfloat);
    Uniform1i: fn(GLint, GLint);
    Uniform1fv: fn(GLint, GLsizei, *const GLfloat);
    Uniform2fv: fn(GLint, GLsizei, *const GLfloat);
    Uniform3fv: fn(GLint, GLsizei, *const GLfloat);
    Uniform4fv: fn(GLint, GLsizei, *const GLfloat);
    UniformMatrix3fv: fn(GLint, GLsizei, GLboolean, *const GLfloat);
    UniformMatrix4fv: fn(GLint, GLsizei, GLboolean, *const GLfloat);
    GenVertexArrays: fn(GLsizei, *mut GLuint);
    BindVertexArray: fn(GLuint);
    EnableVertexAttribArray: fn(GLuint);
    DisableVertexAttribArray: fn(GLuint);
    VertexAttrib4f: fn(GLuint, GLfloat, GLfloat, GLfloat, GLfloat);
    VertexAttribPointer: fn(GLuint, GLint, GLenum, GLboolean, GLsizei, *const c_void);
    VertexAttribDivisor: fn(GLuint, GLuint);
    DrawArrays: fn(GLenum, GLint, GLsizei);
    DrawArraysInstanced: fn(GLenum, GLint, GLsizei, GLsizei);
    DrawElements: fn(GLenum, GLsizei, GLenum, *const c_void);
    DrawElementsInstanced: fn(GLenum, GLsizei, GLenum, *const c_void, GLsizei);
    ReadPixels: fn(GLint, GLint, GLsizei, GLsizei, GLenum, GLenum, *mut c_void);
    GetString: fn(GLenum) -> *const u8;
    GetError: fn() -> GLenum;
    Finish: fn();
}
