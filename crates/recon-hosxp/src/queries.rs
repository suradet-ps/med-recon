//! SQL statements and pure query-building helpers for HOSxP.
//!
//! All statements are `SELECT`-only, use positional `?` parameters (binary
//! protocol — no string interpolation of user input), and follow the schema
//! reference in AGENTS.md. Every public statement constant is exercised by
//! the read-only guard before execution in `client.rs`.

use recon_core::{EncounterSource, Sig, VisitSummary};

/// Maximum number of patient search results returned.
pub const DEFAULT_SEARCH_LIMIT: u32 = 20;

/// Patient lookup by exact national ID (13 digits).
///
/// Parameters: `(cid, limit)`.
pub const PATIENT_SEARCH_BY_CID: &str = r#"
SELECT hn, cid, pname, fname, lname, birthday
FROM patient
WHERE cid = ?
LIMIT ?"#;

/// Patient lookup by exact hospital HN.
///
/// Parameters: `(hn, limit)`.
pub const PATIENT_SEARCH_BY_HN: &str = r#"
SELECT hn, cid, pname, fname, lname, birthday
FROM patient
WHERE hn = ?
LIMIT ?"#;

/// Name search — prefix match first so existing indexes on
/// `fname`/`lname` are used; falls back to [`PATIENT_SEARCH_NAME_CONTAINS`]
/// when empty.
///
/// Parameters: `(prefix, prefix, prefix, limit)`.
pub const PATIENT_SEARCH_NAME_PREFIX: &str = r#"
SELECT hn, cid, pname, fname, lname, birthday
FROM patient
WHERE fname LIKE ?
   OR lname LIKE ?
   OR CONCAT_WS(' ', pname, fname, lname) LIKE ?
LIMIT ?"#;

/// Name search fallback — contains match used only when the prefix match
/// found nothing.
///
/// Parameters: `(pattern, pattern, pattern, limit)`.
pub const PATIENT_SEARCH_NAME_CONTAINS: &str = r#"
SELECT hn, cid, pname, fname, lname, birthday
FROM patient
WHERE fname LIKE ?
   OR lname LIKE ?
   OR CONCAT_WS(' ', pname, fname, lname) LIKE ?
LIMIT ?"#;

/// Fetch a single patient's identity by HN.
///
/// Parameters: `(hn)`.
pub const PATIENT_BY_HN_SQL: &str = r#"
SELECT hn, cid, pname, fname, lname, birthday
FROM patient
WHERE hn = ?
LIMIT 1"#;

/// OPD dispensing history (orders/dispense via `opitemrece`).
///
/// `qty` is CAST to CHAR because sqlx cannot decode MySQL DECIMAL as `f64`;
/// the value is parsed in the client. `d.strength`/`d.units` are selected
/// with a fallback tier for instances lacking those columns (MySQL 1054).
///
/// Parameters: `(hn, cutoff)`.
pub const OPD_DISPENSE_SQL: &str = r#"
SELECT o.vn AS visit_id, o.hn, o.icode, CAST(o.qty AS CHAR) AS qty,
       o.vstdate AS disp_date,
       d.name AS drug_name, d.strength, d.units
FROM opitemrece o
JOIN drugitems d ON d.icode = o.icode
WHERE o.hn = ?
  AND o.vstdate >= ?
ORDER BY o.vstdate"#;

/// OPD dispensing history without the `strength`/`units` columns — same
/// result shape with `NULL` in their place (MySQL 1054 degradation).
pub const OPD_DISPENSE_SQL_FALLBACK: &str = r#"
SELECT o.vn AS visit_id, o.hn, o.icode, CAST(o.qty AS CHAR) AS qty,
       o.vstdate AS disp_date,
       d.name AS drug_name, NULL AS strength, NULL AS units
FROM opitemrece o
JOIN drugitems d ON d.icode = o.icode
WHERE o.hn = ?
  AND o.vstdate >= ?
ORDER BY o.vstdate"#;

/// IPD in-stay dispensing via `iptitemrece`, joined to the admission for
/// the medication date (`ipt.regdate`).
///
/// Parameters: `(hn, cutoff)`.
pub const IPD_DISPENSE_SQL: &str = r#"
SELECT i.an AS visit_id, i.hn, i.icode, CAST(i.qty AS CHAR) AS qty,
       ipt.regdate AS disp_date,
       d.name AS drug_name, d.strength, d.units
FROM iptitemrece i
JOIN ipt ON ipt.an = i.an
JOIN drugitems d ON d.icode = i.icode
WHERE i.hn = ?
  AND ipt.regdate >= ?
ORDER BY ipt.regdate"#;

/// IPD in-stay dispensing without the `strength`/`units` columns.
pub const IPD_DISPENSE_SQL_FALLBACK: &str = r#"
SELECT i.an AS visit_id, i.hn, i.icode, CAST(i.qty AS CHAR) AS qty,
       ipt.regdate AS disp_date,
       d.name AS drug_name, NULL AS strength, NULL AS units
FROM iptitemrece i
JOIN ipt ON ipt.an = i.an
JOIN drugitems d ON d.icode = i.icode
WHERE i.hn = ?
  AND ipt.regdate >= ?
ORDER BY ipt.regdate"#;

/// IPD take-home dispensing — `opitemrece` rows carrying an admission
/// number instead of `vn` (site variation; used when `iptitemrece` does not
/// exist on the instance).
///
/// Parameters: `(hn, cutoff)`.
pub const IPD_TAKEHOME_SQL: &str = r#"
SELECT o.an AS visit_id, o.hn, o.icode, CAST(o.qty AS CHAR) AS qty,
       o.vstdate AS disp_date,
       d.name AS drug_name, d.strength, d.units
FROM opitemrece o
JOIN drugitems d ON d.icode = o.icode
WHERE o.hn = ?
  AND o.vstdate >= ?
  AND o.an IS NOT NULL
ORDER BY o.vstdate"#;

/// IPD take-home dispensing without the `strength`/`units` columns.
pub const IPD_TAKEHOME_SQL_FALLBACK: &str = r#"
SELECT o.an AS visit_id, o.hn, o.icode, CAST(o.qty AS CHAR) AS qty,
       o.vstdate AS disp_date,
       d.name AS drug_name, NULL AS strength, NULL AS units
FROM opitemrece o
JOIN drugitems d ON d.icode = o.icode
WHERE o.hn = ?
  AND o.vstdate >= ?
  AND o.an IS NOT NULL
ORDER BY o.vstdate"#;

/// Sig (directions for use) from `medusage`, joined through `opitemrece`
/// so the HN filter needs no unconfirmed `medusage` columns.
///
/// **Gated behind `HosxpConfig::use_medusage_sig`** — the `medusage`
/// columns `qty_per_dose` / `frequency` / `unit` must be confirmed against
/// the live schema before enabling.
///
/// Parameters: `(hn, cutoff)`.
pub const MEDUSAGE_SIG_SQL: &str = r#"
SELECT m.vn, m.icode, m.qty_per_dose, m.frequency, m.unit
FROM medusage m
JOIN opitemrece o ON o.vn = m.vn AND o.icode = m.icode
WHERE o.hn = ?
  AND o.vstdate >= ?"#;

/// Allergy / ADR records.
///
/// Parameters: `(hn)`.
pub const ALLERGY_SQL: &str = r#"
SELECT hn, agent, symptom, allergy_group_id, severy_id, reporter
FROM opd_allergy
WHERE hn = ?
ORDER BY agent"#;

/// OPD visit history.
///
/// Parameters: `(hn, cutoff)`.
pub const OPD_VISIT_SQL: &str = r#"
SELECT vn, vstdate, main_dep
FROM ovst
WHERE hn = ?
  AND vstdate >= ?
ORDER BY vstdate"#;

/// IPD admission history.
///
/// Parameters: `(hn, cutoff)`.
pub const IPD_VISIT_SQL: &str = r#"
SELECT an, regdate, dchdate, ward
FROM ipt
WHERE hn = ?
  AND regdate >= ?
ORDER BY regdate"#;

/// Escape `%`, `_`, and `\` so a user query cannot act as a LIKE wildcard.
pub fn escape_like(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '%' | '_' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

/// Wrap a user query as a case-insensitive substring pattern.
pub fn like_pattern(query: &str) -> String {
    format!("%{}%", escape_like(query.trim()))
}

/// Wrap a user query as a case-insensitive prefix pattern.
pub fn prefix_pattern(query: &str) -> String {
    format!("{}%", escape_like(query.trim()))
}

/// Parse a HOSxP frequency value into sig dose/frequency.
///
/// Handles the common encodings seen in Thai HOSxP sites:
/// * plain number `"3"` → 3 times per day
/// * `"1x3"` / `"2 X 3"` → dose `1`/`2`, 3 times per day
/// * `"3/7"` → 3 times per week, approximated as `3/7` per day
/// * anything else → `None`
///
/// Heuristic, site-dependent — verify against live data before relying on it.
pub fn parse_frequency(raw: &str, qty_per_dose: Option<f64>) -> Option<(Option<f64>, f64)> {
    let cleaned = raw.trim().to_lowercase();
    if cleaned.is_empty() {
        return None;
    }

    if let Ok(n) = cleaned.parse::<f64>() {
        return Some((qty_per_dose, n));
    }

    let (a, b) = cleaned
        .split_once('x')
        .or_else(|| cleaned.split_once('X'))?;
    let dose = a.trim().parse::<f64>().ok()?;
    let freq = b.trim().parse::<f64>().ok()?;
    Some((Some(dose), freq))
}

/// Build a [`Sig`] from raw `medusage` fields.
pub fn sig_from_raw(
    qty_per_dose: Option<f64>,
    frequency: Option<&str>,
    note: Option<&str>,
) -> Option<Sig> {
    let (dose, freq) = parse_frequency(frequency?, qty_per_dose)?;
    Some(Sig {
        dose_per_admin: dose,
        frequency_per_day: Some(freq),
        note: note
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

/// Map a raw (visit_id, source, date, department) tuple to a [`VisitSummary`].
pub fn visit_summary(
    visit_id: String,
    source: EncounterSource,
    date: chrono::NaiveDate,
    department: Option<String>,
) -> VisitSummary {
    VisitSummary {
        visit_id,
        source,
        date,
        department,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_like_neutralizes_wildcards() {
        assert_eq!(escape_like("a%b_c\\d"), "a\\%b\\_c\\\\d");
    }

    #[test]
    fn like_pattern_wraps_and_escapes() {
        assert_eq!(like_pattern(" 50% "), "%50\\%%");
    }

    #[test]
    fn prefix_pattern_escapes_and_suffixes() {
        assert_eq!(prefix_pattern(" สมชาย "), "สมชาย%");
    }

    #[test]
    fn parse_frequency_plain_number() {
        assert_eq!(parse_frequency("3", None), Some((None, 3.0)));
        assert_eq!(parse_frequency(" 3 ", Some(1.0)), Some((Some(1.0), 3.0)));
    }

    #[test]
    fn parse_frequency_dose_x_times() {
        assert_eq!(parse_frequency("1x3", None), Some((Some(1.0), 3.0)));
        assert_eq!(parse_frequency("2 X 4", None), Some((Some(2.0), 4.0)));
    }

    #[test]
    fn parse_frequency_garbage_is_none() {
        assert_eq!(parse_frequency("หลังอาหาร", None), None);
        assert_eq!(parse_frequency("", None), None);
        assert_eq!(parse_frequency("3/7", None), None);
    }

    #[test]
    fn sig_from_raw_combines_fields() {
        let sig = sig_from_raw(Some(1.0), Some("1x3"), Some(" หลังอาหาร ")).unwrap();
        assert_eq!(sig.dose_per_admin, Some(1.0));
        assert_eq!(sig.frequency_per_day, Some(3.0));
        assert_eq!(sig.note.as_deref(), Some("หลังอาหาร"));
    }

    #[test]
    fn sig_from_raw_missing_frequency_is_none() {
        assert!(sig_from_raw(Some(1.0), None, None).is_none());
    }

    #[test]
    fn all_statements_are_read_only() {
        for stmt in [
            PATIENT_SEARCH_BY_CID,
            PATIENT_SEARCH_BY_HN,
            PATIENT_SEARCH_NAME_PREFIX,
            PATIENT_SEARCH_NAME_CONTAINS,
            PATIENT_BY_HN_SQL,
            OPD_DISPENSE_SQL,
            OPD_DISPENSE_SQL_FALLBACK,
            IPD_DISPENSE_SQL,
            IPD_DISPENSE_SQL_FALLBACK,
            IPD_TAKEHOME_SQL,
            IPD_TAKEHOME_SQL_FALLBACK,
            MEDUSAGE_SIG_SQL,
            ALLERGY_SQL,
            OPD_VISIT_SQL,
            IPD_VISIT_SQL,
        ] {
            assert!(
                crate::readonly::assert_read_only(stmt).is_ok(),
                "statement must be read-only: {stmt}"
            );
        }
    }
}
