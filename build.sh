#!/usr/bin/zsh

# need to remove uutils cp from the path because it doesn't handle --parents
# and --archive correctly
path=("${(@)path:#"$HOME/.local/bin"}")
export PATH
yarn tauri build
