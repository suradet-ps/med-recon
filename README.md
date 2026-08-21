# Med Recon

> Read-only cross-visit medication & allergy history lookup for HOSxP hospitals.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![CI](https://github.com/suradet-ps/med-recon/actions/workflows/ci.yml/badge.svg)](https://github.com/suradet-ps/med-recon/actions/workflows/ci.yml)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-dea584.svg)](#requirements)

Med Recon is a **read-only** desktop application that answers inter-hospital
inquiries about a patient's medication history. Search by **name, HN, or CID**
to see:

- the **Best Possible Medication History (BPMH)** aggregated across visits —
  deduplicated by drug, with derived days-supply and an **active / lapsed**
  inference per item
- **allergy / adverse drug reaction** records
- **visit history** (OPD + IPD)
- a printable **HTML report** for the requesting hospital

It connects to HOSxP (MySQL/MariaDB) **read-only**: every statement is validated
against an allow-list of `SELECT`/`SHOW`/`DESCRIBE`/`EXPLAIN` keywords before
execution, on top of the recommended read-only DB role.

```
Tauri 2 (shell) ── Leptos 0.8 CSR (WASM UI)
        │
        ├── med-recon-core      pure domain: BPMH engine, date-era normalization, redaction
        ├── med-recon-hosxp     sqlx MySQL repository (read-only guard)
        ├── med-recon-config    settings store: encrypted connection.json + plain settings.json
        └── med-recon-bridge    wasm IPC wrapper
```

## Security & privacy

- **No plaintext credentials on disk.** Connection settings are encrypted with
  `encryptman` (AES-256-GCM, HKDF-derived keys); the master key lives in the
  OS keychain via `encryptman-keyring` (macOS Keychain / Windows Credential
  Manager / Linux Secret Service). Credentials live in `connection.json`;
  non-secret site settings (site name, history window, current-medication
  list) live in a separate plain `settings.json`.
- **PHI discipline.** HN/CID are redacted in logs; names are never logged.
  The UI always shows the BPMH disclaimer — dispensing-derived data is one
  source among several, not a verified medication list.
- **Read-only by construction.** See `crates/med-recon-hosxp/src/readonly.rs`.

## Screens

| Screen | Purpose |
|---|---|
| Setup | Two sections: HOSxP connection (host/port/db/user/password, test + save) and site settings (ชื่อสถานบริการ, history window, ตั้งค่ายา) |
| Search | Name / HN / CID search with result list |
| Patient | BPMH (active / lapsed), allergies, visits, HTML report export |
| About | Project information |

The UI is Thai-only (ภาษาไทย).

## Requirements

- Rust **1.85+** (edition 2024) with the `wasm32-unknown-unknown` target
  (added automatically via `rust-toolchain.toml`)
- [Trunk](https://trunkrs.dev) — `cargo install trunk` (frontend builds to WASM)
- Tauri 2 system dependencies for your platform
  (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))
- A HOSxP MySQL/MariaDB database (a read-only account is recommended)

## Build & run

```bash
# development (hot-reload UI + native shell)
cargo tauri dev

# release bundle for the current platform
cargo tauri build

# or build the pieces individually
trunk build apps/med-recon-app/frontend/index.html --release   # wasm UI
cargo build -p med-recon-app                                    # native shell
```

Pre-built binaries (when published) are attached to
[GitHub Releases](https://github.com/suradet-ps/med-recon/releases).

## Development

Local checks (mirror the CI):

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p med-recon-core -p med-recon-hosxp -p med-recon-config -p med-recon-bridge -p med-recon-app
```

> `apps/med-recon-app/frontend` (`med-recon-frontend`) is a `wasm32` crate and is not
> built by `cargo test` on the host. It is verified by the CI's `wasm-frontend`
> and `wasm-tests` jobs (Trunk build + `wasm32` clippy/test).

- `AGENTS.md` — project conventions + the confirmed HOSxP schema reference
- `docs/AGENTS-RUST.md` — Rust style/lint rules
- `docs/DESIGN.md` — product scope, UX flows, design system, and BPMH data-model rationale
- `crates/med-recon-core` — BPMH aggregation, days-supply, active/lapsed inference
- `crates/med-recon-hosxp` — SQL statements (all `SELECT` only) and row mapping

## Site-specific configuration

HOSxP sites differ. Before relying on any of these, verify against your live
schema (see the open items in `AGENTS.md`):

- **Date era** — auto-detected per value (stored year ≥ 2500 ⇒ พ.ศ., converted
  to ค.ศ.); no setting needed, mixed-era sites are handled.
- **Sig (directions-for-use) lookup** — read from the `drugusage`/`sp_use`
  lookup tables via `opitemrece`; missing tables degrade to a warning.
- **Current medications (ยาที่ใช้อยู่)** — the BPMH active/lapsed verdict is
  operator-configured: curate a `drugitems` list in Settings; only listed
  drugs show as active, regardless of dispense recency.
- **`tmt_tp_code` / `tmt_gp_code`** — TMT mapping may be empty on some sites;
  cross-hospital drug matching should not assume it is populated.

## Contributing

Contributions are welcome under the MIT OR Apache-2.0 license.

- Read `AGENTS.md` for project conventions and the confirmed HOSxP schema
  reference before changing any query.
- Backend crates and the Tauri shell are checked with `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test`.
- The Leptos frontend (`apps/med-recon-app/frontend`) is a wasm32 crate built with
  Trunk; it is verified on CI, not via `cargo test --workspace` on the host.
- All HOSxP access is **read-only** — never add `INSERT`/`UPDATE`/`DELETE`/DDL.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at
your option.

This project is not affiliated with HOSxP, BMS, or any hospital system vendor.
