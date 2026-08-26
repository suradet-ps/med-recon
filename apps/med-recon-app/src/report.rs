//! Medication history report: content assembly + A4 PDF generation.
//!
//! Content assembly (which strings, in what order) lives here as a pure
//! [`ReportModel`]; [`crate::pdf`] lays the model out on A4 pages and
//! renders it to PDF bytes via `pdf-writer`. The report always carries the
//! BPMH disclaimer: dispensing-derived data is one source among several
//! and must not be presented as a complete or verified list.
//!
//! Every user-visible string comes from [`ReportLabels`], resolved by the
//! frontend as fixed Thai - this module hard-codes no display text.

use chrono::Datelike;
use med_recon_core::{MedicationItem, MedicationStatus, PatientHistory};

/// User-visible strings for the report - fixed Thai, resolved by the
/// frontend (the UI is Thai-only).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportLabels {
    /// Report h1 heading.
    pub heading: String,
    /// Header sub-line template with `{date}`.
    pub generated: String,
    /// Fallback site label when no site name is configured.
    pub site_default: String,
    /// BPMH disclaimer paragraph.
    pub disclaimer: String,
    /// Patient identity section heading.
    pub section_patient: String,
    /// Allergy section heading template with `{n}`.
    pub section_allergy: String,
    /// Active-medications section heading template with `{n}` - same token
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
    /// Template with `{n}` - e.g. `dispense {n} ครั้ง`.
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
    /// Footer page-number template with `{page}` and `{total}`.
    pub page_of: String,
    /// PHI handling notice in the report footer.
    pub footer_phi: String,
}

/// Everything the PDF engine needs to know about the report. Pure data -
/// the layout stage never touches labels or the history again.
#[derive(Debug, Clone)]
pub struct ReportModel {
    /// Report h1 heading.
    pub heading: String,
    /// `{site} · {generated}` header sub-line.
    pub sub_line: String,
    /// BPMH disclaimer paragraph.
    pub disclaimer: String,
    /// Data-completeness warnings heading.
    pub warnings_title: String,
    /// Data-completeness warnings (already wrapped-ready strings).
    pub warnings: Vec<String>,
    /// Patient display name.
    pub patient_name: String,
    /// `HN x` with optional ` · CID y`.
    pub patient_meta: String,
    /// Allergy section heading (label + count filled).
    pub allergy_title: String,
    /// Allergy entries.
    pub allergies: Vec<AllergyEntry>,
    /// Medication sections (active, lapsed).
    pub sections: Vec<MedSection>,
    /// Visit history section heading (label + count filled).
    pub visits_title: String,
    /// Visit table column headers.
    pub visit_headers: Vec<String>,
    /// Visit table rows: date, type, department, visit id.
    pub visits: Vec<[String; 4]>,
    /// PHI handling notice.
    pub footer_phi: String,
    /// `Med Recon v{version}` line.
    pub version_line: String,
    /// Footer page-number template with `{page}`/`{total}`.
    pub page_of: String,
}

/// One medication section (ยาที่ผู้ป่วยเคยได้รับ / ยาตามอาการ).
#[derive(Debug, Clone)]
pub struct MedSection {
    /// Section heading (label + count filled).
    pub title: String,
    /// The medication rows.
    pub items: Vec<MedItem>,
}

/// One medication row.
#[derive(Debug, Clone)]
pub struct MedItem {
    /// Drug name (bold).
    pub title: String,
    /// Muted strength/units suffix, e.g. ` · 500 mg · เม็ด`.
    pub sub: Option<String>,
    /// Muted meta line: last dispense, visit count, total qty, sources.
    pub meta: Option<String>,
    /// Days-supply chip text, e.g. `supply ≈ 30 วัน`.
    pub chip: Option<String>,
    /// Directions-for-use line (brand green).
    pub sig: Option<String>,
}

/// One allergy entry.
#[derive(Debug, Clone)]
pub struct AllergyEntry {
    /// Agent (bold).
    pub agent: String,
    /// Muted detail line (symptom · reported on · by · note).
    pub detail: Option<String>,
}

/// Substitute `{key}` placeholders in a label template.
fn fill(tpl: &str, pairs: &[(&str, &str)]) -> String {
    let mut out = tpl.to_string();
    for (key, value) in pairs {
        out = out.replace(&format!("{{{key}}}"), value);
    }
    out
}

/// Assemble the report model for a patient history.
pub fn build_model(
    history: &PatientHistory,
    site_name: &str,
    labels: &ReportLabels,
    now: chrono::NaiveDate,
) -> ReportModel {
    let patient = &history.patient;
    // Empty site name falls back to the label so the header never renders a
    // bare separator.
    let site_label = if site_name.trim().is_empty() {
        labels.site_default.trim()
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
                parts.push(s.to_string());
            }
            if let Some(d) = a.report_date {
                parts.push(fill(&labels.reported_on, &[("date", &format_date(d))]));
            }
            if let Some(r) = a.reporter.as_deref() {
                parts.push(fill(&labels.by, &[("name", r)]));
            }
            if let Some(n) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                parts.push(fill(&labels.note, &[("note", n)]));
            }
            AllergyEntry {
                agent: a.agent.clone(),
                detail: (!parts.is_empty()).then(|| parts.join(" · ")),
            }
        })
        .collect();

    let med_item = |m: &MedicationItem| -> MedItem {
        let strength = m.strength.as_deref().unwrap_or_default();
        let units = m.units.as_deref().unwrap_or_default();
        let mut sub = String::new();
        if !strength.is_empty() {
            sub.push_str(" · ");
            sub.push_str(strength);
        }
        if !units.is_empty() {
            sub.push(' ');
            sub.push_str(units);
        }
        let sig = m.sig.as_ref().map(|s| {
            let dose = s.dose_per_admin.map(|d| format!("{d}"));
            let freq = s.frequency_per_day.map(|f| format!("{f}"));
            let note = s.note.as_deref().unwrap_or_default();
            match (dose, freq) {
                (Some(d), Some(f)) => format!("{d} × {f}{}", labels.freq_per_day),
                _ => note.to_string(),
            }
        });
        let sources = m
            .sources
            .iter()
            .map(|s| match s {
                med_recon_core::EncounterSource::Opd => "OPD",
                med_recon_core::EncounterSource::Ipd => "IPD",
            })
            .collect::<Vec<_>>()
            .join(" / ");
        MedItem {
            title: m.drug_name.clone(),
            sub: (!sub.is_empty()).then_some(sub),
            meta: Some(format!(
                "{} {} · {} · {} {} · {}",
                labels.last_dispensed,
                format_date(m.last_dispense),
                fill(&labels.dispenses, &[("n", &m.visit_count.to_string())]),
                labels.total,
                m.total_qty,
                sources,
            )),
            chip: m
                .days_supply
                .map(|d| fill(&labels.supply, &[("n", &d.to_string())])),
            sig: sig.filter(|s| !s.is_empty()),
        }
    };

    let sections = vec![
        MedSection {
            title: fill(&labels.section_active, &[("n", &active.len().to_string())]),
            items: active.iter().map(|m| med_item(m)).collect(),
        },
        MedSection {
            title: fill(&labels.section_lapsed, &[("n", &lapsed.len().to_string())]),
            items: lapsed.iter().map(|m| med_item(m)).collect(),
        },
    ];

    let visits = history
        .visits
        .iter()
        .map(|v| {
            let kind = match v.source {
                med_recon_core::EncounterSource::Opd => "OPD",
                med_recon_core::EncounterSource::Ipd => "IPD",
            };
            [
                format_date(v.date),
                kind.to_string(),
                v.department.clone().unwrap_or_default(),
                v.visit_id.clone(),
            ]
        })
        .collect();

    let mut patient_meta = format!("HN {}", patient.hn);
    if let Some(cid) = patient.cid.as_deref() {
        patient_meta.push_str(" · CID ");
        patient_meta.push_str(cid);
    }

    ReportModel {
        heading: labels.heading.clone(),
        sub_line: format!(
            "{site_label} · {}",
            fill(&labels.generated, &[("date", &format_date(now))])
        ),
        disclaimer: labels.disclaimer.clone(),
        warnings_title: labels.warnings_title.clone(),
        warnings: history.warnings.clone(),
        patient_name: patient.display_name(),
        patient_meta,
        allergy_title: fill(
            &labels.section_allergy,
            &[("n", &history.allergies.len().to_string())],
        ),
        allergies,
        sections,
        visits_title: fill(
            &labels.section_visits,
            &[("n", &history.visits.len().to_string())],
        ),
        visit_headers: vec![
            labels.col_date.clone(),
            labels.col_type.clone(),
            labels.col_dept.clone(),
            labels.col_visit.clone(),
        ],
        visits,
        footer_phi: labels.footer_phi.clone(),
        version_line: format!("Med Recon v{}", env!("CARGO_PKG_VERSION")),
        page_of: labels.page_of.clone(),
    }
}

/// Build the full A4 PDF report for a patient history.
pub fn build_report(history: &PatientHistory, site_name: &str, labels: &ReportLabels) -> Vec<u8> {
    let model = build_model(
        history,
        site_name,
        labels,
        chrono::Local::now().date_naive(),
    );
    let fonts = crate::pdf::Fonts::new();
    let mut pages = crate::pdf::layout(&model, &fonts);
    crate::pdf::finalize_pages(&mut pages);
    crate::pdf::write_pdf(&pages, &fonts)
}

/// Format a date in the Thai locale style (ค.ศ. year).
fn format_date(d: chrono::NaiveDate) -> String {
    format!("{:02}/{:02}/{}", d.day(), d.month(), d.year())
}

#[cfg(test)]
mod tests {
    use super::*;
    use med_recon_core::{AllergyRecord, Dispense, EncounterSource, PatientSummary, VisitSummary};

    /// Thai labels for tests - mirrors the frontend's fixed report labels.
    fn th_labels() -> ReportLabels {
        ReportLabels {
            heading: "ประวัติยาและการใช้ยา - Med Recon".into(),
            generated: "สร้างเมื่อ {date}".into(),
            site_default: "สถานบริการ".into(),
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
            page_of: "หน้า {page} / {total}".into(),
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

    fn model() -> ReportModel {
        build_model(
            &sample_history(),
            "รพ.ทดสอบ",
            &th_labels(),
            chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap(),
        )
    }

    #[test]
    fn model_contains_core_sections() {
        let m = model();
        assert_eq!(m.heading, "ประวัติยาและการใช้ยา - Med Recon");
        assert!(m.sub_line.starts_with("รพ.ทดสอบ ·"));
        assert_eq!(m.patient_name, "นาย สมชาย ใจดี");
        assert_eq!(m.patient_meta, "HN 0012345 · CID 1103700123456");
        assert_eq!(m.allergies.len(), 1);
        assert_eq!(m.allergies[0].agent, "Penicillin");
        assert!(m.allergies[0].detail.as_deref().unwrap().contains("ผื่น"));
        assert_eq!(m.sections.len(), 2);
        assert_eq!(m.sections[0].title, "ยาที่ผู้ป่วยเคยได้รับ (1)");
        assert_eq!(m.sections[0].items[0].title, "Paracetamol");
        assert!(
            m.sections[0].items[0]
                .sub
                .as_deref()
                .unwrap()
                .contains("500 mg")
        );
        assert!(
            m.sections[0].items[0]
                .meta
                .as_deref()
                .unwrap()
                .contains("dispense 1 ครั้ง")
        );
        assert_eq!(m.sections[1].title, "ยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ) (1)");
        assert_eq!(m.sections[1].items[0].title, "Metformin");
        assert_eq!(m.visits_title, "ประวัติการเข้ารับบริการ (1)");
        assert_eq!(m.visits[0][0], "01/07/2026");
        assert_eq!(m.visit_headers.len(), 4);
        assert_eq!(m.page_of, "หน้า {page} / {total}");
        assert!(m.version_line.starts_with("Med Recon v"));
    }

    #[test]
    fn model_renders_warnings() {
        let mut h = sample_history();
        h.warnings = vec!["ไม่พบตาราง drugusage/sp_use".to_string()];
        let m = build_model(
            &h,
            "รพ.ทดสอบ",
            &th_labels(),
            chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap(),
        );
        assert_eq!(m.warnings, vec!["ไม่พบตาราง drugusage/sp_use"]);
    }

    #[test]
    fn model_passes_through_arbitrary_user_text() {
        // No HTML layer anymore: report text is embedded verbatim (the PDF
        // writer hex-encodes every glyph), so markup-like input must
        // survive unchanged instead of being escaped.
        let mut h = sample_history();
        h.patient.first_name = "<script>alert(1)</script>".into();
        let m = build_model(
            &h,
            "site",
            &th_labels(),
            chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap(),
        );
        assert_eq!(m.patient_name, "นาย <script>alert(1)</script> ใจดี");
    }

    #[test]
    fn model_uses_labels_instead_of_hardcoded_text() {
        let mut labels = th_labels();
        labels.section_active = "ACTIVE_SECTION {n}".into();
        labels.heading = "HEADING".into();
        labels.supply = "SUPPLY {n}".into();
        let m = build_model(
            &sample_history(),
            "",
            &labels,
            chrono::NaiveDate::from_ymd_opt(2026, 8, 26).unwrap(),
        );
        assert_eq!(m.sections[0].title, "ACTIVE_SECTION 1");
        assert_eq!(m.heading, "HEADING");
        assert!(
            m.sub_line.starts_with("สถานบริการ ·"),
            "empty site falls back to the label"
        );
        assert!(!m.sub_line.contains("รพ."));
    }

    #[test]
    fn build_report_returns_valid_pdf() {
        let bytes = build_report(&sample_history(), "รพ.ทดสอบ", &th_labels());
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes.ends_with(b"%%EOF"));
        assert!(bytes.len() > 10_000, "fonts must be embedded");
    }
}
