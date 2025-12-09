#!/bin/bash

echo "=== Animation Loading Debug Test ==="
echo ""

# Start debug monitoring
echo "🔍 Starting debug monitoring..."
./debug_resources.sh &
MONITOR_PID=$!
sleep 2

echo "📊 Monitor started (PID: $MONITOR_PID)"
echo ""
echo "🎯 Test Steps:"
echo "1. Run: ./target/release/ma_blocks"
echo "2. Load an animated AVIF or WebP image"
echo "3. Click on the image to trigger animation loading"
echo "4. Watch the debug output in both terminals:"
echo ""
echo "   🎬 Animation load requested"
echo "   📁 Starting animation load"
echo "   🎞️ Decoding animation with format"
echo "   ✅ AVIF/GIF/WebP decoded: X frames"
echo "   🎬 AnimationLoaded received"
echo "   🖼️ Converting X frames to textures"
echo "   🎨 Rendering: block_id, frames, current_idx"
echo ""
echo "🚨 If you see '⚠️ No texture available' - that's the white block issue!"
echo ""
echo "🛑 To stop: ./debug_resources.sh stop"
echo ""

echo "Press any key to continue..."
read -n 1

echo "✅ Ready! Start the app and test animation loading."