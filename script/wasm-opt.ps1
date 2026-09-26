# wasm-opt.ps1 - Windows port of wasm-opt.sh: post-process the trunk WASM
# output with binaryen for maximum size reduction.
#
# Why this exists: wasm-bindgen emits bulk-memory (memory.copy/fill) and
# nontrapping-float-to-int instructions into the wasm. The wasm-opt binary
# rejects those unless the matching --enable-* features are passed, and
# trunk does not pass them - so trunk's built-in wasm-opt step silently
# fails on modern rustc output. We disable trunk's step (data-wasm-opt="0"
# in frontend/index.html) and run wasm-opt ourselves here with the
# features enabled.
#
# If wasm-opt is not on PATH (fresh machines do not have binaryen), a
# pinned binaryen release is downloaded into
# %LOCALAPPDATA%\med-recon-wasm-opt and used from there.
#
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File script/wasm-opt.ps1
# (tauri.windows.conf.json runs this from the app directory)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

# Resolve the repo root from this script's location (script/..).
$RepoRoot = Split-Path -Parent $PSScriptRoot
$Dist = Join-Path $RepoRoot 'apps\med-recon-app\frontend\dist'
$BinaryenVersion = '132'

function Resolve-WasmOpt {
    $onPath = Get-Command wasm-opt -ErrorAction SilentlyContinue
    if ($onPath) {
        return $onPath.Source
    }

    $cacheRoot = if ($env:LOCALAPPDATA) { $env:LOCALAPPDATA } else { Join-Path $HOME '.cache' }
    $cacheDir = Join-Path $cacheRoot 'med-recon-wasm-opt'
    $dir = Join-Path $cacheDir "binaryen-version_$BinaryenVersion"
    $exe = Join-Path $dir 'bin\wasm-opt.exe'
    if (Test-Path -LiteralPath $exe) {
        return $exe
    }

    $arch = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'arm64' } else { 'x86_64' }
    $url = "https://github.com/WebAssembly/binaryen/releases/download/version_$BinaryenVersion/binaryen-version_$BinaryenVersion-$arch-windows.tar.gz"
    New-Item -ItemType Directory -Force -Path $cacheDir | Out-Null
    $archive = Join-Path $cacheDir 'binaryen.tar.gz'
    Write-Host "wasm-opt.ps1: wasm-opt not found, downloading binaryen $BinaryenVersion ($arch/windows)"
    Invoke-WebRequest -Uri $url -OutFile $archive
    tar -xzf $archive -C $cacheDir
    if (-not (Test-Path -LiteralPath $exe)) {
        throw "wasm-opt.ps1: binaryen downloaded but wasm-opt.exe not found under $dir"
    }
    return $exe
}

$wasm = Get-ChildItem -Path (Join-Path $Dist '*_bg.wasm') -ErrorAction SilentlyContinue |
    Select-Object -First 1
if (-not $wasm) {
    Write-Host "wasm-opt.ps1: no *_bg.wasm found in $Dist, nothing to do"
    exit 0
}

$wasmOpt = Resolve-WasmOpt
Write-Host "wasm-opt.ps1: optimizing $($wasm.FullName)"
& $wasmOpt -Oz --strip-debug --low-memory-unused `
    --enable-bulk-memory-opt --enable-nontrapping-float-to-int `
    --enable-mutable-globals -o $wasm.FullName $wasm.FullName
if ($LASTEXITCODE -ne 0) {
    throw "wasm-opt.ps1: wasm-opt failed with exit code $LASTEXITCODE"
}
Write-Host "wasm-opt.ps1: done"
