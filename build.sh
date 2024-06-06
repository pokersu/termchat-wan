#!/bin/bash

cargo build --bin=app-arm64 --release --target=aarch64-unknown-linux-musl
cargo build --bin=app-lin --release --target=x86_64-unknown-linux-musl
cargo build --bin=app-darwin --release --target=x86_64-apple-darwin
cargo build --bin=app-win --release --target=x86_64-pc-windows-gnu
cargo build --bin=server --release --target=x86_64-unknown-linux-musl