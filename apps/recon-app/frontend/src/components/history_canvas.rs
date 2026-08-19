//! Main canvas — the complete medication history for the selected patient:
//! patient bar, data-completeness warnings, allergy bands, BPMH active /
//! lapsed sections, and the visit timeline.

use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icons::{
    IconAlert, IconCheckCircle, IconChevron, IconPrinter, IconUser, IconXCircle,
};
use crate::i18n::{tr, tr_f};
use crate::state::AppState;
use recon_core::{MedicationItem, MedicationStatus, PatientHistory, Sig};

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
    let exporting = RwSignal::new(false);
    let export_msg = RwSignal::new(None::<(bool, String)>);
    let on_export = move |_| {
        let Some(patient) = state.patient.get() else {
            return;
        };
        exporting.set(true);
        export_msg.set(None);
        spawn_local(async move {
            match api::export_report(&patient.hn).await {
                Ok(path) => export_msg.set(Some((
                    true,
                    tr_f(
                        lang.get_untracked(),
                        "canvas.export_done",
                        &[("path", &path)],
                    ),
                ))),
                Err(e) => export_msg.set(Some((false, e.message))),
            }
            exporting.set(false);
        });
    };
    let patient = history.patient.clone();
    let name = patient.display_name();
    let hn = patient.hn.clone();
    let cid = patient.cid.clone();
    let birthday = patient
        .birthday
        .map(|d| format!("{:02}/{:02}/{}", d.day(), d.month(), d.year()));
    let age = patient.birthday.map(age_years);

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
            <div class="patient-bar">
                <IconUser class="patient-bar__icon" />
                <div class="patient-bar__info">
                    <h2 class="patient-bar__name">{name}</h2>
                    <div class="patient-bar__meta">
                        <span class="code">{"HN "}{hn.clone()}</span>
                        {move || cid.as_ref().map(|c| view! { <><span class="sep">"·"</span><span class="code">{c.clone()}</span></> })}
                        {move || birthday.as_ref().map(|b| view! { <><span class="sep">"·"</span><span>{b.clone()}</span></> })}
                        {move || age.map(|a| view! { <><span class="sep">"·"</span><span>{tr_f(lang.get(), "canvas.age", &[("n", &a.to_string())])}</span></> })}
                    </div>
                </div>
                <div class="patient-bar__change">
                    <button
                        class="button-primary button-primary--inline"
                        on:click=on_export
                        prop:disabled=move || exporting.get()
                    >
                        <IconPrinter class="icon" />
                        {move || if exporting.get() { tr(lang.get(), "canvas.exporting") } else { tr(lang.get(), "canvas.export") }}
                    </button>
                </div>
            </div>

            {move || export_msg.get().map(|(ok, msg)| {
                if ok {
                    view! { <div class="banner-warning" style="border-color:var(--verdict-found-border);background:var(--verdict-found);color:var(--verdict-found-text)">{msg.clone()}</div> }.into_any()
                } else {
                    view! { <div class="banner-warning">{msg.clone()}</div> }.into_any()
                }
            })}

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
                        view! {
                            <div class="verdict-band verdict-notfound verdict-band--compact">
                                <IconAlert class="verdict-band__icon" />
                                <div class="verdict-band__content">
                                    <p class="verdict-band__term">{agent}</p>
                                    {move || symptom.as_ref().map(|s| view! {
                                        <p class="verdict-band__detail">{s.clone()}</p>
                                    })}
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

/// Whole-years age as of today, computed from a birthday.
///
/// Uses the browser's local date (`js_sys::Date`) — `std::time::SystemTime`
/// panics under `wasm32`.
fn age_years(birthday: NaiveDate) -> i32 {
    let today = js_sys::Date::new_0();
    let year = today.get_full_year() as i32;
    let month = today.get_month() + 1;
    let day = today.get_date();
    let today = match NaiveDate::from_ymd_opt(year, month, day) {
        Some(d) => d,
        None => return 0,
    };
    let mut age = today.year() - birthday.year();
    if (month, day) < (birthday.month(), birthday.day()) {
        age -= 1;
    }
    age
}
