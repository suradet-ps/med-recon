//! Lightweight Thai/English UI dictionary.
//!
//! International OSS baseline: the UI ships bilingual (ไทย / English).
//! Backend error messages (see `ApiError`) are Thai and shown verbatim.

/// Supported UI languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// ภาษาไทย
    Thai,
    /// English
    English,
}

impl Lang {
    /// Native label for the language toggle.
    pub fn label(self) -> &'static str {
        match self {
            Lang::Thai => "English",
            Lang::English => "ไทย",
        }
    }

    /// Toggle to the other language.
    pub fn toggle(self) -> Lang {
        match self {
            Lang::Thai => Lang::English,
            Lang::English => Lang::Thai,
        }
    }

    /// Persisted key.
    pub fn storage_key(self) -> &'static str {
        match self {
            Lang::Thai => "th",
            Lang::English => "en",
        }
    }

    /// Load from `localStorage`, defaulting to Thai.
    pub fn from_storage() -> Lang {
        if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage())
            && let Ok(Some(v)) = storage.get_item("recon.lang")
            && v == "en"
        {
            return Lang::English;
        }
        Lang::Thai
    }

    /// Persist the choice.
    pub fn save(self) {
        if let Some(Ok(Some(storage))) = web_sys::window().map(|w| w.local_storage()) {
            let _ = storage.set_item("recon.lang", self.storage_key());
        }
    }
}

/// UI strings. Keys are short identifiers; every key has both languages.
pub const KEYS: &[(&str, &str, &str)] = &[
    ("app.name", "Recon", "Recon"),
    (
        "app.tagline",
        "ประวัติยาผู้ป่วยจาก HOSxP",
        "Patient medication history from HOSxP",
    ),
    ("top.health.connected", "เชื่อมต่อแล้ว", "Connected"),
    (
        "top.health.disconnected",
        "ไม่สามารถเชื่อมต่อได้",
        "Disconnected",
    ),
    ("top.health.unconfigured", "ยังไม่ได้ตั้งค่า", "Not configured"),
    ("top.settings", "ตั้งค่า", "Settings"),
    ("settings.title", "ตั้งค่า HOSxP", "HOSxP settings"),
    (
        "settings.status_ok",
        "เชื่อมต่อแล้ว — เข้ารหัสเก็บในเครื่อง",
        "Connected — stored encrypted",
    ),
    ("settings.status_none", "ยังไม่ได้ตั้งค่า", "Not configured"),
    ("settings.host", "Host", "Host"),
    ("settings.port", "Port", "Port"),
    ("settings.database", "Database", "Database"),
    (
        "settings.user",
        "User (แนะนำ: บัญชีอ่านอย่างเดียว)",
        "User (read-only role recommended)",
    ),
    ("settings.password", "Password", "Password"),
    ("settings.site_name", "ชื่อสถานบริการ", "Site name"),
    (
        "settings.site_name_placeholder",
        "เช่น โรงพยาบาลสมมติ (แสดงในรายงาน)",
        "e.g. Somdej Hospital (shown on reports)",
    ),
    (
        "settings.section_connection",
        "การเชื่อมต่อ HOSxP",
        "HOSxP connection",
    ),
    (
        "settings.tab_connection",
        "การเชื่อมต่อ",
        "Connection",
    ),
    (
        "settings.tab_site",
        "ตั้งค่าอื่นๆ",
        "Site settings",
    ),
    (
        "settings.section_site",
        "การตั้งค่าอื่นๆ",
        "Site settings",
    ),
    (
        "settings.history_days",
        "ค้นประวัติย้อนหลัง (วัน)",
        "History window (days)",
    ),
    ("settings.test", "ทดสอบ", "Test"),
    ("settings.save", "บันทึก", "Save"),
    ("settings.cancel", "ปิด", "Close"),
    ("settings.testing", "กำลังทดสอบ…", "Testing…"),
    ("settings.saving", "กำลังบันทึก…", "Saving…"),
    (
        "settings.meds_title",
        "ตั้งค่ายาที่ใช้อยู่",
        "Set current medications",
    ),
    (
        "settings.meds_note",
        "เฉพาะยาที่ตั้งค่าไว้เท่านั้นจะแสดงในหัวข้อ ยาที่คาดว่ายังใช้อยู่ — ยาที่ไม่ตั้งค่า (แม้เพิ่งได้รับ) จะถือว่าหยุดใช้แล้ว",
        "Only configured drugs are shown under 'likely active' — any drug not configured (even recently dispensed) is treated as stopped.",
    ),
    (
        "settings.meds_search",
        "ค้นหาชื่อยา…",
        "Search drug name…",
    ),
    (
        "settings.meds_add",
        "เพิ่ม",
        "Add",
    ),
    (
        "settings.meds_remove",
        "ลบ",
        "Remove",
    ),
    (
        "settings.meds_results",
        "ผลการค้นหา ({n})",
        "Search results ({n})",
    ),
    (
        "settings.meds_selected",
        "ยาที่ตั้งค่าไว้ ({n})",
        "Configured medications ({n})",
    ),
    (
        "settings.meds_no_results",
        "ไม่พบรายการยา",
        "No drugs found",
    ),
    (
        "settings.meds_empty",
        "ยังไม่ได้ตั้งค่ายา — ยาทั้งหมดจะถือว่าหยุดใช้แล้ว",
        "No medications configured — all drugs are treated as stopped.",
    ),
    ("settings.save_settings", "บันทึกการตั้งค่า", "Save settings"),
    (
        "settings.note",
        "ข้อมูลการเชื่อมต่อถูกเข้ารหัส AES-256-GCM เก็บ master key ใน Keychain ของระบบปฏิบัติการ",
        "Connection details are AES-256-GCM encrypted; the master key lives in the OS keychain.",
    ),
    (
        "settings.error_required",
        "กรอก Site name, Host, Database, User ให้ครบ",
        "Fill in Site name, Host, Database, and User",
    ),
    ("settings.error_port", "พอร์ตไม่ถูกต้อง", "Invalid port"),
    (
        "settings.test_ok",
        "เชื่อมต่อได้ (latency {ms} ms)",
        "Connection OK ({ms} ms)",
    ),
    (
        "settings.test_fail",
        "เชื่อมต่อไม่สำเร็จ: {msg}",
        "Connection failed: {msg}",
    ),
    (
        "settings.save_ok",
        "บันทึกการตั้งค่าและเชื่อมต่อแล้ว",
        "Settings saved and connected",
    ),
    (
        "search.placeholder",
        "ชื่อ-สกุล, HN หรือ CID",
        "Name, HN, or CID",
    ),
    (
        "search.hint.cid",
        "ค้นหาด้วยเลขบัตรประชาชน 13 หลัก",
        "Searching by 13-digit national ID",
    ),
    (
        "search.hint.hn",
        "ค้นหาด้วย HN ของโรงพยาบาล",
        "Searching by hospital HN",
    ),
    (
        "search.hint.name",
        "ค้นหาด้วยชื่อ — พิมพ์อย่างน้อย 2 ตัวอักษร",
        "Searching by name — type at least 2 characters",
    ),
    ("search.results", "พบ {n} ราย", "{n} result(s)"),
    (
        "search.no_results",
        "ไม่พบผู้ป่วยที่ตรงกับคำค้นหา",
        "No patients match the query",
    ),
    ("search.error", "ค้นหาไม่สำเร็จ: {msg}", "Search failed: {msg}"),
    (
        "search.required",
        "กรอกคำค้นหาอย่างน้อย 2 ตัวอักษร",
        "Enter at least 2 characters",
    ),
    (
        "canvas.empty_title",
        "เลือกผู้ป่วยเพื่อดูประวัติยา",
        "Select a patient to view medication history",
    ),
    (
        "canvas.empty_sub",
        "ค้นหาด้วยชื่อ-สกุล, HN หรือ CID ทางซ้าย แล้วเลือกผู้ป่วย",
        "Search by name, HN, or CID on the left, then pick a patient.",
    ),
    (
        "canvas.bpmh_note",
        "ข้อมูลจากระบบการจ่ายยา (dispensing) เป็นแหล่งข้อมูลหนึ่งสำหรับ BPMH — ยังไม่ถือว่าเป็นรายการยาที่สมบูรณ์ ควรสอบทานกับผู้ป่วยเสมอ",
        "Dispensing-derived data is one BPMH source among several — not a verified list; always reconcile with the patient.",
    ),
    (
        "canvas.warnings",
        "คำเตือนความครบถ้วนของข้อมูล",
        "Data-completeness warnings",
    ),
    (
        "canvas.active",
        "ยาที่คาดว่ายังใช้อยู่ ({n})",
        "Likely active ({n})",
    ),
    (
        "canvas.lapsed",
        "ยาที่คาดว่าหยุดใช้แล้ว ({n})",
        "Likely lapsed ({n})",
    ),
    (
        "canvas.allergies",
        "แพ้ยา / อาการไม่พึงประสงค์ ({n})",
        "Allergies / ADR ({n})",
    ),
    (
        "canvas.no_allergies",
        "ไม่พบประวัติแพ้ยาในระบบ",
        "No allergy records on file",
    ),
    (
        "canvas.no_medications",
        "ไม่พบประวัติการจ่ายยาในช่วงเวลาที่กำหนด",
        "No dispensing records in the configured window",
    ),
    (
        "canvas.visits",
        "ประวัติการเข้ารับบริการ ({n})",
        "Visit history ({n})",
    ),
    (
        "canvas.no_visits",
        "ไม่พบประวัติการเข้ารับบริการ",
        "No visit records",
    ),
    ("canvas.export", "ส่งออกรายงาน", "Export report"),
    ("canvas.exporting", "กำลังส่งออก…", "Exporting…"),
    (
        "canvas.export_done",
        "บันทึกรายงานแล้ว: {path}",
        "Report saved: {path}",
    ),
    (
        "canvas.load_error",
        "โหลดประวัติไม่สำเร็จ: {msg}",
        "Failed to load history: {msg}",
    ),
    ("med.active", "ยังใช้อยู่", "Active"),
    ("med.lapsed", "หยุดแล้ว", "Lapsed"),
    (
        "med.last_dispense",
        "จ่ายล่าสุด {date}",
        "Last dispensed {date}",
    ),
    ("med.total_qty", "รวม {qty} {units}", "Total {qty} {units}"),
    ("med.visits", "{n} ครั้ง", "{n} visits"),
    ("med.days_supply", "ยาเผื่อ {n} วัน", "Est. supply {n} days"),
    ("med.sig", "วิธีใช้: {sig}", "Sig: {sig}"),
    ("visit.date", "วันที่", "Date"),
    ("visit.type", "ประเภท", "Type"),
    ("visit.department", "แผนก / หอผู้ป่วย", "Department / Ward"),
    ("visit.id", "รหัส visit", "Visit ID"),
    ("visit.opd", "OPD", "OPD"),
    ("visit.ipd", "IPD", "IPD"),
    (
        "about.license",
        "v0.1.0 · MIT/Apache-2.0",
        "v0.1.0 · MIT/Apache-2.0",
    ),
];

/// Translate a key for a language, falling back to the key itself.
///
/// Returns a `'static` string when the key is known, otherwise the input
/// key unchanged.
pub fn tr(lang: Lang, key: &str) -> &str {
    KEYS.iter()
        .find(|(k, _, _)| *k == key)
        .map(|(_, th, en)| match lang {
            Lang::Thai => *th,
            Lang::English => *en,
        })
        .unwrap_or(key)
}

/// Translate with `{placeholder}` substitution.
pub fn tr_f(lang: Lang, key: &str, args: &[(&str, &str)]) -> String {
    let mut out = tr(lang, key).to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_has_both_languages() {
        for (key, th, en) in KEYS {
            assert!(!th.is_empty(), "missing Thai for {key}");
            assert!(!en.is_empty(), "missing English for {key}");
        }
    }

    #[test]
    fn tr_returns_translation_or_key() {
        assert_eq!(tr(Lang::Thai, "canvas.active"), "ยาที่คาดว่ายังใช้อยู่ ({n})");
        assert_eq!(tr(Lang::English, "canvas.active"), "Likely active ({n})");
        assert_eq!(tr(Lang::Thai, "no.such.key"), "no.such.key");
    }

    #[test]
    fn tr_f_substitutes_placeholders() {
        let s = tr_f(Lang::Thai, "search.results", &[("n", "3")]);
        assert_eq!(s, "พบ 3 ราย");
    }
}
