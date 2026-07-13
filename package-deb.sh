#!/usr/bin/env bash

set -eu

# If you are a bash knower, either make this better or shhhhhhh
SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

DEB_BUILD_DIR="$SCRIPT_DIR/target/deb-build"
DEB_USR_BIN_DIR="$DEB_BUILD_DIR/usr/bin"
DEB_OPT_DIR="$DEB_BUILD_DIR/opt/hyperdeck-monitor"

rm -rf $DEB_BUILD_DIR
mkdir -p $DEB_BUILD_DIR
mkdir -p $DEB_USR_BIN_DIR
mkdir -p $DEB_OPT_DIR

NODE_PROCESS_PATH="/opt/hyperdeck-monitor/hyperdeck-bridge.js" cargo build --release --target x86_64-unknown-linux-musl

cp "$SCRIPT_DIR/target/x86_64-unknown-linux-musl/release/hyperdeck-monitor" "$DEB_USR_BIN_DIR/"
cp -r "$SCRIPT_DIR/deb-build/DEBIAN" "$DEB_BUILD_DIR/"
cp -r "$SCRIPT_DIR/deb-build/etc" "$DEB_BUILD_DIR/"
cp "$SCRIPT_DIR/package.json" "$DEB_OPT_DIR/"
cp "$SCRIPT_DIR/package-lock.json" "$DEB_OPT_DIR/"
cp "$SCRIPT_DIR/index.js" "$DEB_OPT_DIR/hyperdeck-bridge.js"
cp -r "$SCRIPT_DIR/node_modules" "$DEB_OPT_DIR/"
dpkg-deb --root-owner-group --build $DEB_BUILD_DIR "$SCRIPT_DIR/hyperdeck-monitor-amd64.deb"
