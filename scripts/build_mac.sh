#!/bin/bash

# Add required rust targets
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build both architecture versions (release mode)
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Ensure the output directory exists
mkdir -p universal-macos

# Use lipo to merge dylib and staticlib
lipo -create -output universal-macos/libtypst_c.dylib \
    target/x86_64-apple-darwin/release/libtypst_c.dylib \
    target/aarch64-apple-darwin/release/libtypst_c.dylib

lipo -create -output universal-macos/libtypst_c.a \
    target/x86_64-apple-darwin/release/libtypst_c.a \
    target/aarch64-apple-darwin/release/libtypst_c.a

echo "Universal binaries (x86_64 & aarch64) have been created in the universal-macos directory."
