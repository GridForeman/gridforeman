use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use serde::Serialize;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct StationConfigurationEntry {
    pub(crate) key: String,
    pub(crate) readonly: bool,
    pub(crate) value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub(crate) struct StationConfigurationSnapshot {
    pub(crate) configuration_keys: Vec<StationConfigurationEntry>,
    pub(crate) unknown_keys: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum StationCommand {
    BlockStation {
        blocked: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RemoteStartTransaction {
        connector_id: u32,
        badge_code: String,
        reply: oneshot::Sender<Result<(), String>>,
    },
    RemoteStopTransaction {
        transaction_id: Option<i32>,
        transaction_ref: Option<String>,
        reply: oneshot::Sender<Result<(), String>>,
    },
    GetConfiguration {
        reply: oneshot::Sender<Result<StationConfigurationSnapshot, String>>,
    },
    SetConnectorActive {
        connector_id: u32,
        evse_id: Option<i32>,
        active: bool,
        reply: oneshot::Sender<Result<(), String>>,
    },
    UnlockConnector {
        connector_id: u32,
        evse_id: Option<i32>,
        reply: oneshot::Sender<Result<(), String>>,
    },
}

#[derive(Clone, Default)]
pub(crate) struct ConnectionRegistry {
    inner: Arc<Mutex<HashMap<String, mpsc::UnboundedSender<StationCommand>>>>,
}

impl ConnectionRegistry {
    pub(crate) fn register(
        &self,
        station_id: String,
        sender: mpsc::UnboundedSender<StationCommand>,
    ) {
        self.inner
            .lock()
            .expect("connection registry poisoned")
            .insert(station_id, sender);
    }

    pub(crate) fn unregister(&self, station_id: &str) {
        self.inner
            .lock()
            .expect("connection registry poisoned")
            .remove(station_id);
    }

    pub(crate) fn sender(
        &self,
        station_id: &str,
    ) -> Option<mpsc::UnboundedSender<StationCommand>> {
        self.inner
            .lock()
            .expect("connection registry poisoned")
            .get(station_id)
            .cloned()
    }
}
