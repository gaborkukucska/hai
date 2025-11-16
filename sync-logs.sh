#!/bin/bash
# Sync system service logs to workspace logs directory
# This script creates symlinks for system service logs in the workspace logs folder

WORKSPACE_LOGS="/home/tom/hai/logs"
SYSTEM_LOGS="/var/log/hainet/logs"

# Create symlinks for all hainet-chain logs
for log in "$SYSTEM_LOGS"/hainet-chain-*.log; do
    if [ -f "$log" ]; then
        ln -sf "$log" "$WORKSPACE_LOGS/"
    fi
done

# Create symlinks for all hainet-core logs
for log in "$SYSTEM_LOGS"/hainet-core-*.log; do
    if [ -f "$log" ]; then
        ln -sf "$log" "$WORKSPACE_LOGS/"
    fi
done

echo "Log symlinks updated in $WORKSPACE_LOGS"
