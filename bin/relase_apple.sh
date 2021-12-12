#!/usr/bin/env bash

set -euo pipefail

cargo build --release --target=x86_64-apple-darwin
cargo build --release --target=aarch64-apple-darwin
mkdir -p target/release/apple-darwin
lipo -create -output \
     target/release/apple-darwin/rs-git-fsmonitor \
     target/aarch64-apple-darwin/release/rs-git-fsmonitor \
     target/x86_64-apple-darwin/release/rs-git-fsmonitor
