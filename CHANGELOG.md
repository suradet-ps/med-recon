# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.1.0] — 2026-08-16

Initial release.

### Added

- **BPMH engine** (`recon-core`): cross-visit dispensing aggregation
  deduplicated by drug code, days-supply derivation from sig data
  (`qty / (dose × frequency)`), and active/lapsed inference with a grace
  period and a fallback window for unknown sigs.
- **HOSxP repository** (`recon-hosxp`): sqlx-based MySQL/MariaDB client with
  patient search (HN / CID / name), OPD + IPD dispensing history, allergy
  records, and visit history. Every statement passes a read-only guard
  (allow-list of `SELECT` / `SHOW` / `DESCRIBE` / `EXPLAIN`).
- **Encrypted site configuration** (`recon-config`): AES-256-GCM at rest via
  `encryptman`, master key in the OS keychain via `encryptman-keyring`.
  Passwords never touch disk in plaintext.
- **Desktop app** (`recon-app`): Tauri 2 shell + Leptos 0.8 CSR frontend.
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
