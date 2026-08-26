# Med Recon - Design System & Product Design

Med Recon is a **read-only medication history lookup desktop application** for
allergy assessment and medication reconciliation. This document defines the
product scope, UX flows, the visual design system, and the data-model
rationale behind the Best Possible Medication History (BPMH) engine.

---

## 1. Product Scope

- **Who uses it:** pharmacists and physicians at a hospital answering
  inter-hospital inquiries ("does this patient have a medication history?
  have they received drug X? do they have allergies?").
- **What it does:** looks up a patient by **name, HN, or CID** and shows the
  **complete cross-visit medication history** (BPMH) plus allergy records,
  all derived read-only from HOSxP (MySQL/MariaDB).
- **What it is not:** not a prescribing system, not an EHR, not a verified
  medication list. Dispensing-derived history is one BPMH source among
  several - the UI always says so.
- **OSS baseline:** international open source (MIT/Apache-2.0), Thai-only UI.

### Core constraints

1. **Read-only against HOSxP** - enforced by the read-only guard and a
   read-only DB role.
2. **PHI handling** - HN/CID redacted in logs; names never logged; HTML
   reports carry a PHI handling notice.
3. **BPMH framing** - never present dispensing history as complete or
   verified; show source-type and data-recency indicators.
4. **Schema drift tolerance** - HOSxP table/column names vary by site;
   queries degrade through fallback tiers and surface warnings instead of
   failing.

---

## 2. UX Flows

### 2.1 First launch → connection setup

The settings dialog has **two independent sections**, each saved to its
own file (see 4.6):

1. App starts with no connection config → the **settings dialog opens
   automatically**.
2. **HOSxP connection section** - host, port, database, user, password.
   **Test** runs against the typed values before anything is saved
   (latency shown). **Save** connects first, then persists the config
   **encrypted** (`connection.json`, AES-256-GCM; master key in the OS
   keychain).
3. **Site settings section** - site name (shown on exported reports),
   history window, and the current-medication list (ตั้งค่ายา). Saved as
   plain JSON (`settings.json`); usable before a connection exists.
4. Success updates the status dot; the dialog stays open so the operator
   can continue with site settings.

### 2.2 Search

1. One search box in the sidebar; input kind is **auto-detected**:
   13-digit → CID, 5–10 digits → HN, otherwise name.
2. Debounced 250 ms after the last keystroke; the hint line tells the
   operator which kind was detected.
3. Results list (max 20) shows name + HN + CID; clicking a result loads the
   full history into the main canvas.

### 2.3 Patient history (main canvas)

Top to bottom:

1. **Patient bar** - name, HN, CID, birthday + the export button.
2. **Data-completeness warnings** - amber banner when a HOSxP table was
   missing and a section was skipped (e.g. no `drugusage`/`sp_use` lookup →
   sig data unavailable).
3. **BPMH note** - persistent shield banner: dispensing-derived data is one
   source among several.
4. **ยาเดิมที่ผู้ป่วยเคยได้รับและคาดว่ายังคงใช้อยู่ (likely active)** - green verdict bands, one per
   deduplicated drug.
5. **ยาเดิมที่ผู้ป่วยเคยได้รับและคาดว่าหยุดใช้แล้ว (likely lapsed)** - neutral bands.
6. **แพ้ยา / อาการไม่พึงประสงค์** - red bands.
7. **ประวัติการเข้ารับบริการ** - visit timeline (date, OPD/IPD badge,
   department, visit id).

### 2.4 Export

"พิมพ์ประวัติการได้รับยา" saves a self-contained HTML report (same BPMH disclaimer +
PHI notice) through the native save dialog.

### 2.5 Settings (revisiting)

The gear button reopens the dialog; "Close" zeroizes the typed password
buffers; Escape closes from anywhere.

---

## 3. Visual Design System

Two-panel desktop layout adapted from the AllerX design language, with a
**green medical brand** distinct from AllerX's red.

### 3.1 Tokens

| Token | Value | Role |
|---|---|---|
| `--brand` | `#00754A` | Primary green - filled buttons, active accents |
| `--brand-dark` | `#005C38` | Hover/down state of primary buttons |
| `--brand-soft` | `#DCF2EA` | Patient bar, selection tint |
| `--status-connected` | `#43A047` | Top-bar health dot |
| `--status-disconnected` | `#C62828` | Top-bar health dot (error) |
| `--verdict-found` | `#E8F5E9` | Active-medication band background |
| `--verdict-found-text` | `#2E7D32` | Active-medication band text |
| `--verdict-found-border` | `#A5D6A7` | Active-medication band border |
| `--verdict-notfound` | `#FFEBEE` | Allergy / error band background |
| `--verdict-notfound-text` | `#C62828` | Allergy / error text |
| `--verdict-notfound-border` | `#FFCDD2` | Allergy / error border |
| `--verdict-lapsed` | `#FAFAFA` | Lapsed-medication band |
| `--verdict-lapsed-text` | `#616161` | Lapsed text |
| `--verdict-lapsed-border` | `#E0E0E0` | Lapsed border |
| `--canvas` | `#FAFAFA` | Page/sidebar canvas |
| `--canvas-raised` | `#FFFFFF` | Cards, modal, inputs |
| `--surface-muted` | `#F5F5F5` | Hover fills, chips |
| `--hairline` | `#E0E0E0` | Borders, separators |
| `--ink` | `#212121` | Primary text |
| `--slate` | `#616161` | Secondary text |
| `--steel` | `#9E9E9E` | Placeholder / disabled text |
| `--warning-bg` | `#FFF8E1` | Warning banner |
| `--warning-text` | `#8a6420` | Warning text (WCAG AA on the banner) |
| `--warning-border` | `#FFE082` | Warning border |

Radii: `--rounded-sm 4px`, `--rounded-md 6px`, `--rounded-lg 8px`,
`--rounded-full 9999px`. Elevation: `--elev-3 0 8px 24px rgba(0,0,0,0.18)`
for the modal only; everything else is flat with hairlines.

Typography: **IBM Plex Sans Thai** (UI) + **IBM Plex Mono** (codes, dates,
HN/CID) from Google Fonts. Base 14 px, line-height 1.5.

### 3.2 Layout

```
┌─────────────────────────────── top-bar (44px) ───────────────────────────────┐
│ logo Med Recon        ● status   [ไทย]   [ตั้งค่า]                                 │
├───────────────────────┬───────────────────────────────────────────────────────┤
│ sidebar (360px)       │ main canvas                                            │
│  search input         │  patient bar + export                                  │
│  hints / results list │  warnings / BPMH note / active / lapsed / allergies /  │
│                       │  visits timeline                                       │
└───────────────────────┴───────────────────────────────────────────────────────┘
```

- Sidebar scrolls independently; the canvas scrolls independently.
- Breakpoints: `<720px` stacks sidebar above canvas; `720–959px` sidebar
  shrinks to 300px.
- `prefers-reduced-motion` disables transitions.

### 3.3 Components

- **Top bar** - 44 px, raised surface, hairline bottom border; logo +
  title, status dot (7 px, green/red), settings button.
- **Search input** - 38 px, mono-capable, focus ring = 2 px brand + 3 px
  soft halo; magnifier icon left, kind hint below.
- **Result list** - hairline-bordered list, hover = muted surface, active
  = brand-soft.
- **Patient bar** - brand-soft fill + found-border, name (600), mono HN/CID.
- **Buttons** - primary (brand fill, white), secondary (raised, hairline
  border); inline variants; `translateY(1px)` on active; disabled = 0.4
  opacity.
- **Verdict bands** - the signature component: icon + headline + detail,
  tinted per state (found/notfound/lapsed/pending); compact variant for
  per-drug rows.
- **Timeline** - hairline list rows: mono date, OPD/IPD badge, dept, visit
  id.
- **Badges/chips** - 10 px, 600 weight, pill radius, muted surface.
- **Settings modal** - centered, max-width 460 px, blur backdrop, elev-3
  shadow; form fields with 12 px labels, 36 px inputs, focus ring; success
  (green) / error (red) message panels.
- **Warning banner** - amber tint, alert icon, used for completeness
  warnings and degraded-connection messages.

---

## 4. Data Model Rationale

### 4.1 Identity

- `patient.hn` is the cross-visit join key; `cid` and name are search
  entry points. `hn_change_log` merges are a documented open item (resolve
  on-site before relying on HN immutability).

### 4.2 Dispensing events

- OPD and IPD dispensing both live in `opitemrece` (vn-keyed OPD,
  an-keyed IPD) - confirmed at the target site: no `iptitemrece`.
- Sig data comes from the `drugusage`/`sp_use` lookup tables joined via
  `opitemrece.drugusage`/`opitemrece.sp_use`.
- `qty` is DECIMAL in HOSxP; sqlx cannot decode DECIMAL as `f64`, so the
  SQL casts it to CHAR and the client parses it.
- Columns that vary by site (`strength`, `units`) are selected through
  fallback query tiers (MySQL 1054 → degrade; 1146 → skip + warn).

### 4.3 BPMH aggregation (med-recon-core)

Input: raw `Dispense` events (OPD + IPD) + the operator-configured
**current-medication list** (`current_med_codes`, from the settings screen).
Output: `MedicationItem`s.

1. **Dedup by `icode`** - all visits for the same drug merge into one item.
2. **Derived days supply** - `qty / (dose_per_admin × frequency_per_day)`,
   rounded up; `None` when the sig is missing. Display-only - it no longer
   drives the active/lapsed verdict.
3. **Active/lapsed verdict - operator-configured, not inferred.** A drug
   whose `icode` is on the current-medication list is `active` no matter
   when it was last dispensed; every other dispensed drug is `lapsed`
   (stopped). The list is curated in settings (search `drugitems`), stored
   encrypted with the site config. An empty list marks everything lapsed.
4. **Sort** - most recent dispense first.

### 4.4 Dates (Buddhist era, auto-detected)

The Thai calendar is Gregorian +543 with shared leap rules. BE leap days
(Feb 29 of a BE year) have no proleptic-Gregorian representation, so
BE→CE conversion of such a day is clamped to Feb 28 (documented). All
HOSxP dates are normalized to CE **per value** at the repository boundary:
a stored year ≥ 2500 is treated as พ.ศ. and converted, anything else is
already ค.ศ. There is no site-era setting.

The SQL cutoff is always sent in ค.ศ.; on พ.ศ. sites the year comparison
matches every stored date (BE years sort higher), so the exact history
window is enforced client-side after normalization.

### 4.5 Schema drift policy

| Error | Meaning | Handling |
|---|---|---|
| MySQL 1146 | table missing | try fallback tier → skip section + user-visible warning |
| MySQL 1054 | column missing | degrade to the fallback statement (same row shape) |
| other | real failure | surface as typed error (kind + message) |

### 4.6 Settings storage (`med-recon-config`)

Configuration is split into **two JSON files** under the platform config
directory (`%APPDATA%\MedRecon` on Windows, `~/.config/MedRecon` on Linux,
`~/Library/Application Support/MedRecon` on macOS), so connection credentials
and operator preferences are versioned/backed up independently:

| File | Contents | Format |
|---|---|---|
| `connection.json` | `host`, `port`, `database`, `user`, `password` | encrypted blob (AES-256-GCM via `encryptman`; master key in the OS keychain) |
| `settings.json` | `siteName`, `historyDays`, `currentMedCodes` | plain readable JSON (non-secret by design) |

Rules:

- **Credentials never appear in plaintext** - not in `connection.json`,
  logs, or error messages. A missing `connection.json` maps to
  "ยังไม่ได้ตั้งค่า" (NotConfigured) in the UI.
- **Settings file defaults**: absent `settings.json` loads as
  `siteName: ""`, `historyDays: 730`, `currentMedCodes: []`; unknown or
  missing fields in an existing file also fall back via serde defaults.
- **Migration**: a legacy single-file config (`site-config.json`, the
  pre-split encrypted blob) is split into the two files on first open and
  archived as `site-config.json.bak`. Existing files are never
  overwritten.
- The current-medication list (`currentMedCodes`) lives in the plain
  settings file because it is operator configuration, not a secret; it
  still drives the BPMH active/lapsed split via `load_history`.

---

## 5. Reconciliation Workflow (future)

The BPMH engine is designed to support admission/transfer/discharge
medication reconciliation:

1. Clinician reviews the BPMH list (active/lapsed labels + sig + supply).
2. Cross-checks allergies against newly prescribed agents (indication
   linking via `ovstdiag`/`iptdiag` ICD-10 is future work).
3. Notes discrepancies (drug added/stopped/changed) and confirms the final
   list with the patient - the app provides the evidence, never the
   verdict.

---

## 6. Accessibility & Craft

- All interactive elements have visible focus rings (2 px brand).
- Text contrast follows WCAG AA (warning amber tuned to 4.6:1 on its
  banner).
- Icons are decorative (`aria-hidden`), stroke-based Lucide-style, sized
  14–16 px.
- Reduced-motion support; touch-friendly 36–38 px inputs.
- Zero plaintext credentials: settings are encrypted at rest; typed
  password buffers are zeroized on dialog close.
