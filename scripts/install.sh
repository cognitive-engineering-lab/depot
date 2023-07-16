#!/bin/sh
set -e

BASE_URL="https://github.com/cognitive-engineering-lab/depot/releases/latest/download"
INSTALL_DIR=$HOME/.local/bin

download() {
  pushd $(mktemp -d)
  
  wget "${BASE_URL}/$1.tar.gz"
  tar -xf $1.tar.gz

  mkdir -p $INSTALL_DIR
  mv depot $INSTALL_DIR/depot

  popd
}

ARCH=$(uname -m)

pick_target() {
  echo "Selecting target for $ARCH / $OSTYPE..."
  
  if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    if [[ "$ARCH" == "x86_64" ]]; then
      download "x86_64-unknown-linux-gnu"
      return
    fi
  elif [[ "$OSTYPE" == "darwin"* ]]; then
    if [[ "$ARCH" == "arm64" ]]; then
      download "aarch64-apple-darwin"
      return
    elif [[ "$ARCH" == "x86_64" ]]; then
      download "x86_64-apple-darwin"
      return
    fi
  fi

  echo "Prebuilt binary not available, installing from source. This may take a few minutes."
  cargo install depot-js
}

pick_target

PATH=$PATH:$INSTALL_DIR
depot setup