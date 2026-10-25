//! Login gate: verify an `officer` account against HOSxP.
//!
//! Connection setup lives in the settings dialog, not on this screen; the
//! login card stays focused on the officer credentials. Passwords are
//! verified in the Rust backend against the stored MD5 and are never
//! persisted (docs/auth.md).

use leptos::prelude::*;

use crate::api::{self, ApiErrorKind};
use crate::components::icons::{IconArrowRight, IconLock, IconLogo, IconSettings};
use crate::state::{AppState, ConnectionHealth};

/// Full-screen login surface used until a HOSxP officer signs in.
#[component]
pub fn LoginScreen(state: AppState) -> impl IntoView {
    let username = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let login_busy = RwSignal::new(false);
    let login_error = RwSignal::new(None::<String>);

    let open_settings = move |_| state.settings_open.set(true);

    let on_login = move |ev: web_sys::SubmitEvent| {
        ev.prevent_default();
        let login_name = username.get();
        if login_name.trim().is_empty() {
            login_error.set(Some("กรุณากรอกชื่อผู้ใช้".to_string()));
            return;
        }
        let login_password = password.get();
        if login_password.is_empty() {
            login_error.set(Some("กรุณากรอกรหัสผ่าน".to_string()));
            return;
        }
        login_busy.set(true);
        login_error.set(None);
        leptos::task::spawn_local(async move {
            match api::login(login_name.trim(), &login_password).await {
                Ok(user) => {
                    password.set(String::new());
                    state.finish_login(user);
                }
                Err(err) => {
                    if matches!(
                        err.kind,
                        ApiErrorKind::Connection | ApiErrorKind::NotConfigured
                    ) {
                        state.health.set(ConnectionHealth::Disconnected);
                    }
                    login_error.set(Some(err.message));
                }
            }
            login_busy.set(false);
        });
    };

    view! {
        <div class="login">
            <aside class="login__brand">
                <div class="login__brand-top">
                    <IconLogo class="login__brand-mark" />
                    <div class="login__brand-titles">
                        <span class="login__brand-name">"Med Recon"</span>
                        <span class="login__brand-sub">"ระบบตรวจสอบประวัติยา"</span>
                    </div>
                </div>

                <div class="login__stage">
                    <h1 class="login__headline">"ประสานรายการยาผู้ป่วย"</h1>
                    <p class="login__lede">
                        "BPMH · ประวัติการแพ้ยา · ประวัติการเข้ารับบริการ"
                    </p>
                    <div class="login__hero" aria-hidden="true">
                        <div class="login__hero-glow"></div>
                        <div
                            class="login__art"
                            inner_html=include_str!("../../../../../icon-master.svg")
                        ></div>
                    </div>
                </div>
            </aside>

            <main class="login__main">
                <div class="login__toolbar">
                    <button class="button-secondary" on:click=open_settings>
                        <IconSettings class="icon" />
                        "ตั้งค่าการเชื่อมต่อ"
                    </button>
                </div>

                <div class="login__card">
                    <header class="login__card-head">
                        <h2 class="login__card-title">"เข้าสู่ระบบ"</h2>
                    </header>

                    {move || {
                        (!state.configured.get())
                            .then(|| {
                                view! {
                                    <div class="login__notice" role="status">
                                        <div>
                                            <b>"ยังไม่ได้ตั้งค่าการเชื่อมต่อ HOSxP"</b>
                                            <small>
                                                "เปิดการตั้งค่าเพื่อเชื่อมต่อ HOSxP ก่อนเข้าสู่ระบบ"
                                            </small>
                                        </div>
                                        <button
                                            class="button-secondary"
                                            on:click=open_settings
                                        >
                                            "ตั้งค่าการเชื่อมต่อ"
                                        </button>
                                    </div>
                                }
                            })
                    }}

                    <form class="login__form" on:submit=on_login>
                        <label class="login__field">
                            <span>"ชื่อผู้ใช้"</span>
                            <input
                                class="form-input"
                                type="text"
                                autocomplete="username"
                                prop:value=move || username.get()
                                on:input=move |ev| username.set(event_target_value(&ev))
                            />
                        </label>
                        <label class="login__field">
                            <span>"รหัสผ่าน"</span>
                            <input
                                class="form-input"
                                type="password"
                                autocomplete="current-password"
                                prop:value=move || password.get()
                                on:input=move |ev| password.set(event_target_value(&ev))
                            />
                        </label>

                        {move || {
                            login_error
                                .get()
                                .map(|message| {
                                    view! {
                                        <p class="login__error" role="alert">
                                            <IconLock class="icon" />
                                            {message}
                                        </p>
                                    }
                                })
                        }}

                        <div class="login__actions">
                            <button
                                type="submit"
                                class="button-primary login__submit"
                                disabled=move || !state.configured.get() || login_busy.get()
                            >
                                <IconArrowRight class="icon" />
                                {move || {
                                    if login_busy.get() {
                                        "กำลังเข้าสู่ระบบ..."
                                    } else {
                                        "เข้าสู่ระบบ"
                                    }
                                }}
                            </button>
                        </div>
                    </form>
                </div>
            </main>
        </div>
    }
}

/// Minimal startup state while configuration and session are probed.
#[component]
pub fn StartupSplash() -> impl IntoView {
    view! {
        <div class="startup">
            <IconLogo class="startup__logo" />
            <p>"กำลังเตรียมระบบ..."</p>
        </div>
    }
}
