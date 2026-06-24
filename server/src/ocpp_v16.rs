use chrono::Utc;
use rust_ocpp::v1_6::{
    messages::authorize::{
        AuthorizeRequest as AuthorizeRequestV16, AuthorizeResponse as AuthorizeResponseV16,
    },
    messages::boot_notification::{
        BootNotificationRequest as BootNotificationRequestV16,
        BootNotificationResponse as BootNotificationResponseV16,
    },
    messages::heart_beat::{
        HeartbeatRequest as HeartbeatRequestV16, HeartbeatResponse as HeartbeatResponseV16,
    },
    messages::meter_values::{
        MeterValuesRequest as MeterValuesRequestV16, MeterValuesResponse as MeterValuesResponseV16,
    },
    messages::start_transaction::{
        StartTransactionRequest as StartTransactionRequestV16,
        StartTransactionResponse as StartTransactionResponseV16,
    },
    messages::status_notification::{
        StatusNotificationRequest as StatusNotificationRequestV16,
        StatusNotificationResponse as StatusNotificationResponseV16,
    },
    messages::stop_transaction::{
        StopTransactionRequest as StopTransactionRequestV16,
        StopTransactionResponse as StopTransactionResponseV16,
    },
    types::{AuthorizationStatus, IdTagInfo, RegistrationStatus},
};
use serde_json::{Value, json};
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::{
    badges::BadgeId,
    db::Database,
    ocpp_runtime::{
        BadgeAuthorizationDecision, ConnectionContext, OcppCall, SessionState, authorize_badge,
        OcppSink, charging_measurements_from_meter_values_v16, log_unparsed_ocpp_frame,
        maybe_auto_remote_start_on_preparing, record_parse_error, resolve_authorized_badge,
        save_ocpp_event,
        synthetic_v16_energy_measurement, transaction_energy_from_meter_values_v16,
    },
    users::UserId,
};

pub(crate) async fn handle_v16_call(
    context: &ConnectionContext,
    session: &mut SessionState,
    db: &Database,
    sink: &mut OcppSink,
    call: &OcppCall<'_>,
) -> Result<Option<Message>, Error> {
    match call.action {
        "Authorize" => {
            let request: AuthorizeRequestV16 = match serde_json::from_value(call.payload.clone()) {
                Ok(request) => request,
                Err(err) => {
                    record_parse_error(db, context, call, &err).await;
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        err,
                    )));
                }
            };
            let id_tag = request.id_tag.clone();
            let decision = match authorize_badge(db, &id_tag).await {
                Ok(decision) => decision,
                Err(err) => {
                    eprintln!("autorizzazione badge fallita per {}: {}", id_tag, err);
                    return Err(Error::Io(std::io::Error::other(err.to_string())));
                }
            };
            println!(
                "Authorize 1.6 da {}: id_tag={} status={:?}",
                context.station_id, id_tag, decision
            );
            session.active_badge = if decision == BadgeAuthorizationDecision::Accepted {
                match resolve_authorized_badge(db, &id_tag).await {
                    Ok(badge) => badge,
                    Err(err) => {
                        eprintln!("lookup badge fallita per {}: {}", id_tag, err);
                        None
                    }
                }
            } else {
                None
            };

            let response = AuthorizeResponseV16 {
                id_tag_info: IdTagInfo {
                    expiry_date: None,
                    parent_id_tag: None,
                    status: decision.as_v16_status(),
                },
            };

            let reply_text = json!([3, call.unique_id, response]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        "Heartbeat" => {
            let _request: HeartbeatRequestV16 = match serde_json::from_value(call.payload.clone()) {
                Ok(request) => request,
                Err(err) => {
                    record_parse_error(db, context, call, &err).await;
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        err,
                    )));
                }
            };
            let response = HeartbeatResponseV16 {
                current_time: Utc::now(),
            };
            let reply_text = json!([3, call.unique_id, response]).to_string();
            Ok(Some(Message::Text(reply_text)))
        }
        "StartTransaction" => {
            let request: StartTransactionRequestV16 =
                match serde_json::from_value(call.payload.clone()) {
                    Ok(request) => request,
                    Err(err) => {
                        record_parse_error(db, context, call, &err).await;
                        return Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            err,
                        )));
                    }
                };
            let id_tag = request.id_tag.clone();
            let decision = match authorize_badge(db, &id_tag).await {
                Ok(decision) => decision,
                Err(err) => {
                    eprintln!("autorizzazione badge fallita per {}: {}", id_tag, err);
                    return Err(Error::Io(std::io::Error::other(err.to_string())));
                }
            };
            let transaction_id = match db.next_ocpp_transaction_id().await {
                Ok(transaction_id) => transaction_id,
                Err(err) => {
                    eprintln!(
                        "allocazione transaction_id fallita per {}: {}",
                        context.station_id, err
                    );
                    return Err(Error::Io(std::io::Error::other(err.to_string())));
                }
            };
            let badge = match resolve_authorized_badge(db, &id_tag).await {
                Ok(badge) => badge,
                Err(err) => {
                    eprintln!("lookup badge fallita per {}: {}", id_tag, err);
                    None
                }
            };
            if decision == BadgeAuthorizationDecision::Accepted {
                session.active_badge = badge.clone();
                session.active_transaction_id = Some(transaction_id);
                session.active_connector_id = Some(request.connector_id);
                if let Err(err) = db
                    .update_station_connector(&context.station_id, Some(request.connector_id as i32), None)
                    .await
                {
                    eprintln!(
                        "postgres update_station_connector fallito per {}: {}",
                        context.station_id, err
                    );
                }
                if let Err(err) = db
                    .set_connector_transaction(
                        &context.station_id,
                        request.connector_id as i32,
                        None,
                        Some(transaction_id),
                        None,
                    )
                    .await
                {
                    eprintln!(
                        "postgres set_connector_transaction fallito per {}: {}",
                        context.station_id, err
                    );
                }
            }
            if let Err(err) = db
                .create_transaction(
                    &context.station_id,
                    context.version.as_str(),
                    Some(transaction_id),
                    None,
                    Some(request.connector_id as i32),
                    None,
                    badge.as_ref().map(|badge| UserId(badge.user_id)),
                    badge.as_ref().map(|badge| BadgeId(badge.badge_id)),
                    Some(id_tag.as_str()),
                    decision.transaction_status(),
                    request.timestamp,
                    if decision == BadgeAuthorizationDecision::Accepted {
                        None
                    } else {
                        Some(request.timestamp)
                    },
                    Some(request.meter_start as i64),
                    if decision == BadgeAuthorizationDecision::Accepted {
                        None
                    } else {
                        Some(decision.transaction_status())
                    },
                )
                .await
            {
                eprintln!(
                    "postgres create_transaction fallito per {}: {}",
                    context.station_id, err
                );
            }
            db.record_charging_measurements(&[synthetic_v16_energy_measurement(
                context,
                "StartTransaction",
                transaction_id,
                Some(request.connector_id as i32),
                request.timestamp,
                request.meter_start as i64,
                "Transaction.Begin",
            )])
            .await;
            println!(
                "StartTransaction 1.6 da {}: connector={} id_tag={} meter_start={} status={:?}",
                context.station_id, request.connector_id, id_tag, request.meter_start, decision
            );

            let response = StartTransactionResponseV16 {
                id_tag_info: IdTagInfo {
                    expiry_date: None,
                    parent_id_tag: None,
                    status: decision.as_v16_status(),
                },
                transaction_id,
            };

            let reply_text = json!([3, call.unique_id, response]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        "StopTransaction" => {
            let request: StopTransactionRequestV16 =
                match serde_json::from_value(call.payload.clone()) {
                    Ok(request) => request,
                    Err(err) => {
                        record_parse_error(db, context, call, &err).await;
                        return Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            err,
                        )));
                    }
                };
            println!(
                "StopTransaction 1.6 da {}: tx={} meter_stop={}",
                context.station_id, request.transaction_id, request.meter_stop
            );
            if let Err(err) = db
                .finish_transaction_by_id(
                    &context.station_id,
                    request.transaction_id,
                    request.timestamp,
                    Some(request.meter_stop as i64),
                    request.reason.as_ref().map(|reason| format!("{reason:?}")).as_deref(),
                    request.id_tag.as_deref(),
                )
                .await
            {
                eprintln!(
                    "postgres finish_transaction_by_id fallito per {}: {}",
                    context.station_id, err
                );
            }
            db.record_charging_measurements(&[synthetic_v16_energy_measurement(
                context,
                "StopTransaction",
                request.transaction_id,
                session.active_connector_id.map(|value| value as i32),
                request.timestamp,
                request.meter_stop as i64,
                "Transaction.End",
            )])
            .await;
            session.active_transaction_id = None;
            session.active_badge = None;
            if let Some(connector_id) = session.active_connector_id.take() {
                if let Err(err) = db
                    .clear_connector_transaction(&context.station_id, connector_id as i32)
                    .await
                {
                    eprintln!(
                        "postgres clear_connector_transaction fallito per {}: {}",
                        context.station_id, err
                    );
                }
            } else if let Err(err) = db
                .clear_connector_transaction_by_ocpp_id(&context.station_id, request.transaction_id)
                .await
            {
                eprintln!(
                    "postgres clear_connector_transaction_by_ocpp_id fallito per {}: {}",
                    context.station_id, err
                );
            }
            if let Err(err) = db.update_station_connector(&context.station_id, None, None).await {
                eprintln!(
                    "postgres clear current_connector fallito per {}: {}",
                    context.station_id, err
                );
            }

            let response = StopTransactionResponseV16 {
                id_tag_info: Some(IdTagInfo {
                    expiry_date: None,
                    parent_id_tag: None,
                    status: AuthorizationStatus::Accepted,
                }),
            };

            let reply_text = json!([3, call.unique_id, response]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        "MeterValues" => {
            let request: MeterValuesRequestV16 = match serde_json::from_value(call.payload.clone()) {
                Ok(request) => request,
                Err(err) => {
                    record_parse_error(db, context, call, &err).await;
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        err,
                    )));
                }
            };
            println!(
                "MeterValues 1.6 da {}: connector={} samples={}",
                context.station_id,
                request.connector_id,
                request.meter_value.len()
            );
            let measurements = charging_measurements_from_meter_values_v16(
                context,
                "MeterValues",
                request.transaction_id.or(session.active_transaction_id),
                Some(request.connector_id as i32),
                &request.meter_value,
            );
            db.record_charging_measurements(&measurements).await;
            if let Some(transaction_id) = session.active_transaction_id {
                let latest_wh = transaction_energy_from_meter_values_v16(&request.meter_value);
                if let Err(err) = db
                    .update_transaction_progress_by_id(
                        &context.station_id,
                        transaction_id,
                        latest_wh,
                        Some(request.connector_id as i32),
                        None,
                    )
                    .await
                {
                    eprintln!(
                        "postgres update_transaction_progress_by_id fallito per {}: {}",
                        context.station_id, err
                    );
                }
            }

            let response = MeterValuesResponseV16 {};
            let reply_text = json!([3, call.unique_id, response]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        "StatusNotification" => {
            let request: StatusNotificationRequestV16 =
                match serde_json::from_value(call.payload.clone()) {
                    Ok(request) => request,
                    Err(err) => {
                        record_parse_error(db, context, call, &err).await;
                        return Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            err,
                        )));
                    }
                };
            println!(
                "StatusNotification 1.6 da {}: connector={} status={:?} error={:?}",
                context.station_id, request.connector_id, request.status, request.error_code
            );
            let status_text = format!("{:?}", request.status);
            let error_text = format!("{:?}", request.error_code);

            if request.connector_id > 0 {
                maybe_auto_remote_start_on_preparing(
                    context,
                    session,
                    db,
                    sink,
                    request.connector_id as i32,
                    &status_text,
                )
                .await;
            }

            if request.connector_id > 0
                && let Err(err) = db
                    .upsert_connector_status(
                        &context.station_id,
                        request.connector_id as i32,
                        None,
                        Some(status_text.clone()),
                        Some(error_text.clone()),
                        Some(request.timestamp.unwrap_or_else(Utc::now)),
                    )
                    .await
            {
                eprintln!(
                    "postgres upsert_connector_status fallito per {}: {}",
                    context.station_id, err
                );
            }

            if let Err(err) = db
                .update_station_status(
                    &context.station_id,
                    Some(status_text),
                    Some(error_text),
                    if request.connector_id > 0 {
                        Some(request.connector_id as i32)
                    } else {
                        None
                    },
                    None,
                    Some(request.timestamp.unwrap_or_else(Utc::now)),
                )
                .await
            {
                eprintln!(
                    "postgres update_station_status fallito per {}: {}",
                    context.station_id, err
                );
            }

            let response = StatusNotificationResponseV16 {};
            let reply_text = json!([3, call.unique_id, response]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        "SecurityEventNotification" => {
            let type_value = call.payload.get("type").and_then(Value::as_str).unwrap_or("");
            let timestamp = call
                .payload
                .get("timestamp")
                .and_then(Value::as_str)
                .unwrap_or("");
            let tech_info = call.payload.get("techInfo").and_then(Value::as_str);

            println!(
                "SecurityEventNotification 1.6 da {}: type={} timestamp={} tech_info={:?}",
                context.station_id, type_value, timestamp, tech_info
            );

            let reply_text = json!([3, call.unique_id, {}]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        "BootNotification" => {
            let request: BootNotificationRequestV16 =
                match serde_json::from_value(call.payload.clone()) {
                    Ok(request) => request,
                    Err(err) => {
                        record_parse_error(db, context, call, &err).await;
                        return Err(Error::Io(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            err,
                        )));
                    }
                };
            println!(
                "BootNotification 1.6 da {}: vendor={} model={}",
                context.station_id, request.charge_point_vendor, request.charge_point_model
            );

            if let Err(err) = db.record_boot_notification(&context.station_id).await {
                eprintln!(
                    "postgres record_boot_notification fallito per {}: {}",
                    context.station_id, err
                );
            }

            let response = BootNotificationResponseV16 {
                current_time: Utc::now(),
                interval: 30,
                status: RegistrationStatus::Accepted,
            };

            let reply_text = json!([3, call.unique_id, response]).to_string();
            save_ocpp_event(
                db,
                context,
                "outbound",
                Some(3),
                Some(call.unique_id),
                Some(call.action),
                &reply_text,
                None,
                "response",
                None,
            )
            .await;
            Ok(Some(Message::Text(reply_text)))
        }
        _ => {
            println!(
                "msg OCPP non gestito da {}: action={} payload={}",
                context.station_id, call.action, call.payload
            );
            log_unparsed_ocpp_frame(context, "unhandled_action", None, call.text);
            save_ocpp_event(
                db,
                context,
                "inbound",
                Some(call.message_type),
                Some(call.unique_id),
                Some(call.action),
                call.text,
                Some(call.payload),
                "unhandled_action",
                None,
            )
            .await;
            Ok(None)
        }
    }
}
