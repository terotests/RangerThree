// SPDX-License-Identifier: AGPL-3.0-or-later
//! CErXes for a page that answers the script synchronously (the GL bridge):
//!
//!   cx_alloc(n) -> ptr / cx_free(ptr, n)
//!   cx_new() -> engine           register machine, __hostSync wired to the page
//!   cx_jit(engine, on, min)      the baseline JIT (build with `wjit`)
//!   cx_eval(engine, ptr, n)      runs a script; 0 ok, 1 it threw
//!   cx_call(engine, name, nlen, arg, alen)
//!                                calls the global function `name` with one
//!                                string argument (a frame); 0 ok, 1 it threw
//!   cx_result_ptr() / _len(), cx_output_ptr() / _len()
//!
//! Imports from the page: cerxes_host.sync(ptr, n) -> answer length, then
//! cerxes_host.take(ptr) copies that many bytes of the answer to ptr.
use cerxes::value::*;
use cerxes::vm::HostSync;
use cerxes::Engine;
use std::cell::RefCell;

// componentengine's small-block allocator, as its own WASM builds use
mod salloc;

#[global_allocator]
static GLOBAL: salloc::SmallAlloc = salloc::SmallAlloc;

#[link(wasm_import_module = "cerxes_host")]
extern "C" {
    fn sync(p: *const u8, n: usize) -> usize;
    fn take(p: *mut u8);
}

struct PageHost {}

impl HostSync for PageHost {
    fn call(&mut self, request: &str) -> String {
        unsafe {
            let n = sync(request.as_ptr(), request.len());
            let mut buf: Vec<u8> = vec![0; n];
            if n > 0 {
                take(buf.as_mut_ptr());
            }
            String::from_utf8(buf).unwrap_or_default()
        }
    }
}

thread_local! {
    static RESULT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    static OUTPUT: RefCell<Vec<u8>> = RefCell::new(Vec::new());
}

fn text(p: *const u8, n: usize) -> String {
    unsafe { String::from_utf8_lossy(std::slice::from_raw_parts(p, n)).into_owned() }
}

fn finish(engine: &mut Engine, value: String) -> i32 {
    let failed = !engine.error.is_empty();
    let out = engine.output();
    engine.clear_output();
    OUTPUT.with(|b| *b.borrow_mut() = out.into_bytes());
    RESULT.with(|b| *b.borrow_mut() = value.into_bytes());
    failed as i32
}

#[no_mangle]
pub extern "C" fn cx_alloc(n: usize) -> *mut u8 {
    let mut v: Vec<u8> = Vec::with_capacity(n.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

#[no_mangle]
pub unsafe extern "C" fn cx_free(p: *mut u8, n: usize) {
    drop(Vec::from_raw_parts(p, 0, n.max(1)));
}

#[no_mangle]
pub extern "C" fn cx_new() -> *mut Engine {
    let mut e = Engine::new();
    e.vm.regs = true;
    e.vm.set_host_sync(Box::new(PageHost {}));
    Box::into_raw(Box::new(e))
}

#[no_mangle]
pub unsafe extern "C" fn cx_jit(e: *mut Engine, on: i32, min: i32) -> i32 {
    let engine = &mut *e;
    let ok = cerxes::wjit::available();
    engine.vm.jit_on = on != 0 && ok;
    engine.vm.jit_min = min as i64;
    ok as i32
}

#[no_mangle]
pub unsafe extern "C" fn cx_eval(e: *mut Engine, p: *const u8, n: usize) -> i32 {
    let engine = &mut *e;
    let r = engine.eval(text(p, n).as_str());
    finish(engine, r)
}

#[no_mangle]
pub unsafe extern "C" fn cx_call(e: *mut Engine, np: *const u8, nn: usize, ap: *const u8, an: usize) -> i32 {
    let engine = &mut *e;
    engine.error = String::new();
    let a = engine.vm.intern(text(np, nn).as_str());
    let g = engine.vm.global;
    let f = engine.vm.get_obj(g, a, &Val::Obj(g));
    if !engine.vm.is_callable(&f) {
        engine.error = String::from("not a function");
        let msg = engine.error.clone();
        return finish(engine, msg);
    }
    let av = engine.vm.string_val(text(ap, an));
    let r = engine.vm.call_top(f, Val::Undef, vec![av]);
    let value = if engine.vm.throwing {
        let exc = engine.vm.exc.clone();
        engine.vm.throwing = false;
        engine.vm.exc = Val::Undef;
        engine.vm.stack.clear();
        engine.vm.frames.clear();
        engine.vm.handlers.clear();
        engine.vm.native_depth = 0;
        let s = engine.vm.display(&exc);
        engine.error = format!("Uncaught {}", s);
        engine.error.clone()
    } else {
        engine.vm.temp_roots.push(r.clone());
        engine.vm.run_jobs();
        engine.vm.temp_roots.pop();
        match &r {
            Val::Str(s) => engine.vm.sref(*s).to_string(),
            _ => engine.vm.display(&r),
        }
    };
    finish(engine, value)
}

#[no_mangle]
pub extern "C" fn cx_result_ptr() -> *const u8 {
    RESULT.with(|b| b.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn cx_result_len() -> usize {
    RESULT.with(|b| b.borrow().len())
}

#[no_mangle]
pub extern "C" fn cx_output_ptr() -> *const u8 {
    OUTPUT.with(|b| b.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn cx_output_len() -> usize {
    OUTPUT.with(|b| b.borrow().len())
}
