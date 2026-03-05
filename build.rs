// build.rs - 生成 C 头文件
use std::env;
use std::path::PathBuf;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_path = PathBuf::from(&crate_dir).join("include");
    
    // 创建 include 目录
    std::fs::create_dir_all(&out_path).ok();
    
    // 生成 C 头文件
    let config = cbindgen::Config {
        language: cbindgen::Language::C,
        include_guard: Some("TYPST_C_H".to_string()),
        documentation: true,
        documentation_style: cbindgen::DocumentationStyle::C99,
        ..Default::default()
    };
    
    cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_config(config)
        .generate()
        .expect("Unable to generate bindings")
        .write_to_file(out_path.join("typst_c.h"));
    
    println!("cargo:rerun-if-changed=src/lib.rs");
}
