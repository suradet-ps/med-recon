//! User manual (คู่มือการใช้งาน) dialog - a static, scrollable guide
//! covering first-run setup, patient search, history reading, report
//! export, and privacy/read-only cautions.
//!
//! Pure presentation: no backend calls, no signals beyond the shared
//! `help_open` flag. Reuses the settings modal's backdrop/modal CSS.

use leptos::ev;
use leptos::prelude::*;

use crate::components::icons::IconX;
use crate::state::AppState;

#[component]
pub fn HelpModal(state: AppState) -> impl IntoView {
    let close = move || state.help_open.set(false);

    // Escape closes the dialog from anywhere.
    let open_flag = state.help_open;
    let close_on_escape = move |event: ev::KeyboardEvent| {
        if event.key() == "Escape" && open_flag.get_untracked() {
            close();
        }
    };
    let escape_handle = window_event_listener(ev::keydown, close_on_escape);
    let _escape_handle = StoredValue::new(escape_handle);

    view! {
        <div
            class="modal-backdrop"
            style:display=move || {
                if state.help_open.get() {
                    "flex"
                } else {
                    "none"
                }
            }
            on:click=move |_| close()
        >
            <section class="modal modal--help" on:click=move |ev| ev.stop_propagation()>
                <h2 class="modal__title">"คู่มือการใช้งาน"</h2>
                <p class="modal__status">
                    "วิธีใช้งาน Med Recon ตั้งแต่ตั้งค่าครั้งแรกจนถึงพิมพ์รายงาน"
                </p>

                <section class="help-section">
                    <h3 class="help-section__title">"1. ตั้งค่าเบื้องต้น (ครั้งแรก)"</h3>
                    <ol class="help-steps">
                        <li>"กดปุ่ม ตั้งค่า ที่มุมขวาบน"</li>
                        <li>
                            "กรอก Host, Port, Database, User, Password ของระบบ HOSxP"
                        </li>
                        <li>"กด ทดสอบ เพื่อตรวจสอบการเชื่อมต่อก่อนบันทึก"</li>
                        <li>"กด บันทึก เมื่อเชื่อมต่อสำเร็จ"</li>
                        <li>
                            "ไปที่แท็บ ตั้งค่าอื่นๆ: ตั้งชื่อสถานบริการ กำหนดระยะเวลา"
                            "ค้นประวัติย้อนหลัง และเลือกยาที่ต้องการแสดงในหัวข้อ"
                            "ยาที่ผู้ป่วยเคยได้รับ"
                        </li>
                    </ol>
                </section>

                <section class="help-section">
                    <h3 class="help-section__title">"2. ค้นหาผู้ป่วย"</h3>
                    <ol class="help-steps">
                        <li>
                            "พิมพ์ในช่องค้นหาด้านซ้าย: เลขบัตรประชาชน 13 หลัก,"
                            "HN หรือชื่อผู้ป่วย"
                        </li>
                        <li>"คลิกที่ผลลัพธ์เพื่อโหลดประวัติทั้งหมดลงในหน้าจอหลัก"</li>
                    </ol>
                </section>

                <section class="help-section">
                    <h3 class="help-section__title">"3. อ่านผลประวัติ"</h3>
                    <ul class="help-steps help-steps--plain">
                        <li>
                            <strong>"ยาที่ผู้ป่วยเคยได้รับ"</strong>
                            " - ตัวเลขในวงเล็บคือ จำนวนครั้งที่ผู้ป่วยเคยได้รับยานี้"
                            " ซึ่งยาที่ตั้งค่าไว้เท่านั้นจึงจะแสดงในหัวข้อนี้"
                        </li>
                        <li>
                            <strong>"ยาที่คาดว่าหยุดใช้แล้ว"</strong>
                            " - ยาที่ไม่ตั้งค่าไว้จะถือว่าเป็นยาที่ผู้ป่วยเคยได้รับ"
                            "(ยาตามอาการ)"
                        </li>
                        <li>
                            <strong>"แพ้ยา / อาการไม่พึงประสงค์ (แดง)"</strong>
                            " - ประวัติการแพ้ยาที่มีบันทึกใน HOSxP"
                        </li>
                        <li>
                            <strong>"การตรวจ/อาการสำคัญ"</strong>
                            " - แสดง CC และ PE ที่ผู้ป่วยมารับยาในแต่ละ Visit"
                            " เพื่อตรวจสอบว่ามีการเปลี่ยนยาหรือไม่ อย่างไร"
                        </li>
                    </ul>
                    <p class="help-note">
                        "ข้อควรทราบ: ข้อมูลจากระบบจ่ายยาเป็นแหล่งข้อมูลหนึ่ง"
                        "อาจไม่ครบถ้วนหรือไม่ใช่รายการยาที่ผู้ป่วยใช้จริงในปัจจุบัน"
                        "ควรใช้ประกอบการซักประวัติผู้ป่วยเสมอ"
                    </p>
                </section>

                <section class="help-section">
                    <h3 class="help-section__title">"4. พิมพ์รายงาน"</h3>
                    <ol class="help-steps">
                        <li>
                            "กดปุ่ม พิมพ์ประวัติการได้รับยา ในแถบข้อมูลผู้ป่วย"
                            "เพื่อบันทึกรายงาน"
                        </li>
                    </ol>
                </section>

                <section class="help-section">
                    <h3 class="help-section__title">"5. ถ่ายภาพหน้าจอ"</h3>
                    <ol class="help-steps">
                        <li>
                            "กดปุ่ม ถ่ายภาพหน้าจอ ในแถบข้อมูลผู้ป่วย เพื่อถ่ายภาพ"
                            "หน้าจอโปรแกรม แล้วบันทึกเป็นไฟล์รูปภาพ"
                        </li>
                        <li>"รองรับเฉพาะบนระบบปฏิบัติการ Windows เท่านั้น"</li>
                    </ol>
                </section>

                <section class="help-section">
                    <h3 class="help-section__title">"6. ข้อควรระวัง"</h3>
                    <ul class="help-steps help-steps--plain">
                        <li>
                            "ข้อมูลผู้ป่วยเป็นความลับ (PHI) ควรใช้งานบนเครื่อง"
                            "ที่ปลอดภัยและไม่เปิดเผยข้อมูล"
                        </li>
                        <li>"Med Recon อ่านข้อมูลจาก HOSxP แบบอ่านอย่างเดียว ไม่มีการแก้ไขข้อมูล"</li>
                        <li>
                            "วันที่ในระบบอาจแสดงเป็น พ.ศ. หรือ ค.ศ. ตามข้อมูล"
                            "ของโรงพยาบาล โดย Med Recon จะแปลงเป็น ค.ศ. อัตโนมัติ"
                        </li>
                    </ul>
                </section>

                <div class="modal__actions">
                    <button class="button-secondary button-secondary--inline" on:click=move |_| close()>
                        <IconX class="icon" />
                        "ปิด"
                    </button>
                </div>
            </section>
        </div>
    }
}
