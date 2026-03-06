# libtypst

**A C FFI library for the Typst typesetting system.**

`libtypst` provides a C API bridge to [Typst](https://typst.app/), enabling you to embed the full Typst compiler in C, C++, Swift, Python, Go, and any language that supports C FFI.

---

## Features

- ✅ **Complete C API** - Compile Typst documents from files or strings
- 🔠 **System Font Support** - Automatically loads system fonts and Typst's embedded fonts
- 📦 **Package Manager** - Automatically downloads and caches Typst packages from `@preview` namespace (e.g., `#import "@preview/cetz:0.4.2"`)
- 📄 **Multiple Output Modes** - Compile to PDF files or in-memory buffers
- 🦀 **Written in Rust** - Built on the official Typst Rust crates (v0.14.2)

---

## Quick Start

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (cargo)

### Build the Library

```bash
cargo build --release
```

This produces:
- **Static library**: `target/release/libtypst_c.a`
- **Dynamic library**: `target/release/libtypst_c.so` (Linux) or `target/release/libtypst_c.dylib` (macOS)
- **C header**: `include/typst_c.h` (auto-generated via cbindgen)

### macOS Universal Binary (Optional)

To build a universal binary for both Intel and Apple Silicon:

```bash
./scripts/build_mac.sh
```

Output will be in `universal-macos/` directory.

---

## Usage Example

### Compile and Run the Example

**Linux:**
```bash
# Build the library
cargo build --release

# Compile the example
gcc examples/example.c -o example \
  -I include \
  -L target/release -ltypst_c \
  -lm -ldl -lpthread

# Run (make sure the dynamic library is in the library path)
LD_LIBRARY_PATH=target/release ./example
```

**macOS:**
```bash
# Build the library
cargo build --release

# Compile the example (using static library)
clang examples/example.c -o example \
  -I include \
  target/release/libtypst_c.a \
  -framework Security -framework CoreFoundation

# Run
./example
```

### Simple C Code

```c
#include <stdio.h>
#include "typst_c.h"

int main() {
    const char *source = 
        "= Hello from C!\n"
        "This is a *Typst* document.\n"
        "$ E = m c^2 $";
    
    TypstResult result = typst_compile_string(source, "output.pdf", NULL);
    
    if (result == Success) {
        printf("PDF created successfully!\n");
    } else {
        printf("Error: %s\n", typst_result_message(result));
    }
    
    return 0;
}
```

---

## C API Reference

### Functions

| Function | Description |
|----------|-------------|
| `typst_compile_file()` | Compile a `.typ` file to PDF |
| `typst_compile_string()` | Compile a Typst string to PDF file |
| `typst_compile_to_buffer()` | Compile a Typst string to an in-memory PDF buffer |
| `typst_free_buffer()` | Free a PDF buffer allocated by `typst_compile_to_buffer()` |
| `typst_result_message()` | Get a human-readable error message for a result code |
| `typst_version()` | Get the library version string |

### Return Values

All compilation functions return `TypstResult`:

```c
typedef enum {
    Success = 0,
    InvalidInputPath = 1,
    InvalidOutputPath = 2,
    ReadError = 3,
    CompileError = 4,
    ExportError = 5,
    WriteError = 6,
    InvalidUtf8 = 7,
    MemoryError = 8,
} TypstResult;
```

See `include/typst_c.h` for the complete API documentation.

---

## Repository Structure

```
.
├── Cargo.toml          # Rust project configuration
├── build.rs            # Build script (generates C header via cbindgen)
├── src/
│   └── lib.rs          # Rust FFI implementation
├── include/
│   └── typst_c.h       # C header (auto-generated)
├── examples/
│   └── example.c       # Complete C usage example
├── tests/
│   └── *.rs            # Rust integration tests
└── scripts/
    └── build_mac.sh    # macOS universal binary build script
```

---

## Testing

Run the Rust test suite:

```bash
cargo test
```

The tests verify:
- Simple document compilation
- Package downloading and caching
- File-based and string-based compilation
- Memory buffer output
- Error handling

---

## License

Apache License 2.0 (same as [Typst](https://github.com/typst/typst))

---

## Credits

Built on the official [Typst](https://github.com/typst/typst) compiler (v0.14.2).
