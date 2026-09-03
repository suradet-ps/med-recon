# Med Recon — target/ size measurements (raw data)

Method: `cargo clean` before every experiment; fresh full build;
real `du`/`ls` measurements. 2026-09-03, macOS aarch64, rustc 1.98.0,
trunk 0.21.14, binaryen 130→132.

## Baseline — default cargo release profile

| metric | value |
|---|---|
| target/ total | 1.7G |
| target/release | 1.4G (deps 1.3G, build 131M, fingerprint 11M) |
| target/wasm32-unknown-unknown | 312M (deps 307M) |
| native binary | 15M |
| wasm pre-wasm-opt (target) | 2.8M |
| wasm post-wasm-opt (dist) | 2.6M |

## Exp A — profile: codegen-units=1, lto=true, opt-level="s", panic="abort", strip=true

| metric | value |
|---|---|
| native binary | 5.3M |
| wasm pre-wasm-opt | 1.7M |
| wasm dist | 622K (wasm-opt step failed silently; = wasm-bindgen output) |
| target/ total | 1.7G |

## Exp B — same but opt-level="z"

| metric | value |
|---|---|
| native binary | 4.2M |
| wasm pre-wasm-opt | 1.5M |
| wasm dist | 511K (wasm-opt step failed silently) |
| target/ total | 1.6G |

## Exp C — wasm feature experiments

- rustc 1.98 wasm32-unknown-unknown defaults: bulk-memory, multivalue,
  mutable-globals, nontrapping-fptoint, reference-types, sign-ext
- `-C target-feature=-bulk-memory,-nontrapping-fptoint`: NO effect
  (defaults can't be negated)
- `-C target-cpu=mvp`: raw wasm clean (0 memory.copy/fill ops), but
  wasm-bindgen re-introduces them (758 ops in stage wasm) → trunk's
  wasm-opt (no feature flags) still fails
- manual wasm-opt -Oz on stage wasm with
  `--enable-bulk-memory-opt --enable-nontrapping-float-to-int
  --enable-mutable-globals --strip-debug --low-memory-unused` → 478K
  (from 512K stage)
- binaryen 130 and 132 both reject bulk-memory ops without explicit flags

## Exp D (final) — data-wasm-opt="0" + script/wasm-opt.sh hook

Full `cargo tauri build`:

| metric | value |
|---|---|
| native binary | 4.2M |
| wasm dist (after hook -Oz) | 477K |
| Med Recon.app | 4.5M |
| Med Recon_0.4.1_aarch64.dmg | 2.8M |
| target/ total | 1.8G (incl. bundle 7.6M) |

## Target-folder maintenance

| state | size |
|---|---|
| after 5 flag-change builds without clean (accumulated) | 2.2G |
| after single consistent build | 1.6G |
| release-only build + bundle (cargo tauri build) | 1.8G |
| + one `cargo test`/dev build on top (target/debug) | **4.2G** (debug alone 2.5G > release 1.5G) |
| cargo sweep --dry-run --time 30 (fresh single build) | nothing to clean |
| cargo clean | 0 |

## Per-experiment build times (native)

- baseline: 1m38s
- Exp A/B: ~1m40-1m49s
- wasm (trunk): ~43-50s