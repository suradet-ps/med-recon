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
use crate::i18n::{tr, tr_f};
use crate::state::AppState;
use med_recon_core::{
    EncounterSource, MedicationItem, MedicationStatus, OpdScreenRecord, PatientHistory, Sig,
};

#[component]
pub fn HistoryCanvas(state: AppState) -> impl IntoView {
    // Re-fetch history when the window override changes (debounced).
    let last_timeout = RwSignal::new(None::<leptos::prelude::TimeoutHandle>);
    Effect::new(move |prev: Option<()>| {
        let override_val = state.history_days_override.get();
        if prev.is_some()
            && let Some(patient) = state.patient.get_untracked()
        {
            let hn = patient.hn.clone();
            state.history.set(None);
            state.history_error.set(None);
            state.history_loading.set(true);
            if let Some(prev_handle) = last_timeout.get() {
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

    let window_options: &[(u32, &str)] =
        &[(730, "2 ปี"), (1825, "5 ปี"), (3650, "10 ปี"), (5475, "15 ปี")];

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
                    {move || tr_f(lang.get(), "canvas.allergies", &[("n", &allergy_count.to_string())])}
                </h3>
                {move || {
                    if !has_allergies {
                        view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_allergies")}</p> }.into_any()
                    } else {
                        history.allergies.iter().map(|a| {
                            let agent = a.agent.clone();
                            let symptom = a.symptom.clone();
                            let mut meta_parts: Vec<String> = Vec::new();
                            if let Some(d) = a.report_date {
                                meta_parts.push(tr_f(
                                    lang.get(),
                                    "allergy.reported_on",
                                    &[("date", &format!("{:02}/{:02}/{}", d.day(), d.month(), d.year()))],
                                ));
                            }
                            if let Some(r) = a.reporter.as_deref().filter(|r| !r.trim().is_empty()) {
                                meta_parts.push(tr_f(lang.get(), "allergy.reported_by", &[("reporter", r)]));
                            }
                            if let Some(n) = a.note.as_deref().filter(|n| !n.trim().is_empty()) {
                                meta_parts.push(tr_f(lang.get(), "allergy.note", &[("note", n)]));
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
                        {move || tr_f(lang.get(), "canvas.active", &[("n", &active_count.to_string())])}
                    </h3>
                    <div class="segmented">
                        <span class="segmented__label">
                            {move || tr(lang.get(), "canvas.history_window")}
                        </span>
                        {window_options.iter().map(|&(days, label)| {
                            let is_active = move || state.history_days_override.get() == Some(days);
                            let label = label.to_string();
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
                                        let current = state.history_days_override.get();
                                        let new_val = if current == Some(days) { None } else { Some(days) };
                                        state.history_days_override.set(new_val);
                                    }
                                >
                                    {label}
                                </button>
                            }
                        }).collect_view()}
                    </div>
                </div>
                {move || {
                    if !has_active {
                        view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_medications")}</p> }.into_any()
                    } else {
                        med_table(&active, lang).into_any()
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
                    {move || tr_f(lang.get(), "canvas.lapsed", &[("n", &lapsed_count.to_string())])}
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
                    {move || tr_f(lang.get(), "canvas.screen", &[("n", &screen_count.to_string())])}
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
                    <th class="med-table__date">{tr(lang.get(), "med.col_date")}</th>
                    <th>{tr(lang.get(), "med.col_drug")}</th>
                    <th>{tr(lang.get(), "med.col_sig")}</th>
                    <th class="med-table__qty">{tr(lang.get(), "med.col_qty")}</th>
                    <th class="med-table__appt">{tr(lang.get(), "med.col_appt")}</th>
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
                                tr_f(
                                    lang.get_untracked(),
                                    "med.repeat_times",
                                    &[("n", &m.visit_count.to_string())],
                                ),
                            )
                        });
                        // Provenance pill — only when the most recent dispense
                        // was IPD: OPD is the default and stays silent so the
                        // table stays clean.
                        let badge = match m.last_source {
                            EncounterSource::Ipd => {
                                Some(tr(lang.get_untracked(), "visit.ipd").to_string())
                            }
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
