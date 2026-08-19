//! SQL statements and pure query-building helpers for HOSxP.
//!
//! All statements are `SELECT`-only, use positional `?` parameters (binary
//! protocol — no string interpolation of user input), and follow the schema
//! reference in AGENTS.md. Every public statement constant is exercised by
//! the read-only guard before execution in `client.rs`.

use med_recon_core::{EncounterSource, Sig, VisitSummary};

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

/// Patient photo (`patient_image.image`, JPEG/PNG BLOB) keyed by HN.
///
/// Photos are supplementary identity data: if the table or column is
/// missing on this site (MySQL 1146/1054) the client degrades silently to
/// `None` and the UI shows a placeholder avatar — never a load failure.
///
/// Parameters: `(hn)`.
pub const PATIENT_IMAGE_SQL: &str = r#"
SELECT image
FROM patient_image
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
  AND (o.an IS NULL OR TRIM(o.an) = '')
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
  AND (o.an IS NULL OR TRIM(o.an) = '')
ORDER BY o.vstdate"#;

/// IPD dispensing — `opitemrece` rows carrying an admission number instead
/// of a visit number. All dispensing is stored in `opitemrece`; the OPD/IPD
/// split is `vn` = OPD vs `an` = IPD.
///
/// Parameters: `(hn, cutoff)`.
pub const IPD_DISPENSE_SQL: &str = r#"
SELECT o.an AS visit_id, o.hn, o.icode, CAST(o.qty AS CHAR) AS qty,
       o.vstdate AS disp_date,
       d.name AS drug_name, d.strength, d.units
FROM opitemrece o
JOIN drugitems d ON d.icode = o.icode
WHERE o.hn = ?
  AND o.vstdate >= ?
  AND o.an IS NOT NULL AND TRIM(o.an) <> ''
ORDER BY o.vstdate"#;

/// IPD dispensing without the `strength`/`units` columns.
pub const IPD_DISPENSE_SQL_FALLBACK: &str = r#"
SELECT o.an AS visit_id, o.hn, o.icode, CAST(o.qty AS CHAR) AS qty,
       o.vstdate AS disp_date,
        d.name AS drug_name, NULL AS strength, NULL AS units
FROM opitemrece o
JOIN drugitems d ON d.icode = o.icode
WHERE o.hn = ?
  AND o.vstdate >= ?
  AND o.an IS NOT NULL AND TRIM(o.an) <> ''
ORDER BY o.vstdate"#;

/// Sig (directions for use) — `opitemrece.drugusage` / `opitemrece.sp_use`
/// hold codes resolved through the `drugusage` and `sp_use` lookup tables
/// (`name1`/`name2`/`name3` each). LEFT JOIN so rows without a code still
/// come back with empty sig text. Covers both OPD (`vn`) and IPD (`an`)
/// dispensing rows; the client keys each row by whichever visit id is set.
///
/// Parameters: `(hn, cutoff)`.
pub const SIG_SQL: &str = r#"
SELECT o.vn, o.an, o.icode,
       d.name1 AS d_name1, d.name2 AS d_name2, d.name3 AS d_name3,
       s.name1 AS s_name1, s.name2 AS s_name2, s.name3 AS s_name3
FROM opitemrece o
LEFT JOIN drugusage d ON d.drugusage = o.drugusage
LEFT JOIN sp_use s ON s.sp_use = o.sp_use
WHERE o.hn = ?
  AND o.vstdate >= ?"#;

/// OPD screening records (`opdscreen`) — chief complaint and physical exam
/// text. Columns confirmed against the target site's live schema.
///
/// Parameters: `(hn, cutoff)`.
pub const OPD_SCREEN_SQL: &str = r#"
SELECT vn, vstdate, cc, pe
FROM opdscreen
WHERE hn = ?
  AND vstdate >= ?
ORDER BY vstdate DESC"#;

/// Allergy / ADR records.
///
/// Columns confirmed against the live schema: `report_date` (date),
/// `note` (text), `reporter` (varchar). `allergy_group_id` is not loaded —
/// its meaning is site-dependent and it is not displayed.
///
/// Parameters: `(hn)`.
pub const ALLERGY_SQL: &str = r#"
SELECT hn, agent, symptom, reporter, report_date, note
FROM opd_allergy
WHERE hn = ?
ORDER BY agent"#;

/// Drug master search for the current-medication settings.
///
/// Parameters: `(pattern, pattern, limit)` — case-insensitive LIKE against
/// `icode` and `name`, sorted by name.
pub const DRUG_SEARCH_SQL: &str = r#"
SELECT icode, name, strength, units
FROM drugitems
WHERE icode LIKE ?
   OR name LIKE ?
ORDER BY name
LIMIT ?"#;

/// Resolve a list of `icode`s back to drug master rows (current-medication
/// settings display).
///
/// Parameters: one `?` per code.
pub fn drugs_by_codes_sql(codes: &[String]) -> String {
    let placeholders = vec!["?"; codes.len()].join(", ");
    format!(
        "SELECT icode, name, strength, units FROM drugitems WHERE icode IN ({placeholders}) ORDER BY name"
    )
}

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

/// Next-appointment dates from `oapp`, keyed by OPD visit (`vn`).
///
/// A visit may hold several `oapp` rows; the latest planned follow-up
/// (`nextdate`) wins. `nextdate` may be พ.ศ. — era-normalized per value in
/// the client.
///
/// Parameters: `(hn, cutoff)`.
pub const APPOINTMENT_SQL: &str = r#"
SELECT vn, MAX(nextdate) AS nextdate
FROM oapp
WHERE hn = ?
  AND vstdate >= ?
  AND nextdate IS NOT NULL
GROUP BY vn"#;

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

/// Build a [`Sig`] from the `drugusage` / `sp_use` name texts.
///
/// The non-empty names are joined into the sig note. The first
/// `drugusage` name is also parsed as a dose×frequency pattern (e.g.
/// `"1x3"`, `"3"`) when possible; anything else leaves dose/frequency
/// unset and the raw text is carried in `note`.
pub fn sig_from_names(
    drugusage_names: &[Option<String>],
    sp_use_names: &[Option<String>],
) -> Option<Sig> {
    let parts: Vec<String> = drugusage_names
        .iter()
        .chain(sp_use_names)
        .filter_map(|n| {
            n.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
        .collect();
    let note = parts.join(" ");
    if note.is_empty() {
        return None;
    }
    let (dose_per_admin, frequency_per_day) = parts
        .first()
        .and_then(|first| parse_frequency(first, None))
        .map(|(dose, freq)| (dose, Some(freq)))
        .unwrap_or((None, None));
    Some(Sig {
        dose_per_admin,
        frequency_per_day,
        note: Some(note),
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
    fn sig_from_names_joins_and_parses() {
        let sig = sig_from_names(
            &[Some("1x3".into()), Some("หลังอาหาร".into()), None],
            &[Some("รับประทาน".into()), None, None],
        )
        .unwrap();
        assert_eq!(sig.dose_per_admin, Some(1.0));
        assert_eq!(sig.frequency_per_day, Some(3.0));
        assert_eq!(sig.note.as_deref(), Some("1x3 หลังอาหาร รับประทาน"));
    }

    #[test]
    fn sig_from_names_note_only_when_unparseable() {
        let sig = sig_from_names(&[Some("หลังอาหาร".into())], &[]).unwrap();
        assert_eq!(sig.dose_per_admin, None);
        assert_eq!(sig.frequency_per_day, None);
        assert_eq!(sig.note.as_deref(), Some("หลังอาหาร"));
    }

    #[test]
    fn sig_from_names_all_empty_is_none() {
        assert!(sig_from_names(&[None, None, None], &[None]).is_none());
        assert!(sig_from_names(&[], &[]).is_none());
    }

    #[test]
    fn all_statements_are_read_only() {
        for stmt in [
            PATIENT_SEARCH_BY_CID,
            PATIENT_SEARCH_BY_HN,
            PATIENT_SEARCH_NAME_PREFIX,
            PATIENT_SEARCH_NAME_CONTAINS,
            PATIENT_BY_HN_SQL,
            PATIENT_IMAGE_SQL,
            OPD_DISPENSE_SQL,
            OPD_DISPENSE_SQL_FALLBACK,
            IPD_DISPENSE_SQL,
            IPD_DISPENSE_SQL_FALLBACK,
            SIG_SQL,
            ALLERGY_SQL,
            OPD_SCREEN_SQL,
            DRUG_SEARCH_SQL,
            OPD_VISIT_SQL,
            IPD_VISIT_SQL,
            APPOINTMENT_SQL,
        ] {
            assert!(
                crate::readonly::assert_read_only(stmt).is_ok(),
                "statement must be read-only: {stmt}"
            );
        }
    }
}
