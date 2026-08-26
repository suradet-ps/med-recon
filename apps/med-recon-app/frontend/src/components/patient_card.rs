//! Sidebar patient card - the selected patient's photo, identity, and the
//! "print medication history" action.
//!
//! Rendered below the search section after a patient is picked, so the
//! main canvas keeps its full vertical space for the history itself.

use chrono::{Datelike, NaiveDate};
use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::api;
use crate::components::icons::{IconCamera, IconPrinter, IconUser, IconX};
use crate::state::AppState;

#[component]
pub fn PatientCard(state: AppState) -> impl IntoView {
    let exporting = RwSignal::new(false);
    let export_msg = RwSignal::new(None::<(bool, String)>);
    let capturing = RwSignal::new(false);
    let capture_msg = RwSignal::new(None::<(bool, String)>);

    // The status messages belong to one patient - reset them whenever the
    // selection changes so a stale "บันทึกรายงานแล้ว" never lingers.
    Effect::new(move |_| {
        state.patient.get();
        exporting.set(false);
        export_msg.set(None);
        capturing.set(false);
        capture_msg.set(None);
    });

    let on_export = move |_| {
        let Some(patient) = state.patient.get() else {
            return;
        };
        exporting.set(true);
        export_msg.set(None);
        spawn_local(async move {
            let labels = api::report_labels();
            let result = api::export_report(&patient.hn, &labels).await;
            // Guard against a stale completion: the user may have picked a
            // different patient while the save dialog was open.
            if state.patient.get().as_ref() != Some(&patient) {
                return;
            }
            match result {
                Ok(path) => export_msg.set(Some((true, format!("บันทึกรายงาน PDF แล้ว: {path}")))),
                Err(e) => export_msg.set(Some((false, e.message))),
            }
            exporting.set(false);
        });
    };

    let on_capture = move |_| {
        let Some(patient) = state.patient.get() else {
            return;
        };
        capturing.set(true);
        capture_msg.set(None);
        spawn_local(async move {
            // The capture happens before the save dialog opens (back-end
            // order), so the dialog itself never appears in the shot.
            // devicePixelRatio is passed so the backend re-rasterizes at
            // physical resolution (crisp on 125–200% displays).
            let dpr = web_sys::window()
                .map(|w| w.device_pixel_ratio())
                .unwrap_or(1.0);
            let result =
                api::capture_screenshot(&format!("med-recon-screen-{}", patient.hn), dpr).await;
            if state.patient.get().as_ref() != Some(&patient) {
                return;
            }
            match result {
                Ok(path) => capture_msg.set(Some((true, format!("บันทึกภาพแล้ว: {path}")))),
                Err(e) => capture_msg.set(Some((false, e.message))),
            }
            capturing.set(false);
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
                                "ข้อมูลผู้ป่วย"
                            </p>
                            <button
                                class="icon-button"
                                on:click=on_clear
                                aria-label="ยกเลิกการเลือกผู้ป่วย"
                                title="ยกเลิกการเลือกผู้ป่วย"
                            >
                                <IconX class="icon" />
                            </button>
                        </div>
                        <div class="patient-card__main">
                            {move || photo.clone().map(|src| view! {
                                <img
                                    class="patient-card__photo"
                                    src=src
                                    alt="รูปผู้ป่วย"
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
                                        <span class="patient-card__meta-label">"HN"</span>
                                        <span class="code">{hn}</span>
                                    </span>
                                    {move || cid.as_ref().map(|c| view! {
                                        <>
                                            <span class="sep">"·"</span>
                                            <span class="patient-card__meta-item">
                                                <span class="patient-card__meta-label">"CID"</span>
                                                <span class="code">{c.clone()}</span>
                                            </span>
                                        </>
                                    })}
                                </p>
                                <p class="patient-card__meta">
                                    {move || birthday.as_ref().map(|b| view! {
                                        <span class="patient-card__meta-item">
                                            <span class="patient-card__meta-label">"วันเกิด"</span>
                                            <span>{b.clone()}</span>
                                        </span>
                                    })}
                                    {move || age.map(|a| view! {
                                        <>
                                            <span class="sep">"·"</span>
                                            <span class="patient-card__meta-item">
                                                <span class="patient-card__meta-label">"อายุ"</span>
                                                <span>{format!("{a} ปี")}</span>
                                            </span>
                                        </>
                                    })}
                                </p>
                            </div>
                        </div>
                        <div class="patient-card__actions">
                            <button
                                class="button-primary patient-card__export"
                                on:click=on_export
                                prop:disabled=move || exporting.get()
                            >
                                <IconPrinter class="icon" />
                                {move || if exporting.get() { "กำลังส่งออก…" } else { "พิมพ์ประวัติการได้รับยา" }}
                            </button>
                            <button
                                class="button-secondary patient-card__capture"
                                on:click=on_capture
                                prop:disabled=move || capturing.get()
                            >
                                <IconCamera class="icon" />
                                {move || if capturing.get() { "กำลังถ่ายภาพ…" } else { "ถ่ายภาพหน้าจอ" }}
                            </button>
                        </div>
                        {move || export_msg.get().map(|(ok, msg)| {
                            if ok {
                                view! { <p class="patient-card__status patient-card__status--ok">{msg.clone()}</p> }.into_any()
                            } else {
                                view! { <p class="patient-card__status patient-card__status--error">{msg.clone()}</p> }.into_any()
                            }
                        })}
                        {move || capture_msg.get().map(|(ok, msg)| {
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
/// Uses the browser's local date (`js_sys::Date`) - `std::time::SystemTime`
/// panics under `wasm32`. The date arithmetic lives in [`age_years_on`] so it
/// stays testable on the native target.
fn age_years(birthday: NaiveDate) -> i32 {
    let today = js_sys::Date::new_0();
    let year = today.get_full_year() as i32;
    let month = today.get_month() + 1;
    let day = today.get_date();
    let today = match NaiveDate::from_ymd_opt(year, month, day) {
        Some(d) => d,
        None => return 0,
    };
    age_years_on(birthday, today)
}

/// Whole-years age as of a given date (pure; testable on native).
fn age_years_on(birthday: NaiveDate, today: NaiveDate) -> i32 {
    let mut age = today.year() - birthday.year();
    if (today.month(), today.day()) < (birthday.month(), birthday.day()) {
        age -= 1;
    }
    age
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn age_birthday_already_passed_this_year() {
        assert_eq!(age_years_on(date(2000, 6, 15), date(2026, 8, 22)), 26);
    }

    #[test]
    fn age_birthday_later_this_year() {
        assert_eq!(age_years_on(date(2000, 12, 31), date(2026, 8, 22)), 25);
    }

    #[test]
    fn age_exact_birthday_today() {
        assert_eq!(age_years_on(date(2000, 8, 22), date(2026, 8, 22)), 26);
    }

    #[test]
    fn age_leap_day_birthday() {
        assert_eq!(age_years_on(date(2000, 2, 29), date(2026, 3, 1)), 26);
    }
}
