package rangerthree.gpu

import android.opengl.GLES30
import java.nio.Buffer
import java.nio.ByteBuffer

/** Gl over android.opengl.GLES30, for a GLSurfaceView / EGL context made by the app. */
class AndroidGl : Gl {
    private val one = IntArray(1)
    private fun gen(f: (Int, IntArray, Int) -> Unit): Int { f(1, one, 0); return one[0] }
    private fun del(name: Int, f: (Int, IntArray, Int) -> Unit) { one[0] = name; f(1, one, 0) }

    override fun getString(name: Int): String = GLES30.glGetString(name) ?: ""
    override fun genVertexArray(): Int = gen(GLES30::glGenVertexArrays)
    override fun bindVertexArray(vao: Int) = GLES30.glBindVertexArray(vao)

    override fun genBuffer(): Int = gen(GLES30::glGenBuffers)
    override fun deleteBuffer(b: Int) = del(b, GLES30::glDeleteBuffers)
    override fun bindBuffer(target: Int, b: Int) = GLES30.glBindBuffer(target, b)
    override fun bufferData(target: Int, bytes: Int, data: Buffer, usage: Int) = GLES30.glBufferData(target, bytes, data, usage)
    override fun bufferSubData(target: Int, offset: Int, bytes: Int, data: Buffer) = GLES30.glBufferSubData(target, offset, bytes, data)

    override fun createShader(type: Int): Int = GLES30.glCreateShader(type)
    override fun shaderSource(s: Int, source: String) = GLES30.glShaderSource(s, source)
    override fun compileShader(s: Int) = GLES30.glCompileShader(s)
    override fun shaderCompiled(s: Int): Boolean { GLES30.glGetShaderiv(s, GLES30.GL_COMPILE_STATUS, one, 0); return one[0] != 0 }
    override fun shaderInfoLog(s: Int): String = GLES30.glGetShaderInfoLog(s)
    override fun deleteShader(s: Int) = GLES30.glDeleteShader(s)
    override fun createProgram(): Int = GLES30.glCreateProgram()
    override fun attachShader(p: Int, s: Int) = GLES30.glAttachShader(p, s)
    override fun linkProgram(p: Int) = GLES30.glLinkProgram(p)
    override fun programLinked(p: Int): Boolean { GLES30.glGetProgramiv(p, GLES30.GL_LINK_STATUS, one, 0); return one[0] != 0 }
    override fun programInfoLog(p: Int): String = GLES30.glGetProgramInfoLog(p)
    override fun deleteProgram(p: Int) = GLES30.glDeleteProgram(p)
    override fun useProgram(p: Int) = GLES30.glUseProgram(p)
    override fun getUniformLocation(p: Int, name: String): Int = GLES30.glGetUniformLocation(p, name)

    override fun uniform1i(loc: Int, v: Int) = GLES30.glUniform1i(loc, v)
    override fun uniform1fv(loc: Int, count: Int, v: FloatArray, offset: Int) = GLES30.glUniform1fv(loc, count, v, offset)
    override fun uniform2fv(loc: Int, count: Int, v: FloatArray, offset: Int) = GLES30.glUniform2fv(loc, count, v, offset)
    override fun uniform3fv(loc: Int, count: Int, v: FloatArray, offset: Int) = GLES30.glUniform3fv(loc, count, v, offset)
    override fun uniform4fv(loc: Int, count: Int, v: FloatArray, offset: Int) = GLES30.glUniform4fv(loc, count, v, offset)
    override fun uniformMatrix3fv(loc: Int, count: Int, v: FloatArray, offset: Int) = GLES30.glUniformMatrix3fv(loc, count, false, v, offset)
    override fun uniformMatrix4fv(loc: Int, count: Int, v: FloatArray, offset: Int) = GLES30.glUniformMatrix4fv(loc, count, false, v, offset)

    override fun genTexture(): Int = gen(GLES30::glGenTextures)
    override fun deleteTexture(t: Int) = del(t, GLES30::glDeleteTextures)
    override fun activeTexture(unit: Int) = GLES30.glActiveTexture(unit)
    override fun bindTexture(target: Int, t: Int) = GLES30.glBindTexture(target, t)
    override fun pixelStorei(name: Int, v: Int) = GLES30.glPixelStorei(name, v)
    override fun texImage2D(target: Int, level: Int, internal: Int, w: Int, h: Int, format: Int, type: Int, data: Buffer?) =
        GLES30.glTexImage2D(target, level, internal, w, h, 0, format, type, data)
    override fun texParameteri(target: Int, name: Int, v: Int) = GLES30.glTexParameteri(target, name, v)
    override fun generateMipmap(target: Int) = GLES30.glGenerateMipmap(target)

    override fun genFramebuffer(): Int = gen(GLES30::glGenFramebuffers)
    override fun deleteFramebuffer(fb: Int) = del(fb, GLES30::glDeleteFramebuffers)
    override fun bindFramebuffer(target: Int, fb: Int) = GLES30.glBindFramebuffer(target, fb)
    override fun framebufferTexture2D(target: Int, attachment: Int, texTarget: Int, t: Int, level: Int) =
        GLES30.glFramebufferTexture2D(target, attachment, texTarget, t, level)
    override fun genRenderbuffer(): Int = gen(GLES30::glGenRenderbuffers)
    override fun deleteRenderbuffer(rb: Int) = del(rb, GLES30::glDeleteRenderbuffers)
    override fun bindRenderbuffer(target: Int, rb: Int) = GLES30.glBindRenderbuffer(target, rb)
    override fun renderbufferStorage(target: Int, internal: Int, w: Int, h: Int) = GLES30.glRenderbufferStorage(target, internal, w, h)
    override fun renderbufferStorageMultisample(target: Int, samples: Int, internal: Int, w: Int, h: Int) =
        GLES30.glRenderbufferStorageMultisample(target, samples, internal, w, h)
    override fun framebufferRenderbuffer(target: Int, attachment: Int, rbTarget: Int, rb: Int) =
        GLES30.glFramebufferRenderbuffer(target, attachment, rbTarget, rb)
    override fun drawBuffers(bufs: IntArray) = GLES30.glDrawBuffers(bufs.size, bufs, 0)
    override fun readBuffer(src: Int) = GLES30.glReadBuffer(src)
    override fun checkFramebufferStatus(target: Int): Int = GLES30.glCheckFramebufferStatus(target)
    override fun blitFramebuffer(sx0: Int, sy0: Int, sx1: Int, sy1: Int, dx0: Int, dy0: Int, dx1: Int, dy1: Int, mask: Int, filter: Int) =
        GLES30.glBlitFramebuffer(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, mask, filter)

    override fun viewport(x: Int, y: Int, w: Int, h: Int) = GLES30.glViewport(x, y, w, h)
    override fun clearColor(r: Float, g: Float, b: Float, a: Float) = GLES30.glClearColor(r, g, b, a)
    override fun clearDepthf(d: Float) = GLES30.glClearDepthf(d)
    override fun clear(mask: Int) = GLES30.glClear(mask)
    override fun enable(cap: Int) = GLES30.glEnable(cap)
    override fun disable(cap: Int) = GLES30.glDisable(cap)
    override fun depthMask(on: Boolean) = GLES30.glDepthMask(on)
    override fun colorMask(r: Boolean, g: Boolean, b: Boolean, a: Boolean) = GLES30.glColorMask(r, g, b, a)
    override fun depthFunc(f: Int) = GLES30.glDepthFunc(f)
    override fun blendEquationSeparate(rgb: Int, alpha: Int) = GLES30.glBlendEquationSeparate(rgb, alpha)
    override fun blendFuncSeparate(srcRgb: Int, dstRgb: Int, srcAlpha: Int, dstAlpha: Int) = GLES30.glBlendFuncSeparate(srcRgb, dstRgb, srcAlpha, dstAlpha)
    override fun blendColor(r: Float, g: Float, b: Float, a: Float) = GLES30.glBlendColor(r, g, b, a)
    override fun cullFace(mode: Int) = GLES30.glCullFace(mode)
    override fun frontFace(mode: Int) = GLES30.glFrontFace(mode)
    override fun polygonOffset(factor: Float, units: Float) = GLES30.glPolygonOffset(factor, units)

    override fun enableVertexAttribArray(i: Int) = GLES30.glEnableVertexAttribArray(i)
    override fun disableVertexAttribArray(i: Int) = GLES30.glDisableVertexAttribArray(i)
    override fun vertexAttrib4f(i: Int, x: Float, y: Float, z: Float, w: Float) = GLES30.glVertexAttrib4f(i, x, y, z, w)
    override fun vertexAttribPointer(i: Int, size: Int, type: Int, normalized: Boolean, stride: Int, offset: Int) =
        GLES30.glVertexAttribPointer(i, size, type, normalized, stride, offset)
    override fun vertexAttribDivisor(i: Int, divisor: Int) = GLES30.glVertexAttribDivisor(i, divisor)
    override fun drawArrays(mode: Int, first: Int, count: Int) = GLES30.glDrawArrays(mode, first, count)
    override fun drawArraysInstanced(mode: Int, first: Int, count: Int, instances: Int) = GLES30.glDrawArraysInstanced(mode, first, count, instances)
    override fun drawElements(mode: Int, count: Int, type: Int, offset: Int) = GLES30.glDrawElements(mode, count, type, offset)
    override fun drawElementsInstanced(mode: Int, count: Int, type: Int, offset: Int, instances: Int) =
        GLES30.glDrawElementsInstanced(mode, count, type, offset, instances)

    override fun readPixels(x: Int, y: Int, w: Int, h: Int, format: Int, type: Int, out: ByteBuffer) = GLES30.glReadPixels(x, y, w, h, format, type, out)
}
