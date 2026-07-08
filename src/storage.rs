use serde::{Deserialize, Serialize};

use crate::api::message::HyperdeckState;

#[derive(Serialize, Deserialize)]
pub struct StoredHyperdeck {
    pub name: String,
    pub ip: String,
    pub port: u16,
}

impl From<HyperdeckState> for StoredHyperdeck {
    fn from(value: HyperdeckState) -> Self {
        StoredHyperdeck {
            name: value.name,
            ip: value.ip,
            port: value.port,
        }
    }
}

pub async fn load_hyperdecks_file(hyperdecks_file_path: &std::path::Path) -> Vec<StoredHyperdeck> {
    if matches!(tokio::fs::try_exists(&hyperdecks_file_path).await, Ok(true)) {
        let file_contents = tokio::fs::read_to_string(hyperdecks_file_path)
            .await
            .unwrap();
        let stored_hyperdecks: Vec<StoredHyperdeck> = serde_json::from_str(&file_contents).unwrap();
        return stored_hyperdecks;
    }

    vec![]
}

pub async fn write_hyperdecks_to_file(
    hyperdecks_file_path: &std::path::Path,
    hyperdecks: Vec<StoredHyperdeck>,
) {
    let hyperdecks_json = serde_json::to_string(&hyperdecks).unwrap();
    tokio::fs::write(hyperdecks_file_path, hyperdecks_json)
        .await
        .unwrap();
}
