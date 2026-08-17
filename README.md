# Recon

**Cross-visit medication & allergy history lookup for HOSxP hospitals.**

Recon is a read-only desktop application that answers inter-hospital inquiries
about a patient's medication history. Search by **name, HN, or CID** to see:

- the **Best Possible Medication History (BPMH)** aggregated across visits —
  deduplicated by drug, with derived days-supply and an **active / lapsed**
  inference per item
- **allergy / adverse drug reaction** records
- **visit history** (OPD + IPD)
- a printable **HTML report** for the requesting hospital

It connects to HOSxP (MySQL/MariaDB) **read-only**: every statement is
validated against an allow-list of `SELECT`/`SHOW`/`DESCRIBE`/`EXPLAIN`
keywords before execution, on top of the recommended read-only DB role.

```
Tauri 2 (shell) ── Leptos 0.8 CSR (WASM UI)
        │
        ├── recon-core      pure domain: BPMH engine, era conversion, redaction
        ├── recon-hosxp     sqlx MySQL repository (read-only guard)
        ├── recon-config    encrypted settings (encryptman AES-256-GCM + OS keychain)
        └── recon-bridge    wasm IPC wrapper
```

## Security & privacy

- **No plaintext credentials on disk.** Connection settings are encrypted with
  `encryptman` (AES-256-GCM, HKDF-derived keys); the master key lives in the
  OS keychain via `encryptman-keyring` (macOS Keychain / Windows Credential
  Manager / Linux Secret Service).
- **PHI discipline.** HN/CID are redacted in logs; names are never logged.
  The UI always shows the BPMH disclaimer — dispensing-derived data is one
  source among several, not a verified medication list.
- **Read-only by construction.** See `crates/recon-hosxp/src/readonly.rs`.

## Screens

| Screen | Purpose |
|---|---|
| Setup | HOSxP connection form (host/port/db/user/password, date era, history window), test + save |
| Search | Name / HN / CID search with result list |
| Patient | BPMH (active / lapsed), allergies, visits, HTML report export |
| Settings | View / edit / clear the encrypted configuration |
| About | Project information |

UI ships bilingual (**ไทย / English**) with a language toggle.

## Requirements

- Rust 1.88+ with `wasm32-unknown-unknown` target
- `trunk` (`cargo install trunk` or `brew install trunk`)
- Tauri 2 prerequisites for your platform
  (see [tauri.app](https://tauri.app/start/prerequisites/))
- A HOSxP MySQL/MariaDB database (read-only account recommended)

## Build & run

```bash
# install the wasm target (auto-applied via rust-toolchain.toml on first build)
rustup target add wasm32-unknown-unknown

# development (hot-reload UI + native shell)
npx @tauri-apps/cli@2 dev

# release bundle (macOS: .app + .dmg)
npx @tauri-apps/cli@2 build

# or build pieces individually
trunk build apps/recon-app/frontend/index.html --release   # wasm UI
cargo build -p recon-app                                    # native shell
```

## Development

```bash
cargo fmt --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo doc --no-deps --workspace
```

- `AGENTS.md` — project conventions + the confirmed HOSxP schema reference
- `AGENTS-RUST.md` — Rust style/lint rules
- `DESIGN.md` — product scope, UX flows, design system, and BPMH data-model rationale
- `crates/recon-core` — BPMH aggregation, days-supply, active/lapsed inference
- `crates/recon-hosxp` — SQL statements (all `SELECT` only) and row mapping

## Site-specific configuration

HOSxP sites differ. Before relying on any of these, verify against your live
schema (see the open items in `AGENTS.md`):

- **Date era** — some sites store dates in พ.ศ. (Buddhist era). Set the era in
  Setup; conversion is handled at the repository boundary.
- **Sig (directions-for-use) lookup** — read from the `drugusage`/`sp_use`
  lookup tables via `opitemrece`; missing tables degrade to a warning.
- **`tmt_tp_code` / `tmt_gp_code`** — TMT mapping may be empty on some sites;
  cross-hospital drug matching should not assume it is populated.

## License

MIT OR Apache-2.0 — see `LICENSE-MIT` and `LICENSE-APACHE`.

This project is not affiliated with HOSxP, BMS, or any hospital system vendor.
