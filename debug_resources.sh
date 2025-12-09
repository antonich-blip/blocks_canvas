#!/bin/bash

# Debug script to monitor resource usage during image processing
# Run this while testing the app with AVIF/WebP images

LOG_FILE="resource_monitor.log"
PID_FILE="debug_resources.pid"

# Cleanup function
cleanup() {
    echo ""
    echo "Stopping resource monitor..."
    if [ -f "$PID_FILE" ]; then
        rm -f "$PID_FILE"
    fi
    echo "Monitor stopped. Log saved to: $LOG_FILE"
    exit 0
}

# Set up signal handlers
trap cleanup SIGINT SIGTERM

# Write PID for control
echo $$ > "$PID_FILE"

echo "=== Resource Monitor for MA Blocks ==="
echo "Monitoring disk space and memory usage..."
echo "Controls:"
echo "  - Press Ctrl+C to stop monitoring"
echo "  - Run './debug_resources.sh stop' to stop from another terminal"
echo "  - Log file: $LOG_FILE"
echo ""

# Handle stop command
if [ "$1" = "stop" ]; then
    if [ -f "$PID_FILE" ]; then
        MONITOR_PID=$(cat "$PID_FILE")
        if kill -0 "$MONITOR_PID" 2>/dev/null; then
            kill "$MONITOR_PID"
            echo "Sent stop signal to monitor (PID: $MONITOR_PID)"
        else
            echo "Monitor process not found"
        fi
        rm -f "$PID_FILE"
    else
        echo "No monitor process found"
    fi
    exit 0
fi

# Initialize log
echo "Timestamp,Memory_RSS_MB,Swap_MB,Disk_Free_MB,Texture_Estimate_MB,Block_Count" > $LOG_FILE

while true; do
    # Get memory usage of ma_blocks process
    PID=$(pgrep -f "target/release/ma_blocks\|target/debug/ma_blocks" | head -1)
    if [ -n "$PID" ]; then
        MEMORY=$(ps -p $PID -o rss= | tr -d ' ')
        SWAP=$(ps -p $PID -o swap= | tr -d ' ')
        MEMORY_MB=$((MEMORY / 1024))
        SWAP_MB=$((SWAP / 1024))
        
        # Try to get texture memory estimate from app logs (if available)
        TEXTURE_MB="N/A"
        BLOCK_COUNT="N/A"
    else
        MEMORY_MB="N/A"
        SWAP_MB="N/A"
        TEXTURE_MB="N/A"
        BLOCK_COUNT="N/A"
    fi
    
    # Get disk free space in current directory
    DISK_FREE=$(df . | tail -1 | awk '{print $4}')
    DISK_FREE_MB=$((DISK_FREE / 1024))
    
    # Log and display
    TIMESTAMP=$(date '+%H:%M:%S')
    echo "$TIMESTAMP,$MEMORY_MB,$SWAP_MB,$DISK_FREE_MB,$TEXTURE_MB,$BLOCK_COUNT" >> $LOG_FILE
    
    # Display with color coding
    if [ "$MEMORY_MB" != "N/A" ] && [ "$MEMORY_MB" -gt 500 ]; then
        echo -e "\033[31m[$TIMESTAMP] Memory: ${MEMORY_MB}MB, Swap: ${SWAP_MB}MB, Disk: ${DISK_FREE_MB}MB\033[0m"
    elif [ "$MEMORY_MB" != "N/A" ] && [ "$MEMORY_MB" -gt 300 ]; then
        echo -e "\033[33m[$TIMESTAMP] Memory: ${MEMORY_MB}MB, Swap: ${SWAP_MB}MB, Disk: ${DISK_FREE_MB}MB\033[0m"
    else
        echo "[$TIMESTAMP] Memory: ${MEMORY_MB}MB, Swap: ${SWAP_MB}MB, Disk: ${DISK_FREE_MB}MB"
    fi
    
    sleep 2
done