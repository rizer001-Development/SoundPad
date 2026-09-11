#!/bin/bash
cd "$(dirname "$0")/.."

if [ -x "target/release/soundpad" ]; then
    exec "./target/release/soundpad"
fi

echo "Release binary not found. Building..."
cargo build --release || exit 1
exec "./target/release/soundpad"
