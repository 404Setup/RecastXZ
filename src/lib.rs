use jni::JNIEnv;
use jni::objects::JClass;
use jni::sys::{JNI_FALSE, JNI_TRUE, jboolean, jint, jlong};
use libdeflater::{CompressionLvl, Compressor, Decompressor};
use std::ffi::c_int;
use std::ptr;

enum DeflateResult {
    Success(usize),
    InsufficientSpace,
    Error,
}

enum InflateResult {
    Success,
    InsufficientSpace,
    BadData,
    Error,
}

struct DeflateContext {
    compressor: Compressor,
}

struct InflateContext {
    decompressor: Decompressor,
}

fn deflate_init(level: c_int) -> Option<*mut DeflateContext> {
    match CompressionLvl::new(level) {
        Ok(lvl) => {
            let compressor = Compressor::new(lvl);
            let context = Box::new(DeflateContext { compressor });
            Some(Box::into_raw(context))
        }
        Err(_) => None,
    }
}

unsafe fn deflate_process(
    ctx: *mut DeflateContext,
    source_ptr: *const u8,
    source_len: usize,
    dest_ptr: *mut u8,
    dest_len: usize,
) -> DeflateResult {
    if ctx.is_null() {
        return DeflateResult::Error;
    }
    let context = &mut *ctx;

    let source_slice = std::slice::from_raw_parts(source_ptr, source_len);
    let dest_slice = std::slice::from_raw_parts_mut(dest_ptr, dest_len);

    match context.compressor.zlib_compress(source_slice, dest_slice) {
        Ok(sz) => DeflateResult::Success(sz),
        Err(_) => DeflateResult::InsufficientSpace,
    }
}

unsafe fn inflate_process(
    ctx: *mut InflateContext,
    source_ptr: *const u8,
    source_len: usize,
    dest_ptr: *mut u8,
    dest_len: usize,
) -> InflateResult {
    if ctx.is_null() {
        return InflateResult::Error;
    }
    let context = &mut *ctx;

    let source_slice = std::slice::from_raw_parts(source_ptr, source_len);
    let dest_slice = std::slice::from_raw_parts_mut(dest_ptr, dest_len);

    match context
        .decompressor
        .zlib_decompress(source_slice, dest_slice)
    {
        Ok(_) => InflateResult::Success,
        Err(libdeflater::DecompressionError::InsufficientSpace) => InflateResult::InsufficientSpace,
        Err(libdeflater::DecompressionError::BadData) => InflateResult::BadData,
        Err(_) => InflateResult::Error,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_velocitypowered_natives_compression_NativeZlibDeflate_init(
    mut env: JNIEnv,
    _class: JClass,
    level: jint,
) -> jlong {
    match deflate_init(level) {
        Some(ctx) => ctx as jlong,
        None => {
            let exception_class = env
                .find_class("java/lang/OutOfMemoryError")
                .unwrap();
            env.throw_new(exception_class, "libdeflate allocate compressor")
                .unwrap();
            0
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_velocitypowered_natives_compression_NativeZlibDeflate_free(
    _env: JNIEnv,
    _class: JClass,
    ctx: jlong,
) {
    if ctx != 0 {
        let _ = Box::from_raw(ctx as *mut DeflateContext);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_velocitypowered_natives_compression_NativeZlibDeflate_process(
    mut env: JNIEnv,
    _class: JClass,
    ctx: jlong,
    source_address: jlong,
    source_length: jint,
    destination_address: jlong,
    destination_length: jint,
) -> jint {
    let res = deflate_process(
        ctx as *mut DeflateContext,
        source_address as *const u8,
        source_length as usize,
        destination_address as *mut u8,
        destination_length as usize,
    );

    match res {
        DeflateResult::Success(size) => size as jint,
        DeflateResult::InsufficientSpace => 0,
        DeflateResult::Error => 0,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_velocitypowered_natives_compression_NativeZlibInflate_init(
    mut env: JNIEnv,
    _class: JClass,
) -> jlong {
    let decompressor = Decompressor::new();
    let context = Box::new(InflateContext { decompressor });
    Box::into_raw(context) as jlong
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_velocitypowered_natives_compression_NativeZlibInflate_free(
    _env: JNIEnv,
    _class: JClass,
    ctx: jlong,
) {
    if ctx != 0 {
        let _ = Box::from_raw(ctx as *mut InflateContext);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn Java_com_velocitypowered_natives_compression_NativeZlibInflate_process(
    mut env: JNIEnv,
    _class: JClass,
    ctx: jlong,
    source_address: jlong,
    source_length: jint,
    destination_address: jlong,
    destination_length: jint,
) -> jboolean {
    let res = inflate_process(
        ctx as *mut InflateContext,
        source_address as *const u8,
        source_length as usize,
        destination_address as *mut u8,
        destination_length as usize,
    );

    match res {
        InflateResult::Success => JNI_TRUE,
        InflateResult::BadData => {
            let exception_class = env.find_class("java/util/zip/DataFormatException").unwrap();
            env.throw_new(exception_class, "inflate data is bad")
                .unwrap();
            JNI_FALSE
        }
        InflateResult::InsufficientSpace => {
            let exception_class = env.find_class("java/util/zip/DataFormatException").unwrap();
            env.throw_new(exception_class, "uncompressed size is inaccurate")
                .unwrap();
            JNI_FALSE
        }
        InflateResult::Error => {
            let exception_class = env.find_class("java/util/zip/DataFormatException").unwrap();
            env.throw_new(exception_class, "unknown libdeflate return code")
                .unwrap();
            JNI_FALSE
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rxz_deflate_init(level: c_int) -> *mut DeflateContext {
    deflate_init(level).unwrap_or_else(|| ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rxz_deflate_free(ctx: *mut DeflateContext) {
    if !ctx.is_null() {
        let _ = Box::from_raw(ctx);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rxz_deflate_process(
    ctx: *mut DeflateContext,
    source: *const u8,
    source_length: c_int,
    destination: *mut u8,
    destination_length: c_int,
) -> c_int {
    let res = deflate_process(
        ctx,
        source,
        source_length as usize,
        destination,
        destination_length as usize,
    );
    match res {
        DeflateResult::Success(sz) => sz as c_int,
        DeflateResult::InsufficientSpace => 0,
        DeflateResult::Error => -1,
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rxz_inflate_init() -> *mut InflateContext {
    let decompressor = Decompressor::new();
    let context = Box::new(InflateContext { decompressor });
    Box::into_raw(context)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rxz_inflate_free(ctx: *mut InflateContext) {
    if !ctx.is_null() {
        let _ = Box::from_raw(ctx);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rxz_inflate_process(
    ctx: *mut InflateContext,
    source: *const u8,
    source_length: c_int,
    destination: *mut u8,
    destination_length: c_int,
) -> c_int {
    let res = inflate_process(
        ctx,
        source,
        source_length as usize,
        destination,
        destination_length as usize,
    );
    match res {
        InflateResult::Success => 0,
        InflateResult::InsufficientSpace => 1,
        InflateResult::BadData => 2,
        InflateResult::Error => 3,
    }
}
