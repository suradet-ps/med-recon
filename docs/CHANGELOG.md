# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- **Report export is now a PDF file** - "พิมพ์ประวัติการได้รับยา" saves an
  A4 PDF (595.28 × 841.89 pt) instead of a printable HTML document. The
  report is generated entirely in Rust: Sarabun fonts (the UI's font
  family, OFL) are embedded in the file, Thai text is shaped with
  HarfBuzz (via `rustybuzz`) so vowels/tone marks compose correctly, and
  the fixed layout carries the BPMH disclaimer, patient card, allergy /
  medication / visit sections, and a PHI footer with page numbers on
  every page. No browser or system fonts are involved, so output is
  byte-identical on every OS.

## [0.2.0] - 2026-08-26

### Added

- **User manual (คู่มือการใช้งาน)** dialog - new คู่มือ button in the top
  bar next to ตั้งค่า, covering first-run setup, patient search, reading
  the history sections, report export, screenshot capture (Windows only),
  and PHI/read-only cautions.
- **History window selector** - segmented control on the active-medication
  header to override the configured search window per patient; the
  ค่าเริ่มต้น segment always shows the configured default.
- **Native unit tests** for pure frontend UI helpers.

### Changed

- **Site name shown in the top bar** - the configured สถานบริการ label
  replaces the hardcoded brand text.
- **Thai-only UI** - the Thai/English i18n system was removed; all UI text
  is hardcoded Thai (settings hints trimmed accordingly).
- **HOSxP schema alignment** (confirmed against the live site): IPD
  dispensing read from `opitemrece` (an-keyed) instead of `iptitemrece`;
  sig/directions-for-use read from the `drugusage`/`sp_use` lookup tables
  joined via `opitemrece.drugusage`/`sp_use`; dropped the `medusage` table
  and the `use_medusage_sig` config flag (sig reading is always on,
  degrading to a warning when the tables are missing); dropped
  `opd_allergy.severy_id`.
- **Date era auto-detection** - removed the era setting. Every date value
  read from HOSxP is normalized to ค.ศ. individually (stored year ≥ 2500
  ⇒ พ.ศ.), tolerating BE/CE/mixed sites without configuration.
- **BPMH verdict is operator-configured** - the settings screen curates a
  current-medication list (search `drugitems`); only listed drugs are
  labelled "ยาเดิมที่ผู้ป่วยเคยได้รับและคาดว่ายังคงใช้อยู่" regardless of dispense recency. Days
  supply remains display-only.
- **Settings split into two JSON files** - `connection.json` (encrypted
  credentials) and `settings.json` (plain, non-secret: site name, history
  window, current-medication list). Legacy single-file configs are
  migrated automatically on first open.
- **ASCII hyphenation** - em dashes replaced with ASCII hyphens across
  the codebase (docs/CI hygiene, no runtime effect).

## [0.1.0] - 2026-08-16

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
