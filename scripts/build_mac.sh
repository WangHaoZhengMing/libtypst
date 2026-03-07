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

# Detect current architecture
ARCH=$(uname -m)
if [ "$ARCH" = "x86_64" ]; then
    TARGET="x86_64-apple-darwin"
elif [ "$ARCH" = "arm64" ]; then
    TARGET="aarch64-apple-darwin"
else
    echo "Unsupported architecture: $ARCH"
    exit 1
fi

echo "Detected architecture: $ARCH"
echo "Target: $TARGET"

# Add required rust target
rustup target add $TARGET

# Build for current architecture
echo "Building for $TARGET..."
cargo build --profile $PROFILE --target $TARGET

# Ensure the output directory exists
mkdir -p universal-macos

# Determine the output directory based on profile
if [ "$PROFILE" = "release-opt" ]; then
    OUTPUT_DIR="release-opt"
else
    OUTPUT_DIR="$PROFILE"
fi

# Copy artifacts to output directory
echo "Copying artifacts to universal-macos directory..."
cp target/$TARGET/$OUTPUT_DIR/libtypst_c.dylib universal-macos/libtypst_c.dylib
cp target/$TARGET/$OUTPUT_DIR/libtypst_c.a universal-macos/libtypst_c.a

echo "Binaries for $ARCH have been created in the universal-macos directory."

# Show binary sizes
echo "=== Binary Size Information ==="
echo "Dynamic library:"
ls -lh universal-macos/libtypst_c.dylib
echo "Static library:"
ls -lh universal-macos/libtypst_c.a
echo "================================"
