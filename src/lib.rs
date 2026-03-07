//! Typst C FFI Bridge - 用于从 C 代码编译 Typst 文档到 PDF
//!
//! 提供简单的 C API 来编译 Typst 源文件或字符串到 PDF 文件或内存缓冲区。
#![allow(unsafe_op_in_unsafe_fn)]

pub mod ffi;
pub mod package;
pub mod world;

pub use ffi::*;
