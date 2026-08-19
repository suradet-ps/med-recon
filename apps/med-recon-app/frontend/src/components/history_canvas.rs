//! Main canvas — the complete medication history for the selected patient:
//! data-completeness warnings, allergy bands, BPMH active / lapsed
//! sections, and the CC/PE screening table. The patient identity lives in
//! the sidebar card, so this panel keeps its full vertical space for data.

use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;

use crate::components::icons::{
    IconAlert, IconCheckCircle, IconChevron, IconClipboard, IconUser, IconXCircle,
};
use crate::i18n::{tr, tr_f};
use crate::state::AppState;
use med_recon_core::{MedicationItem, MedicationStatus, OpdScreenRecord, PatientHistory, Sig};

#[component]
pub fn HistoryCanvas(state: AppState) -> impl IntoView {
    view! {
        <main class="main-canvas">
            {move || {
                if state.patient.get().is_none() {
                    view! { <EmptyState state=state/> }.into_any()
                } else if state.history_loading.get() {
                    view! { <div class="canvas-loading">"…"</div> }.into_any()
                } else if let Some(err) = state.history_error.get() {
                    view! { <div class="banner-warning">{err}</div> }.into_any()
                } else {
                    match state.history.get() {
                        Some(history) => view! { <HistoryView history=history state=state/> }.into_any(),
                        None => view! { <EmptyState state=state/> }.into_any(),
                    }
                }
            }}
        </main>
    }
}

/// Shown before a patient is selected.
#[component]
fn EmptyState(state: AppState) -> impl IntoView {
    let lang = state.lang;
    view! {
        <div class="canvas-empty">
            <IconUser class="canvas-empty__icon" />
            <h2 class="canvas-empty__title">{move || tr(lang.get(), "canvas.empty_title")}</h2>
            <p class="canvas-empty__sub">{move || tr(lang.get(), "canvas.empty_sub")}</p>
        </div>
    }
}

#[component]
fn HistoryView(history: PatientHistory, state: AppState) -> impl IntoView {
    let lang = state.lang;

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
    let active_title = tr_f(
        lang.get_untracked(),
        "canvas.active",
        &[("n", &active_count.to_string())],
    );
    let has_active = active_count > 0;
    let lapsed_count = lapsed.len();
    let lapsed_title = tr_f(
        lang.get_untracked(),
        "canvas.lapsed",
        &[("n", &lapsed_count.to_string())],
    );
    let has_lapsed = lapsed_count > 0;
    let lapsed_open = RwSignal::new(false);
    let screen_records = history.screen_records.clone();
    let screen_count = screen_records.len();
    let screen_title = tr_f(
        lang.get_untracked(),
        "canvas.screen",
        &[("n", &screen_count.to_string())],
    );
    let has_screen = screen_count > 0;
    let screen_open = RwSignal::new(false);
    let allergy_count = history.allergies.len();
    let allergies_title = tr_f(
        lang.get_untracked(),
        "canvas.allergies",
        &[("n", &allergy_count.to_string())],
    );
    let has_allergies = allergy_count > 0;
    let warnings = history.warnings.clone();

    view! {
        <>
            {move || {
                if !warnings.is_empty() {
                    Some(view! {
                        <div class="banner-warning">
                            <IconAlert class="banner-warning__icon" />
                            <span>
                                <strong>{tr(lang.get(), "canvas.warnings")}</strong>
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
                    {allergies_title}
                </h3>
                {if !has_allergies {
                    view! { <p class="canvas-empty__sub">{tr(lang.get_untracked(), "canvas.no_allergies")}</p> }.into_any()
                } else {
                    history.allergies.iter().map(|a| {
                        let agent = a.agent.clone();
                        let symptom = a.symptom.clone();
                        let mut meta_parts: Vec<String> = Vec::new();
                        if let Some(d) = a.report_date {
                            meta_parts.push(tr_f(
                                lang.get_untracked(),
                                "allergy.reported_on",
                                &[("date", &format!("{:02}/{:02}/{}", d.day(), d.month(), d.year()))],
                            ));
                        }
                        if let Some(r) = a.reporter.as_deref().filter(|r| !r.trim().is_empty()) {
                            meta_parts.push(tr_f(lang.get_untracked(), "allergy.reported_by", &[("reporter", r)]));
                        }
                        if let Some(n) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                            meta_parts.push(tr_f(lang.get_untracked(), "allergy.note", &[("note", n)]));
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
                }}
            </section>

            <section class="canvas-section">
                <h3 class="timeline-header">
                    <IconCheckCircle class="icon" />
                    {active_title}
                </h3>
                {if !has_active {
                    view! { <p class="canvas-empty__sub">{tr(lang.get_untracked(), "canvas.no_medications")}</p> }.into_any()
                } else {
                    med_table(&active, lang).into_any()
                }}
            </section>

            <section class="canvas-section">
                <button
                    class="timeline-header timeline-header--button"
                    on:click=move |_| lapsed_open.update(|v| *v = !*v)
                    aria-expanded=move || if lapsed_open.get() { "true" } else { "false" }
                >
                    <IconXCircle class="icon" />
                    {lapsed_title}
                    <IconChevron class="timeline-header__chevron" />
                </button>
                {move || {
                    if !has_lapsed {
                        view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_medications")}</p> }.into_any()
                    } else if lapsed_open.get() {
                        med_table(&lapsed, lang).into_any()
                    } else {
                        view! { <p class="canvas-empty__sub">{tr_f(
                            lang.get(),
                            "canvas.lapsed_collapsed",
                            &[("n", &lapsed_count.to_string())],
                        )}</p> }.into_any()
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
                    {screen_title}
                    <IconChevron class="timeline-header__chevron" />
                </button>
                {move || {
                    if !has_screen {
                        view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_screen")}</p> }.into_any()
                    } else if screen_open.get() {
                        screen_table(&screen_records, lang).into_any()
                    } else {
                        view! { <p class="canvas-empty__sub">{tr_f(
                            lang.get(),
                            "canvas.screen_collapsed",
                            &[("n", &screen_count.to_string())],
                        )}</p> }.into_any()
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
fn med_table(items: &[MedicationItem], lang: RwSignal<crate::i18n::Lang>) -> impl IntoView {
    view! {
        <table class="med-table">
            <thead>
                <tr>
                    <th class="med-table__no">{tr(lang.get(), "med.col_no")}</th>
                    <th>{tr(lang.get(), "med.col_date")}</th>
                    <th>{tr(lang.get(), "med.col_drug")}</th>
                    <th>{tr(lang.get(), "med.col_sig")}</th>
                    <th>{tr(lang.get(), "med.col_qty")}</th>
                    <th>{tr(lang.get(), "med.col_appt")}</th>
                </tr>
            </thead>
            <tbody>
                {{
                    let mut band = false;
                    let mut last_date: Option<NaiveDate> = None;
                    items.iter().enumerate().map(|(i, m)| {
                        let date_changed = last_date.as_ref() != Some(&m.last_dispense);
                        if date_changed {
                            band = !band;
                            last_date = Some(m.last_dispense);
                        }
                        let row_band = band;
                        let no = (i + 1).to_string();
                        let date = format!("{:02}/{:02}/{}", m.last_dispense.day(), m.last_dispense.month(), m.last_dispense.year());
                        let drug = drug_label(m);
                        let sig = m.sig.as_ref().map(format_sig).unwrap_or_default();
                        let qty = format_qty(m.last_qty);
                        let units = m.units.clone().unwrap_or_default();
                        let appt = m.appointment_date.map(|d| {
                            format!("{:02}/{:02}/{}", d.day(), d.month(), d.year())
                        }).unwrap_or_default();
                        view! {
                            <tr class=move || if row_band { "med-table__row med-table__row--band" } else { "med-table__row" }>
                                <td class="med-table__no">{no}</td>
                                <td class="med-table__date">{date}</td>
                                <td class="med-table__drug">{drug}</td>
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
fn screen_table(items: &[OpdScreenRecord], lang: RwSignal<crate::i18n::Lang>) -> impl IntoView {
    view! {
        <table class="med-table med-table--screen">
            <thead>
                <tr>
                    <th class="med-table__no">{tr(lang.get(), "med.col_no")}</th>
                    <th>{tr(lang.get(), "visit.date")}</th>
                    <th>{tr(lang.get(), "med.col_cc")}</th>
                    <th>{tr(lang.get(), "med.col_pe")}</th>
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
