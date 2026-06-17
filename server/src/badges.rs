use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::users::UserId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BadgeId(pub i64);

#[derive(Debug, Clone, Serialize)]
pub struct Badge {
    pub id: BadgeId,
    pub user_id: Option<UserId>,
    pub badge_code: String,
    pub label: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewBadge {
    pub user_id: Option<UserId>,
    pub badge_code: String,
    pub label: Option<String>,
}
