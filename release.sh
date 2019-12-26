#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'

VERSION=$(grep "^version" Cargo.toml | awk -F'=' '{print $NF}' | sed 's/[ "]//g')

cargo build --release
cargo build --release --target x86_64-pc-windows-gnu
tar zcvf "target/release/gitlab-push-and-mr.${VERSION}.tar.gz" \
  "target/release/gitlab-push-and-mr"
zip "target/x86_64-pc-windows-gnu/release/gitlab-push-and-mr.${VERSION}.zip" \
  "target/x86_64-pc-windows-gnu/release/gitlab-push-and-mr.exe"
