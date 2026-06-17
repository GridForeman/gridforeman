use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserId(pub i64);

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: UserId,
    pub display_name: String,
    pub email: Option<String>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub display_name: String,
    pub email: Option<String>,
}
