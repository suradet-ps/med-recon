#!/usr/bin/env sh
# wasm-opt.sh - post-process the trunk WASM output with binaryen for
# maximum size reduction.
#
# Why this exists: wasm-bindgen emits bulk-memory (memory.copy/fill) and
# nontrapping-float-to-int instructions into the wasm. The wasm-opt binary
# rejects those unless the matching --enable-* features are passed, and
# trunk does not pass them - so trunk's built-in wasm-opt step silently
# fails on modern rustc output. We disable trunk's step (data-wasm-opt="0"
# in frontend/index.html) and run wasm-opt ourselves here with the
# features enabled.
#
# If wasm-opt is not on PATH (CI runners do not install binaryen), a
# pinned binaryen release is downloaded into
# ${XDG_CACHE_HOME:-$HOME/.cache}/med-recon-wasm-opt and used from there,
# so this script works on any machine with curl + tar.
#
# Usage:
#   sh script/wasm-opt.sh          # from the repo root (tauri beforeBuildCommand runs it)

set -eu

cd "$(dirname "$0")/.."

DIST="apps/med-recon-app/frontend/dist"
BINARYEN_VERSION="132"

resolve_wasm_opt() {
    if command -v wasm-opt >/dev/null 2>&1; then
        echo "wasm-opt"
        return 0
    fi

    CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/med-recon-wasm-opt"
    DIR="$CACHE_DIR/binaryen-version_${BINARYEN_VERSION}"
    if [ -x "$DIR/bin/wasm-opt" ]; then
        echo "$DIR/bin/wasm-opt"
        return 0
    fi
    if [ -x "$DIR/bin/wasm-opt.exe" ]; then
        echo "$DIR/bin/wasm-opt.exe"
        return 0
    fi
    download_wasm_opt "$CACHE_DIR" "$DIR"
}

download_wasm_opt() {
    CACHE_DIR="$1"
    DIR="$2"

    case "$(uname -s)" in
        Linux)
            OS_TAG="linux"
            case "$(uname -m)" in
                x86_64|amd64) ARCH_TAG="x86_64" ;;
                arm64|aarch64) ARCH_TAG="aarch64" ;;
                *) echo "wasm-opt.sh: unsupported architecture $(uname -m)" >&2; exit 1 ;;
            esac
            ;;
        Darwin)
            OS_TAG="macos"
            case "$(uname -m)" in
                x86_64) ARCH_TAG="x86_64" ;;
                arm64) ARCH_TAG="arm64" ;;
                *) echo "wasm-opt.sh: unsupported architecture $(uname -m)" >&2; exit 1 ;;
            esac
            ;;
        MINGW*|MSYS*|CYGWIN*)
            OS_TAG="windows"
            case "$(uname -m)" in
                x86_64|amd64) ARCH_TAG="x86_64" ;;
                arm64|aarch64) ARCH_TAG="arm64" ;;
                *) echo "wasm-opt.sh: unsupported architecture $(uname -m)" >&2; exit 1 ;;
            esac
            ;;
        *)
            echo "wasm-opt.sh: unsupported platform $(uname -s)" >&2
            exit 1
            ;;
    esac

    URL="https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-${ARCH_TAG}-${OS_TAG}.tar.gz"
    mkdir -p "$CACHE_DIR"
    echo "wasm-opt.sh: wasm-opt not found, downloading binaryen $BINARYEN_VERSION ($OS_TAG/$ARCH_TAG)" >&2
    curl -LsSf "$URL" -o "$CACHE_DIR/binaryen.tar.gz"
    tar -xzf "$CACHE_DIR/binaryen.tar.gz" -C "$CACHE_DIR"

    if [ -x "$DIR/bin/wasm-opt" ]; then
        echo "$DIR/bin/wasm-opt"
    elif [ -x "$DIR/bin/wasm-opt.exe" ]; then
        echo "$DIR/bin/wasm-opt.exe"
    else
        echo "wasm-opt.sh: binaryen downloaded but wasm-opt binary not found under $DIR" >&2
        exit 1
    fi
}

WASM=$(ls "$DIST"/*_bg.wasm 2>/dev/null | head -1 || true)
if [ -z "$WASM" ]; then
    echo "wasm-opt.sh: no *_bg.wasm found in $DIST, nothing to do" >&2
    exit 0
fi

WASM_OPT=$(resolve_wasm_opt)
echo "wasm-opt.sh: optimizing $WASM"
"$WASM_OPT" \
    -Oz \
    --strip-debug \
    --low-memory-unused \
    --enable-bulk-memory-opt \
    --enable-nontrapping-float-to-int \
    --enable-mutable-globals \
    -o "$WASM" \
    "$WASM"
echo "wasm-opt.sh: done"