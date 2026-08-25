#!/bin/bash
set -e

cd "$(git rev-parse --show-toplevel)"
args=(
    --exe ~/win/rs/deploy/archive/win2k/mfc42.dll
    --out out/mfc42
    --exports
)
cargo run -p tc -- "${args[@]}"
