package rangerthree.gpu

import com.sun.jna.Library
import com.sun.jna.Native
import com.sun.jna.Pointer
import java.nio.Buffer
import java.nio.ByteBuffer

/** libGLESv2 through JNA, for testing the player on a desktop JVM (see Headless). */
@Suppress("FunctionName")
interface GLES : Library {
    fun glGetString(name: Int): String?
    fun glGenVertexArrays(n: Int, out: IntArray)
    fun glBindVertexArray(vao: Int)
    fun glGenBuffers(n: Int, out: IntArray)
    fun glDeleteBuffers(n: Int, names: IntArray)
    fun glBindBuffer(target: Int, b: Int)
    fun glBufferData(target: Int, size: Long, data: Buffer?, usage: Int)
    fun glBufferSubData(target: Int, offset: Long, size: Long, data: Buffer?)
    fun glCreateShader(type: Int): Int
    fun glShaderSource(s: Int, count: Int, strings: Array<String>, lengths: IntArray?)
    fun glCompileShader(s: Int)
    fun glGetShaderiv(s: Int, pname: Int, out: IntArray)
    fun glGetShaderInfoLog(s: Int, size: Int, length: IntArray, log: ByteArray)
    fun glDeleteShader(s: Int)
    fun glCreateProgram(): Int
    fun glAttachShader(p: Int, s: Int)
    fun glLinkProgram(p: Int)
    fun glGetProgramiv(p: Int, pname: Int, out: IntArray)
    fun glGetProgramInfoLog(p: Int, size: Int, length: IntArray, log: ByteArray)
    fun glDeleteProgram(p: Int)
    fun glUseProgram(p: Int)
    fun glGetUniformLocation(p: Int, name: String): Int
    fun glGetUniformBlockIndex(p: Int, name: String): Int
    fun glUniformBlockBinding(p: Int, index: Int, binding: Int)
    fun glBindBufferRange(target: Int, index: Int, b: Int, offset: Long, size: Long)
    fun glUniform1i(loc: Int, v: Int)
    fun glUniform1fv(loc: Int, count: Int, v: FloatArray)
    fun glUniform2fv(loc: Int, count: Int, v: FloatArray)
    fun glUniform3fv(loc: Int, count: Int, v: FloatArray)
    fun glUniform4fv(loc: Int, count: Int, v: FloatArray)
    fun glUniformMatrix3fv(loc: Int, count: Int, transpose: Byte, v: FloatArray)
    fun glUniformMatrix4fv(loc: Int, count: Int, transpose: Byte, v: FloatArray)
    fun glGenTextures(n: Int, out: IntArray)
    fun glDeleteTextures(n: Int, names: IntArray)
    fun glActiveTexture(unit: Int)
    fun glBindTexture(target: Int, t: Int)
    fun glPixelStorei(name: Int, v: Int)
    fun glTexImage2D(target: Int, level: Int, internal: Int, w: Int, h: Int, border: Int, format: Int, type: Int, data: Buffer?)
    fun glTexParameteri(target: Int, name: Int, v: Int)
    fun glGenerateMipmap(target: Int)
    fun glGenFramebuffers(n: Int, out: IntArray)
    fun glDeleteFramebuffers(n: Int, names: IntArray)
    fun glBindFramebuffer(target: Int, fb: Int)
    fun glFramebufferTexture2D(target: Int, attachment: Int, texTarget: Int, t: Int, level: Int)
    fun glGenRenderbuffers(n: Int, out: IntArray)
    fun glDeleteRenderbuffers(n: Int, names: IntArray)
    fun glBindRenderbuffer(target: Int, rb: Int)
    fun glRenderbufferStorage(target: Int, internal: Int, w: Int, h: Int)
    fun glRenderbufferStorageMultisample(target: Int, samples: Int, internal: Int, w: Int, h: Int)
    fun glFramebufferRenderbuffer(target: Int, attachment: Int, rbTarget: Int, rb: Int)
    fun glDrawBuffers(n: Int, bufs: IntArray)
    fun glReadBuffer(src: Int)
    fun glCheckFramebufferStatus(target: Int): Int
    fun glBlitFramebuffer(sx0: Int, sy0: Int, sx1: Int, sy1: Int, dx0: Int, dy0: Int, dx1: Int, dy1: Int, mask: Int, filter: Int)
    fun glViewport(x: Int, y: Int, w: Int, h: Int)
    fun glClearColor(r: Float, g: Float, b: Float, a: Float)
    fun glClearDepthf(d: Float)
    fun glClear(mask: Int)
    fun glEnable(cap: Int)
    fun glDisable(cap: Int)
    fun glDepthMask(on: Byte)
    fun glColorMask(r: Byte, g: Byte, b: Byte, a: Byte)
    fun glDepthFunc(f: Int)
    fun glBlendEquationSeparate(rgb: Int, alpha: Int)
    fun glBlendFuncSeparate(sr: Int, dr: Int, sa: Int, da: Int)
    fun glBlendColor(r: Float, g: Float, b: Float, a: Float)
    fun glCullFace(mode: Int)
    fun glFrontFace(mode: Int)
    fun glPolygonOffset(factor: Float, units: Float)
    fun glEnableVertexAttribArray(i: Int)
    fun glDisableVertexAttribArray(i: Int)
    fun glVertexAttrib4f(i: Int, x: Float, y: Float, z: Float, w: Float)
    fun glVertexAttribPointer(i: Int, size: Int, type: Int, normalized: Byte, stride: Int, offset: Pointer?)
    fun glVertexAttribDivisor(i: Int, divisor: Int)
    fun glDrawArrays(mode: Int, first: Int, count: Int)
    fun glDrawArraysInstanced(mode: Int, first: Int, count: Int, instances: Int)
    fun glDrawElements(mode: Int, count: Int, type: Int, offset: Pointer?)
    fun glDrawElementsInstanced(mode: Int, count: Int, type: Int, offset: Pointer?, instances: Int)
    fun glReadPixels(x: Int, y: Int, w: Int, h: Int, format: Int, type: Int, out: Buffer)
}

class JnaGl : Gl {
    private val g: GLES = Native.load("GLESv2", GLES::class.java)
    private val one = IntArray(1)
    private fun b(v: Boolean): Byte = if (v) 1 else 0
    private fun ptr(offset: Int): Pointer? = if (offset == 0) null else Pointer.createConstant(offset.toLong())
    private fun slice(v: FloatArray, offset: Int, n: Int): FloatArray = if (offset == 0 && n == v.size) v else v.copyOfRange(offset, offset + n)
    private fun log(get: (Int, IntArray, ByteArray) -> Unit): String {
        val buf = ByteArray(8192)
        val len = IntArray(1)
        get(buf.size, len, buf)
        return String(buf, 0, len[0])
    }

    override fun getString(name: Int): String = g.glGetString(name) ?: ""
    override fun genVertexArray(): Int { g.glGenVertexArrays(1, one); return one[0] }
    override fun bindVertexArray(vao: Int) = g.glBindVertexArray(vao)
    override fun genBuffer(): Int { g.glGenBuffers(1, one); return one[0] }
    override fun deleteBuffer(b: Int) = g.glDeleteBuffers(1, intArrayOf(b))
    override fun bindBuffer(target: Int, b: Int) = g.glBindBuffer(target, b)
    override fun bufferData(target: Int, bytes: Int, data: Buffer, usage: Int) = g.glBufferData(target, bytes.toLong(), data, usage)
    override fun bufferSubData(target: Int, offset: Int, bytes: Int, data: Buffer) = g.glBufferSubData(target, offset.toLong(), bytes.toLong(), data)
    override fun createShader(type: Int): Int = g.glCreateShader(type)
    override fun shaderSource(s: Int, source: String) = g.glShaderSource(s, 1, arrayOf(source), null)
    override fun compileShader(s: Int) = g.glCompileShader(s)
    override fun shaderCompiled(s: Int): Boolean { g.glGetShaderiv(s, 0x8B81, one); return one[0] != 0 }
    override fun shaderInfoLog(s: Int): String = log { n, l, buf -> g.glGetShaderInfoLog(s, n, l, buf) }
    override fun deleteShader(s: Int) = g.glDeleteShader(s)
    override fun createProgram(): Int = g.glCreateProgram()
    override fun attachShader(p: Int, s: Int) = g.glAttachShader(p, s)
    override fun linkProgram(p: Int) = g.glLinkProgram(p)
    override fun programLinked(p: Int): Boolean { g.glGetProgramiv(p, 0x8B82, one); return one[0] != 0 }
    override fun programInfoLog(p: Int): String = log { n, l, buf -> g.glGetProgramInfoLog(p, n, l, buf) }
    override fun deleteProgram(p: Int) = g.glDeleteProgram(p)
    override fun useProgram(p: Int) = g.glUseProgram(p)
    override fun getUniformLocation(p: Int, name: String): Int = g.glGetUniformLocation(p, name)
    override fun getUniformBlockIndex(p: Int, name: String): Int = g.glGetUniformBlockIndex(p, name)
    override fun uniformBlockBinding(p: Int, index: Int, binding: Int) = g.glUniformBlockBinding(p, index, binding)
    override fun bindBufferRange(target: Int, index: Int, b: Int, offset: Int, bytes: Int) = g.glBindBufferRange(target, index, b, offset.toLong(), bytes.toLong())
    override fun uniform1i(loc: Int, v: Int) = g.glUniform1i(loc, v)
    override fun uniform1fv(loc: Int, count: Int, v: FloatArray, offset: Int) = g.glUniform1fv(loc, count, slice(v, offset, count))
    override fun uniform2fv(loc: Int, count: Int, v: FloatArray, offset: Int) = g.glUniform2fv(loc, count, slice(v, offset, count * 2))
    override fun uniform3fv(loc: Int, count: Int, v: FloatArray, offset: Int) = g.glUniform3fv(loc, count, slice(v, offset, count * 3))
    override fun uniform4fv(loc: Int, count: Int, v: FloatArray, offset: Int) = g.glUniform4fv(loc, count, slice(v, offset, count * 4))
    override fun uniformMatrix3fv(loc: Int, count: Int, v: FloatArray, offset: Int) = g.glUniformMatrix3fv(loc, count, 0, slice(v, offset, count * 9))
    override fun uniformMatrix4fv(loc: Int, count: Int, v: FloatArray, offset: Int) = g.glUniformMatrix4fv(loc, count, 0, slice(v, offset, count * 16))
    override fun genTexture(): Int { g.glGenTextures(1, one); return one[0] }
    override fun deleteTexture(t: Int) = g.glDeleteTextures(1, intArrayOf(t))
    override fun activeTexture(unit: Int) = g.glActiveTexture(unit)
    override fun bindTexture(target: Int, t: Int) = g.glBindTexture(target, t)
    override fun pixelStorei(name: Int, v: Int) = g.glPixelStorei(name, v)
    override fun texImage2D(target: Int, level: Int, internal: Int, w: Int, h: Int, format: Int, type: Int, data: Buffer?) =
        g.glTexImage2D(target, level, internal, w, h, 0, format, type, data)
    override fun texParameteri(target: Int, name: Int, v: Int) = g.glTexParameteri(target, name, v)
    override fun generateMipmap(target: Int) = g.glGenerateMipmap(target)
    override fun genFramebuffer(): Int { g.glGenFramebuffers(1, one); return one[0] }
    override fun deleteFramebuffer(fb: Int) = g.glDeleteFramebuffers(1, intArrayOf(fb))
    override fun bindFramebuffer(target: Int, fb: Int) = g.glBindFramebuffer(target, fb)
    override fun framebufferTexture2D(target: Int, attachment: Int, texTarget: Int, t: Int, level: Int) = g.glFramebufferTexture2D(target, attachment, texTarget, t, level)
    override fun genRenderbuffer(): Int { g.glGenRenderbuffers(1, one); return one[0] }
    override fun deleteRenderbuffer(rb: Int) = g.glDeleteRenderbuffers(1, intArrayOf(rb))
    override fun bindRenderbuffer(target: Int, rb: Int) = g.glBindRenderbuffer(target, rb)
    override fun renderbufferStorage(target: Int, internal: Int, w: Int, h: Int) = g.glRenderbufferStorage(target, internal, w, h)
    override fun renderbufferStorageMultisample(target: Int, samples: Int, internal: Int, w: Int, h: Int) = g.glRenderbufferStorageMultisample(target, samples, internal, w, h)
    override fun framebufferRenderbuffer(target: Int, attachment: Int, rbTarget: Int, rb: Int) = g.glFramebufferRenderbuffer(target, attachment, rbTarget, rb)
    override fun drawBuffers(bufs: IntArray) = g.glDrawBuffers(bufs.size, bufs)
    override fun readBuffer(src: Int) = g.glReadBuffer(src)
    override fun checkFramebufferStatus(target: Int): Int = g.glCheckFramebufferStatus(target)
    override fun blitFramebuffer(sx0: Int, sy0: Int, sx1: Int, sy1: Int, dx0: Int, dy0: Int, dx1: Int, dy1: Int, mask: Int, filter: Int) =
        g.glBlitFramebuffer(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter)
    override fun viewport(x: Int, y: Int, w: Int, h: Int) = g.glViewport(x, y, w, h)
    override fun clearColor(r: Float, g2: Float, b: Float, a: Float) = g.glClearColor(r, g2, b, a)
    override fun clearDepthf(d: Float) = g.glClearDepthf(d)
    override fun clear(mask: Int) = g.glClear(mask)
    override fun enable(cap: Int) = g.glEnable(cap)
    override fun disable(cap: Int) = g.glDisable(cap)
    override fun depthMask(on: Boolean) = g.glDepthMask(b(on))
    override fun colorMask(r: Boolean, g2: Boolean, b2: Boolean, a: Boolean) = g.glColorMask(b(r), b(g2), b(b2), b(a))
    override fun depthFunc(f: Int) = g.glDepthFunc(f)
    override fun blendEquationSeparate(rgb: Int, alpha: Int) = g.glBlendEquationSeparate(rgb, alpha)
    override fun blendFuncSeparate(srcRgb: Int, dstRgb: Int, srcAlpha: Int, dstAlpha: Int) = g.glBlendFuncSeparate(srcRgb, dstRgb, srcAlpha, dstAlpha)
    override fun blendColor(r: Float, g2: Float, b: Float, a: Float) = g.glBlendColor(r, g2, b, a)
    override fun cullFace(mode: Int) = g.glCullFace(mode)
    override fun frontFace(mode: Int) = g.glFrontFace(mode)
    override fun polygonOffset(factor: Float, units: Float) = g.glPolygonOffset(factor, units)
    override fun enableVertexAttribArray(i: Int) = g.glEnableVertexAttribArray(i)
    override fun disableVertexAttribArray(i: Int) = g.glDisableVertexAttribArray(i)
    override fun vertexAttrib4f(i: Int, x: Float, y: Float, z: Float, w: Float) = g.glVertexAttrib4f(i, x, y, z, w)
    override fun vertexAttribPointer(i: Int, size: Int, type: Int, normalized: Boolean, stride: Int, offset: Int) =
        g.glVertexAttribPointer(i, size, type, b(normalized), stride, ptr(offset))
    override fun vertexAttribDivisor(i: Int, divisor: Int) = g.glVertexAttribDivisor(i, divisor)
    override fun drawArrays(mode: Int, first: Int, count: Int) = g.glDrawArrays(mode, first, count)
    override fun drawArraysInstanced(mode: Int, first: Int, count: Int, instances: Int) = g.glDrawArraysInstanced(mode, first, count, instances)
    override fun drawElements(mode: Int, count: Int, type: Int, offset: Int) = g.glDrawElements(mode, count, type, ptr(offset))
    override fun drawElementsInstanced(mode: Int, count: Int, type: Int, offset: Int, instances: Int) = g.glDrawElementsInstanced(mode, count, type, ptr(offset), instances)
    override fun readPixels(x: Int, y: Int, w: Int, h: Int, format: Int, type: Int, out: ByteBuffer) = g.glReadPixels(x, y, w, h, format, type, out)
}

/** libEGL through JNA: a surfaceless (Mesa) OpenGL ES 3.0 context with an RGBA8 + depth/stencil framebuffer. */
@Suppress("FunctionName")
interface EGL : Library {
    fun eglGetPlatformDisplay(platform: Int, native: Pointer?, attribs: Pointer?): Pointer?
    fun eglInitialize(dpy: Pointer, major: IntArray, minor: IntArray): Int
    fun eglBindAPI(api: Int): Int
    fun eglChooseConfig(dpy: Pointer, attribs: IntArray, configs: Array<Pointer?>, size: Int, count: IntArray): Int
    fun eglCreateContext(dpy: Pointer, config: Pointer?, share: Pointer?, attribs: IntArray): Pointer?
    fun eglMakeCurrent(dpy: Pointer, draw: Pointer?, read: Pointer?, ctx: Pointer?): Int
    fun eglGetError(): Int
}

class Headless(val width: Int, val height: Int) {
    // held for the context's life: JNA unloads a library whose proxy is collected, and libEGL with it
    private val egl: EGL = Native.load("EGL", EGL::class.java)
    val gl: JnaGl
    val fb: Int

    init {
        fun fail(what: String): Nothing = throw IllegalStateException("$what failed (EGL error 0x${Integer.toHexString(egl.eglGetError())})")
        val dpy = egl.eglGetPlatformDisplay(0x31DD, null, null) ?: fail("eglGetPlatformDisplay(surfaceless)")
        if (egl.eglInitialize(dpy, IntArray(1), IntArray(1)) == 0) fail("eglInitialize")
        if (egl.eglBindAPI(0x30A0) == 0) fail("eglBindAPI")
        val configs = arrayOfNulls<Pointer>(1)
        val count = IntArray(1)
        if (egl.eglChooseConfig(dpy, intArrayOf(0x3033, 0x0001, 0x3040, 0x0040, 0x3038), configs, 1, count) == 0 || count[0] == 0) fail("eglChooseConfig")
        val ctx = egl.eglCreateContext(dpy, configs[0], null, intArrayOf(0x3098, 3, 0x30FB, 0, 0x3038)) ?: fail("eglCreateContext")
        if (egl.eglMakeCurrent(dpy, null, null, ctx) == 0) fail("eglMakeCurrent")
        gl = JnaGl()
        fb = gl.genFramebuffer()
        gl.bindFramebuffer(GL.FRAMEBUFFER, fb)
        val color = gl.genRenderbuffer()
        gl.bindRenderbuffer(GL.RENDERBUFFER, color)
        gl.renderbufferStorage(GL.RENDERBUFFER, 0x8058, width, height) // RGBA8
        gl.framebufferRenderbuffer(GL.FRAMEBUFFER, GL.COLOR_ATTACHMENT0, GL.RENDERBUFFER, color)
        val depth = gl.genRenderbuffer()
        gl.bindRenderbuffer(GL.RENDERBUFFER, depth)
        gl.renderbufferStorage(GL.RENDERBUFFER, GL.DEPTH24_STENCIL8, width, height)
        gl.framebufferRenderbuffer(GL.FRAMEBUFFER, GL.DEPTH_STENCIL_ATTACHMENT, GL.RENDERBUFFER, depth)
        if (gl.checkFramebufferStatus(GL.FRAMEBUFFER) != GL.FRAMEBUFFER_COMPLETE) throw IllegalStateException("offscreen framebuffer is incomplete")
    }
}
