use std::{env, sync::Arc};

use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio_postgres::Row;
use tokio_postgres::{Client, Error, NoTls};

use crate::badges::{Badge, BadgeId, NewBadge};
use crate::greptime::{
    ChargingMeasurementRecord, EnergyMeterMeasurementRecord, EnergyMeterMeasurementRow,
    GreptimeWriter, OcppMessageRecord,
};
use crate::site_config::{SiteConfigSnapshot, SiteConfigTopology, SiteEnergyMeter};
use crate::users::{NewUser, User, UserId};

#[derive(Clone)]
pub struct Database {
    client: Arc<Client>,
    greptime: Option<Arc<GreptimeWriter>>,
}

#[derive(Debug, Clone)]
pub struct StationLocation {
    pub station_name: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_label: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StationSummary {
    pub station_id: String,
    pub station_name: Option<String>,
    pub blocked: bool,
    pub ocpp_version: String,
    pub peer_addr: String,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub last_boot_at: Option<DateTime<Utc>>,
    pub boot_count: i32,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub location_label: Option<String>,
    pub address: Option<String>,
    pub notes: Option<String>,
    pub location_updated_at: Option<DateTime<Utc>>,
    pub current_status: Option<String>,
    pub current_error_code: Option<String>,
    pub current_connector_id: Option<i32>,
    pub current_evse_id: Option<i32>,
    pub current_status_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectorSummary {
    pub station_id: String,
    pub connector_id: i32,
    pub evse_id: Option<i32>,
    pub active: bool,
    pub auto_remote_start_badge_code: Option<String>,
    pub current_status: Option<String>,
    pub current_error_code: Option<String>,
    pub current_status_at: Option<DateTime<Utc>>,
    pub active_transaction_id: Option<i32>,
    pub active_transaction_ref: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChargingTransaction {
    pub id: i64,
    pub station_id: String,
    pub ocpp_version: String,
    pub ocpp_transaction_id: Option<i32>,
    pub ocpp_transaction_ref: Option<String>,
    pub connector_id: Option<i32>,
    pub evse_id: Option<i32>,
    pub user_id: Option<i64>,
    pub badge_id: Option<i64>,
    pub badge_code: Option<String>,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub meter_start_wh: Option<i64>,
    pub meter_stop_wh: Option<i64>,
    pub last_meter_wh: Option<i64>,
    pub energy_wh: Option<i64>,
    pub stop_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnergyMeter {
    pub id: String,
    pub name: String,
    pub catalog_key: Option<String>,
    pub host: Option<String>,
    pub port: Option<i32>,
    pub unit_id: Option<i32>,
    pub poll_interval_ms: Option<i64>,
    pub meter_label: Option<String>,
    pub max_current_a: Option<f64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnergyMeterRuntimeStatus {
    pub meter_id: String,
    pub is_online: bool,
    pub last_attempt_at: DateTime<Utc>,
    pub last_ok_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
    pub last_poll_duration_ms: Option<i64>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnergyMeterLatestReading {
    pub meter_id: String,
    pub metric_key: String,
    pub unit: Option<String>,
    pub value_text: String,
    pub value_num: Option<f64>,
    pub measured_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnergyMeterStatusView {
    pub meter_id: String,
    pub runtime: Option<EnergyMeterRuntimeStatus>,
    pub latest_readings: Vec<EnergyMeterLatestReading>,
}

#[derive(Debug, Clone)]
pub struct EnergyMeterRuntimeStatusUpdate {
    pub meter_id: String,
    pub is_online: bool,
    pub last_attempt_at: DateTime<Utc>,
    pub last_ok_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub consecutive_failures: i32,
    pub last_poll_duration_ms: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct EnergyMeterLatestReadingUpsert {
    pub meter_id: String,
    pub metric_key: String,
    pub unit: Option<String>,
    pub value_text: String,
    pub value_num: Option<f64>,
    pub measured_at: DateTime<Utc>,
}

impl Database {
    pub async fn connect_from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let url = env::var("DATABASE_URL").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "missing DATABASE_URL environment variable",
            )
        })?;

        let (client, connection) = tokio_postgres::connect(&url, NoTls).await?;

        tokio::spawn(async move {
            if let Err(err) = connection.await {
                eprintln!("errore connessione postgres: {err}");
            }
        });

        let db = Self {
            client: Arc::new(client),
            greptime: GreptimeWriter::from_env()?.map(Arc::new),
        };
        if db.greptime.is_some() {
            println!("greptime writer pronto per messaggi OCPP");
        } else {
            println!("greptime writer disattivato: GREPTIME_URL non impostata");
        }
        db.init_schema().await?;
        println!("database postgres connesso e schema pronto");
        Ok(db)
    }

    async fn init_schema(&self) -> Result<(), Error> {
        self.client
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS charging_stations (
                    station_id TEXT PRIMARY KEY,
                    station_name TEXT,
                    blocked BOOLEAN NOT NULL DEFAULT FALSE,
                    ocpp_version TEXT NOT NULL,
                    peer_addr TEXT NOT NULL,
                    first_seen_at TIMESTAMPTZ NOT NULL,
                    last_seen_at TIMESTAMPTZ NOT NULL,
                    last_boot_at TIMESTAMPTZ,
                    boot_count INTEGER NOT NULL DEFAULT 0,
                    latitude DOUBLE PRECISION,
                    longitude DOUBLE PRECISION,
                    location_label TEXT,
                    address TEXT,
                    notes TEXT,
                    location_updated_at TIMESTAMPTZ,
                    updated_at TIMESTAMPTZ NOT NULL
                );

                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS station_name TEXT;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS blocked BOOLEAN NOT NULL DEFAULT FALSE;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS latitude DOUBLE PRECISION;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS longitude DOUBLE PRECISION;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS location_label TEXT;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS address TEXT;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS notes TEXT;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS location_updated_at TIMESTAMPTZ;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS current_status TEXT;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS current_error_code TEXT;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS current_connector_id INTEGER;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS current_evse_id INTEGER;
                ALTER TABLE charging_stations
                    ADD COLUMN IF NOT EXISTS current_status_at TIMESTAMPTZ;

                CREATE TABLE IF NOT EXISTS users (
                    id BIGSERIAL PRIMARY KEY,
                    display_name TEXT NOT NULL,
                    email TEXT UNIQUE,
                    active BOOLEAN NOT NULL DEFAULT TRUE,
                    created_at TIMESTAMPTZ NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL
                );

                CREATE TABLE IF NOT EXISTS badges (
                    id BIGSERIAL PRIMARY KEY,
                    user_id BIGINT REFERENCES users(id) ON DELETE CASCADE,
                    badge_code TEXT NOT NULL UNIQUE,
                    label TEXT,
                    active BOOLEAN NOT NULL DEFAULT TRUE,
                    created_at TIMESTAMPTZ NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL
                );

                CREATE TABLE IF NOT EXISTS energy_meters (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    catalog_key TEXT,
                    host TEXT,
                    port INTEGER,
                    unit_id INTEGER,
                    poll_interval_ms BIGINT,
                    meter_label TEXT,
                    max_current_a DOUBLE PRECISION,
                    notes TEXT,
                    created_at TIMESTAMPTZ NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL
                );

                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS catalog_key TEXT;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS host TEXT;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS port INTEGER;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS unit_id INTEGER;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS poll_interval_ms BIGINT;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS meter_label TEXT;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS max_current_a DOUBLE PRECISION;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS notes TEXT;
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                ALTER TABLE energy_meters
                    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

                CREATE TABLE IF NOT EXISTS energy_meter_runtime_state (
                    meter_id TEXT PRIMARY KEY REFERENCES energy_meters(id) ON DELETE CASCADE,
                    is_online BOOLEAN NOT NULL DEFAULT FALSE,
                    last_attempt_at TIMESTAMPTZ NOT NULL,
                    last_ok_at TIMESTAMPTZ,
                    last_error TEXT,
                    consecutive_failures INTEGER NOT NULL DEFAULT 0,
                    last_poll_duration_ms BIGINT,
                    updated_at TIMESTAMPTZ NOT NULL
                );

                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS is_online BOOLEAN NOT NULL DEFAULT FALSE;
                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS last_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS last_ok_at TIMESTAMPTZ;
                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS last_error TEXT;
                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS consecutive_failures INTEGER NOT NULL DEFAULT 0;
                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS last_poll_duration_ms BIGINT;
                ALTER TABLE energy_meter_runtime_state
                    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

                CREATE TABLE IF NOT EXISTS energy_meter_latest_readings (
                    meter_id TEXT NOT NULL REFERENCES energy_meters(id) ON DELETE CASCADE,
                    metric_key TEXT NOT NULL,
                    unit TEXT,
                    value_text TEXT NOT NULL,
                    value_num DOUBLE PRECISION,
                    measured_at TIMESTAMPTZ NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL,
                    PRIMARY KEY (meter_id, metric_key)
                );

                ALTER TABLE energy_meter_latest_readings
                    ADD COLUMN IF NOT EXISTS unit TEXT;
                ALTER TABLE energy_meter_latest_readings
                    ADD COLUMN IF NOT EXISTS value_text TEXT NOT NULL DEFAULT '';
                ALTER TABLE energy_meter_latest_readings
                    ADD COLUMN IF NOT EXISTS value_num DOUBLE PRECISION;
                ALTER TABLE energy_meter_latest_readings
                    ADD COLUMN IF NOT EXISTS measured_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                ALTER TABLE energy_meter_latest_readings
                    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

                ALTER TABLE badges
                    ALTER COLUMN user_id DROP NOT NULL;

                CREATE TABLE IF NOT EXISTS charging_connectors (
                    station_id TEXT NOT NULL REFERENCES charging_stations(station_id) ON DELETE CASCADE,
                    connector_id INTEGER NOT NULL,
                    evse_id INTEGER,
                    active BOOLEAN NOT NULL DEFAULT TRUE,
                    auto_remote_start_badge_code TEXT,
                    current_status TEXT,
                    current_error_code TEXT,
                    current_status_at TIMESTAMPTZ,
                    active_transaction_id INTEGER,
                    active_transaction_ref TEXT,
                    updated_at TIMESTAMPTZ NOT NULL,
                    PRIMARY KEY (station_id, connector_id)
                );

                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS evse_id INTEGER;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS active BOOLEAN NOT NULL DEFAULT TRUE;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS auto_remote_start_badge_code TEXT;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS current_status TEXT;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS current_error_code TEXT;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS current_status_at TIMESTAMPTZ;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS active_transaction_id INTEGER;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS active_transaction_ref TEXT;
                ALTER TABLE charging_connectors
                    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

                CREATE TABLE IF NOT EXISTS charging_transactions (
                    id BIGSERIAL PRIMARY KEY,
                    station_id TEXT NOT NULL REFERENCES charging_stations(station_id) ON DELETE CASCADE,
                    ocpp_version TEXT NOT NULL,
                    ocpp_transaction_id INTEGER,
                    ocpp_transaction_ref TEXT,
                    connector_id INTEGER,
                    evse_id INTEGER,
                    user_id BIGINT REFERENCES users(id) ON DELETE SET NULL,
                    badge_id BIGINT REFERENCES badges(id) ON DELETE SET NULL,
                    badge_code TEXT,
                    status TEXT NOT NULL,
                    started_at TIMESTAMPTZ NOT NULL,
                    ended_at TIMESTAMPTZ,
                    meter_start_wh BIGINT,
                    meter_stop_wh BIGINT,
                    last_meter_wh BIGINT,
                    energy_wh BIGINT,
                    stop_reason TEXT,
                    created_at TIMESTAMPTZ NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL
                );

                CREATE SEQUENCE IF NOT EXISTS ocpp_transaction_id_seq;

                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS ocpp_version TEXT NOT NULL DEFAULT 'ocpp1.6';
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS ocpp_transaction_id INTEGER;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS ocpp_transaction_ref TEXT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS connector_id INTEGER;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS evse_id INTEGER;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS user_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS badge_id BIGINT REFERENCES badges(id) ON DELETE SET NULL;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS badge_code TEXT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS status TEXT NOT NULL DEFAULT 'in_progress';
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS ended_at TIMESTAMPTZ;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS meter_start_wh BIGINT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS meter_stop_wh BIGINT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS last_meter_wh BIGINT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS energy_wh BIGINT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS stop_reason TEXT;
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                ALTER TABLE charging_transactions
                    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

                CREATE UNIQUE INDEX IF NOT EXISTS charging_transactions_station_tx_id_idx
                    ON charging_transactions (station_id, ocpp_transaction_id)
                    WHERE ocpp_transaction_id IS NOT NULL;

                CREATE UNIQUE INDEX IF NOT EXISTS charging_transactions_station_tx_ref_idx
                    ON charging_transactions (station_id, ocpp_transaction_ref)
                    WHERE ocpp_transaction_ref IS NOT NULL;

                CREATE TABLE IF NOT EXISTS site_settings (
                    site_key TEXT PRIMARY KEY,
                    site_name TEXT,
                    timezone TEXT NOT NULL DEFAULT 'Europe/Zurich',
                    operator_name TEXT,
                    notes TEXT,
                    topology JSONB NOT NULL DEFAULT '{"energy_meters":[],"management_groups":[]}'::JSONB,
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                ALTER TABLE site_settings
                    ADD COLUMN IF NOT EXISTS site_name TEXT;
                ALTER TABLE site_settings
                    ADD COLUMN IF NOT EXISTS timezone TEXT NOT NULL DEFAULT 'Europe/Zurich';
                ALTER TABLE site_settings
                    ADD COLUMN IF NOT EXISTS operator_name TEXT;
                ALTER TABLE site_settings
                    ADD COLUMN IF NOT EXISTS notes TEXT;
                ALTER TABLE site_settings
                    ADD COLUMN IF NOT EXISTS topology JSONB NOT NULL DEFAULT '{"energy_meters":[],"management_groups":[]}'::JSONB;
                ALTER TABLE site_settings
                    ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();
                "#,
            )
            .await
    }

    pub async fn station_exists(&self, station_id: &str) -> Result<bool, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT 1
                FROM charging_stations
                WHERE station_id = $1
                "#,
                &[&station_id],
            )
            .await?;

        Ok(row.is_some())
    }

    pub async fn next_ocpp_transaction_id(&self) -> Result<i32, Error> {
        let row = self
            .client
            .query_one("SELECT nextval('ocpp_transaction_id_seq')::INT4 AS id", &[])
            .await?;

        Ok(row.get("id"))
    }

    pub async fn list_stations(&self) -> Result<Vec<StationSummary>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT
                    station_id,
                    station_name,
                    blocked,
                    ocpp_version,
                    peer_addr,
                    first_seen_at,
                    last_seen_at,
                    last_boot_at,
                    boot_count,
                    latitude,
                    longitude,
                    location_label,
                    address,
                    notes,
                    location_updated_at,
                    current_status,
                    current_error_code,
                    current_connector_id,
                    current_evse_id,
                    current_status_at,
                    updated_at
                FROM charging_stations
                ORDER BY last_seen_at DESC, station_id ASC
                "#,
                &[],
            )
            .await?;

        Ok(rows.iter().map(row_to_station_summary).collect())
    }

    pub async fn get_station(&self, station_id: &str) -> Result<Option<StationSummary>, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT
                    station_id,
                    station_name,
                    blocked,
                    ocpp_version,
                    peer_addr,
                    first_seen_at,
                    last_seen_at,
                    last_boot_at,
                    boot_count,
                    latitude,
                    longitude,
                    location_label,
                    address,
                    notes,
                    location_updated_at,
                    current_status,
                    current_error_code,
                    current_connector_id,
                    current_evse_id,
                    current_status_at,
                    updated_at
                FROM charging_stations
                WHERE station_id = $1
                "#,
                &[&station_id],
            )
            .await?;

        Ok(row.map(|row| row_to_station_summary(&row)))
    }

    pub async fn touch_station(
        &self,
        station_id: &str,
        ocpp_version: &str,
        peer_addr: &str,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO charging_stations (
                    station_id,
                    ocpp_version,
                    peer_addr,
                    first_seen_at,
                    last_seen_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $4, $4)
                ON CONFLICT (station_id) DO UPDATE SET
                    ocpp_version = EXCLUDED.ocpp_version,
                    peer_addr = EXCLUDED.peer_addr,
                    last_seen_at = EXCLUDED.last_seen_at,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[&station_id, &ocpp_version, &peer_addr, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn record_boot_notification(&self, station_id: &str) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_stations
                SET last_boot_at = $2,
                    boot_count = boot_count + 1,
                    updated_at = $2
                WHERE station_id = $1
                "#,
                &[&station_id, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn update_station_location(
        &self,
        station_id: &str,
        location: StationLocation,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_stations
                SET latitude = $2,
                    longitude = $3,
                    station_name = $4,
                    location_label = $5,
                    address = $6,
                    notes = $7,
                    location_updated_at = $8,
                    updated_at = $8
                WHERE station_id = $1
                "#,
                &[
                    &station_id,
                    &location.latitude,
                    &location.longitude,
                    &location.station_name,
                    &location.location_label,
                    &location.address,
                    &location.notes,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn set_station_blocked(&self, station_id: &str, blocked: bool) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_stations
                SET blocked = $2,
                    updated_at = $3
                WHERE station_id = $1
                "#,
                &[&station_id, &blocked, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn update_station_status(
        &self,
        station_id: &str,
        current_status: Option<String>,
        current_error_code: Option<String>,
        current_connector_id: Option<i32>,
        current_evse_id: Option<i32>,
        current_status_at: Option<DateTime<Utc>>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_stations
                SET current_status = $2,
                    current_error_code = $3,
                    current_connector_id = $4,
                    current_evse_id = $5,
                    current_status_at = $6,
                    updated_at = $7
                WHERE station_id = $1
                "#,
                &[
                    &station_id,
                    &current_status,
                    &current_error_code,
                    &current_connector_id,
                    &current_evse_id,
                    &current_status_at,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn update_station_connector(
        &self,
        station_id: &str,
        current_connector_id: Option<i32>,
        current_evse_id: Option<i32>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_stations
                SET current_connector_id = $2,
                    current_evse_id = $3,
                    updated_at = $4
                WHERE station_id = $1
                "#,
                &[&station_id, &current_connector_id, &current_evse_id, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn list_station_connectors(
        &self,
        station_id: &str,
    ) -> Result<Vec<ConnectorSummary>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT
                    station_id,
                    connector_id,
                    evse_id,
                    active,
                    auto_remote_start_badge_code,
                    current_status,
                    current_error_code,
                    current_status_at,
                    active_transaction_id,
                    active_transaction_ref,
                    updated_at
                FROM charging_connectors
                WHERE station_id = $1
                ORDER BY connector_id ASC
                "#,
                &[&station_id],
            )
            .await?;

        Ok(rows.iter().map(row_to_connector_summary).collect())
    }

    pub async fn list_connectors(&self) -> Result<Vec<ConnectorSummary>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT
                    station_id,
                    connector_id,
                    evse_id,
                    active,
                    auto_remote_start_badge_code,
                    current_status,
                    current_error_code,
                    current_status_at,
                    active_transaction_id,
                    active_transaction_ref,
                    updated_at
                FROM charging_connectors
                ORDER BY station_id ASC, connector_id ASC
                "#,
                &[],
            )
            .await?;

        Ok(rows.iter().map(row_to_connector_summary).collect())
    }

    pub async fn connector_for_station(
        &self,
        station_id: &str,
        connector_id: i32,
    ) -> Result<Option<ConnectorSummary>, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT
                    station_id,
                    connector_id,
                    evse_id,
                    active,
                    auto_remote_start_badge_code,
                    current_status,
                    current_error_code,
                    current_status_at,
                    active_transaction_id,
                    active_transaction_ref,
                    updated_at
                FROM charging_connectors
                WHERE station_id = $1
                  AND connector_id = $2
                "#,
                &[&station_id, &connector_id],
            )
            .await?;

        Ok(row.map(|row| row_to_connector_summary(&row)))
    }

    pub async fn set_connector_active(
        &self,
        station_id: &str,
        connector_id: i32,
        active: bool,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO charging_connectors (
                    station_id,
                    connector_id,
                    active,
                    updated_at
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (station_id, connector_id) DO UPDATE
                SET active = EXCLUDED.active,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[&station_id, &connector_id, &active, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn set_connector_auto_remote_start_badge_code(
        &self,
        station_id: &str,
        connector_id: i32,
        auto_remote_start_badge_code: Option<&str>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO charging_connectors (
                    station_id,
                    connector_id,
                    auto_remote_start_badge_code,
                    updated_at
                )
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (station_id, connector_id) DO UPDATE
                SET auto_remote_start_badge_code = EXCLUDED.auto_remote_start_badge_code,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[&station_id, &connector_id, &auto_remote_start_badge_code, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn upsert_connector_status(
        &self,
        station_id: &str,
        connector_id: i32,
        evse_id: Option<i32>,
        current_status: Option<String>,
        current_error_code: Option<String>,
        current_status_at: Option<DateTime<Utc>>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO charging_connectors (
                    station_id,
                    connector_id,
                    evse_id,
                    current_status,
                    current_error_code,
                    current_status_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (station_id, connector_id) DO UPDATE
                SET evse_id = EXCLUDED.evse_id,
                    current_status = EXCLUDED.current_status,
                    current_error_code = EXCLUDED.current_error_code,
                    current_status_at = EXCLUDED.current_status_at,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &station_id,
                    &connector_id,
                    &evse_id,
                    &current_status,
                    &current_error_code,
                    &current_status_at,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn set_connector_transaction(
        &self,
        station_id: &str,
        connector_id: i32,
        evse_id: Option<i32>,
        active_transaction_id: Option<i32>,
        active_transaction_ref: Option<String>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO charging_connectors (
                    station_id,
                    connector_id,
                    evse_id,
                    active_transaction_id,
                    active_transaction_ref,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (station_id, connector_id) DO UPDATE
                SET evse_id = EXCLUDED.evse_id,
                    active_transaction_id = EXCLUDED.active_transaction_id,
                    active_transaction_ref = EXCLUDED.active_transaction_ref,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &station_id,
                    &connector_id,
                    &evse_id,
                    &active_transaction_id,
                    &active_transaction_ref,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn clear_connector_transaction(
        &self,
        station_id: &str,
        connector_id: i32,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_connectors
                SET active_transaction_id = NULL,
                    active_transaction_ref = NULL,
                    updated_at = $3
                WHERE station_id = $1 AND connector_id = $2
                "#,
                &[&station_id, &connector_id, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn clear_connector_transaction_by_ocpp_id(
        &self,
        station_id: &str,
        ocpp_transaction_id: i32,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_connectors
                SET active_transaction_id = NULL,
                    updated_at = $3
                WHERE station_id = $1
                  AND active_transaction_id = $2
                "#,
                &[&station_id, &ocpp_transaction_id, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn clear_connector_transaction_by_ocpp_ref(
        &self,
        station_id: &str,
        ocpp_transaction_ref: &str,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_connectors
                SET active_transaction_ref = NULL,
                    updated_at = $3
                WHERE station_id = $1
                  AND active_transaction_ref = $2
                "#,
                &[&station_id, &ocpp_transaction_ref, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn active_connector_for_station(
        &self,
        station_id: &str,
    ) -> Result<Option<ConnectorSummary>, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT
                    station_id,
                    connector_id,
                    evse_id,
                    active,
                    auto_remote_start_badge_code,
                    current_status,
                    current_error_code,
                    current_status_at,
                    active_transaction_id,
                    active_transaction_ref,
                    updated_at
                FROM charging_connectors
                WHERE station_id = $1
                  AND (active_transaction_id IS NOT NULL OR active_transaction_ref IS NOT NULL)
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                &[&station_id],
            )
            .await?;

        Ok(row.map(|row| ConnectorSummary {
            station_id: row.get("station_id"),
            connector_id: row.get("connector_id"),
            evse_id: row.get("evse_id"),
            active: row.get("active"),
            auto_remote_start_badge_code: row.get("auto_remote_start_badge_code"),
            current_status: row.get("current_status"),
            current_error_code: row.get("current_error_code"),
            current_status_at: row.get("current_status_at"),
            active_transaction_id: row.get("active_transaction_id"),
            active_transaction_ref: row.get("active_transaction_ref"),
            updated_at: row.get("updated_at"),
        }))
    }

    pub async fn record_ocpp_message(&self, record: OcppMessageRecord<'_>) {
        let Some(writer) = self.greptime.as_ref() else {
            return;
        };

        if let Err(err) = writer.write_ocpp_message(&record).await {
            eprintln!(
                "greptime write fallito per {} ({}) action={:?} unique_id={:?}: {}",
                record.station_id, record.direction, record.action, record.unique_id, err
            );
        }
    }

    pub async fn record_charging_measurements(&self, records: &[ChargingMeasurementRecord]) {
        let Some(writer) = self.greptime.as_ref() else {
            return;
        };

        if let Err(err) = writer.write_charging_measurements(records).await {
            let station_id = records
                .first()
                .map(|record| record.station_id.as_str())
                .unwrap_or("-");
            eprintln!(
                "greptime write fallito per misure ricarica {} ({} righe): {}",
                station_id,
                records.len(),
                err
            );
        }
    }

    pub async fn record_energy_meter_measurements(&self, records: &[EnergyMeterMeasurementRecord]) {
        let Some(writer) = self.greptime.as_ref() else {
            return;
        };

        if let Err(err) = writer.write_energy_meter_measurements(records).await {
            let meter_id = records
                .first()
                .map(|record| record.meter_id.as_str())
                .unwrap_or("-");
            eprintln!(
                "greptime write fallito per misure meter {} ({} righe): {}",
                meter_id,
                records.len(),
                err
            );
        }
    }

    pub async fn list_ocpp_messages(
        &self,
        limit: i64,
        station_ids: Option<&[String]>,
    ) -> Result<Vec<crate::greptime::OcppEventRow>, String> {
        let Some(writer) = self.greptime.as_ref() else {
            return Err("greptime writer disattivato: GREPTIME_URL non impostata".to_string());
        };

        writer.query_ocpp_messages(limit, station_ids).await
    }

    pub async fn list_energy_meter_measurements(
        &self,
        meter_id: &str,
        limit: i64,
    ) -> Result<Vec<EnergyMeterMeasurementRow>, String> {
        let Some(writer) = self.greptime.as_ref() else {
            return Ok(Vec::new());
        };

        writer.query_energy_meter_measurements(meter_id, limit).await
    }

    pub async fn station_ids_for_station_name(
        &self,
        station_name: &str,
    ) -> Result<Vec<String>, Error> {
        if station_name == "__unnamed__" {
            let rows = self
                .client
                .query(
                    r#"
                    SELECT station_id
                    FROM charging_stations
                    WHERE station_name IS NULL
                    ORDER BY station_id
                    "#,
                    &[],
                )
                .await?;

            return Ok(rows.iter().map(|row| row.get("station_id")).collect());
        }

        let rows = self
            .client
            .query(
                r#"
                SELECT station_id
                FROM charging_stations
                WHERE station_name = $1
                ORDER BY station_id
                "#,
                &[&station_name],
            )
            .await?;

        Ok(rows.iter().map(|row| row.get("station_id")).collect())
    }

    pub async fn get_station_location(
        &self,
        station_id: &str,
    ) -> Result<Option<StationLocation>, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT station_name, latitude, longitude, location_label, address, notes
                FROM charging_stations
                WHERE station_id = $1
                "#,
                &[&station_id],
            )
            .await?;

        Ok(row.map(|row| StationLocation {
            station_name: row.get("station_name"),
            latitude: row.get("latitude"),
            longitude: row.get("longitude"),
            location_label: row.get("location_label"),
            address: row.get("address"),
            notes: row.get("notes"),
        }))
    }

    pub async fn load_site_config(
        &self,
    ) -> Result<SiteConfigSnapshot, Box<dyn std::error::Error + Send + Sync>> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT
                    site_name,
                    timezone,
                    operator_name,
                    notes,
                    topology::TEXT AS topology,
                    updated_at
                FROM site_settings
                WHERE site_key = 'default'
                "#,
                &[],
            )
            .await?;

        let Some(row) = row else {
            return Ok(SiteConfigSnapshot::default());
        };

        let topology_text: String = row.get("topology");
        let topology: SiteConfigTopology = serde_json::from_str(&topology_text)?;
        let energy_meters = self.list_energy_meters().await?;

        Ok(SiteConfigSnapshot {
            site_name: row.get("site_name"),
            timezone: row.get("timezone"),
            operator_name: row.get("operator_name"),
            notes: row.get("notes"),
            energy_meters: energy_meters.iter().map(site_energy_meter_from_db).collect(),
            management_groups: topology.management_groups,
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn save_site_config(
        &self,
        mut snapshot: SiteConfigSnapshot,
    ) -> Result<SiteConfigSnapshot, Box<dyn std::error::Error + Send + Sync>> {
        let energy_meter_ids = self
            .list_energy_meters()
            .await?
            .into_iter()
            .map(|meter| meter.id)
            .collect::<Vec<_>>();
        normalize_and_validate_site_config(
            &mut snapshot,
            &self.list_stations().await?,
            &energy_meter_ids,
        )?;

        let topology = SiteConfigTopology {
            energy_meters: Vec::new(),
            management_groups: snapshot.management_groups.clone(),
        };
        let topology_json = serde_json::to_string(&topology)?;
        let now = Utc::now();

        self.client
            .execute(
                r#"
                INSERT INTO site_settings (
                    site_key,
                    site_name,
                    timezone,
                    operator_name,
                    notes,
                    topology,
                    updated_at
                )
                VALUES ('default', $1, $2, $3, $4, $5::TEXT::JSONB, $6)
                ON CONFLICT (site_key) DO UPDATE SET
                    site_name = EXCLUDED.site_name,
                    timezone = EXCLUDED.timezone,
                    operator_name = EXCLUDED.operator_name,
                    notes = EXCLUDED.notes,
                    topology = EXCLUDED.topology,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &snapshot.site_name,
                    &snapshot.timezone,
                    &snapshot.operator_name,
                    &snapshot.notes,
                    &topology_json,
                    &now,
                ],
            )
            .await?;

        snapshot.updated_at = Some(now);
        snapshot.energy_meters = self
            .list_energy_meters()
            .await?
            .iter()
            .map(site_energy_meter_from_db)
            .collect();
        Ok(snapshot)
    }

    pub async fn create_energy_meter(
        &self,
        mut meter: SiteEnergyMeter,
    ) -> Result<EnergyMeter, Box<dyn std::error::Error + Send + Sync>> {
        normalize_energy_meter(&mut meter)?;
        let now = Utc::now();
        let row = self
            .client
            .query_one(
                r#"
                INSERT INTO energy_meters (
                    id,
                    name,
                    catalog_key,
                    host,
                    port,
                    unit_id,
                    poll_interval_ms,
                    meter_label,
                    max_current_a,
                    notes,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
                RETURNING id, name, catalog_key, host, port, unit_id, poll_interval_ms, meter_label, max_current_a, notes, created_at, updated_at
                "#,
                &[
                    &meter.id,
                    &meter.name,
                    &meter.catalog_key,
                    &meter.host,
                    &meter.port.map(|value| value as i32),
                    &meter.unit_id.map(|value| value as i32),
                    &meter.poll_interval_ms.map(|value| value as i64),
                    &meter.meter_label,
                    &meter.max_current_a,
                    &meter.notes,
                    &now,
                ],
            )
            .await?;
        Ok(row_to_energy_meter(&row))
    }

    pub async fn list_energy_meters(&self) -> Result<Vec<EnergyMeter>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT id, name, catalog_key, host, port, unit_id, poll_interval_ms, meter_label, max_current_a, notes, created_at, updated_at
                FROM energy_meters
                ORDER BY id
                "#,
                &[],
            )
            .await?;
        Ok(rows.iter().map(row_to_energy_meter).collect())
    }

    pub async fn list_energy_meter_statuses(&self) -> Result<Vec<EnergyMeterStatusView>, Error> {
        let runtime_rows = self
            .client
            .query(
                r#"
                SELECT meter_id, is_online, last_attempt_at, last_ok_at, last_error, consecutive_failures, last_poll_duration_ms, updated_at
                FROM energy_meter_runtime_state
                ORDER BY meter_id
                "#,
                &[],
            )
            .await?;
        let latest_rows = self
            .client
            .query(
                r#"
                SELECT meter_id, metric_key, unit, value_text, value_num, measured_at, updated_at
                FROM energy_meter_latest_readings
                ORDER BY meter_id, metric_key
                "#,
                &[],
            )
            .await?;

        use std::collections::HashMap;

        let mut by_meter: HashMap<String, EnergyMeterStatusView> = HashMap::new();
        for row in runtime_rows {
            let runtime = row_to_energy_meter_runtime_status(&row);
            by_meter.insert(
                runtime.meter_id.clone(),
                EnergyMeterStatusView {
                    meter_id: runtime.meter_id.clone(),
                    runtime: Some(runtime),
                    latest_readings: Vec::new(),
                },
            );
        }

        for row in latest_rows {
            let reading = row_to_energy_meter_latest_reading(&row);
            by_meter
                .entry(reading.meter_id.clone())
                .or_insert_with(|| EnergyMeterStatusView {
                    meter_id: reading.meter_id.clone(),
                    runtime: None,
                    latest_readings: Vec::new(),
                })
                .latest_readings
                .push(reading);
        }

        let mut items: Vec<_> = by_meter.into_values().collect();
        items.sort_by(|left, right| left.meter_id.cmp(&right.meter_id));
        Ok(items)
    }

    pub async fn update_energy_meter(
        &self,
        meter_id: &str,
        mut meter: SiteEnergyMeter,
    ) -> Result<Option<EnergyMeter>, Box<dyn std::error::Error + Send + Sync>> {
        normalize_energy_meter(&mut meter)?;
        let now = Utc::now();
        let row = self
            .client
            .query_opt(
                r#"
                UPDATE energy_meters
                SET id = $2,
                    name = $3,
                    catalog_key = $4,
                    host = $5,
                    port = $6,
                    unit_id = $7,
                    poll_interval_ms = $8,
                    meter_label = $9,
                    max_current_a = $10,
                    notes = $11,
                    updated_at = $12
                WHERE id = $1
                RETURNING id, name, catalog_key, host, port, unit_id, poll_interval_ms, meter_label, max_current_a, notes, created_at, updated_at
                "#,
                &[
                    &meter_id,
                    &meter.id,
                    &meter.name,
                    &meter.catalog_key,
                    &meter.host,
                    &meter.port.map(|value| value as i32),
                    &meter.unit_id.map(|value| value as i32),
                    &meter.poll_interval_ms.map(|value| value as i64),
                    &meter.meter_label,
                    &meter.max_current_a,
                    &meter.notes,
                    &now,
                ],
            )
            .await?;
        if meter_id != meter.id {
            self.sync_site_config_energy_meter_reference(meter_id, Some(&meter.id))
                .await?;
        }
        Ok(row.map(|row| row_to_energy_meter(&row)))
    }

    pub async fn delete_energy_meter(
        &self,
        meter_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.client
            .execute("DELETE FROM energy_meters WHERE id = $1", &[&meter_id])
            .await?;
        self.sync_site_config_energy_meter_reference(meter_id, None)
            .await?;
        Ok(())
    }

    pub async fn upsert_energy_meter_runtime_status(
        &self,
        update: &EnergyMeterRuntimeStatusUpdate,
    ) -> Result<(), Error> {
        let now = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO energy_meter_runtime_state (
                    meter_id,
                    is_online,
                    last_attempt_at,
                    last_ok_at,
                    last_error,
                    consecutive_failures,
                    last_poll_duration_ms,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                ON CONFLICT (meter_id) DO UPDATE
                SET is_online = EXCLUDED.is_online,
                    last_attempt_at = EXCLUDED.last_attempt_at,
                    last_ok_at = CASE
                        WHEN EXCLUDED.last_ok_at IS NOT NULL THEN EXCLUDED.last_ok_at
                        ELSE energy_meter_runtime_state.last_ok_at
                    END,
                    last_error = EXCLUDED.last_error,
                    consecutive_failures = CASE
                        WHEN EXCLUDED.is_online THEN 0
                        ELSE energy_meter_runtime_state.consecutive_failures + EXCLUDED.consecutive_failures
                    END,
                    last_poll_duration_ms = EXCLUDED.last_poll_duration_ms,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &update.meter_id,
                    &update.is_online,
                    &update.last_attempt_at,
                    &update.last_ok_at,
                    &update.last_error,
                    &update.consecutive_failures,
                    &update.last_poll_duration_ms,
                    &now,
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn replace_energy_meter_latest_readings(
        &self,
        meter_id: &str,
        readings: Vec<EnergyMeterLatestReadingUpsert>,
    ) -> Result<(), Error> {
        self.client
            .execute(
            "DELETE FROM energy_meter_latest_readings WHERE meter_id = $1",
            &[&meter_id],
        )
            .await?;

        let now = Utc::now();
        for reading in readings {
            self.client
                .execute(
                r#"
                INSERT INTO energy_meter_latest_readings (
                    meter_id,
                    metric_key,
                    unit,
                    value_text,
                    value_num,
                    measured_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
                &[
                    &reading.meter_id,
                    &reading.metric_key,
                    &reading.unit,
                    &reading.value_text,
                    &reading.value_num,
                    &reading.measured_at,
                    &now,
                ],
            )
                .await?;
        }
        Ok(())
    }

    async fn sync_site_config_energy_meter_reference(
        &self,
        old_meter_id: &str,
        new_meter_id: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT topology::TEXT AS topology
                FROM site_settings
                WHERE site_key = 'default'
                "#,
                &[],
            )
            .await?;

        let Some(row) = row else {
            return Ok(());
        };

        let topology_text: String = row.get("topology");
        let mut topology: SiteConfigTopology = serde_json::from_str(&topology_text)?;

        for group in &mut topology.management_groups {
            let mut next_ids = Vec::with_capacity(group.energy_meter_ids.len());
            for meter_id in &group.energy_meter_ids {
                if meter_id == old_meter_id {
                    if let Some(new_id) = new_meter_id {
                        if !next_ids.iter().any(|value| value == new_id) {
                            next_ids.push(new_id.to_string());
                        }
                    }
                } else if !next_ids.iter().any(|value| value == meter_id) {
                    next_ids.push(meter_id.clone());
                }
            }
            group.energy_meter_ids = next_ids;
        }

        let topology_json = serde_json::to_string(&topology)?;
        self.client
            .execute(
                r#"
                UPDATE site_settings
                SET topology = $1::TEXT::JSONB,
                    updated_at = $2
                WHERE site_key = 'default'
                "#,
                &[&topology_json, &Utc::now()],
            )
            .await?;
        Ok(())
    }

    pub async fn create_user(&self, user: NewUser) -> Result<User, Error> {
        let now: DateTime<Utc> = Utc::now();
        let row = self
            .client
            .query_one(
                r#"
                INSERT INTO users (display_name, email, active, created_at, updated_at)
                VALUES ($1, $2, TRUE, $3, $3)
                RETURNING id, display_name, email, active, created_at, updated_at
                "#,
                &[&user.display_name, &user.email, &now],
            )
            .await?;

        Ok(row_to_user(&row))
    }

    pub async fn list_users(&self) -> Result<Vec<User>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT id, display_name, email, active, created_at, updated_at
                FROM users
                ORDER BY id
                "#,
                &[],
            )
            .await?;

        Ok(rows.iter().map(row_to_user).collect())
    }

    pub async fn get_user(&self, id: UserId) -> Result<Option<User>, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT id, display_name, email, active, created_at, updated_at
                FROM users
                WHERE id = $1
                "#,
                &[&id.0],
            )
            .await?;

        Ok(row.map(|row| row_to_user(&row)))
    }

    pub async fn set_user_active(&self, id: UserId, active: bool) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE users
                SET active = $2,
                    updated_at = $3
                WHERE id = $1
                "#,
                &[&id.0, &active, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn update_user(
        &self,
        id: UserId,
        display_name: String,
        email: Option<String>,
    ) -> Result<Option<User>, Error> {
        let now: DateTime<Utc> = Utc::now();
        let row = self
            .client
            .query_opt(
                r#"
                UPDATE users
                SET display_name = $2,
                    email = $3,
                    updated_at = $4
                WHERE id = $1
                RETURNING id, display_name, email, active, created_at, updated_at
                "#,
                &[&id.0, &display_name, &email, &now],
            )
            .await?;

        Ok(row.map(|row| row_to_user(&row)))
    }

    pub async fn create_badge(&self, badge: NewBadge) -> Result<Badge, Error> {
        let now: DateTime<Utc> = Utc::now();
        let row = self
            .client
            .query_one(
                r#"
                INSERT INTO badges (user_id, badge_code, label, active, created_at, updated_at)
                VALUES ($1, $2, $3, TRUE, $4, $4)
                RETURNING id, user_id, badge_code, label, active, created_at, updated_at
                "#,
                &[
                    &badge.user_id.as_ref().map(|user_id| user_id.0),
                    &badge.badge_code,
                    &badge.label,
                    &now,
                ],
            )
            .await?;

        Ok(row_to_badge(&row))
    }

    pub async fn list_badges(&self) -> Result<Vec<Badge>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT id, user_id, badge_code, label, active, created_at, updated_at
                FROM badges
                ORDER BY id
                "#,
                &[],
            )
            .await?;

        Ok(rows.iter().map(row_to_badge).collect())
    }

    pub async fn get_badge_by_code(&self, badge_code: &str) -> Result<Option<Badge>, Error> {
        let row = self
            .client
            .query_opt(
                r#"
                SELECT id, user_id, badge_code, label, active, created_at, updated_at
                FROM badges
                WHERE badge_code = $1
                "#,
                &[&badge_code],
            )
            .await?;

        Ok(row.map(|row| row_to_badge(&row)))
    }

    pub async fn create_transaction(
        &self,
        station_id: &str,
        ocpp_version: &str,
        ocpp_transaction_id: Option<i32>,
        ocpp_transaction_ref: Option<&str>,
        connector_id: Option<i32>,
        evse_id: Option<i32>,
        user_id: Option<UserId>,
        badge_id: Option<BadgeId>,
        badge_code: Option<&str>,
        status: &str,
        started_at: DateTime<Utc>,
        ended_at: Option<DateTime<Utc>>,
        meter_start_wh: Option<i64>,
        stop_reason: Option<&str>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                INSERT INTO charging_transactions (
                    station_id,
                    ocpp_version,
                    ocpp_transaction_id,
                    ocpp_transaction_ref,
                    connector_id,
                    evse_id,
                    user_id,
                    badge_id,
                    badge_code,
                    status,
                    started_at,
                    ended_at,
                    meter_start_wh,
                    last_meter_wh,
                    stop_reason,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13, $14, $15, $15)
                ON CONFLICT DO NOTHING
                "#,
                &[
                    &station_id,
                    &ocpp_version,
                    &ocpp_transaction_id,
                    &ocpp_transaction_ref,
                    &connector_id,
                    &evse_id,
                    &user_id.as_ref().map(|id| id.0),
                    &badge_id.as_ref().map(|id| id.0),
                    &badge_code,
                    &status,
                    &started_at,
                    &ended_at,
                    &meter_start_wh,
                    &stop_reason,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn update_transaction_progress_by_id(
        &self,
        station_id: &str,
        ocpp_transaction_id: i32,
        meter_wh: Option<i64>,
        connector_id: Option<i32>,
        evse_id: Option<i32>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_transactions
                SET status = 'in_progress',
                    connector_id = COALESCE($3, connector_id),
                    evse_id = COALESCE($4, evse_id),
                    last_meter_wh = COALESCE($5, last_meter_wh),
                    energy_wh = CASE
                        WHEN COALESCE($5, last_meter_wh) IS NOT NULL AND meter_start_wh IS NOT NULL
                            THEN GREATEST(COALESCE($5, last_meter_wh) - meter_start_wh, 0)
                        ELSE energy_wh
                    END,
                    updated_at = $6
                WHERE station_id = $1
                  AND ocpp_transaction_id = $2
                "#,
                &[
                    &station_id,
                    &ocpp_transaction_id,
                    &connector_id,
                    &evse_id,
                    &meter_wh,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn finish_transaction_by_id(
        &self,
        station_id: &str,
        ocpp_transaction_id: i32,
        ended_at: DateTime<Utc>,
        meter_stop_wh: Option<i64>,
        stop_reason: Option<&str>,
        badge_code: Option<&str>,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_transactions
                SET status = 'completed',
                    ended_at = $3,
                    meter_stop_wh = COALESCE($4, meter_stop_wh),
                    last_meter_wh = COALESCE($4, last_meter_wh),
                    energy_wh = CASE
                        WHEN COALESCE($4, last_meter_wh) IS NOT NULL AND meter_start_wh IS NOT NULL
                            THEN GREATEST(COALESCE($4, last_meter_wh) - meter_start_wh, 0)
                        ELSE energy_wh
                    END,
                    stop_reason = COALESCE($5, stop_reason),
                    badge_code = COALESCE($6, badge_code),
                    updated_at = $7
                WHERE station_id = $1
                  AND ocpp_transaction_id = $2
                "#,
                &[
                    &station_id,
                    &ocpp_transaction_id,
                    &ended_at,
                    &meter_stop_wh,
                    &stop_reason,
                    &badge_code,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn update_transaction_progress_by_ref(
        &self,
        station_id: &str,
        ocpp_transaction_ref: &str,
        meter_wh: Option<i64>,
        connector_id: Option<i32>,
        evse_id: Option<i32>,
        status: &str,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_transactions
                SET status = $3,
                    connector_id = COALESCE($4, connector_id),
                    evse_id = COALESCE($5, evse_id),
                    last_meter_wh = COALESCE($6, last_meter_wh),
                    energy_wh = CASE
                        WHEN COALESCE($6, last_meter_wh) IS NOT NULL AND meter_start_wh IS NOT NULL
                            THEN GREATEST(COALESCE($6, last_meter_wh) - meter_start_wh, 0)
                        ELSE energy_wh
                    END,
                    updated_at = $7
                WHERE station_id = $1
                  AND ocpp_transaction_ref = $2
                "#,
                &[
                    &station_id,
                    &ocpp_transaction_ref,
                    &status,
                    &connector_id,
                    &evse_id,
                    &meter_wh,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn finish_transaction_by_ref(
        &self,
        station_id: &str,
        ocpp_transaction_ref: &str,
        ended_at: DateTime<Utc>,
        meter_stop_wh: Option<i64>,
        stop_reason: Option<&str>,
        status: &str,
    ) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE charging_transactions
                SET status = $3,
                    ended_at = $4,
                    meter_stop_wh = COALESCE($5, meter_stop_wh),
                    last_meter_wh = COALESCE($5, last_meter_wh),
                    energy_wh = CASE
                        WHEN COALESCE($5, last_meter_wh) IS NOT NULL AND meter_start_wh IS NOT NULL
                            THEN GREATEST(COALESCE($5, last_meter_wh) - meter_start_wh, 0)
                        ELSE energy_wh
                    END,
                    stop_reason = COALESCE($6, stop_reason),
                    updated_at = $7
                WHERE station_id = $1
                  AND ocpp_transaction_ref = $2
                "#,
                &[
                    &station_id,
                    &ocpp_transaction_ref,
                    &status,
                    &ended_at,
                    &meter_stop_wh,
                    &stop_reason,
                    &now,
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn list_transactions(&self, limit: i64) -> Result<Vec<ChargingTransaction>, Error> {
        let rows = self
            .client
            .query(
                r#"
                SELECT
                    id,
                    station_id,
                    ocpp_version,
                    ocpp_transaction_id,
                    ocpp_transaction_ref,
                    connector_id,
                    evse_id,
                    user_id,
                    badge_id,
                    badge_code,
                    status,
                    started_at,
                    ended_at,
                    meter_start_wh,
                    meter_stop_wh,
                    last_meter_wh,
                    energy_wh,
                    stop_reason,
                    created_at,
                    updated_at
                FROM charging_transactions
                ORDER BY started_at DESC, id DESC
                LIMIT $1
                "#,
                &[&limit],
            )
            .await?;

        Ok(rows.iter().map(row_to_charging_transaction).collect())
    }

    pub async fn set_badge_active(&self, id: BadgeId, active: bool) -> Result<(), Error> {
        let now: DateTime<Utc> = Utc::now();
        self.client
            .execute(
                r#"
                UPDATE badges
                SET active = $2,
                    updated_at = $3
                WHERE id = $1
                "#,
                &[&id.0, &active, &now],
            )
            .await?;

        Ok(())
    }

    pub async fn update_badge(
        &self,
        id: BadgeId,
        user_id: Option<UserId>,
        badge_code: String,
        label: Option<String>,
    ) -> Result<Option<Badge>, Error> {
        let now: DateTime<Utc> = Utc::now();
        let row = self
            .client
            .query_opt(
                r#"
                UPDATE badges
                SET user_id = $2,
                    badge_code = $3,
                    label = $4,
                    updated_at = $5
                WHERE id = $1
                RETURNING id, user_id, badge_code, label, active, created_at, updated_at
                "#,
                &[
                    &id.0,
                    &user_id.as_ref().map(|user_id| user_id.0),
                    &badge_code,
                    &label,
                    &now,
                ],
            )
            .await?;

        Ok(row.map(|row| row_to_badge(&row)))
    }
}

fn row_to_user(row: &Row) -> User {
    User {
        id: UserId(row.get::<_, i64>("id")),
        display_name: row.get("display_name"),
        email: row.get("email"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_badge(row: &Row) -> Badge {
    Badge {
        id: BadgeId(row.get::<_, i64>("id")),
        user_id: row.get::<_, Option<i64>>("user_id").map(UserId),
        badge_code: row.get("badge_code"),
        label: row.get("label"),
        active: row.get("active"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_station_summary(row: &Row) -> StationSummary {
    StationSummary {
        station_id: row.get("station_id"),
        station_name: row.get("station_name"),
        blocked: row.get("blocked"),
        ocpp_version: row.get("ocpp_version"),
        peer_addr: row.get("peer_addr"),
        first_seen_at: row.get("first_seen_at"),
        last_seen_at: row.get("last_seen_at"),
        last_boot_at: row.get("last_boot_at"),
        boot_count: row.get("boot_count"),
        latitude: row.get("latitude"),
        longitude: row.get("longitude"),
        location_label: row.get("location_label"),
        address: row.get("address"),
        notes: row.get("notes"),
        location_updated_at: row.get("location_updated_at"),
        current_status: row.get("current_status"),
        current_error_code: row.get("current_error_code"),
        current_connector_id: row.get("current_connector_id"),
        current_evse_id: row.get("current_evse_id"),
        current_status_at: row.get("current_status_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_connector_summary(row: &Row) -> ConnectorSummary {
    ConnectorSummary {
        station_id: row.get("station_id"),
        connector_id: row.get("connector_id"),
        evse_id: row.get("evse_id"),
        active: row.get("active"),
        auto_remote_start_badge_code: row.get("auto_remote_start_badge_code"),
        current_status: row.get("current_status"),
        current_error_code: row.get("current_error_code"),
        current_status_at: row.get("current_status_at"),
        active_transaction_id: row.get("active_transaction_id"),
        active_transaction_ref: row.get("active_transaction_ref"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_charging_transaction(row: &Row) -> ChargingTransaction {
    ChargingTransaction {
        id: row.get("id"),
        station_id: row.get("station_id"),
        ocpp_version: row.get("ocpp_version"),
        ocpp_transaction_id: row.get("ocpp_transaction_id"),
        ocpp_transaction_ref: row.get("ocpp_transaction_ref"),
        connector_id: row.get("connector_id"),
        evse_id: row.get("evse_id"),
        user_id: row.get("user_id"),
        badge_id: row.get("badge_id"),
        badge_code: row.get("badge_code"),
        status: row.get("status"),
        started_at: row.get("started_at"),
        ended_at: row.get("ended_at"),
        meter_start_wh: row.get("meter_start_wh"),
        meter_stop_wh: row.get("meter_stop_wh"),
        last_meter_wh: row.get("last_meter_wh"),
        energy_wh: row.get("energy_wh"),
        stop_reason: row.get("stop_reason"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn normalize_optional_text(value: &mut Option<String>) {
    if let Some(current) = value {
        let trimmed = current.trim();
        if trimmed.is_empty() {
            *value = None;
        } else if trimmed.len() != current.len() {
            *current = trimmed.to_string();
        }
    }
}

fn normalize_required_text(value: &mut String, field: &str) -> Result<(), std::io::Error> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("{field} obbligatorio"),
        ));
    }
    if trimmed.len() != value.len() {
        *value = trimmed.to_string();
    }
    Ok(())
}

fn normalize_energy_meter(meter: &mut SiteEnergyMeter) -> Result<(), std::io::Error> {
    normalize_required_text(&mut meter.id, "id misuratore energia")?;
    normalize_required_text(&mut meter.name, "nome misuratore energia")?;
    normalize_optional_text(&mut meter.catalog_key);
    normalize_optional_text(&mut meter.host);
    normalize_optional_text(&mut meter.meter_label);
    normalize_optional_text(&mut meter.notes);
    if let Some(port) = meter.port
        && port == 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("misuratore energia {} ha porta Modbus non valida", meter.name),
        ));
    }
    if let Some(unit_id) = meter.unit_id
        && unit_id == 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("misuratore energia {} ha unit id non valido", meter.name),
        ));
    }
    if let Some(poll_interval_ms) = meter.poll_interval_ms
        && poll_interval_ms == 0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "misuratore energia {} ha poll_interval_ms non valido",
                meter.name
            ),
        ));
    }
    if let Some(max_current_a) = meter.max_current_a
        && max_current_a <= 0.0
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("misuratore energia {} ha max_current_a non valido", meter.name),
        ));
    }
    Ok(())
}

fn normalize_and_validate_site_config(
    snapshot: &mut SiteConfigSnapshot,
    stations: &[StationSummary],
    energy_meter_ids: &[String],
) -> Result<(), std::io::Error> {
    use std::collections::HashSet;

    normalize_optional_text(&mut snapshot.site_name);
    normalize_optional_text(&mut snapshot.operator_name);
    normalize_optional_text(&mut snapshot.notes);
    normalize_required_text(&mut snapshot.timezone, "timezone")?;
    let feed_ids = energy_meter_ids.iter().cloned().collect::<HashSet<_>>();

    let station_ids = stations
        .iter()
        .map(|station| station.station_id.clone())
        .collect::<HashSet<_>>();
    let mut group_ids = HashSet::new();

    for group in &mut snapshot.management_groups {
        normalize_required_text(&mut group.id, "id gruppo")?;
        normalize_required_text(&mut group.name, "nome gruppo")?;
        normalize_required_text(&mut group.control_mode, "modalita gruppo")?;
        normalize_optional_text(&mut group.notes);

        if !group_ids.insert(group.id.clone()) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("gruppo duplicato: {}", group.id),
            ));
        }

        let mut dedup_feed_ids = HashSet::new();
        group.energy_meter_ids.retain(|meter_id| {
            let trimmed = meter_id.trim();
            !trimmed.is_empty() && dedup_feed_ids.insert(trimmed.to_string())
        });
        group.energy_meter_ids = group
            .energy_meter_ids
            .iter()
            .map(|meter_id| meter_id.trim().to_string())
            .collect();

        for meter_id in &group.energy_meter_ids {
            if !feed_ids.contains(meter_id) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "gruppo {} referenzia misuratore energia inesistente {}",
                        group.name, meter_id
                    ),
                ));
            }
        }

        let mut dedup_station_ids = HashSet::new();
        group.station_ids.retain(|station_id| {
            let trimmed = station_id.trim();
            !trimmed.is_empty() && dedup_station_ids.insert(trimmed.to_string())
        });
        group.station_ids = group
            .station_ids
            .iter()
            .map(|station_id| station_id.trim().to_string())
            .collect();

        for station_id in &group.station_ids {
            if !station_ids.contains(station_id) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "gruppo {} referenzia colonnina inesistente {}",
                        group.name, station_id
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn row_to_energy_meter(row: &Row) -> EnergyMeter {
    EnergyMeter {
        id: row.get("id"),
        name: row.get("name"),
        catalog_key: row.get("catalog_key"),
        host: row.get("host"),
        port: row.get("port"),
        unit_id: row.get("unit_id"),
        poll_interval_ms: row.get("poll_interval_ms"),
        meter_label: row.get("meter_label"),
        max_current_a: row.get("max_current_a"),
        notes: row.get("notes"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_energy_meter_runtime_status(row: &Row) -> EnergyMeterRuntimeStatus {
    EnergyMeterRuntimeStatus {
        meter_id: row.get("meter_id"),
        is_online: row.get("is_online"),
        last_attempt_at: row.get("last_attempt_at"),
        last_ok_at: row.get("last_ok_at"),
        last_error: row.get("last_error"),
        consecutive_failures: row.get("consecutive_failures"),
        last_poll_duration_ms: row.get("last_poll_duration_ms"),
        updated_at: row.get("updated_at"),
    }
}

fn row_to_energy_meter_latest_reading(row: &Row) -> EnergyMeterLatestReading {
    EnergyMeterLatestReading {
        meter_id: row.get("meter_id"),
        metric_key: row.get("metric_key"),
        unit: row.get("unit"),
        value_text: row.get("value_text"),
        value_num: row.get("value_num"),
        measured_at: row.get("measured_at"),
        updated_at: row.get("updated_at"),
    }
}

fn site_energy_meter_from_db(meter: &EnergyMeter) -> SiteEnergyMeter {
    SiteEnergyMeter {
        id: meter.id.clone(),
        name: meter.name.clone(),
        catalog_key: meter.catalog_key.clone(),
        host: meter.host.clone(),
        port: meter.port.and_then(|value| u16::try_from(value).ok()),
        unit_id: meter.unit_id.and_then(|value| u8::try_from(value).ok()),
        poll_interval_ms: meter.poll_interval_ms.and_then(|value| u64::try_from(value).ok()),
        meter_label: meter.meter_label.clone(),
        max_current_a: meter.max_current_a,
        notes: meter.notes.clone(),
    }
}
