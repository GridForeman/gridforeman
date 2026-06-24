use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiteEnergyMeter {
    pub id: String,
    pub name: String,
    pub catalog_key: Option<String>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub unit_id: Option<u8>,
    pub poll_interval_ms: Option<u64>,
    pub meter_label: Option<String>,
    pub max_current_a: Option<f64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteManagementGroup {
    pub id: String,
    pub name: String,
    pub control_mode: String,
    #[serde(alias = "current_feed_ids")]
    #[serde(alias = "power_feed_ids")]
    pub energy_meter_ids: Vec<String>,
    pub station_ids: Vec<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteConfigSnapshot {
    pub site_name: Option<String>,
    pub timezone: String,
    pub operator_name: Option<String>,
    pub notes: Option<String>,
    #[serde(alias = "current_feeds")]
    #[serde(alias = "power_feeds")]
    pub energy_meters: Vec<SiteEnergyMeter>,
    pub management_groups: Vec<SiteManagementGroup>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SiteConfigTopology {
    #[serde(alias = "current_feeds")]
    #[serde(alias = "power_feeds")]
    pub energy_meters: Vec<SiteEnergyMeter>,
    pub management_groups: Vec<SiteManagementGroup>,
}

impl Default for SiteConfigSnapshot {
    fn default() -> Self {
        Self {
            site_name: None,
            timezone: "Europe/Zurich".to_string(),
            operator_name: None,
            notes: None,
            energy_meters: Vec::new(),
            management_groups: Vec::new(),
            updated_at: None,
        }
    }
}
