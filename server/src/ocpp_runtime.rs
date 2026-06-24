use chrono::Utc;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use rust_ocpp::{
    v1_6::types::AuthorizationStatus,
    v2_0_1::{
        datatypes::id_token_info_type::IdTokenInfoType,
        enumerations::authorization_status_enum_type::AuthorizationStatusEnumType,
    },
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::mpsc,
};
use tokio::sync::{oneshot, oneshot::Sender};
use tokio_tungstenite::{
    accept_hdr_async,
    WebSocketStream,
    tungstenite::{
        Error, Message,
        handshake::server::{Request, Response},
        http::{HeaderMap, HeaderValue, StatusCode, header::SEC_WEBSOCKET_PROTOCOL},
    },
};
use uuid::Uuid;

use crate::{
    app_state::{
        ConnectionRegistry, StationCommand, StationConfigurationEntry,
        StationConfigurationSnapshot,
    },
    db::Database,
    greptime::{ChargingMeasurementRecord, OcppMessageRecord},
    ocpp_v16::handle_v16_call,
    ocpp_v201::handle_v201_call,
    realtime::RealtimeNotifier,
};

pub(crate) struct OcppCall<'a> {
    pub(crate) message_type: i64,
    pub(crate) unique_id: &'a str,
    pub(crate) action: &'a str,
    pub(crate) text: &'a str,
    pub(crate) payload: &'a Value,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum OcppVersion {
    V16,
    V201,
}

impl OcppVersion {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            OcppVersion::V16 => "1.6",
            OcppVersion::V201 => "2.0.1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BadgeAuthorizationDecision {
    Accepted,
    Blocked,
    Invalid,
}

impl BadgeAuthorizationDecision {
    pub(crate) fn as_v16_status(self) -> AuthorizationStatus {
        match self {
            Self::Accepted => AuthorizationStatus::Accepted,
            Self::Blocked => AuthorizationStatus::Blocked,
            Self::Invalid => AuthorizationStatus::Invalid,
        }
    }

    pub(crate) fn as_v201_status(self) -> AuthorizationStatusEnumType {
        match self {
            Self::Accepted => AuthorizationStatusEnumType::Accepted,
            Self::Blocked => AuthorizationStatusEnumType::Blocked,
            Self::Invalid => AuthorizationStatusEnumType::Invalid,
        }
    }

    pub(crate) fn transaction_status(self) -> &'static str {
        match self {
            Self::Accepted => "in_progress",
            Self::Blocked => "blocked",
            Self::Invalid => "invalid",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConnectionContext {
    pub(crate) station_id: String,
    pub(crate) version: OcppVersion,
    pub(crate) peer_addr: String,
}

#[derive(Debug, Default)]
pub(crate) struct SessionState {
    pub(crate) active_transaction_id: Option<i32>,
    pub(crate) active_connector_id: Option<u32>,
    pub(crate) active_transaction_id_v201: Option<String>,
    pub(crate) active_connector_id_v201: Option<i32>,
    pub(crate) active_badge: Option<AuthorizedBadge>,
    pub(crate) pending_requests: HashMap<String, Sender<Value>>,
    pub(crate) ignored_responses: HashMap<String, Instant>,
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorizedBadge {
    pub(crate) badge_id: i64,
    pub(crate) user_id: i64,
    pub(crate) badge_code: String,
}

pub(crate) type OcppSink = SplitSink<WebSocketStream<TcpStream>, Message>;

pub(crate) async fn handle_connection(
    stream: TcpStream,
    peer: SocketAddr,
    db: Database,
    connections: ConnectionRegistry,
    notifier: RealtimeNotifier,
) {
    if let Err(err) = accept_charge_point(stream, peer, db, connections, notifier).await {
        eprintln!("connessione {peer} chiusa con errore: {err}");
    }
}

async fn accept_charge_point(
    stream: TcpStream,
    peer: SocketAddr,
    db: Database,
    connections: ConnectionRegistry,
    notifier: RealtimeNotifier,
) -> Result<(), Error> {
    let context = Arc::new(Mutex::new(None::<ConnectionContext>));
    let context_for_handshake = Arc::clone(&context);

    let ws = accept_hdr_async(stream, move |request: &Request, mut response: Response| {
        let station_id = match parse_station_id(request.uri().path()) {
            Some(station_id) => station_id.to_owned(),
            None => {
                return Err(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Some("missing /ocpp/<station_id> path".to_string()))
                    .expect("bad request response"));
            }
        };

        let version = match pick_ocpp_protocol(request.headers()) {
            Some(OcppVersion::V16) => {
                response
                    .headers_mut()
                    .insert(SEC_WEBSOCKET_PROTOCOL, HeaderValue::from_static("ocpp1.6"));
                OcppVersion::V16
            }
            Some(OcppVersion::V201) => {
                response.headers_mut().insert(
                    SEC_WEBSOCKET_PROTOCOL,
                    HeaderValue::from_static("ocpp2.0.1"),
                );
                OcppVersion::V201
            }
            None => {
                return Err(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Some("missing supported Sec-WebSocket-Protocol".to_string()))
                    .expect("bad request response"));
            }
        };

        *context_for_handshake
            .lock()
            .expect("handshake context poisoned") = Some(ConnectionContext {
            station_id,
            version,
            peer_addr: peer.to_string(),
        });

        Ok(response)
    })
    .await?;

    let context = context
        .lock()
        .expect("handshake context poisoned")
        .clone()
        .expect("handshake context missing");

    let station_exists = match db.station_exists(&context.station_id).await {
        Ok(exists) => exists,
        Err(err) => {
            eprintln!(
                "postgres station_exists fallito per {}: {}",
                context.station_id, err
            );
            false
        }
    };

    if station_exists {
        println!(
            "Colonnina riconnessa {} da {} ({:?})",
            context.station_id, peer, context.version
        );
    } else {
        println!(
            "Nuova colonnina {} da {} ({:?})",
            context.station_id, peer, context.version
        );
    }

    if let Err(err) = db
        .touch_station(
            &context.station_id,
            context.version.as_str(),
            &context.peer_addr,
        )
        .await
    {
        eprintln!(
            "postgres touch_station fallito per {}: {}",
            context.station_id, err
        );
    } else {
        notifier.notify();
    }

    match db.get_station(&context.station_id).await {
        Ok(Some(station)) if station.blocked => {
            eprintln!(
                "colonnina {} bloccata da gui, connessione chiusa",
                context.station_id
            );
            return Ok(());
        }
        Ok(_) => {}
        Err(err) => {
            eprintln!(
                "postgres get_station fallito per {}: {}",
                context.station_id, err
            );
        }
    }

    let mut session = SessionState::default();
    let (mut sink, mut stream) = ws.split();
    let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<StationCommand>();
    connections.register(context.station_id.clone(), cmd_tx);

    struct RegistryGuard<'a> {
        connections: &'a ConnectionRegistry,
        station_id: &'a str,
    }

    impl Drop for RegistryGuard<'_> {
        fn drop(&mut self) {
            self.connections.unregister(self.station_id);
        }
    }

    let _registry_guard = RegistryGuard {
        connections: &connections,
        station_id: &context.station_id,
    };

    loop {
        tokio::select! {
            maybe_message = stream.next() => {
                let Some(message) = maybe_message else {
                    break;
                };
                let message = message?;

                match message {
                    Message::Text(text) => {
                        if let Some(reply) = handle_ocpp_text(&context, &mut session, &text, &db, &mut sink).await? {
                            if let Message::Text(reply_text) = &reply
                                && !is_heartbeat_response_text(reply_text)
                            {
                                log_ocpp_packet(
                                    &context,
                                    "outbound",
                                    Some("ws_text_reply"),
                                    None,
                                    None,
                                    reply_text.as_str(),
                                );
                            }
                            sink.send(reply).await?;
                        }
                        notifier.notify();
                    }
                    Message::Binary(data) => {
                        eprintln!("OCPP bin da {}: {} bytes", context.station_id, data.len());
                    }
                    Message::Close(_) => {
                        eprintln!("colonnina {} chiusa", context.station_id);
                        break;
                    }
                    _ => {}
                }
            }
            maybe_cmd = cmd_rx.recv() => {
                let Some(cmd) = maybe_cmd else {
                    continue;
                };
                match cmd {
                    StationCommand::BlockStation { blocked, reply } => {
                        let result = handle_block_station_command(&context, &mut session, &mut sink, blocked).await;
                        let _ = reply.send(result);
                    }
                    StationCommand::RemoteStartTransaction { connector_id, badge_code, reply } => {
                        let result = handle_remote_start_transaction_command(
                            &context,
                            &mut session,
                            &mut sink,
                            connector_id,
                            &badge_code,
                        )
                        .await;
                        let _ = reply.send(result);
                    }
                    StationCommand::RemoteStopTransaction {
                        transaction_id,
                        transaction_ref,
                        reply,
                    } => {
                        let result = handle_remote_stop_transaction_command(
                            &context,
                            &mut session,
                            &mut sink,
                            transaction_id,
                            transaction_ref.as_deref(),
                        )
                        .await;
                        let _ = reply.send(result);
                    }
                    StationCommand::GetConfiguration { reply } => {
                        let result = handle_get_configuration_command(
                            &context,
                            &mut session,
                            &mut sink,
                        )
                        .await;
                        let _ = reply.send(result);
                    }
                    StationCommand::SetConnectorActive { connector_id, evse_id, active, reply } => {
                        let result = handle_set_connector_active_command(
                            &context,
                            &mut session,
                            &mut sink,
                            connector_id,
                            evse_id,
                            active,
                        )
                        .await;
                        let _ = reply.send(result);
                    }
                    StationCommand::UnlockConnector { connector_id, evse_id, reply } => {
                        let result = handle_unlock_connector_command(
                            &context,
                            &mut session,
                            &mut sink,
                            connector_id,
                            evse_id,
                        )
                        .await;
                        let _ = reply.send(result);
                    }
                }
            }
        }
    }

    let _ = sink.close().await;
    Ok(())
}

async fn handle_ocpp_text(
    context: &ConnectionContext,
    session: &mut SessionState,
    text: &str,
    db: &Database,
    sink: &mut OcppSink,
) -> Result<Option<Message>, Error> {
    let frame: Value = match serde_json::from_str(text) {
        Ok(frame) => frame,
        Err(err) => {
            log_unparsed_ocpp_frame(context, "invalid_json", Some(&err.to_string()), text);
            save_ocpp_event(
                db,
                context,
                "inbound",
                None,
                None,
                None,
                text,
                None,
                "invalid_json",
                Some(err.to_string()),
            )
            .await;
            return Ok(None);
        }
    };

    let Some(items) = frame.as_array() else {
        log_unparsed_ocpp_frame(context, "not_array", Some("frame non array"), text);
        save_ocpp_event(
            db,
            context,
            "inbound",
            None,
            None,
            None,
            text,
            Some(&frame),
            "not_array",
            Some("frame non array".to_string()),
        )
        .await;
        return Ok(None);
    };

    let Some(message_type) = items[0].as_u64() else {
        log_unparsed_ocpp_frame(
            context,
            "missing_message_type",
            Some("message type mancante"),
            text,
        );
        save_ocpp_event(
            db,
            context,
            "inbound",
            None,
            None,
            None,
            text,
            Some(&frame),
            "missing_message_type",
            Some("message type mancante".to_string()),
        )
        .await;
        return Ok(None);
    };

    let expected_len = match message_type {
        2 => 4,
        3 => 3,
        4 => 4,
        _ => 2,
    };
    if items.len() < expected_len {
        log_unparsed_ocpp_frame(context, "too_short", Some("frame troppo corto"), text);
        save_ocpp_event(
            db,
            context,
            "inbound",
            Some(message_type as i64),
            items.get(1).and_then(Value::as_str),
            None,
            text,
            Some(&frame),
            "too_short",
            Some("frame troppo corto".to_string()),
        )
        .await;
        return Ok(None);
    }

    let unique_id = items[1].as_str().unwrap_or("").to_string();
    let action = match message_type {
        2 => items[2].as_str().unwrap_or(""),
        3 => "",
        4 => items[2].as_str().unwrap_or(""),
        _ => "",
    };
    let payload = match message_type {
        2 => items[3].clone(),
        3 => items[2].clone(),
        4 => items.get(3).cloned().unwrap_or(Value::Null),
        _ => Value::Null,
    };

    if !is_heartbeat_action(action) {
        log_ocpp_packet(
            context,
            "inbound",
            Some("frame"),
            Some(unique_id.as_str()),
            Some(action),
            text,
        );
    }

    if message_type == 3 {
        if let Some(reply) = session.pending_requests.remove(&unique_id) {
            let _ = reply.send(payload.clone());
        } else if session.ignored_responses.remove(&unique_id).is_some() {
        } else {
            eprintln!(
                "risposta OCPP inattesa da {}: unique_id={} payload={}",
                context.station_id, unique_id, payload
            );
        }

        save_ocpp_event(
            db,
            context,
            "inbound",
            Some(3),
            Some(unique_id.as_str()),
            Some(action),
            text,
            Some(&payload),
            "response",
            None,
        )
        .await;
        return Ok(None);
    }

    if message_type != 2 {
        save_ocpp_event(
            db,
            context,
            "inbound",
            Some(message_type as i64),
            Some(unique_id.as_str()),
            Some(action),
            text,
            Some(&payload),
            "ignored_message_type",
            None,
        )
        .await;
        return Ok(None);
    }

    if !is_heartbeat_action(action) {
        save_ocpp_event(
            db,
            context,
            "inbound",
            Some(message_type as i64),
            Some(unique_id.as_str()),
            Some(action),
            text,
            Some(&payload),
            "parsed",
            None,
        )
        .await;
    }

    if let Err(err) = db
        .touch_station(
            &context.station_id,
            context.version.as_str(),
            &context.peer_addr,
        )
        .await
    {
        eprintln!(
            "postgres touch_station fallito per {}: {}",
            context.station_id, err
        );
    }

    let call = OcppCall {
        message_type: message_type as i64,
        unique_id: unique_id.as_str(),
        action,
        text,
        payload: &payload,
    };

    match context.version {
        OcppVersion::V16 => handle_v16_call(context, session, db, sink, &call).await,
        OcppVersion::V201 => handle_v201_call(context, session, db, sink, &call).await,
    }
}

pub(crate) async fn authorize_badge(
    db: &Database,
    badge_code: &str,
) -> Result<BadgeAuthorizationDecision, Box<dyn std::error::Error + Send + Sync>> {
    let Some(badge) = db.get_badge_by_code(badge_code).await? else {
        return Ok(BadgeAuthorizationDecision::Invalid);
    };

    if !badge.active || badge.user_id.is_none() {
        return Ok(BadgeAuthorizationDecision::Blocked);
    }

    Ok(BadgeAuthorizationDecision::Accepted)
}

pub(crate) async fn resolve_authorized_badge(
    db: &Database,
    badge_code: &str,
) -> Result<Option<AuthorizedBadge>, Box<dyn std::error::Error + Send + Sync>> {
    let Some(badge) = db.get_badge_by_code(badge_code).await? else {
        return Ok(None);
    };
    let Some(user_id) = badge.user_id else {
        return Ok(None);
    };
    if !badge.active {
        return Ok(None);
    }

    Ok(Some(AuthorizedBadge {
        badge_id: badge.id.0,
        user_id: user_id.0,
        badge_code: badge.badge_code,
    }))
}

pub(crate) fn transaction_energy_from_meter_values(
    meter_values: Option<&Vec<rust_ocpp::v2_0_1::datatypes::meter_value_type::MeterValueType>>,
) -> Option<i64> {
    let meter_values = meter_values?;

    meter_values
        .iter()
        .rev()
        .flat_map(|meter_value| meter_value.sampled_value.iter())
        .find_map(sampled_value_to_wh)
}

fn sampled_value_to_wh(
    sampled_value: &rust_ocpp::v2_0_1::datatypes::sampled_value_type::SampledValueType,
) -> Option<i64> {
    use rust_ocpp::v2_0_1::enumerations::measurand_enum_type::MeasurandEnumType;

    let measurand = sampled_value
        .measurand
        .clone()
        .unwrap_or(MeasurandEnumType::EnergyActiveImportRegister);
    if measurand != MeasurandEnumType::EnergyActiveImportRegister
        && measurand != MeasurandEnumType::EnergyActiveImportInterval
    {
        return None;
    }

    let raw = sampled_value.value.to_string().parse::<f64>().ok()?;
    let multiplier = sampled_value
        .unit_of_measure
        .as_ref()
        .and_then(|unit| unit.multiplier)
        .unwrap_or(0);
    let unit_factor = match sampled_value
        .unit_of_measure
        .as_ref()
        .and_then(|unit| unit.unit.as_deref())
    {
        Some("kWh") => 1_000.0,
        Some("MWh") => 1_000_000.0,
        _ => 1.0,
    };

    Some((raw * 10f64.powi(multiplier) * unit_factor).round() as i64)
}

pub(crate) fn transaction_energy_from_meter_values_v16(
    meter_values: &[rust_ocpp::v1_6::types::MeterValue],
) -> Option<i64> {
    use rust_ocpp::v1_6::types::{Measurand, UnitOfMeasure};

    meter_values
        .iter()
        .rev()
        .flat_map(|meter_value| meter_value.sampled_value.iter())
        .find_map(|sampled_value| {
            let measurand = sampled_value
                .measurand
                .clone()
                .unwrap_or(Measurand::EnergyActiveImportRegister);
            if measurand != Measurand::EnergyActiveImportRegister {
                return None;
            }

            let raw = sampled_value.value.parse::<f64>().ok()?;
            let unit_factor = match sampled_value.unit.clone().unwrap_or(UnitOfMeasure::Wh) {
                UnitOfMeasure::KWh => 1_000.0,
                _ => 1.0,
            };
            Some((raw * unit_factor).round() as i64)
        })
}

pub(crate) fn charging_measurements_from_meter_values_v16(
    context: &ConnectionContext,
    source_action: &str,
    transaction_id: Option<i32>,
    connector_id: Option<i32>,
    meter_values: &[rust_ocpp::v1_6::types::MeterValue],
) -> Vec<ChargingMeasurementRecord> {
    meter_values
        .iter()
        .flat_map(|meter_value| {
            meter_value.sampled_value.iter().map(move |sampled_value| {
                let value_wh = sampled_value_to_wh_v16(sampled_value);
                let value_text = sampled_value.value.clone();
                let value_num = value_text
                    .parse::<f64>()
                    .ok()
                    .filter(|value| value.is_finite());
                ChargingMeasurementRecord {
                    station_id: context.station_id.clone(),
                    ocpp_version: context.version.as_str().to_string(),
                    source_action: source_action.to_string(),
                    transaction_id: transaction_id.map(i64::from),
                    transaction_ref: None,
                    connector_id,
                    evse_id: None,
                    meter_timestamp: meter_value.timestamp,
                    sampled_value_context: sampled_value
                        .context
                        .as_ref()
                        .map(|value| format!("{value:?}")),
                    measurand: format!(
                        "{:?}",
                        sampled_value.measurand.clone().unwrap_or(
                            rust_ocpp::v1_6::types::Measurand::EnergyActiveImportRegister
                        )
                    ),
                    phase: sampled_value
                        .phase
                        .as_ref()
                        .map(|value| format!("{value:?}")),
                    location: sampled_value
                        .location
                        .as_ref()
                        .map(|value| format!("{value:?}")),
                    unit: sampled_value.unit.as_ref().map(|value| format!("{value:?}")),
                    unit_multiplier: None,
                    value_text,
                    value_num,
                    value_wh,
                }
            })
        })
        .collect()
}

pub(crate) fn charging_measurements_from_meter_values_v201(
    context: &ConnectionContext,
    source_action: &str,
    transaction_ref: Option<&str>,
    evse_id: Option<i32>,
    connector_id: Option<i32>,
    meter_values: &[rust_ocpp::v2_0_1::datatypes::meter_value_type::MeterValueType],
) -> Vec<ChargingMeasurementRecord> {
    meter_values
        .iter()
        .flat_map(|meter_value| {
            meter_value.sampled_value.iter().map(move |sampled_value| {
                let value_text = sampled_value.value.to_string();
                let value_num = value_text.parse::<f64>().ok().filter(|value| value.is_finite());
                ChargingMeasurementRecord {
                    station_id: context.station_id.clone(),
                    ocpp_version: context.version.as_str().to_string(),
                    source_action: source_action.to_string(),
                    transaction_id: None,
                    transaction_ref: transaction_ref.map(str::to_string),
                    connector_id,
                    evse_id,
                    meter_timestamp: meter_value.timestamp,
                    sampled_value_context: sampled_value
                        .context
                        .as_ref()
                        .map(|value| format!("{value:?}")),
                    measurand: format!(
                        "{:?}",
                        sampled_value
                            .measurand
                            .clone()
                            .unwrap_or(
                                rust_ocpp::v2_0_1::enumerations::measurand_enum_type::MeasurandEnumType::EnergyActiveImportRegister,
                            )
                    ),
                    phase: sampled_value.phase.as_ref().map(|value| format!("{value:?}")),
                    location: sampled_value.location.as_ref().map(|value| format!("{value:?}")),
                    unit: sampled_value
                        .unit_of_measure
                        .as_ref()
                        .and_then(|value| value.unit.clone()),
                    unit_multiplier: sampled_value
                        .unit_of_measure
                        .as_ref()
                        .and_then(|value| value.multiplier),
                    value_text,
                    value_num,
                    value_wh: sampled_value_to_wh(sampled_value),
                }
            })
        })
        .collect()
}

pub(crate) fn synthetic_v16_energy_measurement(
    context: &ConnectionContext,
    source_action: &str,
    transaction_id: i32,
    connector_id: Option<i32>,
    timestamp: chrono::DateTime<Utc>,
    meter_wh: i64,
    reading_context: &'static str,
) -> ChargingMeasurementRecord {
    ChargingMeasurementRecord {
        station_id: context.station_id.clone(),
        ocpp_version: context.version.as_str().to_string(),
        source_action: source_action.to_string(),
        transaction_id: Some(i64::from(transaction_id)),
        transaction_ref: None,
        connector_id,
        evse_id: None,
        meter_timestamp: timestamp,
        sampled_value_context: Some(reading_context.to_string()),
        measurand: "EnergyActiveImportRegister".to_string(),
        phase: None,
        location: None,
        unit: Some("Wh".to_string()),
        unit_multiplier: None,
        value_text: meter_wh.to_string(),
        value_num: Some(meter_wh as f64),
        value_wh: Some(meter_wh),
    }
}

fn sampled_value_to_wh_v16(sampled_value: &rust_ocpp::v1_6::types::SampledValue) -> Option<i64> {
    use rust_ocpp::v1_6::types::{Measurand, UnitOfMeasure};

    let measurand = sampled_value
        .measurand
        .clone()
        .unwrap_or(Measurand::EnergyActiveImportRegister);
    if measurand != Measurand::EnergyActiveImportRegister {
        return None;
    }

    let raw = sampled_value.value.parse::<f64>().ok()?;
    let unit_factor = match sampled_value.unit.clone().unwrap_or(UnitOfMeasure::Wh) {
        UnitOfMeasure::KWh => 1_000.0,
        _ => 1.0,
    };
    Some((raw * unit_factor).round() as i64)
}

pub(crate) fn decision_from_id_token_info(
    id_token_info: &Option<IdTokenInfoType>,
) -> Option<BadgeAuthorizationDecision> {
    match id_token_info.as_ref()?.status {
        AuthorizationStatusEnumType::Accepted => Some(BadgeAuthorizationDecision::Accepted),
        AuthorizationStatusEnumType::Blocked => Some(BadgeAuthorizationDecision::Blocked),
        AuthorizationStatusEnumType::Invalid => Some(BadgeAuthorizationDecision::Invalid),
        _ => None,
    }
}

pub(crate) async fn handle_block_station_command(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
    blocked: bool,
) -> Result<(), String> {
    if !matches!(context.version, OcppVersion::V16) {
        return Err("blocco OCPP attivo solo per 1.6".to_string());
    }

    if blocked && session.active_transaction_id.is_some() {
        handle_remote_stop_transaction_command(context, session, sink, None, None).await?;
    }

    let change_response = send_ocpp_request_and_wait(
        context,
        session,
        sink,
        "ChangeAvailability",
        serde_json::to_value(rust_ocpp::v1_6::messages::change_availability::ChangeAvailabilityRequest {
            connector_id: 0,
            kind: if blocked {
                rust_ocpp::v1_6::types::AvailabilityType::Inoperative
            } else {
                rust_ocpp::v1_6::types::AvailabilityType::Operative
            },
        })
        .map_err(|err| err.to_string())?,
    )
    .await?;

    let change_status = change_response
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "ChangeAvailability.conf senza status".to_string())?;
    if change_status != "Accepted" && change_status != "Scheduled" {
        return Err(format!("ChangeAvailability rifiutato: {change_status}"));
    }

    Ok(())
}

pub(crate) async fn handle_remote_stop_transaction_command(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
    transaction_id: Option<i32>,
    transaction_ref: Option<&str>,
) -> Result<(), String> {
    match context.version {
        OcppVersion::V16 => {
            let transaction_id = transaction_id
                .or(session.active_transaction_id)
                .ok_or_else(|| "RemoteStopTransaction richiede transaction_id attivo".to_string())?;

            let stop_response = send_ocpp_request_and_wait(
                context,
                session,
                sink,
                "RemoteStopTransaction",
                serde_json::to_value(
                    rust_ocpp::v1_6::messages::remote_stop_transaction::RemoteStopTransactionRequest {
                        transaction_id,
                    },
                )
                .map_err(|err| err.to_string())?,
            )
            .await?;

            let stop_status = stop_response
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "RemoteStopTransaction.conf senza status".to_string())?;
            if stop_status != "Accepted" {
                return Err(format!("RemoteStopTransaction rifiutato: {stop_status}"));
            }
        }
        OcppVersion::V201 => {
            let transaction_ref = transaction_ref
                .or(session.active_transaction_id_v201.as_deref())
                .ok_or_else(|| "RequestStopTransaction richiede transaction_id attivo".to_string())?
                .to_string();

            let stop_response = send_ocpp_request_and_wait(
                context,
                session,
                sink,
                "RequestStopTransaction",
                serde_json::to_value(
                    rust_ocpp::v2_0_1::messages::request_stop_transaction::RequestStopTransactionRequest {
                        transaction_id: transaction_ref.clone(),
                    },
                )
                .map_err(|err| err.to_string())?,
            )
            .await?;

            let stop_status = stop_response
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "RequestStopTransaction.conf senza status".to_string())?;
            if stop_status != "Accepted" {
                return Err(format!("RequestStopTransaction rifiutato: {stop_status}"));
            }
        }
    }

    Ok(())
}

pub(crate) async fn handle_remote_start_transaction_command(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
    connector_id: u32,
    badge_code: &str,
) -> Result<(), String> {
    if !matches!(context.version, OcppVersion::V16) {
        return Err("remote start OCPP attivo solo per 1.6".to_string());
    }

    if connector_id == 0 {
        return Err("connector_id non valido".to_string());
    }

    if badge_code.trim().is_empty() {
        return Err("badge_code mancante".to_string());
    }

    if session.active_transaction_id.is_some() {
        return Err("transazione gia attiva sulla colonnina".to_string());
    }

    let start_response = send_ocpp_request_and_wait(
        context,
        session,
        sink,
        "RemoteStartTransaction",
        serde_json::to_value(
            rust_ocpp::v1_6::messages::remote_start_transaction::RemoteStartTransactionRequest {
                connector_id: Some(connector_id),
                id_tag: badge_code.to_string(),
                charging_profile: None,
            },
        )
        .map_err(|err| err.to_string())?,
    )
    .await?;

    let start_status = start_response
        .get("status")
        .and_then(Value::as_str)
        .ok_or_else(|| "RemoteStartTransaction.conf senza status".to_string())?;
    if start_status != "Accepted" {
        return Err(format!("RemoteStartTransaction rifiutato: {start_status}"));
    }

    Ok(())
}

pub(crate) async fn maybe_auto_remote_start_on_preparing(
    context: &ConnectionContext,
    session: &mut SessionState,
    db: &Database,
    sink: &mut OcppSink,
    connector_id: i32,
    next_status: &str,
) {
    if next_status != "Preparing" || connector_id <= 0 {
        return;
    }

    let connector = match db.connector_for_station(&context.station_id, connector_id).await {
        Ok(Some(connector)) => connector,
        Ok(None) => return,
        Err(err) => {
            eprintln!(
                "lookup connector auto remote start fallito per {}#{}: {}",
                context.station_id, connector_id, err
            );
            return;
        }
    };

    if connector.current_status.as_deref() == Some("Preparing") {
        return;
    }

    if !connector.active {
        return;
    }

    if connector.active_transaction_id.is_some() || connector.active_transaction_ref.is_some() {
        return;
    }

    let Some(badge_code) = connector.auto_remote_start_badge_code.as_deref() else {
        return;
    };

    let badge_code = badge_code.trim();
    if badge_code.is_empty() {
        return;
    }

    if !matches!(context.version, OcppVersion::V16) {
        eprintln!(
            "auto remote start ignorato per {}#{}: supportato solo su OCPP 1.6",
            context.station_id, connector_id
        );
        return;
    }

    if let Err(err) = handle_remote_start_transaction_command(
        context,
        session,
        sink,
        connector_id as u32,
        badge_code,
    )
    .await
    {
        eprintln!(
            "auto remote start fallito per {}#{} con badge {}: {}",
            context.station_id, connector_id, badge_code, err
        );
    }
}

pub(crate) async fn handle_get_configuration_command(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
) -> Result<StationConfigurationSnapshot, String> {
    if !matches!(context.version, OcppVersion::V16) {
        return Err("GetConfiguration OCPP attivo solo per 1.6".to_string());
    }

    let response = send_ocpp_request_and_wait(
        context,
        session,
        sink,
        "GetConfiguration",
        serde_json::to_value(
            rust_ocpp::v1_6::messages::get_configuration::GetConfigurationRequest { key: None },
        )
        .map_err(|err| err.to_string())?,
    )
    .await?;

    let parsed: rust_ocpp::v1_6::messages::get_configuration::GetConfigurationResponse =
        serde_json::from_value(response).map_err(|err| err.to_string())?;

    Ok(StationConfigurationSnapshot {
        configuration_keys: parsed
            .configuration_key
            .unwrap_or_default()
            .into_iter()
            .map(|entry| StationConfigurationEntry {
                key: entry.key,
                readonly: entry.readonly,
                value: entry.value,
            })
            .collect(),
        unknown_keys: parsed.unknown_key.unwrap_or_default(),
    })
}

pub(crate) async fn handle_set_connector_active_command(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
    connector_id: u32,
    evse_id: Option<i32>,
    active: bool,
) -> Result<(), String> {
    if connector_id == 0 {
        return Err("connector_id non valido".to_string());
    }

    match context.version {
        OcppVersion::V16 => {
            let change_response = send_ocpp_request_and_wait(
                context,
                session,
                sink,
                "ChangeAvailability",
                serde_json::to_value(
                    rust_ocpp::v1_6::messages::change_availability::ChangeAvailabilityRequest {
                        connector_id,
                        kind: if active {
                            rust_ocpp::v1_6::types::AvailabilityType::Operative
                        } else {
                            rust_ocpp::v1_6::types::AvailabilityType::Inoperative
                        },
                    },
                )
                .map_err(|err| err.to_string())?,
            )
            .await?;

            let change_status = change_response
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "ChangeAvailability.conf senza status".to_string())?;
            if change_status != "Accepted" && change_status != "Scheduled" {
                return Err(format!("ChangeAvailability rifiutato: {change_status}"));
            }
        }
        OcppVersion::V201 => {
            let evse_id = evse_id.ok_or_else(|| "evse_id mancante per OCPP 2.0.1".to_string())?;
            let change_response = send_ocpp_request_and_wait(
                context,
                session,
                sink,
                "ChangeAvailability",
                serde_json::to_value(
                    rust_ocpp::v2_0_1::messages::change_availability::ChangeAvailabilityRequest {
                        operational_status: if active {
                            rust_ocpp::v2_0_1::enumerations::operational_status_enum_type::OperationalStatusEnumType::Operative
                        } else {
                            rust_ocpp::v2_0_1::enumerations::operational_status_enum_type::OperationalStatusEnumType::Inoperative
                        },
                        evse: Some(rust_ocpp::v2_0_1::datatypes::evse_type::EVSEType {
                            id: evse_id,
                            connector_id: Some(connector_id as i32),
                        }),
                    },
                )
                .map_err(|err| err.to_string())?,
            )
            .await?;

            let change_status = change_response
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "ChangeAvailability.conf senza status".to_string())?;
            if change_status != "Accepted" && change_status != "Scheduled" {
                return Err(format!("ChangeAvailability rifiutato: {change_status}"));
            }
        }
    }

    Ok(())
}

pub(crate) async fn handle_unlock_connector_command(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
    connector_id: u32,
    evse_id: Option<i32>,
) -> Result<(), String> {
    if connector_id == 0 {
        return Err("UnlockConnector richiede un connector_id valido".to_string());
    }

    match context.version {
        OcppVersion::V16 => {
            let unlock_response = send_ocpp_request_and_wait(
                context,
                session,
                sink,
                "UnlockConnector",
                serde_json::to_value(
                    rust_ocpp::v1_6::messages::unlock_connector::UnlockConnectorRequest {
                        connector_id,
                    },
                )
                .map_err(|err| err.to_string())?,
            )
            .await?;

            let unlock_status = unlock_response
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "UnlockConnector.conf senza status".to_string())?;
            if unlock_status != "Unlocked" {
                return Err(format!("UnlockConnector rifiutato: {unlock_status}"));
            }
        }
        OcppVersion::V201 => {
            let evse_id = evse_id.ok_or_else(|| "evse_id mancante per OCPP 2.0.1".to_string())?;
            let unlock_response = send_ocpp_request_and_wait(
                context,
                session,
                sink,
                "UnlockConnector",
                serde_json::to_value(
                    rust_ocpp::v2_0_1::messages::unlock_connector::UnlockConnectorRequest {
                        evse_id,
                        connector_id: connector_id as i32,
                    },
                )
                .map_err(|err| err.to_string())?,
            )
            .await?;

            let unlock_status = unlock_response
                .get("status")
                .and_then(Value::as_str)
                .ok_or_else(|| "UnlockConnector.conf senza status".to_string())?;
            if unlock_status != "Unlocked" {
                return Err(format!("UnlockConnector rifiutato: {unlock_status}"));
            }
        }
    }

    Ok(())
}

async fn send_ocpp_request_and_wait(
    context: &ConnectionContext,
    session: &mut SessionState,
    sink: &mut OcppSink,
    action: &'static str,
    payload: Value,
) -> Result<Value, String> {
    prune_ignored_responses(session);
    let unique_id = Uuid::new_v4().simple().to_string();
    let (tx, rx) = oneshot::channel();
    session.pending_requests.insert(unique_id.clone(), tx);

    let request_text = json!([2, unique_id, action, payload]).to_string();
    log_ocpp_packet(
        context,
        "outbound",
        Some("call"),
        Some(unique_id.as_str()),
        Some(action),
        &request_text,
    );
    if let Err(err) = sink.send(Message::Text(request_text)).await {
        session.pending_requests.remove(&unique_id);
        return Err(err.to_string());
    }

    match tokio::time::timeout(Duration::from_secs(30), rx).await {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(_)) => {
            session.pending_requests.remove(&unique_id);
            Err(format!("{action}.conf perso"))
        }
        Err(_) => {
            session.pending_requests.remove(&unique_id);
            session
                .ignored_responses
                .insert(unique_id, Instant::now() + Duration::from_secs(60));
            Err(format!("timeout aspettando {action}.conf"))
        }
    }
}

fn prune_ignored_responses(session: &mut SessionState) {
    let now = Instant::now();
    session
        .ignored_responses
        .retain(|_, expires_at| *expires_at > now);
}

pub(crate) async fn save_ocpp_event(
    db: &Database,
    context: &ConnectionContext,
    direction: &'static str,
    message_type: Option<i64>,
    unique_id: Option<&str>,
    action: Option<&str>,
    raw_text: &str,
    payload: Option<&Value>,
    parse_status: &'static str,
    error: Option<String>,
) {
    db.record_ocpp_message(OcppMessageRecord {
        station_id: &context.station_id,
        ocpp_version: context.version.as_str(),
        peer_addr: &context.peer_addr,
        direction,
        message_type,
        unique_id,
        action,
        raw_text,
        payload,
        parse_status,
        error: error.as_deref(),
        received_at: Utc::now(),
    })
    .await;
}

pub(crate) async fn record_parse_error(
    db: &Database,
    context: &ConnectionContext,
    call: &OcppCall<'_>,
    err: &serde_json::Error,
) {
    log_unparsed_ocpp_frame(context, "parse_error", Some(&err.to_string()), call.text);
    save_ocpp_event(
        db,
        context,
        "inbound",
        Some(call.message_type),
        Some(call.unique_id),
        Some(call.action),
        call.text,
        Some(call.payload),
        "parse_error",
        Some(err.to_string()),
    )
    .await;
}

pub(crate) fn log_unparsed_ocpp_frame(
    context: &ConnectionContext,
    parse_status: &str,
    error: Option<&str>,
    raw_text: &str,
) {
    match error {
        Some(error) => eprintln!(
            "OCPP non parsato da {}: status={} error={} raw={}",
            context.station_id, parse_status, error, raw_text
        ),
        None => eprintln!(
            "OCPP non parsato da {}: status={} raw={}",
            context.station_id, parse_status, raw_text
        ),
    }
}

pub(crate) fn log_ocpp_packet(
    context: &ConnectionContext,
    direction: &'static str,
    kind: Option<&str>,
    unique_id: Option<&str>,
    action: Option<&str>,
    raw_text: &str,
) {
    eprintln!(
        "OCPP {} da {}: kind={} uid={} action={} raw={}",
        direction,
        context.station_id,
        kind.unwrap_or("-"),
        unique_id.unwrap_or("-"),
        action.unwrap_or("-"),
        raw_text
    );
}

fn is_heartbeat_action(action: &str) -> bool {
    action == "Heartbeat"
}

fn is_heartbeat_response_text(raw_text: &str) -> bool {
    let Ok(frame) = serde_json::from_str::<Value>(raw_text) else {
        return false;
    };
    let Some(items) = frame.as_array() else {
        return false;
    };
    if items.len() != 3 || items.first().and_then(Value::as_i64) != Some(3) {
        return false;
    }
    let Some(payload) = items.get(2).and_then(Value::as_object) else {
        return false;
    };
    payload.len() == 1 && payload.contains_key("currentTime")
}

pub(crate) fn parse_station_id(path: &str) -> Option<&str> {
    path.strip_prefix("/ocpp/")
        .and_then(|rest| rest.split('/').next())
        .filter(|station_id| !station_id.is_empty())
}

pub(crate) fn pick_ocpp_protocol(headers: &HeaderMap) -> Option<OcppVersion> {
    let requested = headers.get(SEC_WEBSOCKET_PROTOCOL)?.to_str().ok()?;

    for value in requested.split(',').map(|value| value.trim()) {
        match value {
            "ocpp1.6" => return Some(OcppVersion::V16),
            "ocpp2.0.1" => return Some(OcppVersion::V201),
            _ => {}
        }
    }

    None
}
