use std::path::PathBuf;

use typst::diag::{FileError, FileResult};
use typst::syntax::package::PackageSpec;
use typst_kit::download::{Downloader, ProgressSink};
use typst_kit::package::PackageStorage;
use std::ffi::CString;

fn get_packages_base_dir() -> PathBuf {
    // 1. 优先尝试从宿主传递的 App Group 共享目录取包 (如果有设置)
    unsafe {
        if let Some(ref group_path) = crate::ffi::SHARED_GROUP_PATH {
            return PathBuf::from(group_path).join("typst_packages");
        }
    }
    
    // 2. 尝试从本地的主 Typst 缓存目录取包 (如果宿主授予了该目录的临时访问权限)
    if let Some(home) = dirs::data_local_dir() {
        let typst_local = home.join("typst").join("packages");
        if typst_local.exists() {
            return typst_local;
        }
    }
    
    // 3. 回退到以前的临时目录
    std::env::temp_dir().join("typst_packages")
}

/// 下载并解压包 (使用 typst-kit 的内部实现，包含更好的缓存、并发控制及速率限制处理)
pub fn download_package(spec: &PackageSpec) -> FileResult<PathBuf> {
    let temp_base = get_packages_base_dir();
    
    // 我们按照官方或者我们约定的格式寻找 {namespace}/{name}/{version}
    let mut package_dir = temp_base.join(spec.namespace.as_str()).join(spec.name.as_str()).join(spec.version.to_string());
    
    // 有时系统存储包的路径有多个版本。我们可以先探测缓存，如果有直接返回成功。
    if package_dir.exists() && package_dir.is_dir() {
        return Ok(package_dir);
    }
    // 也能兼容另外一种不包含 namespace 的存储格式的检测
    let alt_dir = temp_base.join(spec.name.as_str()).join(spec.version.to_string());
    if alt_dir.exists() && alt_dir.is_dir() {
         return Ok(alt_dir);
    }
    
    // 如果设置了基于 Swift(NSURLSession) 的外部网络下载回调，优先由宿主下载 tar.gz 包以突破 mDNS Sandbox 限制
    if let Some(cb) = unsafe { crate::ffi::DOWNLOAD_CALLBACK } {
        let url = format!("https://packages.typst.org/{}/{}-{}.tar.gz", spec.namespace, spec.name, spec.version);
        let tar_gz_path = temp_base.join(format!("{}-{}.tar.gz", spec.name, spec.version));
        
        // 尝试新建目录避免不存在
        let _ = std::fs::create_dir_all(&temp_base);
        
        if let (Ok(url_c), Ok(tar_gz_c)) = (CString::new(url), CString::new(tar_gz_path.to_string_lossy().to_string())) {
            // 调用 Swift 进行同步下载
            if cb(url_c.as_ptr(), tar_gz_c.as_ptr()) {
                if let Ok(file) = std::fs::File::open(&tar_gz_path) {
                    let decompressed = flate2::read::GzDecoder::new(file);
                    let mut archive = tar::Archive::new(decompressed);
                    
                    if std::fs::create_dir_all(&package_dir).is_ok() {
                        if archive.unpack(&package_dir).is_ok() {
                            let _ = std::fs::remove_file(&tar_gz_path);
                            return Ok(package_dir);
                        }
                    }
                }
            }
        }
    }

    // fallback: 如果一切由于沙盒或网络被禁止，那么这一步依然会用 typst-kit 的内置去跑
    let downloader = Downloader::new("libtypst-c/0.1.0");
    let cache_dir = temp_base.join("cache");
    let data_dir = temp_base.join("data");
    
    let storage = PackageStorage::new(Some(cache_dir.clone()), Some(data_dir.clone()), downloader);
    
    storage
        .prepare_package(spec, &mut ProgressSink)
        .map_err(|e| FileError::Package(e))
}
