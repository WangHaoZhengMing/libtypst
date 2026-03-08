use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use chrono::{DateTime, Datelike, Timelike, Utc};
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
    /// 主源文本
    main_text: String,
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
            main_text: source.to_string(),
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

    fn is_sandbox_denied(error: &std::io::Error) -> bool {
        matches!(error.kind(), ErrorKind::PermissionDenied)
    }

    fn placeholder_source(&self, path: &Path) -> String {
        if path.extension().and_then(|ext| ext.to_str()) == Some("typ") {
            if let Some(module_stub) = self.placeholder_module_source(path) {
                return module_stub;
            }
        }

        crate::placeholder::placeholder_source_comment(path)
    }

    fn placeholder_module_source(&self, path: &Path) -> Option<String> {
        let import_suffixes = self.import_path_suffixes(path);
        let import_line = self
            .main_text
            .lines()
            .map(str::trim)
            .find(|line| {
                line.starts_with("#import ")
                    && import_suffixes.iter().any(|suffix| line.contains(&format!("\"{}\"", suffix)))
            })?;

        let exports = if let Some((_, clause)) = import_line.split_once(':') {
            let clause = clause.trim();
            if clause == "*" {
                self.infer_star_import_exports()
            } else {
                self.parse_named_imports(clause)
            }
        } else {
            Vec::new()
        };

        if exports.is_empty() {
            return None;
        }

        let mut stub = format!(
            "// Sandbox blocked access to external Typst module: {}\n",
            path.display()
        );

        for (name, is_function) in exports {
            if is_function {
                stub.push_str(&format!("#let {}(..args) = []\n", name));
            } else {
                stub.push_str(&format!("#let {} = []\n", name));
            }
        }

        Some(stub)
    }

    fn import_path_suffixes(&self, path: &Path) -> Vec<String> {
        let mut suffixes = Vec::new();

        if let Ok(relative) = path.strip_prefix(&self.root) {
            suffixes.push(relative.to_string_lossy().replace('\\', "/"));
        }

        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            let name = name.to_string();
            if !suffixes.contains(&name) {
                suffixes.push(name);
            }
        }

        suffixes
    }

    fn parse_named_imports(&self, clause: &str) -> Vec<(String, bool)> {
        clause
            .split(',')
            .filter_map(|part| {
                let binding = part.trim();
                if binding.is_empty() || binding == "*" {
                    return None;
                }

                let imported_name = binding
                    .split_once(" as ")
                    .map(|(_, alias)| alias.trim())
                    .unwrap_or(binding);

                if imported_name.is_empty() {
                    return None;
                }

                Some((
                    imported_name.to_string(),
                    self.main_text.contains(&format!("#{}(", imported_name)),
                ))
            })
            .collect()
    }

    fn infer_star_import_exports(&self) -> Vec<(String, bool)> {
        let local_defs = self.local_definitions();
        let chars: Vec<char> = self.main_text.chars().collect();
        let mut exports = Vec::new();
        let mut index = 0;

        while index < chars.len() {
            if chars[index] != '#' {
                index += 1;
                continue;
            }

            let mut cursor = index + 1;
            while cursor < chars.len() && chars[cursor].is_whitespace() {
                cursor += 1;
            }

            if cursor >= chars.len() || !Self::is_ident_start(chars[cursor]) {
                index += 1;
                continue;
            }

            let start = cursor;
            cursor += 1;
            while cursor < chars.len() && Self::is_ident_continue(chars[cursor]) {
                cursor += 1;
            }

            let name: String = chars[start..cursor].iter().collect();
            let is_custom = name.chars().any(|ch| !ch.is_ascii());
            if !is_custom || local_defs.iter().any(|defined| defined == &name) {
                index = cursor;
                continue;
            }

            let mut lookahead = cursor;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            let is_function = lookahead < chars.len() && chars[lookahead] == '(';

            if !exports.iter().any(|(existing, _)| existing == &name) {
                exports.push((name, is_function));
            }

            index = cursor;
        }

        exports
    }

    fn local_definitions(&self) -> Vec<String> {
        self.main_text
            .lines()
            .map(str::trim)
            .filter_map(|line| {
                let rest = line.strip_prefix("#let ")?;
                let mut end = 0;
                for ch in rest.chars() {
                    if Self::is_ident_continue(ch) {
                        end += ch.len_utf8();
                    } else {
                        break;
                    }
                }

                if end == 0 {
                    None
                } else {
                    Some(rest[..end].to_string())
                }
            })
            .collect()
    }

    fn is_ident_start(ch: char) -> bool {
        ch == '_' || ch.is_alphabetic()
    }

    fn is_ident_continue(ch: char) -> bool {
        ch == '_' || ch == '-' || ch.is_alphanumeric()
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
        if let Ok(files) = self.files.read() {
            if let Some(source) = files.get(&id) {
                return Ok(source.clone());
            }
        }

        // 从文件系统读取
        let path = self.resolve_path(id)?;

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(error) if Self::is_sandbox_denied(&error) => {
                eprintln!("SANDBOX FALLBACK source: {}", path.display());
                self.placeholder_source(&path)
            }
            Err(error) => return Err(FileError::from_io(error, &path)),
        };

        let source = Source::new(id, content.into());
        if let Ok(mut files) = self.files.write() {
            files.insert(id, source.clone());
        }
        Ok(source)
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let path = self.resolve_path(id)?;
        
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(error) if Self::is_sandbox_denied(&error) => {
                eprintln!("SANDBOX FALLBACK file: {}", path.display());
                crate::placeholder::placeholder_bytes(&path)
            }
            Err(error) => return Err(FileError::from_io(error, &path)),
        };
        
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
