#!/bin/sh
set -eu
cd "$(dirname "$0")"
cargo build --locked --release -j 2
mkdir -p target/package/m8s
cp target/release/m8s-plugin target/package/m8s/m8s
cp plugin.toml README.md target/package/m8s/
target/release/m8s-plugin --print-package-manifest > target/package/m8s/plugin-manifest.json
tar -C target/package -czf target/m8s-0.1.0.tar.gz m8s
printf '%s\n' 'Built plugin/target/m8s-0.1.0.tar.gz'
