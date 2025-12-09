#!/bin/bash

# macOS Packaging Script for MA Blocks
set -e

echo "🍎 Building MA Blocks for macOS..."

# Clean previous builds
echo "🧹 Cleaning previous builds..."
cargo clean

# Build release version
echo "🔨 Building release binary..."
cargo build --release

# Create macOS app bundle
echo "📦 Creating macOS app bundle..."
cargo bundle --format=osx

# Check if bundle was created
if [ -d "target/release/bundle/osx/MA Blocks.app" ]; then
    echo "✅ macOS app bundle created successfully!"
    echo "📍 Location: target/release/bundle/osx/MA Blocks.app"
    
    # Create DMG if requested
    if [ "$1" = "--dmg" ]; then
        echo "💿 Creating DMG installer..."
        DMG_NAME="MA_Blocks_${CARGO_PKG_VERSION:-0.1.0}"
        DMG_PATH="target/release/$DMG_NAME.dmg"
        
        # Create temporary directory for DMG
        TEMP_DIR="target/release/dmg_temp"
        mkdir -p "$TEMP_DIR"
        
        # Copy app bundle to temp directory
        cp -R "target/release/bundle/osx/MA Blocks.app" "$TEMP_DIR/"
        
        # Create DMG
        hdiutil create -volname "MA Blocks" -srcfolder "$TEMP_DIR" -ov -format UDZO "$DMG_PATH"
        
        # Clean up temp directory
        rm -rf "$TEMP_DIR"
        
        echo "✅ DMG created: $DMG_PATH"
    fi
else
    echo "❌ Failed to create macOS app bundle"
    exit 1
fi