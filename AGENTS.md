# AGENTS.md — Recon

## Project Overview

Recon is a read-only medication history lookup desktop application for allergy
assessment and medication reconciliation. It connects to HOSxP (MySQL/MariaDB
hospital information system) to retrieve a patient's cross-visit medication
and allergy history.

For product scope, UX flows, data model rationale, and reconciliation logic,
see **DESIGN.md**. This file covers agent-facing conventions: stack, build
commands, coding rules, and the HOSxP schema reference needed to write
correct queries. Do not duplicate DESIGN.md content here — link to it.

## Tech Stack

- **Shell:** Tauri 2
- **Frontend:** Leptos 0.8, CSR (client-side rendered, compiled to WASM)
- **Database access:** MySQL/MariaDB client (HOSxP), read-only connection only
- **Credential/config encryption:** `encryptman` (AES-256-GCM + HKDF) — all
  stored DB connection strings, credentials, and Tailscale/remote-access
  settings must go through `encryptman`. Never store plaintext credentials
  on disk, in logs, or in error messages. and `encryptman-keyring`. for master key managemaent
- **Documentation convention:** `DESIGN.md` (product/data model),
  `AGENTS.md` (this file), `AGENTS-RUST.md` (Rust-specific style/lint rules)

## Core Constraints

- **Read-only against HOSxP.** No `INSERT`/`UPDATE`/`DELETE`/DDL statements
  against production HOSxP tables under any circumstance. All queries must
  be `SELECT` only, ideally executed via a read-only DB user/role.
- **PHI handling.** HN, CID, patient name, and medication/allergy data are
  PHI. Do not log raw PHI. Redact HN/CID in logs and crash reports.
- **Best Possible Medication History (BPMH) framing.** Dispensing-derived
  history from HOSxP is one source among several (see DESIGN.md). The UI
  must never present it as a complete or verified medication list —
  always show data-recency and source-type indicators.
- **Buddhist Era dates.** Some HOSxP date columns may be stored in พ.ศ.
  Confirm per-table before assuming ค.ศ.; do not hardcode a global offset.

## HOSxP Schema Reference (confirmed against live schema)

Use this as the source of truth for table/column names when writing
queries. If a query needs a table/column not listed here, verify against
the live schema before writing code — do not guess HOSxP field names.

### Patient Identity
| Table | Key fields | Notes |
|---|---|---|
| `patient` | `hn` (PK), `cid`, `pname`, `fname`, `lname`, `birthday` | `hn` is the cross-visit join key. Check `hn_change_log` for HN merge/mapping history before treating `hn` as immutable. |

### Visits / Encounters
| Table | Key fields | Notes |
|---|---|---|
| `ovst` | `seq_id`, `hn`, `vn`, `vstdate`, `vsttime`, `main_dep` | OPD visit |
| `opdscreen` | — | OPD screening point |
| `ipt` | `an`, `hn`, `vn`, `regdate`, `dchdate`, `ward` | IPD admission. `ipt.vn` carries the originating OPD `vn`, so `vn` bridges OPD → IPD for the same episode. |

### Drug Orders / Dispensing
| Table | Key fields | Notes |
|---|---|---|
| `opitemrece` | `vn`, `hn`, `icode`, `qty`, `vstdate`, `income` | OPD order/dispense |
| `iptitemrece` | `an`, `hn`, `icode`, `qty` | IPD order/dispense |
| `medusage`, `drugusage` | — | Sig / directions for use |

**Days supply is not a stored column.** Must be derived as
`qty / (dose × frequency per day)`, or sourced from `sp_use` for
continuous/injectable regimens. This derivation logic belongs in the
domain layer — see DESIGN.md for the BPMH active/lapsed inference rule.

### Drug Master
| Table | Key fields | Notes |
|---|---|---|
| `drugitems` | `icode` (PK), `name`,`strength`,`units`, `therapeutic`, `drugaccount` | `snomed_code`, `tmt_tp_code`, `tmt_gp_code` may carry TMT mapping, depending on HOSxP version and whether the site submits 16-แฟ้ม standard data. Do not assume these are populated — check per-site before relying on TMT for cross-hospital matching. |

### Allergy / ADR
| Table | Key fields | Notes |
|---|---|---|
| `opd_allergy` | `hn`, `agent`, `symptom`, `allergy_group_id`, `severy_id`, `reporter` | `agent` may be free-text or `icode`-linked depending on site configuration — do not assume structured data; plan for text cleaning/normalization. |

### Diagnosis (out of MVP scope, for future indication-linking)
| Table | Key fields | Notes |
|---|---|---|
| `ovstdiag` | `vn`, `hn`, `icd10`, `diagtype` | OPD |
| `iptdiag` | `an`, `hn`, `icd10`, `diagtype` | IPD |

## Development Workflow

- Confirm schema assumptions against a live read-only connection before
  writing a query that touches a new table — this document reflects the
  schema as gathered on the date of writing and may drift by site/version.
- All new query modules go through `encryptman`-backed connection config;
  never hardcode connection strings.
- See `AGENTS-RUST.md` for Rust style, error handling, and workspace
  layout conventions.
- See `DESIGN.md` for the BPMH aggregation/dedup logic, active-vs-lapsed
  medication inference, and reconciliation workflow (admission/transfer/
  discharge) this app is designed to support.

## Open Items to Resolve Before Implementation

- [ ] Confirm date encoding (พ.ศ. vs ค.ศ.) per relevant table on the target site.
- [ ] Confirm whether `tmt_tp_code`/`tmt_gp_code` are populated on the target site.
- [ ] Confirm `opd_allergy.agent` format (free-text vs `icode`-linked) on the target site.
- [ ] Decide read-only DB user/role setup for HOSxP connection.
