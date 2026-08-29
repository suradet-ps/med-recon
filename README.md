# Med Recon

```
███╗   ███╗███████╗██████╗ ██████╗ ███████╗ ██████╗ ██████╗ ███╗   ██╗
████╗ ████║██╔════╝██╔══██╗██╔══██╗██╔════╝██╔════╝██╔═══██╗████╗  ██║
██╔████╔██║█████╗  ██║  ██║██████╔╝█████╗  ██║     ██║   ██║██╔██╗ ██║
██║╚██╔╝██║██╔══╝  ██║  ██║██╔══██╗██╔══╝  ██║     ██║   ██║██║╚██╗██║
██║ ╚═╝ ██║███████╗██████╔╝██║  ██║███████╗╚██████╗╚██████╔╝██║ ╚████║
╚═╝     ╚═╝╚══════╝╚═════╝ ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═════╝ ╚═╝  ╚═══╝
```

---

## ◆ PULSE

Another hospital is asking about this patient - what was dispensed,
what was the allergy, what visits happened. Med Recon answers from
HOSxP, read-only, in one A4 page: the Best Possible Medication History
aggregated across visits, deduplicated by drug, with derived days
supply and an active or lapsed verdict per item; the allergy and ADR
records; and the visit history. The request arrives in Thai, and the
answer leaves as a PDF the requesting hospital can keep.

| BPMH ▣ | Allergies ▣ | Visits ▣ | PDF export ▣ |
|---|---|---|---|

*v0.4.0 - the reconciliation loop is sealed and serving.*

> Built with Tauri 2 + Leptos 0.8, judged by `med-recon-core`, read from
> HOSxP by `med-recon-hosxp` - never a write, never a plaintext secret.
>
> **suradet-ps**, artifact keeper

---

## ◆ IGNITION

One toolchain, two commands.

```
⟫ rustup target add wasm32-unknown-unknown   # via rust-toolchain.toml
⟫ cargo install trunk
⟫ cargo tauri dev
```

The release artifact: `⟫ cargo tauri build`

<details>
<summary>Prerequisites</summary>

- Rust **1.85+** (edition 2024) with the `wasm32-unknown-unknown` target
- [Trunk](https://trunkrs.dev) - installed above
- Tauri 2 system dependencies for your platform
- A HOSxP MySQL/MariaDB database (a read-only account is recommended)

</details>

On first launch the Setup screen asks for the HOSxP connection and the
site identity (ชื่อสถานบริการ) - the credentials are encrypted before
they ever touch disk.

---

## ◆ ANATOMY

Four crates, one boundary that never bends: HOSxP is read, never
written.

- **Judges** - `med-recon-core` is the pure domain: BPMH aggregation,
  days-supply derivation, date-era normalization (พ.ศ. to ค.ศ., mixed
  era handled), and PHI redaction - testable without a database.
- **Reads** - `med-recon-hosxp` is the `sqlx` MySQL repository. Every
  statement is validated against an allow-list of `SELECT` / `SHOW` /
  `DESCRIBE` / `EXPLAIN` keywords before execution - read-only is
  enforced in code, not just recommended in a document.
- **Seals** - `med-recon-config` stores connection settings encrypted
  with `encryptman` (AES-256-GCM, HKDF-derived keys), the master key in
  the OS keychain; site settings stay in a plain, non-secret
  `settings.json`.
- **Speaks** - `med-recon-bridge` carries the answers to the Leptos UI,
  which is Thai-only by design - the room it works in is a Thai
  hospital.
- **Prints** - the PDF report is generated entirely in Rust: Sarabun
  fonts embedded, Thai shaped with HarfBuzz so vowels and tone marks
  compose correctly - byte-identical on every OS, no browser involved.

---

## ◆ RITUALS

**The core ceremony** - an inter-hospital inquiry:

1. Open Med Recon, connect to HOSxP. One configuration, remembered and
   sealed.
2. Search by name, HN, or CID. The result list answers.
3. Read the patient: BPMH with active and lapsed sections, allergies,
   and the visit history. Click a row to strike it when the review says
   "หยุดใช้แล้ว" - a session-local mark, cleared on the next fresh load.
4. Export: the A4 PDF carries the BPMH disclaimer, the patient card,
   and the PHI footer - ready for the requesting hospital.

**The ceremony of the disclaimer** - the BPMH is dispensing-derived
data: one source among several, never a verified medication list. The
UI says so on every screen that matters.

**The ceremony of the redaction** - HN and CID are masked in logs,
names are never logged at all. The patient's identity is a
responsibility, not a convenience.

---

## ◆ ECHOES

**Where this artifact is heading**

```
v0.1   ▸ search, BPMH, allergies, visits ──────────────────────────── ▸ sealed
v0.2   ▸ user manual, setup flows ─────────────────────────────────── ▸ sealed
v0.3   ▸ A4 PDF export: Sarabun embedded, HarfBuzz shaping ────────── ▸ sealed
v0.4   ▸ click-to-strike review aid ───────────────────────────────── ▸ sealed
```

**Raising the artifact** - read `AGENTS.md` before touching a query -
it holds the confirmed HOSxP schema reference and the hard rules:
read-only without exceptions, no plaintext credentials, PHI redaction
in logs. The design language lives in `docs/DESIGN.md`.

**Status** - CI gates every change: fmt, `clippy --workspace
--all-targets -- -D warnings`, host tests for the backend crates, and
separate wasm build/test jobs for the Leptos frontend.
[Watch the gates](.github/workflows).

---

```
  ─────────────────────────────────────────
   Another hospital asked about this patient.
   The answer must be a fact, not a guess.
  ─────────────────────────────────────────
```

Licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option. Not affiliated with HOSxP, BMS, or any hospital system
vendor.