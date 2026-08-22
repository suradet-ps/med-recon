//! Main canvas — the complete medication history for the selected patient:
//! data-completeness warnings, allergy bands, BPMH active / lapsed
//! sections, and the CC/PE screening table. The patient identity lives in
//! the sidebar card, so this panel keeps its full vertical space for data.

use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icons::{
    IconAlert, IconCheckCircle, IconChevron, IconClipboard, IconUser, IconXCircle,
};
use crate::state::AppState;
use med_recon_core::{
    EncounterSource, MedicationItem, MedicationStatus, OpdScreenRecord, PatientHistory, Sig,
};

#[component]
pub fn HistoryCanvas(state: AppState) -> impl IntoView {
    // Re-fetch history when the window override changes (debounced).
    let last_timeout = RwSignal::new(None::<leptos::prelude::TimeoutHandle>);
    Effect::new(move |prev: Option<()>| {
        // Re-fetch only when the operator changes the window (window_epoch).
        // A programmatic reset on patient search bumps nothing here, so the
        // patient-search fetch stays the single in-flight request.
        let _ = state.window_epoch.get();
        if prev.is_some()
            && let Some(patient) = state.patient.get_untracked()
        {
            let override_val = state.history_days_override.get_untracked();
            let hn = patient.hn.clone();
            // Keep the previously loaded history on screen (dimmed via
            // history-stack--loading) instead of blanking to a spinner — the
            // top load bar + active-segment spinner carry the loading state.
            state.history_error.set(None);
            state.history_loading.set(true);
            if let Some(prev_handle) = last_timeout.get_untracked() {
                prev_handle.clear();
            }
            last_timeout.set(Some(
                set_timeout_with_handle(
                    move || {
                        let hn = hn.clone();
                        spawn_local(async move {
                            match api::load_history(&hn, override_val).await {
                                Ok(h) => {
                                    state.history.set(Some(h));
                                    state.history_error.set(None);
                                }
                                Err(e) => {
                                    state.history.set(None);
                                    state.history_error.set(Some(e.message));
                                }
                            }
                            state.history_loading.set(false);
                        });
                    },
                    std::time::Duration::from_millis(300),
                )
                .expect("invariant: setTimeout is available"),
            ));
        }
    });

    view! {
        <main class="main-canvas">
            <div
                class="canvas-loadbar"
                class:canvas-loadbar--active=move || state.history_loading.get()
            ></div>
            {move || {
                if state.patient.get().is_none() {
                    view! { <EmptyState/> }.into_any()
                } else if let Some(err) = state.history_error.get() {
                    view! { <div class="banner-warning">{err}</div> }.into_any()
                } else if let Some(history) = state.history.get() {
                    view! {
                        <div
                            class="history-stack"
                            class:history-stack--loading=move || state.history_loading.get()
                        >
                            <HistoryView history=history state=state/>
                        </div>
                    }
                        .into_any()
                } else if state.history_loading.get() {
                    view! {
                        <div class="canvas-loading">
                            <span class="spinner" aria-label="loading"></span>
                        </div>
                    }
                        .into_any()
                } else {
                    view! { <EmptyState/> }.into_any()
                }
            }}
        </main>
    }
}

/// Shown before a patient is selected.
#[component]
fn EmptyState() -> impl IntoView {
    view! {
        <div class="canvas-empty">
            <IconUser class="canvas-empty__icon" />
            <h2 class="canvas-empty__title">"เลือกผู้ป่วยเพื่อดูประวัติยา"</h2>
            <p class="canvas-empty__sub">"ค้นหาด้วยชื่อ-สกุล, HN หรือ CID ทางซ้าย แล้วเลือกผู้ป่วย"</p>
        </div>
    }
}

#[component]
fn HistoryView(history: PatientHistory, state: AppState) -> impl IntoView {
    let active: Vec<MedicationItem> = history
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Active)
        .cloned()
        .collect();
    let lapsed: Vec<MedicationItem> = history
        .medications
        .iter()
        .filter(|m| m.status == MedicationStatus::Lapsed)
        .cloned()
        .collect();

    let active_count = active.len();
    let has_active = active_count > 0;
    let lapsed_count = lapsed.len();
    let has_lapsed = lapsed_count > 0;
    let lapsed_open = RwSignal::new(false);
    let screen_records = history.screen_records.clone();
    let screen_count = screen_records.len();
    let has_screen = screen_count > 0;
    let screen_open = RwSignal::new(false);
    let allergy_count = history.allergies.len();
    let has_allergies = allergy_count > 0;
    let warnings = history.warnings.clone();

    let window_options = Signal::derive(move || {
        let default_days = state.default_history_days.get();
        let mut opts: Vec<(Option<u32>, String)> = vec![(
            None,
            format!("ค่าเริ่มต้น ({})", format_window_years(default_days)),
        )];
        for &(d, lbl) in &[(1825, "5 ปี"), (3650, "10 ปี"), (5475, "15 ปี")] {
            opts.push((Some(d), lbl.to_string()));
        }
        opts
    });

    view! {
        <>
            {move || {
                if !warnings.is_empty() {
                    Some(view! {
                        <div class="banner-warning">
                            <IconAlert class="banner-warning__icon" />
                            <span>
                                <strong>"คำเตือนความครบถ้วนของข้อมูล"</strong>
                                {warnings.iter().map(|w| view! { <div>{w.clone()}</div> }).collect_view()}
                            </span>
                        </div>
                    })
                } else {
                    None
                }
            }}

            <section class="canvas-section">
                <h3 class="timeline-header">
                    <IconAlert class="icon" />
                    {move || format!("แพ้ยา / อาการไม่พึงประสงค์ ({allergy_count})")}
                </h3>
                {move || {
                    if !has_allergies {
                        view! { <p class="canvas-empty__sub">"ไม่พบประวัติแพ้ยาในระบบ"</p> }.into_any()
                    } else {
                        history.allergies.iter().map(|a| {
                            let agent = a.agent.clone();
                            let symptom = a.symptom.clone();
                            let mut meta_parts: Vec<String> = Vec::new();
                            if let Some(d) = a.report_date {
                                meta_parts.push(format!(
                                    "รายงานเมื่อ {:02}/{:02}/{}",
                                    d.day(),
                                    d.month(),
                                    d.year()
                                ));
                            }
                            if let Some(r) = a.reporter.as_deref().filter(|r| !r.trim().is_empty()) {
                                meta_parts.push(format!("โดย {r}"));
                            }
                            if let Some(n) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                                meta_parts.push(format!("หมายเหตุ: {n}"));
                            }
                            let meta = meta_parts.join(" · ");
                            view! {
                                <div class="verdict-band verdict-notfound verdict-band--compact">
                                    <IconAlert class="verdict-band__icon" />
                                    <div class="verdict-band__content">
                                        <p class="verdict-band__term">{agent}</p>
                                        {move || symptom.as_ref().map(|s| view! {
                                            <p class="verdict-band__detail">{s.clone()}</p>
                                        })}
                                        {if meta.is_empty() {
                                            None
                                        } else {
                                            Some(view! { <p class="verdict-band__meta">{meta}</p> }.into_any())
                                        }}
                                    </div>
                                </div>
                            }
                        }).collect_view().into_any()
                    }
                }}
            </section>

            <section class="canvas-section">
                <div class="timeline-header-row">
                    <h3 class="timeline-header" style="margin:0">
                        <IconCheckCircle class="icon" />
                        {move || format!("ยาที่ผู้ป่วยเคยได้รับ ({active_count})")}
                    </h3>
                    <div class="segmented">
                        <span class="segmented__label">
                            "ค้นหาประวัติในรอบ"
                        </span>
                        {move || {
                            window_options.get().into_iter().map(|(days_opt, label)| {
                                let is_active = move || state.history_days_override.get() == days_opt;
                                view! {
                                    <button
                                        class=move || {
                                            if is_active() {
                                                "segmented__btn segmented__btn--active"
                                            } else {
                                                "segmented__btn"
                                            }
                                        }
                                        on:click=move |_| {
                                            state.history_days_override.set(days_opt);
                                            state.window_epoch.update(|e| *e += 1);
                                        }
                                    >
                                        {label}
                                        {move || {
                                            if state.history_loading.get() && is_active() {
                                                view! {
                                                    <span
                                                        class="segmented__spinner"
                                                        aria-hidden="true"
                                                    ></span>
                                                }
                                                    .into_any()
                                            } else {
                                                ().into_any()
                                            }
                                        }}
                                    </button>
                                }
                            }).collect_view()
                        }}
                    </div>
                </div>
                {move || {
                    if !has_active {
                        view! { <p class="canvas-empty__sub">"ไม่พบประวัติการจ่ายยาในช่วงเวลาที่กำหนด"</p> }.into_any()
                    } else {
                        med_table(&active).into_any()
                    }
                }}
            </section>

            <section class="canvas-section">
                <button
                    class="timeline-header timeline-header--button"
                    on:click=move |_| lapsed_open.update(|v| *v = !*v)
                    aria-expanded=move || if lapsed_open.get() { "true" } else { "false" }
                >
                    <IconXCircle class="icon" />
                    {move || format!("ยาที่ผู้ป่วยเคยได้รับ (ยาตามอาการ) ({lapsed_count})")}
                    <IconChevron class="timeline-header__chevron" />
                </button>
                {move || {
                    if !has_lapsed {
                        view! { <p class="canvas-empty__sub">"ไม่พบประวัติการจ่ายยาในช่วงเวลาที่กำหนด"</p> }.into_any()
                    } else if lapsed_open.get() {
                        med_table(&lapsed).into_any()
                    } else {
                        view! { <p class="canvas-empty__sub">{format!("คลิกเพื่อดู {lapsed_count} รายการที่คาดว่าหยุดใช้แล้ว")}</p> }.into_any()
                    }
                }}
            </section>

            <section class="canvas-section">
                <button
                    class="timeline-header timeline-header--button"
                    on:click=move |_| screen_open.update(|v| *v = !*v)
                    aria-expanded=move || if screen_open.get() { "true" } else { "false" }
                >
                    <IconClipboard class="icon" />
                    {move || format!("การตรวจ / อาการสำคัญ (CC/PE) ({screen_count})")}
                    <IconChevron class="timeline-header__chevron" />
                </button>
                {move || {
                    if !has_screen {
                        view! { <p class="canvas-empty__sub">"ไม่พบข้อมูลการตรวจ (CC/PE)"</p> }.into_any()
                    } else if screen_open.get() {
                        screen_table(&screen_records).into_any()
                    } else {
                        view! { <p class="canvas-empty__sub">{format!("คลิกเพื่อดู {screen_count} รายการการตรวจ")}</p> }.into_any()
                    }
                }}
            </section>

        </>
    }
}

/// Render a medication list as a table:
/// ลำดับ / วันที่จ่าย / ชื่อยา + ความแรง / วิธีใช้ / จำนวนที่จ่าย (ครั้งล่าสุด) /
/// วันนัด (`oapp.nextdate` ของ visit ที่จ่ายครั้งล่าสุด, "—" ถ้าไม่มี)
/// Used identically for both the active and lapsed sections.
fn med_table(items: &[MedicationItem]) -> impl IntoView {
    view! {
        <table class="med-table">
            <thead>
                <tr>
                    <th class="med-table__no">"ลำดับ"</th>
                    <th class="med-table__date">"วันที่จ่าย"</th>
                    <th>"ชื่อยา + ความแรง"</th>
                    <th>"วิธีใช้"</th>
                    <th class="med-table__qty">"จำนวนที่จ่าย"</th>
                    <th class="med-table__appt">"วันนัด"</th>
                </tr>
            </thead>
            <tbody>
                {{
                    let mut band = false;
                    let mut last_date: Option<NaiveDate> = None;
                    items.iter().enumerate().map(|(i, m)| {
                        // First date group = the most recent visit; highlight
                        // it above the alternating banding. Rows are sorted
                        // newest-first, so every row sharing the first item's
                        // date belongs to the latest visit.
                        let is_latest = items
                            .first()
                            .is_some_and(|f| f.last_dispense == m.last_dispense);
                        let date_changed = last_date.as_ref() != Some(&m.last_dispense);
                        if date_changed {
                            band = !band;
                            last_date = Some(m.last_dispense);
                        }
                        let row_band = band;
                        let row_class = if is_latest {
                            "med-table__row med-table__row--latest"
                        } else if row_band {
                            "med-table__row med-table__row--band"
                        } else {
                            "med-table__row"
                        };
                        let no = (i + 1).to_string();
                        let date = format!("{:02}/{:02}/{}", m.last_dispense.day(), m.last_dispense.month(), m.last_dispense.year());
                        let drug = drug_label(m);
                        // Repeat-dispensing count — how many visits this
                        // drug was dispensed on. Frequent + recent dispensing
                        // is the BPMH signal for an ongoing medication, so the
                        // count sits right after the drug name. Single
                        // dispenses stay silent to keep the table clean.
                        let repeat = (m.visit_count >= 2).then(|| {
                            (
                                m.visit_count,
                                format!("จ่าย {} ครั้ง", m.visit_count),
                            )
                        });
                        // Provenance pill — only when the most recent dispense
                        // was IPD: OPD is the default and stays silent so the
                        // table stays clean.
                        let badge = match m.last_source {
                            EncounterSource::Ipd => Some("IPD".to_string()),
                            EncounterSource::Opd => None,
                        };
                        let sig = m.sig.as_ref().map(format_sig).unwrap_or_default();
                        let qty = format_qty(m.last_qty);
                        let units = m.units.clone().unwrap_or_default();
                        let appt = m.appointment_date.map(|d| {
                            format!("{:02}/{:02}/{}", d.day(), d.month(), d.year())
                        }).unwrap_or_default();
                        view! {
                            <tr class=row_class>
                                <td class="med-table__no">{no}</td>
                                <td class="med-table__date">{date}</td>
                                <td class="med-table__drug">
                                    {drug}
                                    {repeat.map(|(n, tip)| {
                                    let text = format!("({n})");
                                    view! {
                                        <span class="med-table__repeat" title=tip>{text}</span>
                                    }
                                })}
                                    {badge.map(|b| view! { <span class="badge">{b}</span> })}
                                </td>
                                <td class="med-table__sig">
                                    {if sig.is_empty() { "—".to_string() } else { sig }}
                                </td>
                                <td class="med-table__qty">
                                    {qty}
                                    {if units.is_empty() { String::new() } else { format!(" {units}") }}
                                </td>
                                <td class="med-table__appt">
                                    {if appt.is_empty() { "—".to_string() } else { appt }}
                                </td>
                            </tr>
                        }
                    }).collect_view()
                }}
            </tbody>
        </table>
    }
}

/// Render screening records (CC/PE) as a table styled like `med_table`:
/// ลำดับ / วันที่ / CC / PE, newest visit first, "—" for blanks.
fn screen_table(items: &[OpdScreenRecord]) -> impl IntoView {
    view! {
        <table class="med-table med-table--screen">
            <thead>
                <tr>
                    <th class="med-table__no">"ลำดับ"</th>
                    <th>"วันที่"</th>
                    <th>"CC (อาการสำคัญ)"</th>
                    <th>"PE (ผลตรวจร่างกาย)"</th>
                </tr>
            </thead>
            <tbody>
                {items.iter().enumerate().map(|(i, r)| {
                    let no = (i + 1).to_string();
                    let date = format!("{:02}/{:02}/{}", r.vstdate.day(), r.vstdate.month(), r.vstdate.year());
                    view! {
                        <tr class="med-table__row">
                            <td class="med-table__no">{no}</td>
                            <td class="med-table__date">{date}</td>
                            <td class="med-table__sig">{r.cc.clone().unwrap_or_else(|| "—".into())}</td>
                            <td class="med-table__sig">{r.pe.clone().unwrap_or_else(|| "—".into())}</td>
                        </tr>
                    }
                }).collect_view()}
            </tbody>
        </table>
    }
}

/// "Name · strength" label for the drug column.
fn drug_label(m: &MedicationItem) -> String {
    let mut label = m.drug_name.clone();
    if let Some(strength) = &m.strength {
        label.push_str(&format!(" · {strength}"));
    }
    label
}

/// Format a sig for display.
fn format_sig(sig: &Sig) -> String {
    let dose = sig.dose_per_admin.map(format_qty).unwrap_or_default();
    let freq = sig.frequency_per_day.map(format_qty).unwrap_or_default();
    let note = sig.note.clone().unwrap_or_default();
    if dose.is_empty() && freq.is_empty() {
        note
    } else {
        format!("{dose} × {freq}/วัน {note}")
    }
}

/// Trim trailing zeros from a quantity (e.g. `3.500` → `3.5`).
fn format_qty(q: f64) -> String {
    let mut s = format!("{q:.3}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Format a day count as a Thai year label, e.g. `730` → `"2 ปี"`,
/// `5475` → `"15 ปี"`. Fractional years fall back to one decimal.
fn format_window_years(days: u32) -> String {
    let years = days as f64 / 365.0;
    if years.fract() < 0.01 {
        format!("{} ปี", years as u32)
    } else {
        format!("{:.1} ปี", years)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(drug_name: &str, strength: Option<&str>, sig: Option<Sig>) -> MedicationItem {
        let date = NaiveDate::from_ymd_opt(2026, 7, 1).unwrap();
        MedicationItem {
            icode: "P1".into(),
            drug_name: drug_name.into(),
            strength: strength.map(str::to_string),
            units: None,
            last_dispense: date,
            first_dispense: date,
            last_qty: 30.0,
            total_qty: 30.0,
            visit_count: 1,
            sources: vec![EncounterSource::Opd],
            last_source: EncounterSource::Opd,
            days_supply: None,
            sig,
            appointment_date: None,
            status: MedicationStatus::Active,
            days_since_last_dispense: 0,
        }
    }

    #[test]
    fn format_qty_trims_trailing_zeros() {
        assert_eq!(format_qty(3.500), "3.5");
        assert_eq!(format_qty(3.0), "3");
        assert_eq!(format_qty(0.0), "0");
        assert_eq!(format_qty(1.25), "1.25");
        assert_eq!(format_qty(2.100), "2.1");
    }

    #[test]
    fn format_window_years_labels_integer_years() {
        assert_eq!(format_window_years(730), "2 ปี");
        assert_eq!(format_window_years(1825), "5 ปี");
        assert_eq!(format_window_years(3650), "10 ปี");
        assert_eq!(format_window_years(5475), "15 ปี");
    }

    #[test]
    fn format_window_years_labels_fractional_years() {
        assert_eq!(format_window_years(90), "0.2 ปี");
        assert_eq!(format_window_years(183), "0.5 ปี");
    }

    #[test]
    fn drug_label_appends_strength_only() {
        assert_eq!(
            drug_label(&item("Paracetamol", Some("500 mg"), None)),
            "Paracetamol · 500 mg"
        );
        assert_eq!(drug_label(&item("Metformin", None, None)), "Metformin");
    }

    #[test]
    fn format_sig_combines_dose_frequency_note() {
        let sig = Sig {
            dose_per_admin: Some(1.0),
            frequency_per_day: Some(3.0),
            note: Some("หลังอาหาร".into()),
        };
        assert_eq!(format_sig(&sig), "1 × 3/วัน หลังอาหาร");
    }

    #[test]
    fn format_sig_falls_back_to_note_only() {
        let sig = Sig {
            dose_per_admin: None,
            frequency_per_day: None,
            note: Some("หลังอาหารเช้า".into()),
        };
        assert_eq!(format_sig(&sig), "หลังอาหารเช้า");
    }

    #[test]
    fn format_sig_empty_when_no_fields() {
        let sig = Sig {
            dose_per_admin: None,
            frequency_per_day: None,
            note: None,
        };
        assert_eq!(format_sig(&sig), "");
    }
}
