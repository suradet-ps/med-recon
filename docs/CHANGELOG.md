# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- **HOSxP schema alignment** (confirmed against the live site): IPD
  dispensing read from `opitemrece` (an-keyed) instead of `iptitemrece`;
  sig/directions-for-use read from the `drugusage`/`sp_use` lookup tables
  joined via `opitemrece.drugusage`/`sp_use`; dropped the `medusage` table
  and the `use_medusage_sig` config flag (sig reading is always on,
  degrading to a warning when the tables are missing); dropped
  `opd_allergy.severy_id`.
- **Date era auto-detection** — removed the era setting. Every date value
  read from HOSxP is normalized to ค.ศ. individually (stored year ≥ 2500
  ⇒ พ.ศ.), tolerating BE/CE/mixed sites without configuration.
- **BPMH verdict is operator-configured** — the settings screen curates a
  current-medication list (search `drugitems`); only listed drugs are
  labelled "ยาที่คาดว่ายังใช้อยู่" regardless of dispense recency. Days
  supply remains display-only.
- **Settings split into two JSON files** — `connection.json` (encrypted
  credentials) and `settings.json` (plain, non-secret: site name, history
  window, current-medication list). Legacy single-file configs are
  migrated automatically on first open.

## [0.1.0] — 2026-08-16

Initial release.

### Added

- **BPMH engine** (`med-recon-core`): cross-visit dispensing aggregation
  deduplicated by drug code, days-supply derivation from sig data
  (`qty / (dose × frequency)`), and active/lapsed inference with a grace
  period and a fallback window for unknown sigs.
- **HOSxP repository** (`med-recon-hosxp`): sqlx-based MySQL/MariaDB client with
  patient search (HN / CID / name), OPD + IPD dispensing history, allergy
  records, and visit history. Every statement passes a read-only guard
  (allow-list of `SELECT` / `SHOW` / `DESCRIBE` / `EXPLAIN`).
- **Encrypted site configuration** (`med-recon-config`): AES-256-GCM at rest via
  `encryptman`, master key in the OS keychain via `encryptman-keyring`.
  Passwords never touch disk in plaintext.
- **Desktop app** (`med-recon-app`): Tauri 2 shell + Leptos 0.8 CSR frontend.
  Screens: connection setup (test/save), patient search, patient detail
  (active/lapsed medications, allergies, visits), settings, about.
- **HTML report export**: printable, self-contained medication history
  report with BPMH disclaimer and PHI handling notice.
- **Thai/English UI** with persisted language choice.
- **Date era handling**: พ.ศ. → ค.ศ. normalization at the repository
  boundary, auto-detected per value from the stored year (no site setting;
  Buddhist-era leap-day clamping documented).
- **PHI redaction** helpers used in all logging paths.

### Security

- Read-only guard enforced in code, independent of the DB user.
- Secrets wrapped with `secrecy::SecretString` in memory.
- HTML report escapes all user-provided text.
