use chrono::{DateTime, Utc};
use reqwest::{Client, Url};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct GreptimeConfig {
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub table: String,
    pub charging_measurements_table: String,
    pub energy_meter_measurements_table: String,
    pub username: Option<String>,
    pub password: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OcppMessageRecord<'a> {
    pub station_id: &'a str,
    pub ocpp_version: &'a str,
    pub peer_addr: &'a str,
    pub direction: &'a str,
    pub message_type: Option<i64>,
    pub unique_id: Option<&'a str>,
    pub action: Option<&'a str>,
    pub raw_text: &'a str,
    pub payload: Option<&'a Value>,
    pub parse_status: &'a str,
    pub error: Option<&'a str>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct OcppEventRow {
    pub message_id: String,
    pub station_id: String,
    pub ocpp_version: String,
    pub peer_addr: String,
    pub direction: String,
    pub message_type: Option<i64>,
    pub unique_id: Option<String>,
    pub action: Option<String>,
    pub raw_text: String,
    pub payload: Option<String>,
    pub parse_status: String,
    pub error: Option<String>,
    pub received_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ChargingMeasurementRecord {
    pub station_id: String,
    pub ocpp_version: String,
    pub source_action: String,
    pub transaction_id: Option<i64>,
    pub transaction_ref: Option<String>,
    pub connector_id: Option<i32>,
    pub evse_id: Option<i32>,
    pub meter_timestamp: DateTime<Utc>,
    pub sampled_value_context: Option<String>,
    pub measurand: String,
    pub phase: Option<String>,
    pub location: Option<String>,
    pub unit: Option<String>,
    pub unit_multiplier: Option<i32>,
    pub value_text: String,
    pub value_num: Option<f64>,
    pub value_wh: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct EnergyMeterMeasurementRecord {
    pub meter_id: String,
    pub metric_key: String,
    pub unit: String,
    pub measured_at: DateTime<Utc>,
    pub value_text: String,
    pub value_num: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EnergyMeterMeasurementRow {
    pub meter_id: String,
    pub metric_key: String,
    pub unit: Option<String>,
    pub value_text: String,
    pub value_num: Option<f64>,
    pub measured_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GreptimeSqlResponse {
    output: Vec<GreptimeSqlOutput>,
}

#[derive(Debug, Deserialize)]
struct GreptimeSqlOutput {
    records: Option<GreptimeSqlRecords>,
}

#[derive(Debug, Deserialize)]
struct GreptimeSqlRecords {
    schema: GreptimeSqlSchema,
    rows: Vec<Vec<Value>>,
}

#[derive(Debug, Deserialize)]
struct GreptimeSqlSchema {
    column_schemas: Vec<GreptimeSqlColumnSchema>,
}

#[derive(Debug, Deserialize)]
struct GreptimeSqlColumnSchema {
    name: String,
}

#[derive(Clone)]
pub struct GreptimeWriter {
    client: Client,
    config: GreptimeConfig,
}

impl GreptimeWriter {
    pub fn from_env() -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let Some(url) = std::env::var("GREPTIME_URL").ok() else {
            return Ok(None);
        };

        let parsed = Url::parse(&url)?;
        let protocol = parsed.scheme().to_string();
        let host = parsed
            .host_str()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "GREPTIME_URL missing host",
                )
            })?
            .to_owned();
        let port = parsed.port_or_known_default().unwrap_or(4000);

        let database = std::env::var("GREPTIME_DATABASE").unwrap_or_else(|_| "public".to_string());
        let table = std::env::var("GREPTIME_TABLE").unwrap_or_else(|_| "ocpp_messages".to_string());
        let charging_measurements_table = std::env::var("GREPTIME_CHARGING_MEASUREMENTS_TABLE")
            .unwrap_or_else(|_| "charging_measurements".to_string());
        let energy_meter_measurements_table =
            std::env::var("GREPTIME_ENERGY_METER_MEASUREMENTS_TABLE")
                .unwrap_or_else(|_| "energy_meter_measurements".to_string());
        let username = std::env::var("GREPTIME_USERNAME").ok();
        let password = std::env::var("GREPTIME_PASSWORD").ok();

        Ok(Some(Self {
            client: Client::new(),
            config: GreptimeConfig {
                protocol,
                host,
                port,
                database,
                table,
                charging_measurements_table,
                energy_meter_measurements_table,
                username,
                password,
            },
        }))
    }

    pub async fn write_ocpp_message(&self, record: &OcppMessageRecord<'_>) -> Result<(), String> {
        let body = message_to_line_protocol(&self.config.table, record);
        let mut request = self.client.post(self.write_url()).body(body);
        if let Some(username) = self.config.username.as_deref() {
            request = request.basic_auth(username, self.config.password.as_deref());
        }

        let response = request.send().await.map_err(|err| err.to_string())?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("status={status} body={body}"))
        }
    }

    pub async fn query_ocpp_messages(
        &self,
        limit: i64,
        station_ids: Option<&[String]>,
    ) -> Result<Vec<OcppEventRow>, String> {
        let mut sql = String::from(
            "SELECT message_id, station_id, ocpp_version, peer_addr, direction, message_type, unique_id, action, raw_text, payload, parse_status, greptime_timestamp AS received_at FROM ",
        );
        sql.push_str(&quote_ident(&self.config.table));
        if let Some(station_ids) = station_ids {
            if station_ids.is_empty() {
                return Ok(Vec::new());
            }

            sql.push_str(" WHERE station_id IN (");
            for (index, station_id) in station_ids.iter().enumerate() {
                if index > 0 {
                    sql.push_str(", ");
                }
                sql.push('\'');
                sql.push_str(&escape_sql_literal(station_id));
                sql.push('\'');
            }
            sql.push(')');
        }
        sql.push_str(" ORDER BY greptime_timestamp DESC LIMIT ");
        sql.push_str(&limit.clamp(1, 1000).to_string());

        let mut request = self.client.post(self.sql_url()).form(&[("sql", sql)]);
        if let Some(username) = self.config.username.as_deref() {
            request = request.basic_auth(username, self.config.password.as_deref());
        }

        let response = request.send().await.map_err(|err| err.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("status={status} body={body}"));
        }

        let body = response.text().await.map_err(|err| err.to_string())?;
        let payload: GreptimeSqlResponse =
            serde_json::from_str(&body).map_err(|err| err.to_string())?;
        Ok(map_sql_response(payload))
    }

    pub async fn write_charging_measurements(
        &self,
        records: &[ChargingMeasurementRecord],
    ) -> Result<(), String> {
        if records.is_empty() {
            return Ok(());
        }

        let body = records
            .iter()
            .map(|record| {
                charging_measurement_to_line_protocol(
                    &self.config.charging_measurements_table,
                    record,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut request = self.client.post(self.write_url()).body(body);
        if let Some(username) = self.config.username.as_deref() {
            request = request.basic_auth(username, self.config.password.as_deref());
        }

        let response = request.send().await.map_err(|err| err.to_string())?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("status={status} body={body}"))
        }
    }

    pub async fn write_energy_meter_measurements(
        &self,
        records: &[EnergyMeterMeasurementRecord],
    ) -> Result<(), String> {
        if records.is_empty() {
            return Ok(());
        }

        let body = records
            .iter()
            .map(|record| {
                energy_meter_measurement_to_line_protocol(
                    &self.config.energy_meter_measurements_table,
                    record,
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut request = self.client.post(self.write_url()).body(body);
        if let Some(username) = self.config.username.as_deref() {
            request = request.basic_auth(username, self.config.password.as_deref());
        }

        let response = request.send().await.map_err(|err| err.to_string())?;
        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(format!("status={status} body={body}"))
        }
    }

    pub async fn query_energy_meter_measurements(
        &self,
        meter_id: &str,
        limit: i64,
    ) -> Result<Vec<EnergyMeterMeasurementRow>, String> {
        let sql = format!(
            "SELECT meter_id, metric_key, unit, value_text, value_num, greptime_timestamp AS measured_at FROM {} WHERE meter_id = '{}' ORDER BY greptime_timestamp DESC LIMIT {}",
            quote_ident(&self.config.energy_meter_measurements_table),
            escape_sql_literal(meter_id),
            limit.clamp(1, 1000),
        );

        let mut request = self.client.post(self.sql_url()).form(&[("sql", sql)]);
        if let Some(username) = self.config.username.as_deref() {
            request = request.basic_auth(username, self.config.password.as_deref());
        }

        let response = request.send().await.map_err(|err| err.to_string())?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            if status.as_u16() == 400 && body.contains("Table not found") {
                return Ok(Vec::new());
            }
            return Err(format!("status={status} body={body}"));
        }

        let body = response.text().await.map_err(|err| err.to_string())?;
        let payload: GreptimeSqlResponse =
            serde_json::from_str(&body).map_err(|err| err.to_string())?;
        Ok(map_energy_meter_rows(payload))
    }

    fn write_url(&self) -> Url {
        let mut url = Url::parse(&format!(
            "{}://{}:{}/v1/influxdb/write",
            self.config.protocol, self.config.host, self.config.port
        ))
        .expect("invalid Greptime write url");

        {
            let mut query = url.query_pairs_mut();
            query.append_pair("db", &self.config.database);
            query.append_pair("precision", "ms");
            if let Some(username) = self.config.username.as_deref() {
                query.append_pair("u", username);
            }
            if let Some(password) = self.config.password.as_deref() {
                query.append_pair("p", password);
            }
        }

        url
    }

    fn sql_url(&self) -> Url {
        let mut url = Url::parse(&format!(
            "{}://{}:{}/v1/sql",
            self.config.protocol, self.config.host, self.config.port
        ))
        .expect("invalid Greptime sql url");

        {
            let mut query = url.query_pairs_mut();
            query.append_pair("db", &self.config.database);
            if let Some(username) = self.config.username.as_deref() {
                query.append_pair("u", username);
            }
            if let Some(password) = self.config.password.as_deref() {
                query.append_pair("p", password);
            }
        }

        url
    }
}

fn message_to_line_protocol(table: &str, record: &OcppMessageRecord<'_>) -> String {
    let message_id = format!(
        "{}-{}-{}",
        record.station_id,
        record.received_at.timestamp_micros(),
        record.unique_id.unwrap_or("no-id")
    );

    let tags = [
        format!("message_id={}", escape_tag(&message_id)),
        format!("station_id={}", escape_tag(record.station_id)),
    ];

    let mut fields = vec![
        field_str("ocpp_version", record.ocpp_version),
        field_str("peer_addr", record.peer_addr),
        field_str("direction", record.direction),
        field_str("raw_text", record.raw_text),
        field_str("parse_status", record.parse_status),
        field_str(
            "payload",
            &record
                .payload
                .map(|value| value.to_string())
                .unwrap_or_default(),
        ),
    ];

    if let Some(message_type) = record.message_type {
        fields.push(format!("message_type={message_type}i"));
    }
    if let Some(unique_id) = record.unique_id {
        fields.push(field_str("unique_id", unique_id));
    }
    if let Some(action) = record.action {
        fields.push(field_str("action", action));
    }
    if let Some(error) = record.error {
        fields.push(field_str("error", error));
    }

    format!(
        "{},{} {} {}",
        escape_measurement(table),
        tags.join(","),
        fields.join(","),
        record.received_at.timestamp_millis()
    )
}

fn charging_measurement_to_line_protocol(
    table: &str,
    record: &ChargingMeasurementRecord,
) -> String {
    let tags = [format!("station_id={}", escape_tag(&record.station_id))];

    let mut fields = vec![
        field_str("ocpp_version", &record.ocpp_version),
        field_str("source_action", &record.source_action),
        field_str("measurand", &record.measurand),
        field_str("value_text", &record.value_text),
    ];

    if let Some(transaction_id) = record.transaction_id {
        fields.push(format!("transaction_id={transaction_id}i"));
    }
    if let Some(transaction_ref) = record.transaction_ref.as_deref() {
        fields.push(field_str("transaction_ref", transaction_ref));
    }
    if let Some(connector_id) = record.connector_id {
        fields.push(format!("connector_id={connector_id}i"));
    }
    if let Some(evse_id) = record.evse_id {
        fields.push(format!("evse_id={evse_id}i"));
    }
    if let Some(context) = record.sampled_value_context.as_deref() {
        fields.push(field_str("sampled_value_context", context));
    }
    if let Some(phase) = record.phase.as_deref() {
        fields.push(field_str("phase", phase));
    }
    if let Some(location) = record.location.as_deref() {
        fields.push(field_str("location", location));
    }
    if let Some(unit) = record.unit.as_deref() {
        fields.push(field_str("unit", unit));
    }
    if let Some(unit_multiplier) = record.unit_multiplier {
        fields.push(format!("unit_multiplier={unit_multiplier}i"));
    }
    if let Some(value_num) = record.value_num.filter(|value| value.is_finite()) {
        fields.push(format!("value_num={value_num}"));
    }
    if let Some(value_wh) = record.value_wh {
        fields.push(format!("value_wh={value_wh}i"));
    }

    format!(
        "{},{} {} {}",
        escape_measurement(table),
        tags.join(","),
        fields.join(","),
        record.meter_timestamp.timestamp_millis()
    )
}

fn energy_meter_measurement_to_line_protocol(
    table: &str,
    record: &EnergyMeterMeasurementRecord,
) -> String {
    let tags = [
        format!("meter_id={}", escape_tag(&record.meter_id)),
        format!("metric_key={}", escape_tag(&record.metric_key)),
    ];

    let fields = [
        field_str("unit", &record.unit),
        field_str("value_text", &record.value_text),
        format!("value_num={}", record.value_num),
    ];

    format!(
        "{},{} {} {}",
        escape_measurement(table),
        tags.join(","),
        fields.join(","),
        record.measured_at.timestamp_millis()
    )
}

fn field_str(key: &str, value: &str) -> String {
    format!("{key}=\"{}\"", escape_field_string(value))
}

fn quote_ident(value: &str) -> String {
    format!("`{}`", value.replace('`', "``"))
}

fn escape_sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn escape_measurement(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace(' ', "\\ ")
}

fn escape_tag(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(',', "\\,")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

fn escape_field_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn map_sql_response(payload: GreptimeSqlResponse) -> Vec<OcppEventRow> {
    let Some(records) = payload.output.into_iter().find_map(|output| output.records) else {
        return Vec::new();
    };

    let columns = records.schema.column_schemas;

    let events: Vec<OcppEventRow> = records
        .rows
        .into_iter()
        .map(|row| map_event_row(&columns, row))
        .collect();

    events
}

fn map_energy_meter_rows(payload: GreptimeSqlResponse) -> Vec<EnergyMeterMeasurementRow> {
    let Some(records) = payload.output.into_iter().find_map(|output| output.records) else {
        return Vec::new();
    };

    records
        .rows
        .into_iter()
        .map(|row| map_energy_meter_row(&records.schema.column_schemas, row))
        .collect()
}

fn map_energy_meter_row(
    columns: &[GreptimeSqlColumnSchema],
    row: Vec<Value>,
) -> EnergyMeterMeasurementRow {
    let mut meter_id = None;
    let mut metric_key = None;
    let mut unit = None;
    let mut value_text = None;
    let mut value_num = None;
    let mut measured_at = None;

    for (column, value) in columns.iter().zip(row.into_iter()) {
        match column.name.as_str() {
            "meter_id" => meter_id = value_to_string(&value),
            "metric_key" => metric_key = value_to_string(&value),
            "unit" => unit = value_to_string(&value),
            "value_text" => value_text = value_to_string(&value),
            "value_num" => value_num = value_to_f64(&value),
            "measured_at" => measured_at = value_to_datetime(&value),
            _ => {}
        }
    }

    EnergyMeterMeasurementRow {
        meter_id: meter_id.unwrap_or_else(|| "unknown".to_string()),
        metric_key: metric_key.unwrap_or_else(|| "unknown".to_string()),
        unit,
        value_text: value_text.unwrap_or_default(),
        value_num,
        measured_at: measured_at.unwrap_or_else(Utc::now),
    }
}

fn map_event_row(columns: &[GreptimeSqlColumnSchema], row: Vec<Value>) -> OcppEventRow {
    let mut message_id = None;
    let mut station_id = None;
    let mut ocpp_version = None;
    let mut peer_addr = None;
    let mut direction = None;
    let mut message_type = None;
    let mut unique_id = None;
    let mut action = None;
    let mut raw_text = None;
    let mut payload = None;
    let mut parse_status = None;
    let mut error = None;
    let mut received_at = None;

    for (column, value) in columns.iter().zip(row.into_iter()) {
        match column.name.as_str() {
            "message_id" => message_id = value_to_string(&value),
            "station_id" => station_id = value_to_string(&value),
            "ocpp_version" => ocpp_version = value_to_string(&value),
            "peer_addr" => peer_addr = value_to_string(&value),
            "direction" => direction = value_to_string(&value),
            "message_type" => message_type = value_to_i64(&value),
            "unique_id" => unique_id = value_to_string(&value),
            "action" => action = value_to_string(&value),
            "raw_text" => raw_text = value_to_string(&value),
            "payload" => payload = value_to_string(&value),
            "parse_status" => parse_status = value_to_string(&value),
            "error" => error = value_to_string(&value),
            "received_at" => received_at = value_to_datetime(&value),
            _ => {}
        }
    }

    let station_id = station_id.unwrap_or_else(|| "unknown".to_string());
    let received_at = received_at.unwrap_or_else(Utc::now);

    OcppEventRow {
        message_id: message_id.unwrap_or_else(|| format!("{}-unknown", station_id)),
        station_id,
        ocpp_version: ocpp_version.unwrap_or_else(|| "unknown".to_string()),
        peer_addr: peer_addr.unwrap_or_else(|| "unknown".to_string()),
        direction: direction.unwrap_or_else(|| "unknown".to_string()),
        message_type,
        unique_id,
        action,
        raw_text: raw_text.unwrap_or_default(),
        payload,
        parse_status: parse_status.unwrap_or_else(|| "unknown".to_string()),
        error,
        received_at,
    }
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(boolean) => Some(boolean.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn value_to_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn value_to_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => text.parse().ok(),
        _ => None,
    }
}

fn value_to_datetime(value: &Value) -> Option<DateTime<Utc>> {
    match value {
        Value::Number(number) => number.as_i64().and_then(parse_epoch_millis_or_seconds),
        Value::String(text) => text
            .parse::<i64>()
            .ok()
            .and_then(parse_epoch_millis_or_seconds)
            .or_else(|| {
                chrono::DateTime::parse_from_rfc3339(text)
                    .ok()
                    .map(|value| value.with_timezone(&Utc))
            })
            .or_else(|| {
                chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S%.f")
                    .ok()
                    .map(|value| DateTime::<Utc>::from_naive_utc_and_offset(value, Utc))
            }),
        _ => None,
    }
}

fn parse_epoch_millis_or_seconds(value: i64) -> Option<DateTime<Utc>> {
    if value >= 1_000_000_000_000_000_000 {
        return Some(DateTime::<Utc>::from_timestamp_nanos(value));
    }

    if value >= 1_000_000_000_000 {
        DateTime::<Utc>::from_timestamp_millis(value)
    } else if value >= 1_000_000_000 {
        DateTime::<Utc>::from_timestamp(value, 0)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_protocol_includes_message_metadata() {
        let record = OcppMessageRecord {
            station_id: "CP 1",
            ocpp_version: "1.6",
            peer_addr: "127.0.0.1:1234",
            direction: "inbound",
            message_type: Some(2),
            unique_id: Some("boot-1"),
            action: Some("BootNotification"),
            raw_text: r#"[2,"boot-1","BootNotification",{}]"#,
            payload: Some(&Value::Null),
            parse_status: "parsed",
            error: None,
            received_at: Utc::now(),
        };

        let line = message_to_line_protocol("ocpp messages", &record);
        assert!(line.contains("ocpp\\ messages"));
        assert!(line.contains("station_id=CP\\ 1"));
        assert!(line.contains("message_type=2i"));
        assert!(line.contains("action=\"BootNotification\""));
        assert!(line.contains("parse_status=\"parsed\""));
    }
}
