#ifndef TYPST_C_H
#define TYPST_C_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

// 编译结果状态码
typedef enum TypstResult {
  // 编译成功
  Success = 0,
  // 输入文件路径无效
  InvalidInputPath = 1,
  // 输出文件路径无效
  InvalidOutputPath = 2,
  // 读取文件失败
  ReadError = 3,
  // 编译错误
  CompileError = 4,
  // PDF 导出错误
  ExportError = 5,
  // 写入文件失败
  WriteError = 6,
  // 无效的 UTF-8 字符串
  InvalidUtf8 = 7,
  // 内存分配失败
  MemoryError = 8,
} TypstResult;

// PDF 输出缓冲区
typedef struct TypstPdfBuffer {
  // 指向 PDF 数据的指针
  uint8_t *data;
  // PDF 数据长度
  uintptr_t len;
  // 容量（内部使用）
  uintptr_t capacity;
} TypstPdfBuffer;

// 从文件编译 Typst 到 PDF 文件
//
// # 参数
// - `input_path`: 输入 Typst 文件路径 (UTF-8 C 字符串)
// - `output_path`: 输出 PDF 文件路径 (UTF-8 C 字符串)
//
// # 返回
// `TypstResult` 状态码
enum TypstResult typst_compile_file(const char *input_path, const char *output_path);

// 从字符串编译 Typst 到 PDF 文件
//
// # 参数
// - `source`: Typst 源代码 (UTF-8 C 字符串)
// - `output_path`: 输出 PDF 文件路径 (UTF-8 C 字符串)
// - `root_path`: 根目录路径，用于解析相对路径（可为 NULL）
//
// # 返回
// `TypstResult` 状态码
enum TypstResult typst_compile_string(const char *source,
                                      const char *output_path,
                                      const char *root_path);

// 从字符串编译 Typst 到内存缓冲区
//
// # 参数
// - `source`: Typst 源代码 (UTF-8 C 字符串)
// - `root_path`: 根目录路径（可为 NULL）
// - `out_buffer`: 输出缓冲区指针
//
// # 返回
// `TypstResult` 状态码
//
// # 注意
// 成功后必须调用 `typst_free_buffer` 释放内存
enum TypstResult typst_compile_to_buffer(const char *source,
                                         const char *root_path,
                                         struct TypstPdfBuffer *out_buffer);

// 释放 PDF 缓冲区
//
// # 参数
// - `buffer`: 要释放的缓冲区指针
void typst_free_buffer(struct TypstPdfBuffer *buffer);

// 获取错误信息字符串
//
// # 参数
// - `result`: 编译结果状态码
//
// # 返回
// 错误描述字符串（静态字符串，无需释放）
const char *typst_result_message(enum TypstResult result);

// 获取库版本
//
// # 返回
// 版本字符串（静态字符串，无需释放）
const char *typst_version(void);

#endif  /* TYPST_C_H */
