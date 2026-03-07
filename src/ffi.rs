use std::ffi::{CStr, c_char};
use std::fs;
use std::path::PathBuf;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use typst::layout::PagedDocument;
use typst_pdf::PdfOptions;

use crate::world::SimpleWorld;

pub type TypstDownloadFunc = extern "C" fn(url: *const c_char, dest_path: *const c_char) -> bool;

pub static mut DOWNLOAD_CALLBACK: Option<TypstDownloadFunc> = None;

#[unsafe(no_mangle)]
pub extern "C" fn typst_set_network_downloader(cb: TypstDownloadFunc) {
    unsafe {
        DOWNLOAD_CALLBACK = Some(cb);
    }
}

pub static mut SHARED_GROUP_PATH: Option<String> = None;

static LAST_ERROR_MESSAGE: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

fn last_error_storage() -> &'static Mutex<Vec<u8>> {
    LAST_ERROR_MESSAGE.get_or_init(|| Mutex::new(b"Unknown error\0".to_vec()))
}

fn set_last_error_message(message: impl Into<String>) {
    let mut bytes = message.into().into_bytes();
    bytes.retain(|byte| *byte != 0);
    bytes.push(0);
    if let Ok(mut guard) = last_error_storage().lock() {
        *guard = bytes;
    }
}

fn set_default_error_message(result: TypstResult) {
    let message = match result {
        TypstResult::Success => "Success",
        TypstResult::InvalidInputPath => "Invalid input path",
        TypstResult::InvalidOutputPath => "Invalid output path",
        TypstResult::ReadError => "Failed to read file",
        TypstResult::CompileError => "Compilation error",
        TypstResult::ExportError => "PDF export error",
        TypstResult::WriteError => "Failed to write file",
        TypstResult::InvalidUtf8 => "Invalid UTF-8 string",
        TypstResult::MemoryError => "Memory allocation error",
    };
    set_last_error_message(message);
}

#[unsafe(no_mangle)]
pub extern "C" fn typst_set_shared_group_path(path: *const c_char) {
    if path.is_null() {
        unsafe { SHARED_GROUP_PATH = None; }
        return;
    }
    if let Ok(s) = unsafe { CStr::from_ptr(path).to_str() } {
        unsafe {
            SHARED_GROUP_PATH = Some(s.to_string());
        }
    }
}

/// 编译结果状态码
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[repr(C)]
pub enum TypstResult {
    /// 编译成功
    Success = 0,
    /// 输入文件路径无效
    InvalidInputPath = 1,
    /// 输出文件路径无效
    InvalidOutputPath = 2,
    /// 读取文件失败
    ReadError = 3,
    /// 编译错误
    CompileError = 4,
    /// PDF 导出错误
    ExportError = 5,
    /// 写入文件失败
    WriteError = 6,
    /// 无效的 UTF-8 字符串
    InvalidUtf8 = 7,
    /// 内存分配失败
    MemoryError = 8,
}

/// PDF 输出缓冲区
#[repr(C)]
pub struct TypstPdfBuffer {
    /// 指向 PDF 数据的指针
    pub data: *mut u8,
    /// PDF 数据长度
    pub len: usize,
    /// 容量（内部使用）
    pub capacity: usize,
}

/// 编译错误信息
#[repr(C)]
pub struct TypstError {
    /// 错误消息
    pub message: *mut c_char,
    /// 错误行号（从1开始，0表示未知）
    pub line: u32,
    /// 错误列号（从1开始，0表示未知）
    pub column: u32,
}

/// 错误列表
#[repr(C)]
pub struct TypstErrorList {
    /// 错误数组
    pub errors: *mut TypstError,
    /// 错误数量
    pub count: usize,
}

// ============================================================================
// C FFI 函数
// ============================================================================

/// 从文件编译 Typst 到 PDF 文件
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_compile_file(
    input_path: *const c_char,
    output_path: *const c_char,
) -> TypstResult {
    set_default_error_message(TypstResult::Success);
    if input_path.is_null() {
        set_default_error_message(TypstResult::InvalidInputPath);
        return TypstResult::InvalidInputPath;
    }
    if output_path.is_null() {
        set_default_error_message(TypstResult::InvalidOutputPath);
        return TypstResult::InvalidOutputPath;
    }

    let input = match CStr::from_ptr(input_path).to_str() {
        Ok(s) => PathBuf::from(s),
        Err(_) => {
            set_default_error_message(TypstResult::InvalidUtf8);
            return TypstResult::InvalidUtf8;
        }
    };
    let output = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => PathBuf::from(s),
        Err(_) => {
            set_default_error_message(TypstResult::InvalidUtf8);
            return TypstResult::InvalidUtf8;
        }
    };

    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => {
            set_last_error_message(format!("Failed to read file {}: {}", input.display(), e));
            return TypstResult::ReadError;
        }
    };

    let root = input.parent();
    let world = SimpleWorld::new(&source, root);
    
    match typst::compile::<PagedDocument>(&world).output {
        Ok(document) => {
            let options = PdfOptions::default();
            match typst_pdf::pdf(&document, &options) {
                Ok(pdf_data) => {
                    match fs::write(&output, pdf_data) {
                        Ok(_) => TypstResult::Success,
                        Err(e) => {
                            eprintln!("WRITE ERROR: {:?}", e);
                            set_last_error_message(format!("Failed to write PDF {}: {}", output.display(), e));
                            TypstResult::WriteError
                        }
                    }
                }
                Err(e) => {
                    eprintln!("EXPORT ERROR: {:?}", e);
                    set_last_error_message(format!("PDF export error: {:?}", e));
                    TypstResult::ExportError
                }
            }
        }
        Err(e) => {
            eprintln!("COMPILE ERROR: {:?}", e);
            set_last_error_message(format!("{:?}", e));
            TypstResult::CompileError
        }
    }
}

/// 从字符串编译 Typst 到 PDF 文件
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_compile_string(
    source: *const c_char,
    output_path: *const c_char,
    root_path: *const c_char,
) -> TypstResult {
    set_default_error_message(TypstResult::Success);
    if source.is_null() {
        set_default_error_message(TypstResult::InvalidInputPath);
        return TypstResult::InvalidInputPath;
    }
    if output_path.is_null() {
        set_default_error_message(TypstResult::InvalidOutputPath);
        return TypstResult::InvalidOutputPath;
    }

    let source_str = match CStr::from_ptr(source).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_default_error_message(TypstResult::InvalidUtf8);
            return TypstResult::InvalidUtf8;
        }
    };
    let output = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => PathBuf::from(s),
        Err(_) => {
            set_default_error_message(TypstResult::InvalidUtf8);
            return TypstResult::InvalidUtf8;
        }
    };
    let root = if root_path.is_null() {
        None
    } else {
        match CStr::from_ptr(root_path).to_str() {
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => {
                set_default_error_message(TypstResult::InvalidUtf8);
                return TypstResult::InvalidUtf8;
            }
        }
    };

    let world = SimpleWorld::new(source_str, root.as_deref());
    
    match typst::compile::<PagedDocument>(&world).output {
        Ok(document) => {
            let options = PdfOptions::default();
            match typst_pdf::pdf(&document, &options) {
                Ok(pdf_data) => {
                    match fs::write(&output, pdf_data) {
                        Ok(_) => TypstResult::Success,
                        Err(e) => {
                            set_last_error_message(format!("Failed to write PDF {}: {}", output.display(), e));
                            TypstResult::WriteError
                        }
                    }
                }
                Err(e) => {
                    set_last_error_message(format!("PDF export error: {:?}", e));
                    TypstResult::ExportError
                }
            }
        }
        Err(e) => {
            set_last_error_message(format!("{:?}", e));
            TypstResult::CompileError
        }
    }
}

/// 从字符串编译 Typst 到内存缓冲区
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_compile_to_buffer(
    source: *const c_char,
    root_path: *const c_char,
    out_buffer: *mut TypstPdfBuffer,
) -> TypstResult {
    set_default_error_message(TypstResult::Success);
    if source.is_null() || out_buffer.is_null() {
        set_default_error_message(TypstResult::InvalidInputPath);
        return TypstResult::InvalidInputPath;
    }

    let source_str = match CStr::from_ptr(source).to_str() {
        Ok(s) => s,
        Err(_) => {
            set_default_error_message(TypstResult::InvalidUtf8);
            return TypstResult::InvalidUtf8;
        }
    };
    let root = if root_path.is_null() {
        None
    } else {
        match CStr::from_ptr(root_path).to_str() {
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => {
                set_default_error_message(TypstResult::InvalidUtf8);
                return TypstResult::InvalidUtf8;
            }
        }
    };

    let world = SimpleWorld::new(source_str, root.as_deref());
    
    match typst::compile::<PagedDocument>(&world).output {
        Ok(document) => {
            let options = PdfOptions::default();
            match typst_pdf::pdf(&document, &options) {
                Ok(mut pdf_data) => {
                    let buffer = TypstPdfBuffer {
                        data: pdf_data.as_mut_ptr(),
                        len: pdf_data.len(),
                        capacity: pdf_data.capacity(),
                    };
                    std::mem::forget(pdf_data);
                    *out_buffer = buffer;
                    TypstResult::Success
                }
                Err(e) => {
                    eprintln!("PDF EXPORT ERROR: {:?}", e);
                    set_last_error_message(format!("PDF export error: {:?}", e));
                    TypstResult::ExportError
                }
            }
        }
        Err(e) => {
            eprintln!("COMPILE ERROR: {:?}", e);
            set_last_error_message(format!("{:?}", e));
            TypstResult::CompileError
        }
    }
}

/// 释放 PDF 缓冲区
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_free_buffer(buffer: *mut TypstPdfBuffer) {
    if buffer.is_null() {
        return;
    }
    let buf = &*buffer;
    if !buf.data.is_null() && buf.capacity > 0 {
        let _ = Vec::from_raw_parts(buf.data, buf.len, buf.capacity);
    }
    (*buffer).data = ptr::null_mut();
    (*buffer).len = 0;
    (*buffer).capacity = 0;
}

/// 获取错误信息字符串
#[unsafe(no_mangle)]
pub extern "C" fn typst_result_message(_result: TypstResult) -> *const c_char {
    match last_error_storage().lock() {
        Ok(guard) => guard.as_ptr() as *const c_char,
        Err(_) => b"Unknown error\0".as_ptr() as *const c_char,
    }
}

/// 获取库版本
#[unsafe(no_mangle)]
pub extern "C" fn typst_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}
