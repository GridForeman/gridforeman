use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use serde::Deserialize;

use crate::{
    app_state::{ConnectionRegistry, StationCommand, StationConfigurationSnapshot},
    badges::{BadgeId, NewBadge},
    db::ChargingTransaction,
    db::ConnectorSummary,
    db::Database,
    db::EnergyMeter,
    db::EnergyMeterStatusView,
    db::StationLocation,
    db::StationSummary,
    energy_meter_catalog::EnergyMeterCatalog,
    greptime::{EnergyMeterMeasurementRow, OcppEventRow},
    realtime::{self, RealtimeNotifier, RealtimeState},
    site_config::{SiteConfigSnapshot, SiteEnergyMeter},
    users::{NewUser, UserId},
};

#[derive(Clone)]
pub struct ApiState {
    pub db: Database,
    pub energy_meter_catalog: EnergyMeterCatalog,
    pub connections: ConnectionRegistry,
    pub notifier: RealtimeNotifier,
}

pub async fn run_api_server(
    db: Database,
    energy_meter_catalog: EnergyMeterCatalog,
    connections: ConnectionRegistry,
    notifier: RealtimeNotifier,
) -> Result<(), std::io::Error> {
    let realtime_state = RealtimeState {
        db: db.clone(),
        notifier: notifier.clone(),
    };
    let state = ApiState {
        db,
        energy_meter_catalog,
        connections,
        notifier,
    };
    let app = Router::new()
        .route("/api/stations", get(list_stations))
        .route("/api/stations/{station_id}", get(get_station))
        .route("/api/stations/{station_id}/status", get(get_station_status))
        .route(
            "/api/stations/{station_id}/blocked",
            patch(set_station_blocked),
        )
        .route(
            "/api/stations/{station_id}/connectors",
            get(list_station_connectors),
        )
        .route(
            "/api/stations/{station_id}/connectors/{connector_id}/active",
            patch(set_connector_active),
        )
        .route(
            "/api/stations/{station_id}/connectors/{connector_id}/auto-remote-start-badge",
            patch(set_connector_auto_remote_start_badge),
        )
        .route(
            "/api/stations/{station_id}/connectors/{connector_id}/remote-start",
            patch(remote_start_connector_transaction),
        )
        .route(
            "/api/stations/{station_id}/connectors/{connector_id}/remote-stop",
            patch(remote_stop_connector_transaction),
        )
        .route(
            "/api/stations/{station_id}/connectors/{connector_id}/unlock",
            patch(unlock_connector),
        )
        .route(
            "/api/stations/{station_id}/configuration",
            post(get_station_configuration),
        )
        .route(
            "/api/stations/{station_id}/location",
            patch(update_station_location),
        )
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/{user_id}", patch(update_user))
        .route("/api/users/{user_id}/active", patch(set_user_active))
        .route("/api/badges", get(list_badges).post(create_badge))
        .route("/api/badges/{badge_id}", patch(update_badge))
        .route("/api/badges/{badge_id}/active", patch(set_badge_active))
        .route("/api/energy-meters", get(list_energy_meters).post(create_energy_meter))
        .route("/api/energy-meters/status", get(list_energy_meter_statuses))
        .route("/api/energy-meters/{meter_id}", patch(update_energy_meter).delete(delete_energy_meter))
        .route("/api/energy-meters/{meter_id}/readings", get(list_energy_meter_readings))
        .route(
            "/api/site-config",
            get(get_site_config).put(update_site_config),
        )
        .route("/api/energy-meter-catalog", get(get_energy_meter_catalog))
        .route("/api/events", get(list_events))
        .route("/api/transactions", get(list_transactions))
        .route(
            "/api/state/ws",
            get(realtime::websocket).with_state(realtime_state),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:9001").await?;
    println!("API server ascolta su http://0.0.0.0:9001/api");
    axum::serve(listener, app).await
}

async fn list_stations(
    State(state): State<ApiState>,
) -> Result<Json<Vec<StationSummary>>, (StatusCode, String)> {
    state
        .db
        .list_stations()
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn get_station(
    State(state): State<ApiState>,
    Path(station_id): Path<String>,
) -> Result<Json<StationSummary>, (StatusCode, String)> {
    match state
        .db
        .get_station(&station_id)
        .await
        .map_err(internal_error)?
    {
        Some(station) => Ok(Json(station)),
        None => Err((StatusCode::NOT_FOUND, "station not found".to_string())),
    }
}

async fn get_station_status(
    State(state): State<ApiState>,
    Path(station_id): Path<String>,
) -> Result<Json<StationSummary>, (StatusCode, String)> {
    match state
        .db
        .get_station(&station_id)
        .await
        .map_err(internal_error)?
    {
        Some(station) => Ok(Json(station)),
        None => Err((StatusCode::NOT_FOUND, "station not found".to_string())),
    }
}

async fn list_station_connectors(
    State(state): State<ApiState>,
    Path(station_id): Path<String>,
) -> Result<Json<Vec<ConnectorSummary>>, (StatusCode, String)> {
    state
        .db
        .list_station_connectors(&station_id)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn set_connector_active(
    State(state): State<ApiState>,
    Path((station_id, connector_id)): Path<(String, i32)>,
    Json(payload): Json<SetConnectorActiveRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if connector_id <= 0 {
        return Err((StatusCode::CONFLICT, "connector_id non valido".to_string()));
    }

    let Some(connector) = state
        .db
        .connector_for_station(&station_id, connector_id)
        .await
        .map_err(internal_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "connector not found".to_string()));
    };

    if let Some(sender) = state.connections.sender(&station_id) {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        sender
            .send(StationCommand::SetConnectorActive {
                connector_id: connector_id as u32,
                evse_id: connector.evse_id,
                active: payload.active,
                reply: reply_tx,
            })
            .map_err(|_| {
                (
                    StatusCode::CONFLICT,
                    "colonnina connessa ma coda comandi chiusa".to_string(),
                )
            })?;
        drop(reply_rx);
    }

    state
        .db
        .set_connector_active(&station_id, connector_id, payload.active)
        .await
        .map_err(internal_error)?;
    state.notifier.notify();

    Ok(StatusCode::NO_CONTENT)
}

async fn set_connector_auto_remote_start_badge(
    State(state): State<ApiState>,
    Path((station_id, connector_id)): Path<(String, i32)>,
    Json(payload): Json<SetConnectorAutoRemoteStartBadgeRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if connector_id <= 0 {
        return Err((StatusCode::CONFLICT, "connector_id non valido".to_string()));
    }

    let Some(_connector) = state
        .db
        .connector_for_station(&station_id, connector_id)
        .await
        .map_err(internal_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "connector not found".to_string()));
    };

    let badge_code = payload
        .badge_code
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(badge_code) = badge_code {
        let Some(badge) = state
            .db
            .get_badge_by_code(badge_code)
            .await
            .map_err(internal_error)?
        else {
            return Err((StatusCode::CONFLICT, "badge_code non trovato".to_string()));
        };

        if !badge.active || badge.user_id.is_none() {
            return Err((
                StatusCode::CONFLICT,
                "badge_code non idoneo per remote start automatico".to_string(),
            ));
        }
    }

    state
        .db
        .set_connector_auto_remote_start_badge_code(&station_id, connector_id, badge_code)
        .await
        .map_err(internal_error)?;
    state.notifier.notify();

    Ok(StatusCode::NO_CONTENT)
}

async fn set_station_blocked(
    State(state): State<ApiState>,
    Path(station_id): Path<String>,
    Json(payload): Json<SetStationBlockedRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if let Some(sender) = state.connections.sender(&station_id) {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        sender
            .send(StationCommand::BlockStation {
                blocked: payload.blocked,
                reply: reply_tx,
            })
            .map_err(|_| {
                (
                    StatusCode::CONFLICT,
                    "colonnina connessa ma coda comandi chiusa".to_string(),
                )
            })?;
        match reply_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err((StatusCode::CONFLICT, err)),
            Err(_) => return Err((StatusCode::CONFLICT, "risposta blocco persa".to_string())),
        }
    }

    state
        .db
        .set_station_blocked(&station_id, payload.blocked)
        .await
        .map_err(internal_error)?;
    state.notifier.notify();

    Ok(StatusCode::NO_CONTENT)
}

async fn unlock_connector(
    State(state): State<ApiState>,
    Path((station_id, connector_id)): Path<(String, i32)>,
) -> Result<StatusCode, (StatusCode, String)> {
    if connector_id <= 0 {
        return Err((StatusCode::CONFLICT, "connector_id non valido".to_string()));
    }

    let Some(connector) = state
        .db
        .connector_for_station(&station_id, connector_id)
        .await
        .map_err(internal_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "connector not found".to_string()));
    };
    let connector_id = u32::try_from(connector.connector_id)
        .map_err(|_| (StatusCode::CONFLICT, "connector_id non valido".to_string()))?;

    if let Some(sender) = state.connections.sender(&station_id) {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        sender
            .send(StationCommand::UnlockConnector {
                connector_id,
                evse_id: connector.evse_id,
                reply: reply_tx,
            })
            .map_err(|_| {
                (
                    StatusCode::CONFLICT,
                    "colonnina connessa ma coda comandi chiusa".to_string(),
                )
            })?;
        match reply_rx.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => return Err((StatusCode::CONFLICT, err)),
            Err(_) => return Err((StatusCode::CONFLICT, "risposta unlock persa".to_string())),
        }
    } else {
        return Err((StatusCode::CONFLICT, "colonnina non connessa".to_string()));
    }

    Ok(StatusCode::NO_CONTENT)
}

async fn remote_start_connector_transaction(
    State(state): State<ApiState>,
    Path((station_id, connector_id)): Path<(String, i32)>,
    Json(payload): Json<RemoteStartTransactionRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    if connector_id <= 0 {
        return Err((StatusCode::CONFLICT, "connector_id non valido".to_string()));
    }

    let badge_code = payload.badge_code.trim();
    if badge_code.is_empty() {
        return Err((StatusCode::CONFLICT, "badge_code mancante".to_string()));
    }

    let Some(connector) = state
        .db
        .connector_for_station(&station_id, connector_id)
        .await
        .map_err(internal_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "connector not found".to_string()));
    };

    if connector.active_transaction_id.is_some() || connector.active_transaction_ref.is_some() {
        return Err((
            StatusCode::CONFLICT,
            "connettore con transazione gia attiva".to_string(),
        ));
    }

    let connector_id = u32::try_from(connector.connector_id)
        .map_err(|_| (StatusCode::CONFLICT, "connector_id non valido".to_string()))?;

    let Some(sender) = state.connections.sender(&station_id) else {
        return Err((StatusCode::CONFLICT, "colonnina non connessa".to_string()));
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    sender
        .send(StationCommand::RemoteStartTransaction {
            connector_id,
            badge_code: badge_code.to_string(),
            reply: reply_tx,
        })
        .map_err(|_| {
            (
                StatusCode::CONFLICT,
                "colonnina connessa ma coda comandi chiusa".to_string(),
            )
        })?;

    match reply_rx.await {
        Ok(Ok(())) => Ok(StatusCode::NO_CONTENT),
        Ok(Err(err)) => Err((StatusCode::CONFLICT, err)),
        Err(_) => Err((
            StatusCode::CONFLICT,
            "risposta remote start persa".to_string(),
        )),
    }
}

async fn remote_stop_connector_transaction(
    State(state): State<ApiState>,
    Path((station_id, connector_id)): Path<(String, i32)>,
) -> Result<StatusCode, (StatusCode, String)> {
    if connector_id <= 0 {
        return Err((StatusCode::CONFLICT, "connector_id non valido".to_string()));
    }

    let Some(connector) = state
        .db
        .connector_for_station(&station_id, connector_id)
        .await
        .map_err(internal_error)?
    else {
        return Err((StatusCode::NOT_FOUND, "connector not found".to_string()));
    };

    if connector.active_transaction_id.is_none() && connector.active_transaction_ref.is_none() {
        return Err((
            StatusCode::CONFLICT,
            "connettore senza transazione attiva".to_string(),
        ));
    }

    let Some(sender) = state.connections.sender(&station_id) else {
        return Err((StatusCode::CONFLICT, "colonnina non connessa".to_string()));
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    sender
        .send(StationCommand::RemoteStopTransaction {
            transaction_id: connector.active_transaction_id,
            transaction_ref: connector.active_transaction_ref,
            reply: reply_tx,
        })
        .map_err(|_| {
            (
                StatusCode::CONFLICT,
                "colonnina connessa ma coda comandi chiusa".to_string(),
            )
        })?;

    match reply_rx.await {
        Ok(Ok(())) => Ok(StatusCode::NO_CONTENT),
        Ok(Err(err)) => Err((StatusCode::CONFLICT, err)),
        Err(_) => Err((
            StatusCode::CONFLICT,
            "risposta remote stop persa".to_string(),
        )),
    }
}

async fn get_station_configuration(
    State(state): State<ApiState>,
    Path(station_id): Path<String>,
) -> Result<Json<StationConfigurationSnapshot>, (StatusCode, String)> {
    let Some(sender) = state.connections.sender(&station_id) else {
        return Err((StatusCode::CONFLICT, "colonnina non connessa".to_string()));
    };

    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    sender
        .send(StationCommand::GetConfiguration { reply: reply_tx })
        .map_err(|_| {
            (
                StatusCode::CONFLICT,
                "colonnina connessa ma coda comandi chiusa".to_string(),
            )
        })?;

    match reply_rx.await {
        Ok(Ok(snapshot)) => Ok(Json(snapshot)),
        Ok(Err(err)) => Err((StatusCode::CONFLICT, err)),
        Err(_) => Err((
            StatusCode::CONFLICT,
            "risposta get configuration persa".to_string(),
        )),
    }
}

async fn update_station_location(
    State(state): State<ApiState>,
    Path(station_id): Path<String>,
    Json(payload): Json<UpdateStationLocationRequest>,
) -> Result<Json<StationSummary>, (StatusCode, String)> {
    let location = StationLocation {
        station_name: payload.station_name,
        latitude: payload.latitude,
        longitude: payload.longitude,
        location_label: payload.location_label,
        address: payload.address,
        notes: payload.notes,
    };

    state
        .db
        .update_station_location(&station_id, location)
        .await
        .map_err(internal_error)?;

    match state
        .db
        .get_station(&station_id)
        .await
        .map_err(internal_error)?
    {
        Some(station) => {
            state.notifier.notify();
            Ok(Json(station))
        }
        None => Err((StatusCode::NOT_FOUND, "station not found".to_string())),
    }
}

fn internal_error(err: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
}

async fn list_users(
    State(state): State<ApiState>,
) -> Result<Json<Vec<crate::users::User>>, (StatusCode, String)> {
    state
        .db
        .list_users()
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn create_user(
    State(state): State<ApiState>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<crate::users::User>, (StatusCode, String)> {
    let user = NewUser {
        display_name: payload.display_name,
        email: payload.email,
    };

    state
        .db
        .create_user(user)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn update_user(
    State(state): State<ApiState>,
    Path(user_id): Path<i64>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<crate::users::User>, (StatusCode, String)> {
    match state
        .db
        .update_user(UserId(user_id), payload.display_name, payload.email)
        .await
        .map_err(internal_error)?
    {
        Some(user) => Ok(Json(user)),
        None => Err((StatusCode::NOT_FOUND, "user not found".to_string())),
    }
}

async fn set_user_active(
    State(state): State<ApiState>,
    Path(user_id): Path<i64>,
    Json(payload): Json<SetUserActiveRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .set_user_active(UserId(user_id), payload.active)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_badges(
    State(state): State<ApiState>,
) -> Result<Json<Vec<crate::badges::Badge>>, (StatusCode, String)> {
    state
        .db
        .list_badges()
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn create_badge(
    State(state): State<ApiState>,
    Json(payload): Json<CreateBadgeRequest>,
) -> Result<Json<crate::badges::Badge>, (StatusCode, String)> {
    let badge = NewBadge {
        user_id: payload.user_id.map(UserId),
        badge_code: payload.badge_code,
        label: payload.label,
    };

    state
        .db
        .create_badge(badge)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn update_badge(
    State(state): State<ApiState>,
    Path(badge_id): Path<i64>,
    Json(payload): Json<UpdateBadgeRequest>,
) -> Result<Json<crate::badges::Badge>, (StatusCode, String)> {
    match state
        .db
        .update_badge(
            BadgeId(badge_id),
            payload.user_id.map(UserId),
            payload.badge_code,
            payload.label,
        )
        .await
        .map_err(internal_error)?
    {
        Some(badge) => Ok(Json(badge)),
        None => Err((StatusCode::NOT_FOUND, "badge not found".to_string())),
    }
}

async fn set_badge_active(
    State(state): State<ApiState>,
    Path(badge_id): Path<i64>,
    Json(payload): Json<SetBadgeActiveRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .set_badge_active(BadgeId(badge_id), payload.active)
        .await
        .map_err(internal_error)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct EventsQuery {
    limit: Option<i64>,
    station_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LimitQuery {
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct EnergyMeterReadingsQuery {
    limit: Option<i64>,
}

async fn list_events(
    State(state): State<ApiState>,
    Query(query): Query<EventsQuery>,
) -> Result<Json<Vec<OcppEventRow>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    let station_ids = match query.station_name.as_deref() {
        Some(station_name) => Some(
            state
                .db
                .station_ids_for_station_name(station_name)
                .await
                .map_err(internal_error)?,
        ),
        None => None,
    };
    state
        .db
        .list_ocpp_messages(limit, station_ids.as_deref())
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn list_transactions(
    State(state): State<ApiState>,
    Query(query): Query<LimitQuery>,
) -> Result<Json<Vec<ChargingTransaction>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    state
        .db
        .list_transactions(limit)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn list_energy_meters(
    State(state): State<ApiState>,
) -> Result<Json<Vec<EnergyMeter>>, (StatusCode, String)> {
    state
        .db
        .list_energy_meters()
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn list_energy_meter_statuses(
    State(state): State<ApiState>,
) -> Result<Json<Vec<EnergyMeterStatusView>>, (StatusCode, String)> {
    state
        .db
        .list_energy_meter_statuses()
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn list_energy_meter_readings(
    State(state): State<ApiState>,
    Path(meter_id): Path<String>,
    Query(query): Query<EnergyMeterReadingsQuery>,
) -> Result<Json<Vec<EnergyMeterMeasurementRow>>, (StatusCode, String)> {
    let limit = query.limit.unwrap_or(100).clamp(1, 1000);
    state
        .db
        .list_energy_meter_measurements(&meter_id, limit)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn get_site_config(
    State(state): State<ApiState>,
) -> Result<Json<SiteConfigSnapshot>, (StatusCode, String)> {
    state
        .db
        .load_site_config()
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn get_energy_meter_catalog(
    State(state): State<ApiState>,
) -> Result<Json<EnergyMeterCatalog>, (StatusCode, String)> {
    Ok(Json(state.energy_meter_catalog))
}

async fn update_site_config(
    State(state): State<ApiState>,
    Json(payload): Json<SiteConfigSnapshot>,
) -> Result<Json<SiteConfigSnapshot>, (StatusCode, String)> {
    state
        .db
        .save_site_config(payload)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn create_energy_meter(
    State(state): State<ApiState>,
    Json(payload): Json<SiteEnergyMeter>,
) -> Result<Json<EnergyMeter>, (StatusCode, String)> {
    state
        .db
        .create_energy_meter(payload)
        .await
        .map(Json)
        .map_err(internal_error)
}

async fn update_energy_meter(
    State(state): State<ApiState>,
    Path(meter_id): Path<String>,
    Json(payload): Json<SiteEnergyMeter>,
) -> Result<Json<EnergyMeter>, (StatusCode, String)> {
    match state
        .db
        .update_energy_meter(&meter_id, payload)
        .await
        .map_err(internal_error)?
    {
        Some(meter) => Ok(Json(meter)),
        None => Err((StatusCode::NOT_FOUND, "energy meter not found".to_string())),
    }
}

async fn delete_energy_meter(
    State(state): State<ApiState>,
    Path(meter_id): Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .db
        .delete_energy_meter(&meter_id)
        .await
        .map_err(internal_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct UpdateStationLocationRequest {
    station_name: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    location_label: Option<String>,
    address: Option<String>,
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    display_name: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateUserRequest {
    display_name: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetUserActiveRequest {
    active: bool,
}

#[derive(Debug, Deserialize)]
struct SetStationBlockedRequest {
    blocked: bool,
}

#[derive(Debug, Deserialize)]
struct SetConnectorActiveRequest {
    active: bool,
}

#[derive(Debug, Deserialize)]
struct SetConnectorAutoRemoteStartBadgeRequest {
    badge_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RemoteStartTransactionRequest {
    badge_code: String,
}

#[derive(Debug, Deserialize)]
struct CreateBadgeRequest {
    user_id: Option<i64>,
    badge_code: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateBadgeRequest {
    user_id: Option<i64>,
    badge_code: String,
    label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SetBadgeActiveRequest {
    active: bool,
}
