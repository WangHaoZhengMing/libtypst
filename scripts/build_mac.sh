#!/bin/bash

# Parse command line arguments
PROFILE="release"
while [[ $# -gt 0 ]]; do
  case $1 in
    --profile)
      PROFILE="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

echo "Building with profile: $PROFILE"

# Add required rust targets
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build both architecture versions with specified profile
echo "Building x86_64 architecture..."
cargo build --profile $PROFILE --target x86_64-apple-darwin
echo "Building aarch64 architecture..."
cargo build --profile $PROFILE --target aarch64-apple-darwin

# Ensure the output directory exists
mkdir -p universal-macos

# Determine the output directory based on profile
if [ "$PROFILE" = "release-opt" ]; then
    OUTPUT_DIR="release-opt"
else
    OUTPUT_DIR="$PROFILE"
fi

# Use lipo to merge dylib and staticlib
echo "Creating universal dylib..."
lipo -create -output universal-macos/libtypst_c.dylib \
    target/x86_64-apple-darwin/$OUTPUT_DIR/libtypst_c.dylib \
    target/aarch64-apple-darwin/$OUTPUT_DIR/libtypst_c.dylib

echo "Creating universal static library..."
lipo -create -output universal-macos/libtypst_c.a \
    target/x86_64-apple-darwin/$OUTPUT_DIR/libtypst_c.a \
    target/aarch64-apple-darwin/$OUTPUT_DIR/libtypst_c.a

echo "Universal binaries (x86_64 & aarch64) have been created in the universal-macos directory."

# Show binary sizes
echo "=== Binary Size Information ==="
echo "Dynamic library:"
ls -lh universal-macos/libtypst_c.dylib
echo "Static library:"
ls -lh universal-macos/libtypst_c.a
echo "================================"
