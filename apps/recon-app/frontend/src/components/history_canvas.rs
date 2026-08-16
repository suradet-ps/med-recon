//! Main canvas — the complete medication history for the selected patient:
//! patient bar, data-completeness warnings, allergy bands, BPMH active /
//! lapsed sections, and the visit timeline.

use chrono::Datelike;
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icons::{
    IconActivity, IconAlert, IconCalendar, IconCheckCircle, IconPrinter, IconShield, IconUser,
    IconXCircle,
};
use crate::i18n::{tr, tr_f};
use crate::state::AppState;
use recon_core::{EncounterSource, MedicationItem, MedicationStatus, PatientHistory, Sig};

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

    let active_count = active.len();
    let active_title = tr_f(
        lang.get(),
        "canvas.active",
        &[("n", &active_count.to_string())],
    );
    let has_active = active_count > 0;
    let lapsed_count = lapsed.len();
    let lapsed_title = tr_f(
        lang.get(),
        "canvas.lapsed",
        &[("n", &lapsed_count.to_string())],
    );
    let has_lapsed = lapsed_count > 0;
    let allergy_count = history.allergies.len();
    let allergies_title = tr_f(
        lang.get(),
        "canvas.allergies",
        &[("n", &allergy_count.to_string())],
    );
    let has_allergies = allergy_count > 0;
    let visit_count = history.visits.len();
    let visits_title = tr_f(
        lang.get(),
        "canvas.visits",
        &[("n", &visit_count.to_string())],
    );
    let has_visits = visit_count > 0;
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

            <div class="verdict-band verdict-pending">
                <IconShield class="verdict-band__icon" />
                <div class="verdict-band__content">
                    <p class="verdict-band__detail">{move || tr(lang.get(), "canvas.bpmh_note")}</p>
                </div>
            </div>

            <section class="canvas-section">
                <h3 class="timeline-header">
                    <IconCheckCircle class="icon" />
                    {active_title}
                </h3>
                {if !has_active {
                    view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_medications")}</p> }.into_any()
                } else {
                    active.iter().map(|m| view! { <MedBand item=(*m).clone() lang=lang/> }).collect_view().into_any()
                }}
            </section>

            <section class="canvas-section">
                <h3 class="timeline-header">
                    <IconXCircle class="icon" />
                    {lapsed_title}
                </h3>
                {if !has_lapsed {
                    view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_medications")}</p> }.into_any()
                } else {
                    lapsed.iter().map(|m| view! { <MedBand item=(*m).clone() lang=lang/> }).collect_view().into_any()
                }}
            </section>

            <section class="canvas-section">
                <h3 class="timeline-header">
                    <IconActivity class="icon" />
                    {allergies_title}
                </h3>
                {if !has_allergies {
                    view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_allergies")}</p> }.into_any()
                } else {
                    history.allergies.iter().map(|a| {
                        let agent = a.agent.clone();
                        let symptom = a.symptom.clone();
                        view! {
                            <div class="verdict-band verdict-notfound verdict-band--compact">
                                <IconXCircle class="verdict-band__icon" />
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
                    <IconCalendar class="icon" />
                    {visits_title}
                </h3>
                {if !has_visits {
                    view! { <p class="canvas-empty__sub">{tr(lang.get(), "canvas.no_visits")}</p> }.into_any()
                } else {
                    view! {
                        <ul class="timeline">
                            {history.visits.iter().map(|v| {
                                let date = format!("{:02}/{:02}/{}", v.date.day(), v.date.month(), v.date.year());
                                let kind = match v.source {
                                    EncounterSource::Opd => tr(lang.get(), "visit.opd"),
                                    EncounterSource::Ipd => tr(lang.get(), "visit.ipd"),
                                };
                                let dept = v.department.clone().unwrap_or_default();
                                let vid = v.visit_id.clone();
                                view! {
                                    <li class="timeline-row">
                                        <span class="timeline-row__date">{date}</span>
                                        <span class="timeline-row__badge">
                                            <span class="badge">{kind}</span>
                                        </span>
                                        <div class="timeline-row__main">
                                            <p class="timeline-row__drug">{dept}</p>
                                            <p class="timeline-row__meta">{vid}</p>
                                        </div>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    }.into_any()
                }}
            </section>
        </>
    }
}

/// One BPMH medication — a verdict-style compact band: active (green) or
/// lapsed (neutral), with sig, quantity, and supply details.
#[component]
fn MedBand(item: MedicationItem, lang: RwSignal<crate::i18n::Lang>) -> impl IntoView {
    let is_active = item.status == MedicationStatus::Active;
    let chip = if is_active {
        tr(lang.get(), "med.active").to_string()
    } else {
        tr(lang.get(), "med.lapsed").to_string()
    };
    let name = item.drug_name.clone();
    let strength = item.strength.clone();
    let units = item.units.clone();
    let units_meta = units.clone();
    let last = item.last_dispense;
    let total_qty = format_qty(item.total_qty);
    let visit_count = item.visit_count;
    let supply = item.days_supply;
    let sig = item.sig.clone();
    let sources = item.sources.clone();

    let last_str = format!("{:02}/{:02}/{}", last.day(), last.month(), last.year());
    let meta = tr_f(lang.get(), "med.last_dispense", &[("date", &last_str)]);

    view! {
        <div class=move || {
            if is_active {
                "verdict-band verdict-found verdict-band--compact"
            } else {
                "verdict-band verdict-lapsed verdict-band--compact"
            }
        }>
            {if is_active {
                view! { <IconCheckCircle class="verdict-band__icon" /> }.into_any()
            } else {
                view! { <IconXCircle class="verdict-band__icon" /> }.into_any()
            }}
            <div class="verdict-band__content">
                <div class="med-head">
                    <p class="verdict-band__term">
                        {name}
                        {move || strength.as_ref().map(|s| format!(" · {s}")).unwrap_or_default()}
                        {move || units.as_ref().map(|u| format!(" {u}")).unwrap_or_default()}
                    </p>
                    <div class="med-chips">
                        {sources.iter().map(|s| {
                            let label = match s {
                                EncounterSource::Opd => "OPD",
                                EncounterSource::Ipd => "IPD",
                            };
                            view! { <span class="badge">{label}</span> }
                        }).collect_view()}
                        <span class="badge">{chip}</span>
                    </div>
                </div>
                <p class="verdict-band__detail">
                    {meta}
                    " · " {tr_f(lang.get(), "med.visits", &[("n", &visit_count.to_string())])}
                    " · " {tr_f(lang.get(), "med.total_qty", &[("qty", &total_qty), ("units", units_meta.as_deref().unwrap_or(""))])}
                    {move || supply.map(|d| format!(" · {}", tr_f(lang.get(), "med.days_supply", &[("n", &d.to_string())]))).unwrap_or_default()}
                </p>
                {move || sig.clone().map(|s| view! {
                    <p class="verdict-band__detail" style="opacity:1">
                        {tr_f(lang.get(), "med.sig", &[("sig", &format_sig(&s))])}
                    </p>
                })}
            </div>
        </div>
    }
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
