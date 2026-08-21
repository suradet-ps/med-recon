//! Printable HTML report generation for the medication history export.
//!
//! The report is self-contained (inline CSS, no external assets) so it can
//! be saved, printed, or emailed without a network connection. It always
//! carries the BPMH disclaimer: dispensing-derived data is one source among
//! several and must not be presented as a complete or verified list.
//!
//! Every user-visible string comes from [`ReportLabels`], resolved by the
//! frontend as fixed Thai — this module hard-codes no display text.

use chrono::Datelike;
use med_recon_core::{MedicationItem, MedicationStatus, PatientHistory};

/// User-visible strings for the report — fixed Thai, resolved by the
/// frontend (the UI is Thai-only).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLabels {
    /// `<html lang>` attribute value (`"th"` / `"en"`).
    pub html_lang: String,
    /// Report h1 heading.
    pub heading: String,
    /// Header sub-line template with `{date}`.
    pub generated: String,
    /// Fallback site label when no site name is configured.
    pub site_default: String,
    /// `<title>` template with `{name}` and `{hn}`.
    pub title: String,
    /// BPMH disclaimer paragraph.
    pub disclaimer: String,
    /// Patient identity section heading.
    pub section_patient: String,
    /// Allergy section heading template with `{n}`.
    pub section_allergy: String,
    /// Active-medications section heading template with `{n}` — same token
    /// as the UI canvas so the wording stays identical everywhere.
    pub section_active: String,
    /// Lapsed-medications section heading template with `{n}`.
    pub section_lapsed: String,
    /// Visit history section heading template with `{n}`.
    pub section_visits: String,
    /// Visit table column headers.
    pub col_date: String,
    pub col_type: String,
    pub col_dept: String,
    pub col_visit: String,
    /// Medication meta line labels.
    pub last_dispensed: String,
    /// Template with `{n}` — e.g. `dispense {n} ครั้ง`.
    pub dispenses: String,
    pub total: String,
    /// Days-supply chip template with `{n}`.
    pub supply: String,
    /// Frequency unit suffix, e.g. `/วัน`.
    pub freq_per_day: String,
    /// Allergy meta templates.
    pub reported_on: String,
    pub by: String,
    pub note: String,
    /// Data-completeness warnings heading (same token as the UI).
    pub warnings_title: String,
    /// PHI handling notice in the report footer.
    pub footer_phi: String,
}

/// Substitute `{key}` placeholders in a label template.
fn fill(tpl: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (key, value) in pairs {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

/// Build the full HTML document for a patient history.
pub fn build_report(history: &PatientHistory, site_name: &str, labels: &ReportLabels) -> String {
    let now = chrono::Local::now();
    let patient = &history.patient;
    // Empty site name falls back to the label so the header never renders a
    // bare separator.
    let site_label = if site_name.trim().is_empty() {
        &labels.site_default
    } else {
        site_name.trim()
    };

    let active: Vec<&MedicationItem> = history
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .collect();
    let lapsed: Vec<&MedicationItem> = history
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Lapsed)
        .collect();

    let allergies = history
        .allergies
        .iter()
        .map(|a| {
            let mut parts: Vec<String> = Vec::new();
            if let Some(s) = a.symptom.as_deref() {
                parts.push(escape_html(s).to_string());
            }
            if let Some(d) = a.report_date {
                parts.push(fill(&labels.reported_on, &[("date", &format_date(d))]));
            }
            if let Some(r) = a.reporter.as_deref() {
                parts.push(fill(&labels.by, &[("name", &escape_html(r))]));
            }
            if let Some(n) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                parts.push(fill(&labels.note, &[("note", &escape_html(n))]));
            }
            let detail = if parts.is_empty() {
                String::new()
            } else {
                format!("<span class=\"muted\"> — {}</span>", parts.join(" · "))
            };
            format!(
                "<li class=\"allergy\"><strong>{}</strong>{detail}</li>",
                escape_html(&a.agent)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let meds_html = |items: &[&MedicationItem]| -> String {
        if items.is_empty() {
            return "<p class=\"muted\">—</p>".to_string();
        }
        items
            .iter()
            .map(|m| {
                let strength = m.strength.as_deref().map(escape_html).unwrap_or_default();
                let units = m.units.as_deref().map(escape_html).unwrap_or_default();
                let sig = m.sig.as_ref().map(|s| {
                    let dose = s.dose_per_admin.map(|d| format!("{d}"));
                    let freq = s.frequency_per_day.map(|f| format!("{f}"));
                    let note = s.note.as_deref().map(escape_html).unwrap_or_default();
                    match (dose, freq) {
                        (Some(d), Some(f)) => {
                            format!("{d} × {f}{}", labels.freq_per_day)
                        }
                        _ => note,
                    }
                });
                let sig_html = sig
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("<div class=\"sig\">{s}</div>"))
                    .unwrap_or_default();
                let supply = m
                    .days_supply
                    .map(|d| {
                        format!(
                            "<span class=\"chip\">{}</span>",
                            fill(&labels.supply, &[("n", &d.to_string())])
                        )
                    })
                    .unwrap_or_default();
                let sources = m
                    .sources
                    .iter()
                    .map(|s| match s {
                        med_recon_core::EncounterSource::Opd => "OPD",
                        med_recon_core::EncounterSource::Ipd => "IPD",
                    })
                    .collect::<Vec<_>>()
                    .join(" / ");
                format!(
                    "<li class=\"med\">\
                       <div class=\"med-head\"><strong>{}</strong> <span class=\"muted\">{}{}</span></div>\
                       <div class=\"meta\">{last} {} · {dispenses} · {total} {} · <span class=\"chip\">{}</span>{}</div>\
                       {}\
                     </li>",
                    escape_html(&m.drug_name),
                    if strength.is_empty() { String::new() } else { format!(" · {strength}") },
                    if units.is_empty() { String::new() } else { format!(" {units}") },
                    format_date(m.last_dispense),
                    m.total_qty,
                    sources,
                    supply,
                    sig_html,
                    last = labels.last_dispensed.clone(),
                    dispenses = fill(&labels.dispenses, &[("n", &m.visit_count.to_string())]),
                    total = labels.total.clone(),
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let visits = history
        .visits
        .iter()
        .map(|v| {
            let kind = match v.source {
                med_recon_core::EncounterSource::Opd => "OPD",
                med_recon_core::EncounterSource::Ipd => "IPD",
            };
            let dept = v.department.as_deref().map(escape_html).unwrap_or_default();
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                format_date(v.date),
                kind,
                dept,
                escape_html(&v.visit_id)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let warnings = history
        .warnings
        .iter()
        .map(|w| format!("<li>{}</li>", escape_html(w)))
        .collect::<Vec<_>>()
        .join("\n");
    let warnings_html = if warnings.is_empty() {
        String::new()
    } else {
        format!(
            "<section><h2>{}</h2><ul style='color:#8a6d00'>{warnings}</ul></section>",
            escape_html(&labels.warnings_title)
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="{lang}">
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>
  :root {{
    --green: #006241; --accent: #00754A; --house: #1E3932;
    --canvas: #f2f0eb; --card: #ffffff; --red: #c82014;
    --text: rgba(0,0,0,.87); --text-soft: rgba(0,0,0,.58);
  }}
  * {{ box-sizing: border-box; }}
  body {{
    font-family: "Inter", "Helvetica Neue", Helvetica, Arial, sans-serif;
    margin: 0; background: var(--canvas); color: var(--text);
    letter-spacing: -0.01em; line-height: 1.5;
  }}
  .header {{ background: var(--house); color: #fff; padding: 32px 40px; }}
  .header h1 {{ margin: 0 0 4px; font-size: 24px; font-weight: 600; letter-spacing: -0.16px; }}
  .header .sub {{ color: rgba(255,255,255,.70); font-size: 13px; }}
  .disclaimer {{
    background: var(--house); color: rgba(255,255,255,.90);
    padding: 12px 40px; font-size: 13px; border-top: 1px solid rgba(255,255,255,.15);
  }}
  main {{ padding: 24px 40px 48px; max-width: 900px; }}
  section {{ background: var(--card); border-radius: 12px; padding: 20px 24px; margin-bottom: 20px;
    box-shadow: 0 0 .5px rgba(0,0,0,.14), 0 1px 1px rgba(0,0,0,.24); }}
  h2 {{ color: var(--green); font-size: 19px; font-weight: 600; margin: 0 0 12px; letter-spacing: -0.16px; }}
  ul {{ list-style: none; margin: 0; padding: 0; }}
  .med {{ padding: 10px 0; border-top: 1px solid #e7e7e7; }}
  .med:first-child {{ border-top: none; }}
  .med-head {{ font-size: 15px; }}
  .meta {{ font-size: 13px; color: var(--text-soft); margin-top: 2px; }}
  .sig {{ font-size: 13px; color: var(--green); margin-top: 4px; }}
  .chip {{ display: inline-block; border: 1px solid var(--accent); color: var(--accent);
    border-radius: 50px; padding: 1px 10px; font-size: 12px; }}
  .allergy {{ border: 1px solid var(--red); color: var(--red); border-radius: 8px;
    padding: 8px 12px; margin-top: 6px; font-size: 14px; }}
  .muted {{ color: var(--text-soft); font-weight: 400; }}
  table {{ width: 100%; border-collapse: collapse; font-size: 13px; }}
  th, td {{ text-align: left; padding: 6px 8px; border-bottom: 1px solid #e7e7e7; }}
  th {{ color: var(--text-soft); font-weight: 600; }}
  .footer {{ color: var(--text-soft); font-size: 12px; padding: 0 40px 40px; }}
  @media print {{ body {{ background: #fff; }} section {{ box-shadow: none; }} }}
</style>
</head>
<body>
  <div class="header">
    <h1>{heading}</h1>
    <div class="sub">{site} · {generated}</div>
  </div>
  <div class="disclaimer">
    {disclaimer}
  </div>
  <main>
    <section>
      <h2>{section_patient}</h2>
      <p style="margin:0">{name} — HN <strong>{hn}</strong>{cid}</p>
    </section>
    {warnings_html}
    <section>
      <h2>{section_allergy}</h2>
      <ul>{allergies}</ul>
    </section>
    <section>
      <h2>{section_active}</h2>
      <ul>{active_html}</ul>
    </section>
    <section>
      <h2>{section_lapsed}</h2>
      <ul>{lapsed_html}</ul>
    </section>
    <section>
      <h2>{section_visits}</h2>
      <table>
        <thead><tr><th>{col_date}</th><th>{col_type}</th><th>{col_dept}</th><th>{col_visit}</th></tr></thead>
        <tbody>{visits}</tbody>
      </table>
    </section>
  </main>
  <div class="footer">
    Med Recon v0.1.0 · {footer_phi}
  </div>
</body>
</html>"#,
        lang = escape_html(&labels.html_lang),
        title = escape_html(&fill(
            &labels.title,
            &[("name", &patient.display_name()), ("hn", &patient.hn)],
        )),
        heading = escape_html(&labels.heading),
        site = escape_html(site_label),
        generated = fill(
            &labels.generated,
            &[("date", &format_date(now.date_naive()))]
        ),
        disclaimer = escape_html(&labels.disclaimer),
        section_patient = escape_html(&labels.section_patient),
        name = escape_html(&patient.display_name()),
        hn = escape_html(&patient.hn),
        cid = patient
            .cid
            .as_deref()
            .map(|c| format!(" — CID {}", escape_html(c)))
            .unwrap_or_default(),
        warnings_html = warnings_html,
        section_allergy = escape_html(&fill(
            &labels.section_allergy,
            &[("n", &history.allergies.len().to_string())],
        )),
        allergies = if allergies.is_empty() {
            "<p class=\"muted\">—</p>".to_string()
        } else {
            allergies
        },
        section_active = escape_html(&fill(
            &labels.section_active,
            &[("n", &active.len().to_string())],
        )),
        active_html = meds_html(&active),
        section_lapsed = escape_html(&fill(
            &labels.section_lapsed,
            &[("n", &lapsed.len().to_string())],
        )),
        lapsed_html = meds_html(&lapsed),
        section_visits = escape_html(&fill(
            &labels.section_visits,
            &[("n", &history.visits.len().to_string())],
        )),
        col_date = escape_html(&labels.col_date),
        col_type = escape_html(&labels.col_type),
        col_dept = escape_html(&labels.col_dept),
        col_visit = escape_html(&labels.col_visit),
        visits = if visits.is_empty() {
            "<tr><td colspan=\"4\" class=\"muted\">—</td></tr>".to_string()
        } else {
            visits
        },
        footer_phi = escape_html(&labels.footer_phi),
    )
}

/// Escape HTML special characters in user/patient-provided text.
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Format a date in the Thai locale style (ค.ศ. year).
fn format_date(d: chrono::NaiveDate) -> String {
    format!("{:02}/{:02}/{}", d.day(), d.month(), d.year())
}

#[cfg(test)]
mod tests {
    use super::*;
    use med_recon_core::{AllergyRecord, Dispense, EncounterSource, PatientSummary, VisitSummary};

    /// Thai labels for tests — mirrors the frontend's fixed report labels.
    fn th_labels() -> ReportLabels {
        ReportLabels {
            html_lang: "th".into(),
            heading: "ประวัติยาและการใช้ยา — Med Recon".into(),
            generated: "สร้างเมื่อ {date}".into(),
            site_default: "สถานบริการ".into(),
            title: "ประวัติยา {name} ({hn})".into(),
            disclaimer: "เอกสารนี้สร้างจากข้อมูลการจ่ายยา".into(),
            section_patient: "ข้อมูลผู้ป่วย".into(),
            section_allergy: "แพ้ยา / อาการไม่พึงประสงค์ ({n})".into(),
            section_active: "ยาที่ผู้ป่วยเคยได้รับ ({n})".into(),
            section_lapsed: "ยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ) ({n})".into(),
            section_visits: "ประวัติการเข้ารับบริการ ({n})".into(),
            col_date: "วันที่".into(),
            col_type: "ประเภท".into(),
            col_dept: "แผนก/หอผู้ป่วย".into(),
            col_visit: "รหัส visit".into(),
            last_dispensed: "ครั้งล่าสุด".into(),
            dispenses: "dispense {n} ครั้ง".into(),
            total: "รวม".into(),
            supply: "supply ≈ {n} วัน".into(),
            freq_per_day: "/วัน".into(),
            reported_on: "รายงานเมื่อ {date}".into(),
            by: "โดย {name}".into(),
            note: "หมายเหตุ: {note}".into(),
            warnings_title: "คำเตือนความครบถ้วนของข้อมูล".into(),
            footer_phi: "ข้อมูลนี้เป็นข้อมูลสุขภาพส่วนบุคคล".into(),
        }
    }

    fn sample_history() -> PatientHistory {
        let patient = PatientSummary {
            hn: "0012345".into(),
            cid: Some("1103700123456".into()),
            title: Some("นาย".into()),
            first_name: "สมชาย".into(),
            last_name: "ใจดี".into(),
            birthday: None,
        };
        let disp = |icode: &str, name: &str, qty: f64, date: chrono::NaiveDate| Dispense {
            hn: "0012345".into(),
            visit_id: "vn1".into(),
            source: EncounterSource::Opd,
            icode: icode.into(),
            drug_name: name.into(),
            strength: Some("500 mg".into()),
            units: Some("เม็ด".into()),
            qty,
            date,
            sig: None,
            appointment: None,
        };
        let d1 = disp(
            "P1",
            "Paracetamol",
            30.0,
            chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
        );
        let d2 = disp(
            "M1",
            "Metformin",
            90.0,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        );
        let medications = med_recon_core::aggregate_medications(
            &[d1, d2],
            chrono::Local::now().date_naive(),
            &["P1".to_string()].into_iter().collect(),
        );
        PatientHistory {
            patient,
            medications,
            allergies: vec![AllergyRecord {
                agent: "Penicillin".into(),
                symptom: Some("ผื่น".into()),
                report_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap()),
                note: Some("แจ้งผู้ป่วยแล้ว".into()),
                reporter: Some("นส. nurse".into()),
            }],
            visits: vec![VisitSummary {
                visit_id: "vn1".into(),
                source: EncounterSource::Opd,
                date: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                department: Some("OPD".into()),
            }],
            screen_records: vec![],
            pmh: None,
            warnings: vec![],
        }
    }

    #[test]
    fn report_contains_core_sections() {
        let html = build_report(&sample_history(), "รพ.ทดสอบ", &th_labels());
        for needle in [
            "ประวัติยาและการใช้ยา",
            "ข้อมูลผู้ป่วย",
            "สมชาย ใจดี",
            "แพ้ยา",
            "Penicillin",
            "ยาที่ผู้ป่วยเคยได้รับ",
            "Paracetamol",
            "Metformin",
            "ยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ)",
            "ประวัติการเข้ารับบริการ",
            "รพ.ทดสอบ",
        ] {
            assert!(html.contains(needle), "report must contain {needle:?}");
        }
    }

    #[test]
    fn report_renders_warnings() {
        let mut h = sample_history();
        h.warnings = vec!["ไม่พบตาราง drugusage/sp_use".to_string()];
        let html = build_report(&h, "รพ.ทดสอบ", &th_labels());
        assert!(html.contains("ไม่พบตาราง drugusage/sp_use"));
    }

    #[test]
    fn report_escapes_html_in_user_data() {
        let mut h = sample_history();
        h.patient.first_name = "<script>alert(1)</script>".into();
        let html = build_report(&h, "site", &th_labels());
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }

    #[test]
    fn report_uses_labels_instead_of_hardcoded_text() {
        let mut labels = th_labels();
        labels.section_active = "ACTIVE_SECTION {n}".into();
        labels.heading = "HEADING".into();
        let html = build_report(&sample_history(), "", &labels);
        assert!(html.contains("ACTIVE_SECTION 1"));
        assert!(!html.contains("ยาที่ผู้ป่วยเคยได้รับ (1)"));
        assert!(!html.contains("ประวัติยาและการใช้ยา"));
        assert!(html.contains("HEADING"));
        assert!(
            html.contains("สถานบริการ"),
            "empty site falls back to the label"
        );
    }
}
