#!/bin/bash

echo "=== MA Blocks Memory Management Test ==="
echo ""

# Check if monitor is already running
if [ -f "debug_resources.pid" ]; then
    echo "⚠️  Resource monitor already running. Stopping it first..."
    ./debug_resources.sh stop
    sleep 1
fi

echo "🚀 Starting resource monitor..."
./debug_resources.sh &
MONITOR_PID=$!
sleep 2

echo "📊 Monitor started (PID: $MONITOR_PID)"
echo ""
echo "📋 Test Instructions:"
echo "1. Run: ./target/release/ma_blocks"
echo "2. Load several large animated AVIF/WebP images"
echo "3. Click on images to load animations on-demand"
echo "4. Watch memory usage - should stay under 512MB"
echo "5. Delete some blocks and check memory cleanup"
echo "6. Close the app and verify cleanup"
echo ""
echo "📈 Debug Info Available:"
echo "  - Real-time monitoring in this terminal"
echo "  - Detailed log: resource_monitor.log"
echo "  - App debug info in stderr (check terminal where app runs)"
echo ""
echo "🛑 To stop monitoring: ./debug_resources.sh stop"
echo "📄 To view log: cat resource_monitor.log"
echo ""
echo "Press any key to start monitoring..."
read -n 1

echo "✅ Monitoring active. Start the app now!"