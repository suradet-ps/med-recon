# Authentication - HOSxP officer login

Med Recon signs operators in with their HOSxP **`officer`** account before
any patient data is shown. HOSxP stays read-only: the login session lives
only in the Tauri process, and the app never writes an audit row or a session
token back to HOSxP.

## A. Source of truth

| Table | Observed on the target instance | Use |
|---|---|---|
| `officer` | `officer_login_name` unique; `officer_login_password_md5` 32 hex on every account; `officer_active = 'Y'` on current accounts | **The only login source** |
| `opduser` | Legacy V3 accounts | **Never read** - the gateway blacklists it |
| `onlineuser` | Optional online-status hint only | Never identity |
| `officer_activity_log` | Existing HOSxP audit trail | Med Recon does not write it |
| `user_jwt` | Never read | - |

Additional verified facts (2026-10-21 survey, shared with the sibling
ER-Refill project):

- `officer_authentication_type_id` is `NULL` on all current rows; the query
  coalesces it to **1 = Local Database**. LDAP (2) is rejected for now.
- `officer_login_password` (plaintext) is **never selected**; only
  `officer_login_password_md5` is read.
- `officer_active` is checked before a session is created.
- `officer_group*`, `officer_module`, `officer_task`, and
  `officer_operation` are not a usable permission source (group tables are
  empty). Every signed-in officer gets the same `STAFF` role for now.
- `officer_doctor_code` is present on most accounts. The query uses
  `NULLIF` and falls back to the login name when neither the officer name
  fields nor `doctor.name` yield a display name.
- `emp_id` is 0 on all accounts, so no employee-table join is possible.

## B. Flow

```
Launch
  │
  └─ login gate (the first screen)
        ├─ "ตั้งค่าการเชื่อมต่อ" opens the settings dialog
        │     └─ connection form (host · port · database · read-only user · password)
        │           └─ save: connect → SELECT 1 → encrypt & save → pool
        │
        ├─ no saved config → the login card says so and points at settings
        │
        └─ officer login (username + password)
              ├─ fetch one officer row by login name
              ├─ check active = 'Y'
              ├─ check auth_type = 1 (Local Database)
              ├─ verify MD5(password) against officer_login_password_md5
              └─ store an in-memory SessionUser; the search UI opens
```

The login card handles only officer credentials; connection setup lives in
the settings dialog. A connection change clears the backend session and
returns the UI to the login gate, so patient data from the previous site can
never remain on screen.

## C. Password verification

The app computes `MD5(password)` in Rust (`med_recon_core::verify_password`)
and compares it with the stored 32-hex digest, case-insensitively. The
password is moved into a `secrecy::SecretString` for the verification window;
it is never written to disk, logged, or returned to the frontend.

> **Unverified assumption.** The survey confirmed the *shape* (MD5, 32 hex)
> but could not confirm from HOSxP source code what string is hashed (salt,
> encoding, or normalization). Before the pilot, test one known account with
> read-only access. If the real algorithm differs, login fails for every
> account and this assumption must be corrected first. Thai passwords would
> also require confirming the source encoding (UTF-8 vs TIS-620).

## D. Session and roles

- `SessionUser` is kept in the Tauri `AppState.session` only.
- Restarting the app requires signing in again.
- The frontend never receives the password hash.
- A signed-in session is required for patient and PHI commands:
  `search_patients`, `load_history`, `load_patient_image`, `export_report`,
  and `capture_screenshot`.
- A session is also required for the settings that follow the BPMH rules:
  `search_drugs`, `get_current_meds`, and `save_site_settings`.
- Connection commands (`is_configured`, `get_connection`, `test_connection`,
  `save_connection`, `get_site_settings`, `get_app_status`,
  `connection_health`) stay available before sign-in so the operator can set
  up the site.
- The role is `STAFF` for everyone until app-specific roles are introduced.
  HOSxP group permissions are not consulted.
- Logging out clears the backend session and every patient-scoped signal on
  the frontend (search text, selected patient, photo, history, dismissed
  rows), and invalidates in-flight history requests.

## E. Query

The only login query is `FETCH_OFFICER_SQL` in
`crates/med-recon-hosxp/src/queries.rs`:

```sql
SELECT CAST(o.officer_id AS SIGNED) AS officer_id,
       o.officer_login_name AS login_name,
       COALESCE(o.officer_login_password_md5, '') AS password_md5,
       COALESCE(o.officer_active, 'N') AS active,
       CAST(COALESCE(o.officer_authentication_type_id, 1) AS SIGNED) AS auth_type_id,
       NULLIF(o.officer_doctor_code, '') AS doctor_code,
       COALESCE(
           NULLIF(TRIM(CONCAT(COALESCE(o.officer_pname, ''), COALESCE(o.officer_fname, ''), ' ', COALESCE(o.officer_lname, ''))), ''),
           NULLIF(TRIM(d.name), ''),
           ''
       ) AS display_name
FROM officer o
LEFT JOIN doctor d ON d.code = NULLIF(o.officer_doctor_code, '')
WHERE o.officer_login_name = ?
LIMIT 1
```

It is a single `SELECT` with one bound parameter; `opduser` and the
plaintext password column never appear. The top bar shows this display name;
if both the officer name fields and the doctor name are blank, the backend
falls back to the login name.

## F. Why no app_* tables

The proposed `app_user`, `app_session`, `app_login_log`, `app_role`, and
`app_role_permission` tables are not created:

- `CREATE TABLE`/`CREATE VIEW` are writes, and Med Recon must never write to
  HOSxP.
- Med Recon has no separate application database. Adding one is a product
  decision, not part of this milestone.
- A persistent login log would duplicate HOSxP `officer_activity_log` and
  would store staff identifiers locally; the current privacy rule is to
  avoid that until the hospital approves a local audit store.

If a local application database is approved later, those tables can be
introduced there without touching HOSxP.

## G. Acceptance checklist

- [ ] Verify `MD5(password)` on one real test account (read-only).
- [ ] Confirm the test account's password encoding (ASCII/UTF-8/TIS-620).
- [ ] Confirm `officer_authentication_type_id` really means 1 = Local
      Database on this instance.
- [ ] Confirm the display-name sources: `officer_pname`, `officer_fname`,
      `officer_lname`, and the `doctor.name` fallback via
      `officer_doctor_code`.
- [ ] Confirm whether the legacy `opduser` users were migrated to `officer`;
      users who only exist in `opduser` cannot sign in.
- [ ] Test active/inactive and LDAP rejection paths with test data.
- [ ] Confirm the read-only MySQL user can `SELECT officer` (no write grant).
