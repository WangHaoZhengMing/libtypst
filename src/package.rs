use std::path::PathBuf;

use typst::diag::{FileError, FileResult};
use typst::syntax::package::PackageSpec;
use typst_kit::download::{Downloader, ProgressSink};
use typst_kit::package::PackageStorage;

/// 下载并解压包 (使用 typst-kit 的内部实现，包含更好的缓存、并发控制及速率限制处理)
pub fn download_package(spec: &PackageSpec) -> FileResult<PathBuf> {
    let downloader = Downloader::new("libtypst-c/0.1.0");
    let storage = PackageStorage::new(None, None, downloader);
    
    storage
        .prepare_package(spec, &mut ProgressSink)
        .map_err(|e| FileError::Package(e))
}
