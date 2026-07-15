use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[serde(tag = "type")]
pub enum ClientRequest {
    AddHyperdeck(AddHyperdeckRequest),
    RemoveHyperdeck(RemoveHyperdeckRequest),
    StartRecording(StartRecordingRequest),
    StopRecording(StopRecordingRequest),
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
pub struct StartRecordingRequest {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StopRecordingRequest {
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
    pub recording_status: RecordingState,
    // HashMap to allow for sparse entries.
    pub slots: HashMap<String, HyperdeckRecordBay>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HyperdeckConnectionState {
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordingState {
    Recording,
    NotRecording,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HyperdeckRecordBay {
    /// Recording time available in seconds.
    pub recording_time_remaining: u64,
}
