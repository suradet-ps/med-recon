//! The HOSxP repository client.
//!
//! Wraps a `sqlx` `MySqlPool`. Every public query method runs its statement
//! through the read-only guard first (see [`crate::readonly`]). All date
//! values are normalized from the site's era (auto-detected per value,
//! พ.ศ. or ค.ศ.) to Christian era at the boundary (see
//! [`med_recon_core::normalize_date`]).

use chrono::NaiveDate;
use med_recon_core::{
    AllergyRecord, Dispense, EncounterSource, MedicationItem, OpdScreenRecord, PatientHistory,
    PatientSummary, VisitSummary, aggregate_medications, normalize_date,
};
use secrecy::ExposeSecret;
use sqlx::mysql::{MySqlConnectOptions, MySqlPool, MySqlPoolOptions};
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use tracing::debug;

use crate::config::HosxpConfig;
use crate::error::{Error, Result};
use crate::queries;
use crate::readonly::assert_read_only;

/// Repository for a single HOSxP site. Cheap to clone (pool handle).
#[derive(Clone)]
pub struct HosxpClient {
    pool: MySqlPool,
    config: HosxpConfig,
}

impl HosxpClient {
    /// Open a connection pool to the configured HOSxP database.
    ///
    /// Pool acquisition is capped so an unreachable or slow HOSxP host
    /// fails fast instead of hanging the UI; the wall-clock cap for the
    /// whole connect lives in the command layer.
    pub async fn connect(config: HosxpConfig) -> Result<Self> {
        let opts = MySqlConnectOptions::new()
            .host(&config.host)
            .port(config.port)
            .database(&config.database)
            .username(&config.user)
            .password(config.password.expose_secret());

        let pool = MySqlPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(opts)
            .await
            .map_err(|source| Error::Connect {
                host: config.host.clone(),
                port: config.port,
                database: config.database.clone(),
                source,
            })?;

        debug!(
            "connected to HOSxP at {}:{} (read-only)",
            config.host, config.port
        );
        Ok(Self { pool, config })
    }

    /// Close all pooled connections.
    pub async fn disconnect(&self) {
        self.pool.close().await;
    }

    /// Verify the connection is alive with a trivial read-only query.
    pub async fn ping(&self) -> Result<()> {
        self.fetch_row::<()>("SELECT 1", &[]).await?;
        Ok(())
    }

    /// Search patients, auto-detecting the input kind: 13-digit CID, HN,
    /// or name.
    ///
    /// Exact CID/HN hits are matched via equality. Names are matched
    /// case-insensitively as prefixes first (index-friendly); a contains
    /// match runs only when the prefix search found nothing.
    pub async fn search_patients(&self, query: &str) -> Result<Vec<PatientSummary>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let rows: Vec<PatientRow> = match med_recon_core::detect_query_kind(query) {
            med_recon_core::QueryKind::Cid => {
                self.fetch_rows(
                    queries::PATIENT_SEARCH_BY_CID,
                    &[
                        P::Str(query.to_owned()),
                        P::Int(queries::DEFAULT_SEARCH_LIMIT as i64),
                    ],
                )
                .await?
            }
            med_recon_core::QueryKind::Hn => {
                self.fetch_rows(
                    queries::PATIENT_SEARCH_BY_HN,
                    &[
                        P::Str(query.to_owned()),
                        P::Int(queries::DEFAULT_SEARCH_LIMIT as i64),
                    ],
                )
                .await?
            }
            med_recon_core::QueryKind::Name => {
                let prefix = queries::prefix_pattern(query);
                let prefix_rows: Vec<PatientRow> = self
                    .fetch_rows(
                        queries::PATIENT_SEARCH_NAME_PREFIX,
                        &[
                            P::Str(prefix.clone()),
                            P::Str(prefix.clone()),
                            P::Str(prefix.clone()),
                            P::Int(queries::DEFAULT_SEARCH_LIMIT as i64),
                        ],
                    )
                    .await?;
                if !prefix_rows.is_empty() {
                    prefix_rows
                } else {
                    let pattern = queries::like_pattern(query);
                    self.fetch_rows(
                        queries::PATIENT_SEARCH_NAME_CONTAINS,
                        &[
                            P::Str(pattern.clone()),
                            P::Str(pattern.clone()),
                            P::Str(pattern.clone()),
                            P::Int(queries::DEFAULT_SEARCH_LIMIT as i64),
                        ],
                    )
                    .await?
                }
            }
        };

        Ok(rows.into_iter().map(|r| map_patient(&r)).collect())
    }

    /// Load the full cross-visit history for one patient: identity, BPMH
    /// medication list, allergies, and visits.
    ///
    /// `current_codes` is the operator-configured current-medication list
    /// (`icode`s); it decides which medications are labelled active vs
    /// lapsed in the BPMH list (see [`med_recon_core::aggregate_medications`]).
    ///
    /// Date eras are **auto-detected per value** (see
    /// [`med_recon_core::normalize_date`]); there is no site-era setting. The
    /// SQL queries are sent a Christian-era cutoff: on Buddhist-era sites
    /// the year comparison matches every stored date, so the exact window
    /// is enforced client-side after normalization.
    ///
    /// HOSxP schemas vary by site: if a table used by a query is missing
    /// (error 1146), that section is skipped and a user-visible warning is
    /// recorded in [`PatientHistory::warnings`] instead of failing the whole
    /// load.
    pub async fn load_history(
        &self,
        hn: &str,
        current_codes: &HashSet<String>,
    ) -> Result<PatientHistory> {
        let patient = self.load_patient(hn).await?;

        let cutoff = self
            .config
            .history_cutoff(chrono::Local::now().date_naive());

        let mut warnings = Vec::new();

        let appointments = self.load_appointments(hn, cutoff, &mut warnings).await?;
        let opd_dispenses = self
            .load_opd_dispenses(hn, cutoff, &appointments, &mut warnings)
            .await?;
        let mut ipd_dispenses = self.load_ipd_dispenses(hn, cutoff, &mut warnings).await?;
        let mut dispenses = opd_dispenses;
        dispenses.append(&mut ipd_dispenses);

        let sigs = self.load_sigs(hn, cutoff, &mut warnings).await?;
        for d in &mut dispenses {
            if let Some(sig) = sigs.get(&(d.visit_id.clone(), d.icode.clone())) {
                d.sig = Some(sig.clone());
            }
        }

        let allergies = self.load_allergies(hn, &mut warnings).await?;
        let screen_records = self.load_screen_records(hn, cutoff, &mut warnings).await?;
        let mut visits = self.load_visits(hn, cutoff, &mut warnings).await?;
        visits.sort_by_key(|a| std::cmp::Reverse(a.date));

        let today = chrono::Local::now().date_naive();
        let medications = aggregate_medications(&dispenses, today, current_codes);

        Ok(PatientHistory {
            patient,
            medications,
            allergies,
            screen_records,
            visits,
            warnings,
        })
    }

    /// Fetch one patient's identity by HN.
    pub async fn load_patient(&self, hn: &str) -> Result<PatientSummary> {
        let rows: Vec<PatientRow> = self
            .fetch_rows(queries::PATIENT_BY_HN_SQL, &[P::Str(hn.to_owned())])
            .await?;
        rows.into_iter()
            .next()
            .map(|r| map_patient(&r))
            .ok_or_else(|| {
                Error::NotFound(format!(
                    "patient with hn {} not found",
                    med_recon_core::redact_hn(hn)
                ))
            })
    }

    /// Load the BPMH medication list for a patient (used by the report
    /// export).
    pub async fn load_medications(
        &self,
        hn: &str,
        current_codes: &HashSet<String>,
    ) -> Result<(PatientSummary, Vec<MedicationItem>)> {
        let history = self.load_history(hn, current_codes).await?;
        Ok((history.patient, history.medications))
    }

    /// Search the drug master (`drugitems`) by name or code.
    ///
    /// Used by the current-medication settings picker. Returns up to
    /// [`queries::DEFAULT_SEARCH_LIMIT`] hits, name-ordered.
    pub async fn search_drugs(&self, query: &str) -> Result<Vec<DrugItem>> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let pattern = queries::like_pattern(query);
        self.fetch_rows(
            queries::DRUG_SEARCH_SQL,
            &[
                P::Str(pattern.clone()),
                P::Str(pattern),
                P::Int(queries::DEFAULT_SEARCH_LIMIT as i64),
            ],
        )
        .await
    }

    /// Resolve `icode`s back to drug master rows (name/strength/units),
    /// name-ordered. Missing codes are silently dropped.
    pub async fn load_drugs_by_codes(&self, codes: &[String]) -> Result<Vec<DrugItem>> {
        if codes.is_empty() {
            return Ok(Vec::new());
        }
        let sql = queries::drugs_by_codes_sql(codes);
        assert_read_only(&sql)?;
        let mut query = sqlx::query_as::<_, DrugItem>(&sql);
        for code in codes {
            query = query.bind(code);
        }
        Ok(query.fetch_all(&self.pool).await?)
    }

    async fn load_opd_dispenses(
        &self,
        hn: &str,
        cutoff: NaiveDate,
        appointments: &HashMap<String, NaiveDate>,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Dispense>> {
        let rows: Vec<DispenseRow> = match self
            .fetch_first_working(&[
                (
                    queries::OPD_DISPENSE_SQL,
                    vec![P::Str(hn.to_owned()), P::Date(cutoff)],
                ),
                (
                    queries::OPD_DISPENSE_SQL_FALLBACK,
                    vec![P::Str(hn.to_owned()), P::Date(cutoff)],
                ),
            ])
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "opitemrece", "การจ่ายยา OPD");
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };
        Ok(rows
            .into_iter()
            .filter_map(|r| map_dispense(r, hn, EncounterSource::Opd, cutoff))
            .map(|mut d| {
                d.appointment = appointments.get(&d.visit_id).copied();
                d
            })
            .collect())
    }

    async fn load_ipd_dispenses(
        &self,
        hn: &str,
        cutoff: NaiveDate,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<Dispense>> {
        let rows: Vec<DispenseRow> = match self
            .fetch_first_working(&[
                (
                    queries::IPD_DISPENSE_SQL,
                    vec![P::Str(hn.to_owned()), P::Date(cutoff)],
                ),
                (
                    queries::IPD_DISPENSE_SQL_FALLBACK,
                    vec![P::Str(hn.to_owned()), P::Date(cutoff)],
                ),
            ])
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "opitemrece", "การจ่ายยาผู้ป่วยใน (IPD)");
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };
        Ok(rows
            .into_iter()
            .filter_map(|r| map_dispense(r, hn, EncounterSource::Ipd, cutoff))
            .collect())
    }

    async fn load_sigs(
        &self,
        hn: &str,
        cutoff: NaiveDate,
        warnings: &mut Vec<String>,
    ) -> Result<HashMap<(String, String), med_recon_core::Sig>> {
        let rows: Vec<SigRow> = match self
            .fetch_rows(queries::SIG_SQL, &[P::Str(hn.to_owned()), P::Date(cutoff)])
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "drugusage/sp_use", "วิธีใช้ยา (sig)");
                return Ok(HashMap::new());
            }
            Err(e) => return Err(e),
        };
        Ok(rows
            .into_iter()
            .filter_map(|r| {
                let visit_id = r.an.or(r.vn)?;
                let sig = queries::sig_from_names(
                    &[r.d_name1, r.d_name2, r.d_name3],
                    &[r.s_name1, r.s_name2, r.s_name3],
                )?;
                Some(((visit_id, r.icode), sig))
            })
            .collect())
    }

    /// Load OPD screening records (`opdscreen` CC/PE) for one patient, newest
    /// visit first. If the table is missing on this site (MySQL 1146) the
    /// section degrades to empty with a user-visible warning.
    async fn load_screen_records(
        &self,
        hn: &str,
        cutoff: NaiveDate,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<OpdScreenRecord>> {
        let rows: Vec<OpdScreenRow> = match self
            .fetch_rows(
                queries::OPD_SCREEN_SQL,
                &[P::Str(hn.to_owned()), P::Date(cutoff)],
            )
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "opdscreen", "การตรวจร่างกาย (CC/PE)");
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };
        Ok(rows
            .into_iter()
            .map(|r| OpdScreenRecord {
                vn: r.vn,
                vstdate: r.vstdate,
                cc: r.cc,
                pe: r.pe,
            })
            .collect())
    }

    async fn load_allergies(
        &self,
        hn: &str,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<AllergyRecord>> {
        let rows: Vec<AllergyRow> = match self
            .fetch_rows(queries::ALLERGY_SQL, &[P::Str(hn.to_owned())])
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "opd_allergy", "ประวัติแพ้ยา");
                return Ok(Vec::new());
            }
            Err(e) => return Err(e),
        };
        Ok(rows
            .into_iter()
            .map(|r| AllergyRecord {
                agent: clean_agent(&r.agent),
                symptom: r.symptom,
                report_date: r.report_date.map(normalize_date),
                note: r.note,
                reporter: r.reporter,
            })
            .collect())
    }

    /// Load each OPD visit's next appointment date (`oapp.nextdate`), keyed
    /// by `vn`. The latest planned follow-up wins per visit.
    ///
    /// If the `oapp` table is missing on this site (MySQL 1146) the section
    /// degrades to empty with a user-visible warning — appointment display
    /// is supplementary, not load-critical.
    async fn load_appointments(
        &self,
        hn: &str,
        cutoff: NaiveDate,
        warnings: &mut Vec<String>,
    ) -> Result<HashMap<String, NaiveDate>> {
        let rows: Vec<AppointmentRow> = match self
            .fetch_rows(
                queries::APPOINTMENT_SQL,
                &[P::Str(hn.to_owned()), P::Date(cutoff)],
            )
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "oapp", "วันนัด");
                return Ok(HashMap::new());
            }
            Err(e) => return Err(e),
        };
        Ok(rows
            .into_iter()
            .map(|r| (r.vn, normalize_date(r.nextdate)))
            .collect())
    }

    async fn load_visits(
        &self,
        hn: &str,
        cutoff: NaiveDate,
        warnings: &mut Vec<String>,
    ) -> Result<Vec<VisitSummary>> {
        let opd_rows: Vec<OpdVisitRow> = match self
            .fetch_rows(
                queries::OPD_VISIT_SQL,
                &[P::Str(hn.to_owned()), P::Date(cutoff)],
            )
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "ovst", "ประวัติการเข้ารับบริการ OPD");
                Vec::new()
            }
            Err(e) => return Err(e),
        };
        let ipd_rows: Vec<IpdVisitRow> = match self
            .fetch_rows(
                queries::IPD_VISIT_SQL,
                &[P::Str(hn.to_owned()), P::Date(cutoff)],
            )
            .await
        {
            Ok(rows) => rows,
            Err(e) if is_schema_variation(&e) => {
                warn_missing(warnings, "ipt", "ประวัติการเข้ารับบริการ IPD");
                Vec::new()
            }
            Err(e) => return Err(e),
        };

        let mut visits: Vec<VisitSummary> = opd_rows
            .into_iter()
            .filter_map(|r| {
                let date = normalize_date(r.vstdate);
                (date >= cutoff).then_some(VisitSummary {
                    visit_id: r.vn,
                    source: EncounterSource::Opd,
                    date,
                    department: r.main_dep,
                })
            })
            .collect();

        visits.extend(ipd_rows.into_iter().filter_map(|r| {
            let date = normalize_date(r.regdate);
            (date >= cutoff).then_some(VisitSummary {
                visit_id: r.an,
                source: EncounterSource::Ipd,
                date,
                department: r.ward,
            })
        }));

        Ok(visits)
    }

    /// Runs the first statement that succeeds, in order.
    ///
    /// Statements failing with a schema-variation error (MySQL 1146/1054 —
    /// documented per-instance differences) are skipped for the next tier,
    /// so a richer query can degrade to a safe baseline. Any other failure
    /// propagates immediately. If every candidate hits a schema variation,
    /// the last such error is returned so the caller can warn-and-skip.
    async fn fetch_first_working<T>(&self, candidates: &[(&'static str, Vec<P>)]) -> Result<Vec<T>>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        let mut last_schema: Option<Error> = None;
        for (sql, params) in candidates {
            match self.fetch_rows(sql, params).await {
                Ok(rows) => return Ok(rows),
                Err(e) if is_schema_variation(&e) => last_schema = Some(e),
                Err(e) => return Err(e),
            }
        }
        Err(last_schema.expect("invariant: at least one candidate statement is provided"))
    }

    /// Execute a read-only statement with typed bound parameters.
    async fn fetch_rows<T>(&self, stmt: &str, params: &[P]) -> Result<Vec<T>>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        assert_read_only(stmt)?;
        let mut query = sqlx::query_as::<_, T>(stmt);
        for p in params {
            query = match p {
                P::Str(v) => query.bind(v),
                P::Date(v) => query.bind(*v),
                P::Int(v) => query.bind(*v),
            };
        }
        Ok(query.fetch_all(&self.pool).await?)
    }

    /// Execute a read-only statement and return the first row (if any).
    async fn fetch_row<T>(&self, stmt: &str, params: &[P]) -> Result<Option<T>>
    where
        T: for<'r> sqlx::FromRow<'r, sqlx::mysql::MySqlRow> + Send + Unpin,
    {
        assert_read_only(stmt)?;
        let mut query = sqlx::query_as::<_, T>(stmt);
        for p in params {
            query = match p {
                P::Str(v) => query.bind(v),
                P::Date(v) => query.bind(*v),
                P::Int(v) => query.bind(*v),
            };
        }
        Ok(query.fetch_optional(&self.pool).await?)
    }
}

/// Whether the database error is a documented HOSxP schema variation:
/// MySQL 1146 ("table doesn't exist") or 1054 ("unknown column"). Such
/// errors select a fallback query tier instead of failing the app.
fn is_schema_variation(e: &Error) -> bool {
    matches!(
        e,
        Error::Database(sqlx::Error::Database(db))
            if matches!(db.code().as_deref(), Some("1146") | Some("1054"))
    )
}

/// Record a user-visible warning that a HOSxP table is missing on this
/// site, so the affected history section is skipped instead of failing.
fn warn_missing(warnings: &mut Vec<String>, table: &str, section: &str) {
    let message = format!("ไม่พบตาราง {table} ในฐานข้อมูล HOSxP ของสถานบริการนี้ — ระบบข้ามข้อมูล{section}");
    tracing::warn!(table, "HOSxP table missing, section skipped");
    warnings.push(message);
}

/// A typed SQL parameter. MySQL prepared statements reject `VARCHAR`
/// parameters for `LIMIT` and for DATE column comparisons, so the repository
/// binds each value with its native type.
enum P {
    /// String (VARCHAR).
    Str(String),
    /// Date (DATE).
    Date(NaiveDate),
    /// Integer (BIGINT).
    Int(i64),
}

/// Raw row shape for `patient` queries.
#[derive(sqlx::FromRow)]
struct PatientRow {
    hn: String,
    cid: Option<String>,
    pname: Option<String>,
    fname: String,
    lname: String,
    birthday: Option<NaiveDate>,
}

/// Raw row shape for dispensing queries (OPD and IPD share column aliases).
#[derive(sqlx::FromRow)]
struct DispenseRow {
    visit_id: Option<String>,
    icode: String,
    /// `qty` is DECIMAL in HOSxP; sqlx cannot decode DECIMAL as `f64`, so
    /// the SQL casts it to CHAR and we parse here.
    qty: String,
    drug_name: String,
    strength: Option<String>,
    units: Option<String>,
    disp_date: NaiveDate,
}

/// Raw row shape for `drugusage`/`sp_use` sig queries. `vn` (OPD) and `an`
/// (IPD) are mutually exclusive on a given row; the visit-id key is the
/// populated one.
#[derive(sqlx::FromRow)]
struct SigRow {
    vn: Option<String>,
    an: Option<String>,
    icode: String,
    d_name1: Option<String>,
    d_name2: Option<String>,
    d_name3: Option<String>,
    s_name1: Option<String>,
    s_name2: Option<String>,
    s_name3: Option<String>,
}

/// Raw row shape for `opdscreen` CC/PE queries.
#[derive(sqlx::FromRow)]
struct OpdScreenRow {
    vn: String,
    vstdate: NaiveDate,
    cc: Option<String>,
    pe: Option<String>,
}

/// Raw row shape for `opd_allergy` queries.
#[derive(sqlx::FromRow)]
struct AllergyRow {
    agent: String,
    symptom: Option<String>,
    reporter: Option<String>,
    report_date: Option<NaiveDate>,
    note: Option<String>,
}

/// Raw row shape for `oapp` next-appointment queries.
#[derive(sqlx::FromRow)]
struct AppointmentRow {
    vn: String,
    nextdate: NaiveDate,
}

/// Raw row shape for OPD visit queries.
#[derive(sqlx::FromRow)]
struct OpdVisitRow {
    vn: String,
    vstdate: NaiveDate,
    main_dep: Option<String>,
}

/// Raw row shape for IPD admission queries.
#[derive(sqlx::FromRow)]
struct IpdVisitRow {
    an: String,
    regdate: NaiveDate,
    ward: Option<String>,
}

/// A drug master row (`drugitems`) — non-PHI drug metadata used by the
/// current-medication settings.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DrugItem {
    /// Drug master code.
    pub icode: String,
    /// Drug display name.
    pub name: String,
    /// Strength text, e.g. "500 mg".
    pub strength: Option<String>,
    /// Units text, e.g. "เม็ด" / "tablet".
    pub units: Option<String>,
}

/// Map a `patient` row to a domain summary, normalizing the birthday era.
fn map_patient(r: &PatientRow) -> PatientSummary {
    PatientSummary {
        hn: r.hn.clone(),
        cid: r.cid.clone(),
        title: r.pname.clone(),
        first_name: r.fname.clone(),
        last_name: r.lname.clone(),
        birthday: r.birthday.map(normalize_date),
    }
}

/// Map a dispensing row to a domain event, normalizing the date era.
///
/// The SQL cutoff is a Christian-era date; on Buddhist-era sites it matches
/// every stored year, so the exact history window is enforced here.
fn map_dispense(
    r: DispenseRow,
    hn: &str,
    source: EncounterSource,
    cutoff: NaiveDate,
) -> Option<Dispense> {
    let date = normalize_date(r.disp_date);
    let visit_id = r.visit_id?;
    (date >= cutoff).then(|| Dispense {
        hn: hn.to_string(),
        visit_id,
        source,
        icode: r.icode,
        drug_name: r.drug_name,
        strength: r.strength,
        units: r.units,
        qty: parse_qty(&r.qty),
        date,
        sig: None,
        appointment: None,
    })
}

/// Parse the CHAR-cast DECIMAL quantity into an `f64`.
fn parse_qty(raw: &str) -> f64 {
    raw.trim().parse().unwrap_or(0.0)
}

/// Normalize a free-text allergy agent: collapse whitespace.
fn clean_agent(agent: &str) -> String {
    agent.split_whitespace().collect::<Vec<_>>().join(" ")
}
