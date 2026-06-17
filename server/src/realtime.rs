use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use serde::Serialize;
use tokio::sync::broadcast;

use crate::db::{ConnectorSummary, Database, StationSummary};

#[derive(Clone)]
pub struct RealtimeNotifier {
    tx: broadcast::Sender<()>,
}

impl Default for RealtimeNotifier {
    fn default() -> Self {
        let (tx, _) = broadcast::channel(128);
        Self { tx }
    }
}

impl RealtimeNotifier {
    pub fn notify(&self) {
        let _ = self.tx.send(());
    }

    fn subscribe(&self) -> broadcast::Receiver<()> {
        self.tx.subscribe()
    }
}

#[derive(Clone)]
pub struct RealtimeState {
    pub db: Database,
    pub notifier: RealtimeNotifier,
}

#[derive(Debug, Serialize)]
pub struct RealtimeSnapshot {
    pub stations: Vec<StationSummary>,
    pub connectors: Vec<ConnectorSummary>,
}

pub async fn websocket(
    ws: WebSocketUpgrade,
    State(state): State<RealtimeState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: RealtimeState) {
    let mut rx = state.notifier.subscribe();

    if send_snapshot(&mut socket, &state.db).await.is_err() {
        return;
    }

    while rx.recv().await.is_ok() {
        if send_snapshot(&mut socket, &state.db).await.is_err() {
            return;
        }
    }
}

async fn send_snapshot(socket: &mut WebSocket, db: &Database) -> Result<(), ()> {
    let stations = db.list_stations().await.map_err(|err| {
        eprintln!("realtime list_stations fallito: {err}");
    })?;
    let connectors = db.list_connectors().await.map_err(|err| {
        eprintln!("realtime list_connectors fallito: {err}");
    })?;
    let payload = serde_json::to_string(&RealtimeSnapshot {
        stations,
        connectors,
    })
    .map_err(|err| {
        eprintln!("realtime serialize fallito: {err}");
    })?;

    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|err| {
            eprintln!("realtime websocket send fallito: {err}");
        })
}
