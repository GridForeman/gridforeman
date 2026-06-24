use chrono::Utc;
use rust_ocpp::v2_0_1::{
    datatypes::id_token_info_type::IdTokenInfoType,
    enumerations::{
        registration_status_enum_type::RegistrationStatusEnumType,
        transaction_event_enum_type::TransactionEventEnumType,
    },
    messages::authorize::{
        AuthorizeRequest as AuthorizeRequestV201, AuthorizeResponse as AuthorizeResponseV201,
    },
    messages::boot_notification::{
        BootNotificationRequest as BootNotificationRequestV201,
        BootNotificationResponse as BootNotificationResponseV201,
    },
    messages::heartbeat::{
        HeartbeatRequest as HeartbeatRequestV201, HeartbeatResponse as HeartbeatResponseV201,
    },
    messages::meter_values::{
        MeterValuesRequest as MeterValuesRequestV201, MeterValuesResponse as MeterValuesResponseV201,
    },
    messages::status_notification::{
        StatusNotificationRequest as StatusNotificationRequestV201,
        StatusNotificationResponse as StatusNotificationResponseV201,
    },
    messages::transaction_event::{
        TransactionEventRequest as TransactionEventRequestV201,
        TransactionEventResponse as TransactionEventResponseV201,
    },
};
use serde_json::json;
use tokio_tungstenite::tungstenite::{Error, Message};

use crate::{
    badges::BadgeId,
    db::Database,
    ocpp_runtime::{
        BadgeAuthorizationDecision, ConnectionContext, OcppCall, SessionState, authorize_badge,
        OcppSink, charging_measurements_from_meter_values_v201, decision_from_id_token_info,
        log_unparsed_ocpp_frame, maybe_auto_remote_start_on_preparing, record_parse_error,
        resolve_authorized_badge, save_ocpp_event, transaction_energy_from_meter_values,
    },
    users::UserId,
};

pub(crate) async fn handle_v201_call(
    context: &ConnectionContext,
    session: &mut SessionState,
    db: &Database,
    sink: &mut OcppSink,
    call: &OcppCall<'_>,
) -> Result<Option<Message>, Error> {
    match call.action {
        "Heartbeat" => {
            let _request: HeartbeatRequestV201 = match serde_json::from_value(call.payload.clone()) {
                Ok(request) => request,
                Err(err) => {
                    record_parse_error(db, context, call, &err).await;
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        err,
                    )));
                }
            };
            let response = HeartbeatResponseV201 {
                current_time: Utc::now(),
            };
            let reply_text = json!([3, call.unique_id, response]).to_string();
            Ok(Some(Message::Text(reply_text)))
        }
        "Authorize" => {
            let request: AuthorizeRequestV201 = match serde_json::from_value(call.payload.clone()) {
                Ok(request) => request,
                Err(err) => {
                    record_parse_error(db, context, call, &err).await;
                    return Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        err,
                    )));
                }
            };
            let badge_code = request.id_token.id_token.clone();
            let decision = match authorize_badge(db, &badge_code).await {
                Ok(decision) => decision,
                Err(err) => {
                    eprintln!("autorizzazione badge fallita per {}: {}", badge_code, err);
                    return Err(Error::Io(std::io::Error::other(err.to_string())));
                }
            };
            println!(
                "Authorize 2.0.1 da {}: id_token={:?} status={:?}",
                context.station_id, request.id_token, decision
            );
            session.active_badge = if decision == BadgeAuthorizationDecision::Accepted {
                match resolve_authorized_badge(db, &badge_code).await {
                    Ok(badge) => badge,
                    Err(err) => {
                        eprintln!("lookup badge fallita per {}: {}", badge_code, err);
                        None
                    }
                }
            } else {
                None
            };

            let response = AuthorizeResponseV201 {
                certificate_status: None,
                id_token_info: IdTokenInfoType {
                    status: decision.as_v201_status(),
                    cache_expiry_date_time: None,
                    charging_priority: None,
                    language1: None,
                    evse_id: None,
                    language2: None,
                    group_id_token: None,
                    personal_message: None,
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
        "TransactionEvent" => {
            let request: TransactionEventRequestV201 =
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
                "TransactionEvent 2.0.1 da {}: event={:?} trigger={:?} tx={}",
                context.station_id,
                request.event_type,
                request.trigger_reason,
                request.transaction_info.transaction_id
            );
            let current_connector_id = request.evse.as_ref().and_then(|evse| evse.connector_id);
            let current_meter_wh = transaction_energy_from_meter_values(request.meter_value.as_ref());
            if let Some(meter_values) = request.meter_value.as_ref() {
                let measurements = charging_measurements_from_meter_values_v201(
                    context,
                    "TransactionEvent",
                    Some(request.transaction_info.transaction_id.as_str()),
                    request.evse.as_ref().map(|evse| evse.id),
                    current_connector_id,
                    meter_values,
                );
                db.record_charging_measurements(&measurements).await;
            }

            let id_token_info = match request.id_token.as_ref() {
                Some(id_token) => {
                    let decision = match authorize_badge(db, &id_token.id_token).await {
                        Ok(decision) => decision,
                        Err(err) => {
                            eprintln!(
                                "autorizzazione badge fallita per {}: {}",
                                id_token.id_token, err
                            );
                            return Err(Error::Io(std::io::Error::other(err.to_string())));
                        }
                    };

                    Some(IdTokenInfoType {
                        status: decision.as_v201_status(),
                        cache_expiry_date_time: None,
                        charging_priority: None,
                        language1: None,
                        evse_id: None,
                        language2: None,
                        group_id_token: None,
                        personal_message: None,
                    })
                }
                None => None,
            };
            let badge = match request.id_token.as_ref() {
                Some(id_token) => match resolve_authorized_badge(db, &id_token.id_token).await {
                    Ok(badge) => badge,
                    Err(err) => {
                        eprintln!("lookup badge fallita per {}: {}", id_token.id_token, err);
                        None
                    }
                },
                None => session.active_badge.clone(),
            };

            match request.event_type {
                TransactionEventEnumType::Started => {
                    session.active_badge = badge.clone();
                    session.active_transaction_id_v201 =
                        Some(request.transaction_info.transaction_id.clone());
                    session.active_connector_id_v201 = current_connector_id;
                    if decision_from_id_token_info(&id_token_info)
                        == Some(BadgeAuthorizationDecision::Accepted)
                        && let Some(connector_id) = current_connector_id
                        && let Err(err) = db
                            .set_connector_transaction(
                                &context.station_id,
                                connector_id,
                                request.evse.as_ref().map(|evse| evse.id),
                                None,
                                Some(request.transaction_info.transaction_id.clone()),
                            )
                            .await
                    {
                        eprintln!(
                            "postgres set_connector_transaction fallito per {}: {}",
                            context.station_id, err
                        );
                    }
                    let tx_status = decision_from_id_token_info(&id_token_info)
                        .unwrap_or(BadgeAuthorizationDecision::Accepted)
                        .transaction_status();
                    if let Err(err) = db
                        .create_transaction(
                            &context.station_id,
                            context.version.as_str(),
                            None,
                            Some(request.transaction_info.transaction_id.as_str()),
                            current_connector_id,
                            request.evse.as_ref().map(|evse| evse.id),
                            badge.as_ref().map(|badge| UserId(badge.user_id)),
                            badge.as_ref().map(|badge| BadgeId(badge.badge_id)),
                            badge.as_ref().map(|badge| badge.badge_code.as_str()),
                            tx_status,
                            request.timestamp,
                            if tx_status == "in_progress" {
                                None
                            } else {
                                Some(request.timestamp)
                            },
                            current_meter_wh,
                            if tx_status == "in_progress" {
                                None
                            } else {
                                Some(tx_status)
                            },
                        )
                        .await
                    {
                        eprintln!(
                            "postgres create_transaction fallito per {}: {}",
                            context.station_id, err
                        );
                    }
                }
                TransactionEventEnumType::Ended => {
                    if let Err(err) = db
                        .finish_transaction_by_ref(
                            &context.station_id,
                            &request.transaction_info.transaction_id,
                            request.timestamp,
                            current_meter_wh,
                            request
                                .transaction_info
                                .stopped_reason
                                .as_ref()
                                .map(|reason| format!("{reason:?}"))
                                .as_deref(),
                            "completed",
                        )
                        .await
                    {
                        eprintln!(
                            "postgres finish_transaction_by_ref fallito per {}: {}",
                            context.station_id, err
                        );
                    }
                    session.active_transaction_id_v201 = None;
                    session.active_badge = None;
                    if let Some(connector_id) = session.active_connector_id_v201.take() {
                        if let Err(err) = db
                            .clear_connector_transaction(&context.station_id, connector_id)
                            .await
                        {
                            eprintln!(
                                "postgres clear_connector_transaction fallito per {}: {}",
                                context.station_id, err
                            );
                        }
                    } else if let Err(err) = db
                        .clear_connector_transaction_by_ocpp_ref(
                            &context.station_id,
                            &request.transaction_info.transaction_id,
                        )
                        .await
                    {
                        eprintln!(
                            "postgres clear_connector_transaction_by_ocpp_ref fallito per {}: {}",
                            context.station_id, err
                        );
                    }
                }
                TransactionEventEnumType::Updated => {
                    if let Err(err) = db
                        .update_transaction_progress_by_ref(
                            &context.station_id,
                            &request.transaction_info.transaction_id,
                            current_meter_wh,
                            current_connector_id,
                            request.evse.as_ref().map(|evse| evse.id),
                            "in_progress",
                        )
                        .await
                    {
                        eprintln!(
                            "postgres update_transaction_progress_by_ref fallito per {}: {}",
                            context.station_id, err
                        );
                    }
                }
            }

            let response = TransactionEventResponseV201 {
                id_token_info,
                ..Default::default()
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
            let request: MeterValuesRequestV201 = match serde_json::from_value(call.payload.clone()) {
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
                "MeterValues 2.0.1 da {}: evse={} samples={}",
                context.station_id,
                request.evse_id,
                request.meter_value.len()
            );
            let measurements = charging_measurements_from_meter_values_v201(
                context,
                "MeterValues",
                session.active_transaction_id_v201.as_deref(),
                Some(request.evse_id),
                session.active_connector_id_v201,
                &request.meter_value,
            );
            db.record_charging_measurements(&measurements).await;

            let response = MeterValuesResponseV201 {};
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
            let request: StatusNotificationRequestV201 =
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
                "StatusNotification 2.0.1 da {}: evse={} connector={} status={:?}",
                context.station_id, request.evse_id, request.connector_id, request.connector_status
            );
            let status_text = format!("{:?}", request.connector_status);

            if request.connector_id > 0 {
                maybe_auto_remote_start_on_preparing(
                    context,
                    session,
                    db,
                    sink,
                    request.connector_id,
                    &status_text,
                )
                .await;
            }

            if request.connector_id > 0
                && let Err(err) = db
                    .upsert_connector_status(
                        &context.station_id,
                        request.connector_id,
                        Some(request.evse_id),
                        Some(status_text.clone()),
                        None,
                        Some(request.timestamp),
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
                    None,
                    Some(request.connector_id),
                    Some(request.evse_id),
                    Some(request.timestamp),
                )
                .await
            {
                eprintln!(
                    "postgres update_station_status fallito per {}: {}",
                    context.station_id, err
                );
            }

            let response = StatusNotificationResponseV201 {};
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
        "BootNotification" => {
            let request: BootNotificationRequestV201 =
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
                "BootNotification 2.0.1 da {}: reason={:?}",
                context.station_id, request.reason
            );

            if let Err(err) = db.record_boot_notification(&context.station_id).await {
                eprintln!(
                    "postgres record_boot_notification fallito per {}: {}",
                    context.station_id, err
                );
            }

            let response = BootNotificationResponseV201 {
                current_time: Utc::now(),
                interval: 30,
                status: RegistrationStatusEnumType::Accepted,
                status_info: None,
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
