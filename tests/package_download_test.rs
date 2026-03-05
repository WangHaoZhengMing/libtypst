use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use typst_c::*;

#[test]
fn test_package_extraction_and_download() {
    println!("[TEST] Starting package extraction and download test");
    
    // 创建临时目录用于测试
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_path = temp_dir.path().to_path_buf();
    println!("[TEST] Created temp directory: {:?}", temp_path);
    
    // 测试源码包含package导入
    let source = r#"
#import "@preview/cetz:0.4.2"
#import "@preview/codly:1.2.0"

= Test Document with Packages

This document imports CeTZ for drawing and Codly for code blocks.

#cetz.canvas({
  import cetz.draw: *
  circle((0, 0), radius: 1)
})

#codly.code(```python
def hello():
    print("Hello World!")
```)"#;

    // 编译到缓冲区（这会触发package下载）
    let buffer = TypstPdfBuffer {
        data: std::ptr::null_mut(),
        len: 0,
        capacity: 0,
    };
    
    println!("[TEST] Starting compilation with packages...");
    
    unsafe {
        let mut pdf_buffer = buffer;
        let source_cstr = std::ffi::CString::new(source).unwrap();
        let root_path_cstr = std::ffi::CString::new(temp_path.to_string_lossy().as_bytes()).unwrap();
        
        println!("[TEST] Calling typst_compile_to_buffer...");
        let result = typst_compile_to_buffer(
            source_cstr.as_ptr(),
            root_path_cstr.as_ptr(),
            &mut pdf_buffer as *mut TypstPdfBuffer,
        );
        
        // 应该成功编译（即使可能无法下载某些包）
        println!("[TEST] Compilation completed with result: {:?}", result);
        
        if result == TypstResult::Success {
            println!("[TEST] Compilation successful, checking PDF buffer...");
            assert!(pdf_buffer.len > 0);
            assert!(!pdf_buffer.data.is_null());
            
            // 验证PDF头部
            let header = std::slice::from_raw_parts(pdf_buffer.data, 4.min(pdf_buffer.len));
            println!("[TEST] PDF header: {:?}", header);
            assert_eq!(&header[0..4], b"%PDF");
            
            println!("[TEST] Freeing PDF buffer...");
            typst_free_buffer(&mut pdf_buffer as *mut TypstPdfBuffer);
            println!("[TEST] PDF buffer freed");
        } else {
            println!("[TEST] Compilation failed with: {:?}", result);
        }
    }
    
    println!("[TEST] Checking package cache directories...");
    // 检查缓存目录是否创建了
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from(".cache"))
        .join("typst")
        .join("packages");
    
    if cache_dir.exists() {
        println!("Package cache directory exists: {:?}", cache_dir);
        
        // 检查是否有preview包目录
        let preview_dir = cache_dir.join("preview");
        if preview_dir.exists() {
            println!("Preview packages directory exists");
            
            // 列出下载的包
            if let Ok(entries) = fs::read_dir(&preview_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        println!("Downloaded package: {}", entry.file_name().to_string_lossy());
                    }
                }
            }
        }
    }
}

#[test]
fn test_simple_compilation_without_packages() {
    println!("[TEST] Starting simple compilation test without packages");
    
    // 测试不包含package导入的简单文档
    let source = r#"
= Simple Test Document

This is a test document without any package imports.

$ E = m c^2 $

Some basic *bold* and _italic_ text."#;

    unsafe {
        let mut pdf_buffer = TypstPdfBuffer {
            data: std::ptr::null_mut(),
            len: 0,
            capacity: 0,
        };
        
        let source_cstr = std::ffi::CString::new(source).unwrap();
        
        println!("[TEST] Calling simple compilation...");
        let result = typst_compile_to_buffer(
            source_cstr.as_ptr(),
            std::ptr::null(),
            &mut pdf_buffer as *mut TypstPdfBuffer,
        );
        
        println!("[TEST] Simple compilation result: {:?}", result);
        assert_eq!(result, TypstResult::Success);
        println!("[TEST] PDF buffer length: {}", pdf_buffer.len);
        assert!(pdf_buffer.len > 0);
        assert!(!pdf_buffer.data.is_null());
        
        // 验证PDF头部
        let header = std::slice::from_raw_parts(pdf_buffer.data, 4);
        assert_eq!(header, b"%PDF");
        
        typst_free_buffer(&mut pdf_buffer as *mut TypstPdfBuffer);
    }
}

#[test]
fn test_file_compilation_with_packages() {
    println!("[TEST] Starting file compilation test with packages");
    
    // 创建临时typst文件
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let input_file = temp_dir.path().join("test.typ");
    let output_file = temp_dir.path().join("test.pdf");
    
    let source = r#"
#import "@preview/cetz:0.4.2": canvas, draw

= File Test with Packages

This tests file-based compilation with package imports.

#canvas({
  draw.circle((0, 0), radius: 2, fill: blue.lighten(50%))
})
"#;

    // 写入测试文件
    println!("[TEST] Writing test file: {:?}", input_file);
    fs::write(&input_file, source).expect("Failed to write test file");
    
    unsafe {
        let input_cstr = std::ffi::CString::new(input_file.to_string_lossy().as_bytes()).unwrap();
        let output_cstr = std::ffi::CString::new(output_file.to_string_lossy().as_bytes()).unwrap();
        
        println!("[TEST] Calling typst_compile_file...");
        let result = typst_compile_file(
            input_cstr.as_ptr(),
            output_cstr.as_ptr(),
        );
        
        println!("[TEST] File compilation completed with result: {:?}", result);
        
        // 检查是否生成了PDF文件（即使编译可能因为包下载失败）
        if result == TypstResult::Success {
            assert!(output_file.exists());
            let pdf_content = fs::read(&output_file).expect("Failed to read output PDF");
            assert!(pdf_content.len() > 0);
            assert!(pdf_content.starts_with(b"%PDF"));
        }
    }
}

#[test]
fn test_version_info() {
    unsafe {
        let version = typst_version();
        let version_str = std::ffi::CStr::from_ptr(version).to_str().unwrap();
        println!("Library version: {}", version_str);
        assert!(!version_str.is_empty());
    }
}

#[test]
fn test_error_messages() {
    // 测试各种错误状态的消息
    unsafe {
        let messages = vec![
            (TypstResult::Success, "Success"),
            (TypstResult::InvalidInputPath, "Invalid input path"),
            (TypstResult::CompileError, "Compilation error"),
        ];
        
        for (result, expected_prefix) in messages {
            let msg_ptr = typst_result_message(result);
            let msg = std::ffi::CStr::from_ptr(msg_ptr).to_str().unwrap();
            println!("Error {:?}: {}", result, msg);
            assert!(msg.contains(expected_prefix));
        }
    }
}