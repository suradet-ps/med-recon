//! Printable HTML report generation for the medication history export.
//!
//! The report is self-contained (inline CSS, no external assets) so it can
//! be saved, printed, or emailed without a network connection. It always
//! carries the BPMH disclaimer: dispensing-derived data is one source among
//! several and must not be presented as a complete or verified list.

use chrono::Datelike;
use recon_core::{MedicationItem, MedicationStatus, PatientHistory};

/// Build the full HTML document for a patient history.
pub fn build_report(history: &PatientHistory, site_name: &str) -> String {
    let now = chrono::Local::now();
    let patient = &history.patient;

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
            format!(
                "<li class=\"allergy\"><strong>{}</strong>{}{}</li>",
                escape_html(&a.agent),
                a.symptom
                    .as_deref()
                    .map(|s| format!(" — {}", escape_html(s)))
                    .unwrap_or_default(),
                a.reporter
                    .as_deref()
                    .map(|r| format!(
                        " <span class=\"muted\">(รายงานโดย {})</span>",
                        escape_html(r)
                    ))
                    .unwrap_or_default()
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
                        (Some(d), Some(f)) => format!("{d} × {f}/วัน{note}"),
                        _ => note,
                    }
                });
                let sig_html = sig
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("<div class=\"sig\">{s}</div>"))
                    .unwrap_or_default();
                let supply = m
                    .days_supply
                    .map(|d| format!("<span class=\"chip\">supply ≈ {d} วัน</span>"))
                    .unwrap_or_default();
                let sources = m
                    .sources
                    .iter()
                    .map(|s| match s {
                        recon_core::EncounterSource::Opd => "OPD",
                        recon_core::EncounterSource::Ipd => "IPD",
                    })
                    .collect::<Vec<_>>()
                    .join(" / ");
                format!(
                    "<li class=\"med\">\
                       <div class=\"med-head\"><strong>{}</strong> <span class=\"muted\">{}{}</span></div>\
                       <div class=\"meta\">ครั้งล่าสุด {} · dispense {} ครั้ง · รวม {} · <span class=\"chip\">{}</span>{}</div>\
                       {}\
                     </li>",
                    escape_html(&m.drug_name),
                    if strength.is_empty() { String::new() } else { format!(" · {strength}") },
                    if units.is_empty() { String::new() } else { format!(" {units}") },
                    format_date(m.last_dispense),
                    m.visit_count,
                    m.total_qty,
                    sources,
                    supply,
                    sig_html,
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
                recon_core::EncounterSource::Opd => "OPD",
                recon_core::EncounterSource::Ipd => "IPD",
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
            "<section><h2>คำเตือนความครบถ้วนของข้อมูล</h2><ul style='color:#8a6d00'>{warnings}</ul></section>"
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="th">
<head>
<meta charset="utf-8">
<title>ประวัติยา {name} ({hn})</title>
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
    <h1>ประวัติยาและการใช้ยา — Recon</h1>
    <div class="sub">{site} · สร้างเมื่อ {generated}</div>
  </div>
  <div class="disclaimer">
    ⚠️ เอกสารนี้สร้างจากข้อมูลการจ่ายยา (dispensing) ใน HOSxP ซึ่งเป็นเพียงแหล่งข้อมูลหนึ่งในหลายแหล่ง
    สำหรับ Best Possible Medication History (BPMH) ยังไม่ถือว่าเป็นรายการยาที่สมบูรณ์หรือได้รับการยืนยัน
    ควรสอบทานร่วมกับผู้ป่วย/ญาติก่อนนำไปใช้ทางคลินิก
  </div>
  <main>
    <section>
      <h2>ข้อมูลผู้ป่วย</h2>
      <p style="margin:0">{name} — HN <strong>{hn}</strong>{cid}</p>
    </section>
    {warnings_html}
    <section>
      <h2>แพ้ยา / อาการไม่พึงประสงค์ ({allergy_count})</h2>
      <ul>{allergies}</ul>
    </section>
    <section>
      <h2>ยาที่คาดว่ายังใช้อยู่ (Active) — {active_count}</h2>
      <ul>{active_html}</ul>
    </section>
    <section>
      <h2>ยาที่คาดว่าหยุดใช้แล้ว (Lapsed) — {lapsed_count}</h2>
      <ul>{lapsed_html}</ul>
    </section>
    <section>
      <h2>ประวัติการเข้ารับบริการ ({visit_count})</h2>
      <table>
        <thead><tr><th>วันที่</th><th>ประเภท</th><th>แผนก/หอผู้ป่วย</th><th>รหัส visit</th></tr></thead>
        <tbody>{visits}</tbody>
      </table>
    </section>
  </main>
  <div class="footer">
    Recon v0.1.0 · ข้อมูลนี้เป็นข้อมูลสุขภาพส่วนบุคคล (PHI) ต้องจัดเก็บและส่งต่อตามระเบียบ
    ปฏิบัติด้านการคุ้มครองข้อมูลส่วนบุคคล
  </div>
</body>
</html>"#,
        site = escape_html(site_name),
        generated = format_date(now.date_naive()),
        name = escape_html(&patient.display_name()),
        hn = escape_html(&patient.hn),
        cid = patient
            .cid
            .as_deref()
            .map(|c| format!(" — CID {}", escape_html(c)))
            .unwrap_or_default(),
        warnings_html = warnings_html,
        allergy_count = history.allergies.len(),
        allergies = if allergies.is_empty() {
            "<p class=\"muted\">—</p>".to_string()
        } else {
            allergies
        },
        active_count = active.len(),
        active_html = meds_html(&active),
        lapsed_count = lapsed.len(),
        lapsed_html = meds_html(&lapsed),
        visit_count = history.visits.len(),
        visits = if visits.is_empty() {
            "<tr><td colspan=\"4\" class=\"muted\">—</td></tr>".to_string()
        } else {
            visits
        },
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
    use recon_core::{AllergyRecord, Dispense, EncounterSource, PatientSummary, VisitSummary};

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
        let medications =
            recon_core::aggregate_medications(&[d1, d2], chrono::Local::now().date_naive());
        PatientHistory {
            patient,
            medications,
            allergies: vec![AllergyRecord {
                agent: "Penicillin".into(),
                symptom: Some("ผื่น".into()),
                severity_id: None,
                group_id: None,
                reporter: Some("นส. nurse".into()),
            }],
            visits: vec![VisitSummary {
                visit_id: "vn1".into(),
                source: EncounterSource::Opd,
                date: chrono::NaiveDate::from_ymd_opt(2026, 7, 1).unwrap(),
                department: Some("OPD".into()),
            }],
            warnings: vec![],
        }
    }

    #[test]
    fn report_contains_core_sections() {
        let html = build_report(&sample_history(), "รพ.ทดสอบ");
        for needle in [
            "ประวัติยาและการใช้ยา",
            "ข้อมูลผู้ป่วย",
            "สมชาย ใจดี",
            "แพ้ยา",
            "Penicillin",
            "ยาที่คาดว่ายังใช้อยู่",
            "Paracetamol",
            "Metformin",
            "ยาที่คาดว่าหยุดใช้แล้ว",
            "ประวัติการเข้ารับบริการ",
            "รพ.ทดสอบ",
        ] {
            assert!(html.contains(needle), "report must contain {needle:?}");
        }
    }

    #[test]
    fn report_renders_warnings() {
        let mut h = sample_history();
        h.warnings = vec!["ไม่พบตาราง iptitemrece".to_string()];
        let html = build_report(&h, "รพ.ทดสอบ");
        assert!(html.contains("ไม่พบตาราง iptitemrece"));
    }

    #[test]
    fn report_escapes_html_in_user_data() {
        let mut h = sample_history();
        h.patient.first_name = "<script>alert(1)</script>".into();
        let html = build_report(&h, "site");
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>"));
    }
}
