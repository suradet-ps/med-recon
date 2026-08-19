//! Sidebar patient card — the selected patient's photo, identity, and the
//! "print medication history" action.
//!
//! Rendered below the search section after a patient is picked, so the
//! main canvas keeps its full vertical space for the history itself.

use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icons::{IconPrinter, IconUser, IconX};
use crate::i18n::{tr, tr_f};
use crate::state::AppState;

#[component]
pub fn PatientCard(state: AppState) -> impl IntoView {
    let lang = state.lang;
    let exporting = RwSignal::new(false);
    let export_msg = RwSignal::new(None::<(bool, String)>);

    // The status message belongs to one patient — reset it whenever the
    // selection changes so a stale "บันทึกรายงานแล้ว" never lingers.
    Effect::new(move |_| {
        state.patient.get();
        exporting.set(false);
        export_msg.set(None);
    });

    let on_export = move |_| {
        let Some(patient) = state.patient.get() else {
            return;
        };
        exporting.set(true);
        export_msg.set(None);
        spawn_local(async move {
            let labels = api::report_labels(lang.get_untracked());
            let result = api::export_report(&patient.hn, &labels).await;
            // Guard against a stale completion: the user may have picked a
            // different patient while the save dialog was open.
            if state.patient.get().as_ref() != Some(&patient) {
                return;
            }
            match result {
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

    let on_clear = move |_| {
        state.patient.set(None);
        state.patient_photo.set(None);
        state.history.set(None);
        state.history_error.set(None);
        state.history_loading.set(false);
        state.search_query.set(String::new());
    };

    view! {
        {move || {
            let patient = state.patient.get()?;
            let name = patient.display_name();
            let hn = patient.hn.clone();
            let cid = patient.cid.clone();
            let birthday = patient
                .birthday
                .map(|d| format!("{:02}/{:02}/{}", d.day(), d.month(), d.year()));
            let age = patient.birthday.map(age_years);
            let photo = state.patient_photo.get();
            Some(view! {
                <div class="sidebar__section sidebar__section--patient">
                    <div class="patient-card">
                        <div class="patient-card__head">
                            <p class="sidebar__label patient-card__label">
                                <IconUser class="icon" />
                                {tr(lang.get(), "patient.title")}
                            </p>
                            <button
                                class="icon-button"
                                on:click=on_clear
                                aria-label=move || tr(lang.get(), "patient.clear")
                                title=move || tr(lang.get(), "patient.clear")
                            >
                                <IconX class="icon" />
                            </button>
                        </div>
                        <div class="patient-card__main">
                            {move || photo.clone().map(|src| view! {
                                <img
                                    class="patient-card__photo"
                                    src=src
                                    alt=tr(lang.get(), "patient.photo_alt")
                                />
                            }.into_any()).unwrap_or_else(|| view! {
                                <div class="patient-card__photo patient-card__photo--placeholder">
                                    <IconUser class="patient-card__photo-icon" />
                                </div>
                            }.into_any())}
                            <div class="patient-card__info">
                                <h3 class="patient-card__name">{name}</h3>
                                <p class="patient-card__meta">
                                    <span class="patient-card__meta-item">
                                        <span class="patient-card__meta-label">{tr(lang.get(), "patient.hn")}</span>
                                        <span class="code">{hn}</span>
                                    </span>
                                    {move || cid.as_ref().map(|c| view! {
                                        <>
                                            <span class="sep">"·"</span>
                                            <span class="patient-card__meta-item">
                                                <span class="patient-card__meta-label">{tr(lang.get(), "patient.cid")}</span>
                                                <span class="code">{c.clone()}</span>
                                            </span>
                                        </>
                                    })}
                                </p>
                                <p class="patient-card__meta">
                                    {move || birthday.as_ref().map(|b| view! {
                                        <span class="patient-card__meta-item">
                                            <span class="patient-card__meta-label">{tr(lang.get(), "patient.birthday")}</span>
                                            <span>{b.clone()}</span>
                                        </span>
                                    })}
                                    {move || age.map(|a| view! {
                                        <>
                                            <span class="sep">"·"</span>
                                            <span class="patient-card__meta-item">
                                                <span class="patient-card__meta-label">{tr(lang.get(), "patient.age")}</span>
                                                <span>{tr_f(lang.get(), "patient.age_value", &[("n", &a.to_string())])}</span>
                                            </span>
                                        </>
                                    })}
                                </p>
                            </div>
                        </div>
                        <button
                            class="button-primary patient-card__export"
                            on:click=on_export
                            prop:disabled=move || exporting.get()
                        >
                            <IconPrinter class="icon" />
                            {move || if exporting.get() { tr(lang.get(), "canvas.exporting") } else { tr(lang.get(), "canvas.export") }}
                        </button>
                        {move || export_msg.get().map(|(ok, msg)| {
                            if ok {
                                view! { <p class="patient-card__status patient-card__status--ok">{msg.clone()}</p> }.into_any()
                            } else {
                                view! { <p class="patient-card__status patient-card__status--error">{msg.clone()}</p> }.into_any()
                            }
                        })}
                    </div>
                </div>
            })
        }}
    }
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
