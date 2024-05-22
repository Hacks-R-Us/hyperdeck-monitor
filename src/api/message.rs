use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum ClientRequest {
    AddHyperdeck(AddHyperdeckRequest),
    RemoveHyperdeck(RemoveHyperdeckRequest),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AddHyperdeckRequest {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveHyperdeckRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum ServerEvent {
    HyperdeckMonitorState(HyperdeckMonitorState),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HyperdeckMonitorState {
    pub hyperdecks: HashMap<String, HyperdeckState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperdeckState {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub connection_state: HyperdeckConnectionState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HyperdeckConnectionState {
    Connected,
    Disconnected,
}
