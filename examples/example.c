/**
 * Typst C Bridge 使用示例
 *
 * 编译方法 (macOS):
 *   cd libtypst
 *   cargo build --release
  clang examples/example.c -o example -I include target/release/libtypst_c.a -framework Security -framework CoreFoundation
 *
 * 编译方法 (Linux):
 *   cargo build --release
 *   gcc examples/example.c -o example \
 *       -I include \
 *       -L target/release -ltypst_c \
 *       -lm -ldl -lpthread
 */

#include <stdio.h>
#include <string.h>
#include "typst_c.h"

int main(int argc, char *argv[]) {
    printf("Typst C Bridge Version: %s\n\n", typst_version());

    // 示例 1: 从字符串编译到文件
    printf("=== Example 1: Compile string to file ===\n");
    const char *source = 
        "#set page(paper: \"a4\")\n"
        "#set text(font: \"Arial\", size: 12pt)\n"
        "#import \"@preview/cetz:0.4.2\"\n"
        "\n"
        "= Hello from C with Packages!\n"
        "\n"
        "This PDF was generated using the Typt C bridge, downloading from Typst Universe.\n"
        "\n"
        "#align(center)[\n"
        "  #cetz.canvas({\n"
        "    import cetz.draw: *\n"
        "    circle((0,0), radius: 2, fill: blue.lighten(50%), stroke: blue)\n"
        "    content((0,0), [Typst C Bridge])\n"
        "  })\n"
        "]\n"
        "\n"
        "$ E = m c^2 $\n";

    TypstResult result = typst_compile_string(source, "output_string.pdf", NULL);
    if (result == Success) {
        printf("Successfully created output_string.pdf\n");
    } else {
        printf("Error: %s\n", typst_result_message(result));
    }

    // 示例 2: 从文件编译到文件
    printf("\n=== Example 2: Compile file to file ===\n");
    if (argc > 1) {
        char output_path[256];
        snprintf(output_path, sizeof(output_path), "%s.pdf", argv[1]);
        
        // 移除 .typ 扩展名如果存在
        char *ext = strstr(output_path, ".typ.pdf");
        if (ext) {
            strcpy(ext, ".pdf");
        }
        
        result = typst_compile_file(argv[1], output_path);
        if (result == Success) {
            printf("Successfully created %s\n", output_path);
        } else {
            printf("Error compiling %s: %s\n", argv[1], typst_result_message(result));
        }
    } else {
        printf("Usage: %s <input.typ>\n", argv[0]);
        printf("(Skipping file compilation example)\n");
    }

    // 示例 3: 编译到内存缓冲区
    printf("\n=== Example 3: Compile to memory buffer ===\n");
    TypstPdfBuffer buffer = {0};
    result = typst_compile_to_buffer(source, NULL, &buffer);
    if (result == Success) {
        printf("Successfully compiled to buffer\n");
        printf("PDF size: %zu bytes\n", buffer.len);
        
        // 验证 PDF 头
        if (buffer.len >= 4 && 
            buffer.data[0] == '%' && 
            buffer.data[1] == 'P' && 
            buffer.data[2] == 'D' && 
            buffer.data[3] == 'F') {
            printf("Valid PDF header detected\n");
        }
        
        // 写入文件演示
        FILE *f = fopen("output_buffer.pdf", "wb");
        if (f) {
            fwrite(buffer.data, 1, buffer.len, f);
            fclose(f);
            printf("Saved buffer to output_buffer.pdf\n");
        }
        
        // 释放缓冲区
        typst_free_buffer(&buffer);
        printf("Buffer freed\n");
    } else {
        printf("Error: %s\n", typst_result_message(result));
    }

    printf("\nDone!\n");
    return 0;
}
