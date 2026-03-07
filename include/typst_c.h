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

typedef bool (*TypstDownloadFunc)(const char *url, const char *dest_path);

// PDF 输出缓冲区
typedef struct TypstPdfBuffer {
  // 指向 PDF 数据的指针
  uint8_t *data;
  // PDF 数据长度
  uintptr_t len;
  // 容量（内部使用）
  uintptr_t capacity;
} TypstPdfBuffer;

void typst_set_network_downloader(TypstDownloadFunc cb);

void typst_set_shared_group_path(const char *path);

// 从文件编译 Typst 到 PDF 文件
enum TypstResult typst_compile_file(const char *input_path, const char *output_path);

// 从字符串编译 Typst 到 PDF 文件
enum TypstResult typst_compile_string(const char *source,
                                      const char *output_path,
                                      const char *root_path);

// 从字符串编译 Typst 到内存缓冲区
enum TypstResult typst_compile_to_buffer(const char *source,
                                         const char *root_path,
                                         struct TypstPdfBuffer *out_buffer);

// 释放 PDF 缓冲区
void typst_free_buffer(struct TypstPdfBuffer *buffer);

// 获取错误信息字符串
const char *typst_result_message(enum TypstResult result);

// 获取库版本
const char *typst_version(void);

#endif  /* TYPST_C_H */
