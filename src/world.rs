use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use chrono::{DateTime, Datelike, Timelike, Utc};
use parking_lot::RwLock;
use typst::diag::{FileError, FileResult};
use typst::foundations::{Bytes, Datetime};
use typst::utils::LazyHash;
use typst::syntax::{FileId, Source, VirtualPath};
use typst::text::{Font, FontBook};
use typst::{Library, LibraryExt, World};

// ============================================================================
// ============================================================================

/// 简单的 Typst World 实现，用于编译
pub struct SimpleWorld {
    /// 主源文件
    main: Source,
    /// 字体书
    book: LazyHash<FontBook>,
    /// 字体列表
    fonts: Vec<typst_kit::fonts::FontSlot>,
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
    pub fn new(source: &str, root: Option<&Path>) -> Self {
        let root = root.map(|p| p.to_path_buf()).unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        });
        
        let main_id = FileId::new(None, VirtualPath::new("main.typ"));
        let main = Source::new(main_id, source.into());
        
        let mut searcher = typst_kit::fonts::Fonts::searcher();
        let fonts = searcher.search();
        
        Self {
            main,
            book: LazyHash::new(fonts.book),
            fonts: fonts.fonts,
            library: LazyHash::new(Library::default()),
            files: RwLock::new(HashMap::new()),
            root,
            now: OnceLock::new(),
        }
    }
}

impl SimpleWorld {
    fn resolve_path(&self, id: FileId) -> FileResult<PathBuf> {
        if let Some(spec) = id.package() {
            let package_dir = crate::package::download_package(spec)?;
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
        self.fonts.get(index).and_then(|slot| slot.get())
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
