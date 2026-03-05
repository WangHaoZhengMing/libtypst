//! Typst C FFI Bridge - 用于从 C 代码编译 Typst 文档到 PDF
//!
//! 提供简单的 C API 来编译 Typst 源文件或字符串到 PDF 文件或内存缓冲区。
#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::HashMap;
use std::ffi::{CStr, c_char};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::{Arc, OnceLock};

use chrono::{DateTime, Datelike, Timelike, Utc};
use parking_lot::{RwLock, Mutex};
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::utils::LazyHash;
use typst::layout::PagedDocument;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::{Library, LibraryExt, World};
use typst_pdf::PdfOptions;

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
// 全局包管理
// ============================================================================

/// 全局包缓存，避免重复下载检查
static PACKAGE_CACHE: OnceLock<Arc<Mutex<HashMap<String, PathBuf>>>> = OnceLock::new();

/// 获取包缓存实例
fn get_package_cache() -> &'static Arc<Mutex<HashMap<String, PathBuf>>> {
    PACKAGE_CACHE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
}

// ============================================================================
// 简化的 World 实现
// ============================================================================

/// 简单的 Typst World 实现，用于编译
struct SimpleWorld {
    /// 主源文件
    main: Source,
    /// 字体书
    book: LazyHash<FontBook>,
    /// 字体列表
    fonts: Vec<Font>,
    /// 标准库
    library: LazyHash<Library>,
    /// 文件缓存
    files: RwLock<HashMap<FileId, Source>>,
    /// 根目录
    root: PathBuf,
    /// 当前时间
    now: OnceLock<DateTime<Utc>>,
}

impl SimpleWorld {
    /// 从源代码字符串创建 World
    fn new(source: &str, root: Option<&Path>) -> Self {
        let root = root.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        
        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let main = Source::new(main_id, source.into());
        
        let mut book = FontBook::new();
        let mut fonts = Vec::new();
        
        // 从自带和系统加载字体
        Self::load_fonts(&mut fonts, &mut book);
        
        Self {
            main,
            book: LazyHash::new(book),
            fonts,
            library: LazyHash::new(Library::default()),
            files: RwLock::new(HashMap::new()),
            root,
            now: OnceLock::new(),
        }
    }
}

impl SimpleWorld {
    /// 解析并缓存系统字体
    fn load_fonts(fonts: &mut Vec<Font>, book: &mut FontBook) {
        // 加载 Typst 官方内置字体
        for data in typst_assets::fonts() {
            let buffer = Bytes::new(data);
            for font in Font::iter(buffer) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }
        
        // 加载系统字体
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for face in db.faces() {
            if let fontdb::Source::File(path) = &face.source {
                if let Ok(data) = fs::read(path) {
                    let buffer = Bytes::new(data);
                    for font in Font::iter(buffer) {
                        book.push(font.info().clone());
                        fonts.push(font);
                    }
                }
            }
        }
    }

    /// 下载并解压包（优化版本）
    fn download_package(spec: &typst::syntax::package::PackageSpec) -> FileResult<PathBuf> {
        let package_key = format!("{}/{}/{}", spec.namespace, spec.name, spec.version);
        
        // 检查全局缓存
        {
            let cache = get_package_cache().lock();
            if let Some(path) = cache.get(&package_key) {
                if path.exists() {
                    eprintln!("[CACHE] Using cached package: {}", package_key);
                    return Ok(path.clone());
                }
            }
        }
        
        eprintln!("[DOWNLOAD] Starting download for package: {}", package_key);
        
        // 标准缓存路径
        let cache_dir = dirs::cache_dir().unwrap_or_else(|| PathBuf::from(".cache"))
            .join("typst")
            .join("packages")
            .join(spec.namespace.as_str())
            .join(spec.name.as_str())
            .join(spec.version.to_string());

        let data_dir = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".cache"))
            .join("typst")
            .join("packages")
            .join(spec.namespace.as_str())
            .join(spec.name.as_str())
            .join(spec.version.to_string());

        // 检查本地缓存
        if data_dir.exists() {
            get_package_cache().lock().insert(package_key, data_dir.clone());
            return Ok(data_dir);
        }
        if cache_dir.exists() {
            get_package_cache().lock().insert(package_key.clone(), cache_dir.clone());
            return Ok(cache_dir);
        }
        
        eprintln!("[DOWNLOAD] Package not cached, downloading: {}", package_key);
        
        // 通过GitHub API逐文件下载（优化版本）
        Self::download_package_files(spec, &cache_dir).map(|path| {
            get_package_cache().lock().insert(package_key, path.clone());
            path
        })
    }
    
    /// 通过GitHub API逐文件下载（优化内存使用）
    /// 通过GitHub API逐文件下载（优化内存使用）
    fn download_package_files(spec: &typst::syntax::package::PackageSpec, cache_dir: &Path) -> FileResult<PathBuf> {
        fs::create_dir_all(&cache_dir).map_err(|e| FileError::from_io(e, &cache_dir))?;
        
        let mut file_count = 0;
        
        fn download_directory(path: &str, local_dir: &Path, file_count: &mut usize) -> Result<(), Box<dyn std::error::Error>> {
            let api_url = format!("https://api.github.com/repos/typst/packages/contents/{}", path);
            
            let response = ureq::get(&api_url)
                .set("User-Agent", "libtypst-c/0.1.0")
                .timeout(std::time::Duration::from_secs(30))  // 添加超时
                .call()?;
            
            // 限制响应大小以节省内存
            let json_str = response.into_string()?;
            let entries: Vec<serde_json::Value> = serde_json::from_str(&json_str)?;
            
            for entry in entries {
                if let (Some(name), Some(entry_type)) = (
                    entry["name"].as_str(),
                    entry["type"].as_str(),
                ) {
                    let local_path = local_dir.join(name);
                    
                    if entry_type == "file" {
                        if let Some(download_url) = entry["download_url"].as_str() {
                            *file_count += 1;
                            eprintln!("[DOWNLOAD] Downloading file {}: {}", *file_count, name);
                            
                            // 流式下载，减少内存使用
                            let response = ureq::get(download_url)
                                .timeout(std::time::Duration::from_secs(30))
                                .call()?;
                            
                            if name.ends_with(".wasm") || name.ends_with(".png") || 
                               name.ends_with(".jpg") || name.ends_with(".jpeg") ||
                               name.ends_with(".pdf") {
                                // 二进制文件：直接流式写入
                                let mut file = std::fs::File::create(&local_path)?;
                                let mut reader = response.into_reader();
                                std::io::copy(&mut reader, &mut file)?;
                            } else {
                                // 文本文件：读取为字符串
                                let content = response.into_string()?;
                                std::fs::write(&local_path, content)?;
                            }
                        }
                    } else if entry_type == "dir" {
                        eprintln!("[DOWNLOAD] Entering directory: {}", name);
                        std::fs::create_dir_all(&local_path)?;
                        let sub_path = format!("{}/{}", path, name);
                        download_directory(&sub_path, &local_path, file_count)?;
                    }
                }
            }
            Ok(())
        }
        
        let package_path = format!("packages/{}/{}/{}", spec.namespace, spec.name, spec.version);
        
        download_directory(&package_path, &cache_dir, &mut file_count)
            .map_err(|e| {
                eprintln!("[DOWNLOAD] Failed to download package: {:?}", e);
                FileError::NotFound(cache_dir.to_path_buf())
            })?;
        
        eprintln!("[DOWNLOAD] Package downloaded successfully! {} files total", file_count);
        Ok(cache_dir.to_path_buf())
    }
    fn resolve_path(&self, id: FileId) -> FileResult<PathBuf> {
        if let Some(spec) = id.package() {
            let package_dir = Self::download_package(spec)?;
            id.vpath().resolve(&package_dir).ok_or_else(|| FileError::AccessDenied)
        } else {
            id.vpath().resolve(&self.root).ok_or_else(|| FileError::AccessDenied)
        }
    }
}

impl World for SimpleWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        
        // 检查缓存
        if let Some(source) = self.files.read().get(&id) {
            return Ok(source.clone());
        }

        // 从文件系统读取
        let path = self.resolve_path(id)?;

        let content = fs::read_to_string(&path)
            .map_err(|e| FileError::from_io(e, &path))?;

        let source = Source::new(id, content.into());
        self.files.write().insert(id, source.clone());
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve_path(id)?;
        
        let data = fs::read(&path)
            .map_err(|e| FileError::from_io(e, &path))?;
        
        Ok(Bytes::new(data))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, offset: Option<i64>) -> Option<Datetime> {
        let now = self.now.get_or_init(Utc::now);
        let naive = match offset {
            None => now.naive_local(),
            Some(o) => {
                let offset = chrono::FixedOffset::east_opt((o as i32) * 3600)?;
                now.with_timezone(&offset).naive_local()
            }
        };
        Datetime::from_ymd_hms(
            naive.year(),
            naive.month().try_into().ok()?,
            naive.day().try_into().ok()?,
            naive.hour().try_into().ok()?,
            naive.minute().try_into().ok()?,
            naive.second().try_into().ok()?,
        )
    }
}

// ============================================================================
// C FFI 函数
// ============================================================================

/// 从文件编译 Typst 到 PDF 文件
///
/// # 参数
/// - `input_path`: 输入 Typst 文件路径 (UTF-8 C 字符串)
/// - `output_path`: 输出 PDF 文件路径 (UTF-8 C 字符串)
///
/// # 返回
/// `TypstResult` 状态码
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_compile_file(
    input_path: *const c_char,
    output_path: *const c_char,
) -> TypstResult {
    // 检查空指针
    if input_path.is_null() {
        return TypstResult::InvalidInputPath;
    }
    if output_path.is_null() {
        return TypstResult::InvalidOutputPath;
    }

    // 转换路径
    let input = match CStr::from_ptr(input_path).to_str() {
        Ok(s) => PathBuf::from(s),
        Err(_) => return TypstResult::InvalidUtf8,
    };
    let output = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => PathBuf::from(s),
        Err(_) => return TypstResult::InvalidUtf8,
    };

    // 读取源文件
    let source = match fs::read_to_string(&input) {
        Ok(s) => s,
        Err(_) => return TypstResult::ReadError,
    };

    // 创建 World 并编译
    let root = input.parent();
    let world = SimpleWorld::new(&source, root);
    
    match typst::compile::<PagedDocument>(&world).output {
        Ok(document) => {
            // 导出 PDF
            let options = PdfOptions::default();
            match typst_pdf::pdf(&document, &options) {
                Ok(pdf_data) => {
                    // 写入文件
                    match fs::write(&output, pdf_data) {
                        Ok(_) => TypstResult::Success,
                        Err(_) => TypstResult::WriteError,
                    }
                }
                Err(_) => TypstResult::ExportError,
            }
        }
        Err(_) => TypstResult::CompileError,
    }
}

/// 从字符串编译 Typst 到 PDF 文件
///
/// # 参数
/// - `source`: Typst 源代码 (UTF-8 C 字符串)
/// - `output_path`: 输出 PDF 文件路径 (UTF-8 C 字符串)
/// - `root_path`: 根目录路径，用于解析相对路径（可为 NULL）
///
/// # 返回
/// `TypstResult` 状态码
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_compile_string(
    source: *const c_char,
    output_path: *const c_char,
    root_path: *const c_char,
) -> TypstResult {
    if source.is_null() {
        return TypstResult::InvalidInputPath;
    }
    if output_path.is_null() {
        return TypstResult::InvalidOutputPath;
    }

    let source_str = match CStr::from_ptr(source).to_str() {
        Ok(s) => s,
        Err(_) => return TypstResult::InvalidUtf8,
    };
    let output = match CStr::from_ptr(output_path).to_str() {
        Ok(s) => PathBuf::from(s),
        Err(_) => return TypstResult::InvalidUtf8,
    };
    let root = if root_path.is_null() {
        None
    } else {
        match CStr::from_ptr(root_path).to_str() {
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => return TypstResult::InvalidUtf8,
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
                        Err(_) => TypstResult::WriteError,
                    }
                }
                Err(_) => TypstResult::ExportError,
            }
        }
        Err(_) => TypstResult::CompileError,
    }
}

/// 从字符串编译 Typst 到内存缓冲区
///
/// # 参数
/// - `source`: Typst 源代码 (UTF-8 C 字符串)
/// - `root_path`: 根目录路径（可为 NULL）
/// - `out_buffer`: 输出缓冲区指针
///
/// # 返回
/// `TypstResult` 状态码
///
/// # 注意
/// 成功后必须调用 `typst_free_buffer` 释放内存
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_compile_to_buffer(
    source: *const c_char,
    root_path: *const c_char,
    out_buffer: *mut TypstPdfBuffer,
) -> TypstResult {
    if source.is_null() || out_buffer.is_null() {
        return TypstResult::InvalidInputPath;
    }

    let source_str = match CStr::from_ptr(source).to_str() {
        Ok(s) => s,
        Err(_) => return TypstResult::InvalidUtf8,
    };
    let root = if root_path.is_null() {
        None
    } else {
        match CStr::from_ptr(root_path).to_str() {
            Ok(s) => Some(PathBuf::from(s)),
            Err(_) => return TypstResult::InvalidUtf8,
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
                    std::mem::forget(pdf_data); // 防止 Vec 被释放
                    *out_buffer = buffer;
                    TypstResult::Success
                }
                Err(e) => {
                    eprintln!("PDF EXPORT ERROR: {:?}", e);
                    TypstResult::ExportError
                }
            }
        }
        Err(e) => {
            eprintln!("COMPILE ERROR: {:?}", e);
            TypstResult::CompileError
        }
    }
}

/// 释放 PDF 缓冲区
///
/// # 参数
/// - `buffer`: 要释放的缓冲区指针
#[unsafe(no_mangle)]
pub unsafe extern "C" fn typst_free_buffer(buffer: *mut TypstPdfBuffer) {
    if buffer.is_null() {
        return;
    }
    let buf = &*buffer;
    if !buf.data.is_null() && buf.capacity > 0 {
        // 重建 Vec 并让其自动释放
        let _ = Vec::from_raw_parts(buf.data, buf.len, buf.capacity);
    }
    (*buffer).data = ptr::null_mut();
    (*buffer).len = 0;
    (*buffer).capacity = 0;
}

/// 获取错误信息字符串
///
/// # 参数
/// - `result`: 编译结果状态码
///
/// # 返回
/// 错误描述字符串（静态字符串，无需释放）
#[unsafe(no_mangle)]
pub extern "C" fn typst_result_message(result: TypstResult) -> *const c_char {
    let msg = match result {
        TypstResult::Success => "Success\0",
        TypstResult::InvalidInputPath => "Invalid input path\0",
        TypstResult::InvalidOutputPath => "Invalid output path\0",
        TypstResult::ReadError => "Failed to read file\0",
        TypstResult::CompileError => "Compilation error\0",
        TypstResult::ExportError => "PDF export error\0",
        TypstResult::WriteError => "Failed to write file\0",
        TypstResult::InvalidUtf8 => "Invalid UTF-8 string\0",
        TypstResult::MemoryError => "Memory allocation error\0",
    };
    msg.as_ptr() as *const c_char
}

/// 获取库版本
///
/// # 返回
/// 版本字符串（静态字符串，无需释放）
#[unsafe(no_mangle)]
pub extern "C" fn typst_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

