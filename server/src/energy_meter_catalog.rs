use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyMeterCatalog {
    pub profiles: Vec<EnergyMeterProfile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyMeterProfile {
    pub key: String,
    pub vendor: String,
    pub model: String,
    pub transport: String,
    pub default_port: u16,
    pub registers: Vec<EnergyMeterRegister>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyMeterRegister {
    pub metric_key: String,
    pub address: u16,
    pub length: u16,
    pub function: String,
    pub data_type: String,
    pub endianness: String,
    pub scale: f64,
    pub unit: String,
}

pub fn load_catalog() -> Result<EnergyMeterCatalog, serde_json::Error> {
    serde_json::from_str(include_str!("../config/energy-meter-catalog.json"))
}
