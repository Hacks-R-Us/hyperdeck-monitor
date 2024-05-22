use std::{future, sync::Arc};

use super::message::{ClientRequest, HyperdeckMonitorState};
use crate::api::ServerEvent;
use axum::extract::ws::{Message, WebSocket};
use futures::StreamExt;
use tokio::sync::RwLock;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{debug, error, log::info};
use uuid::Uuid;

use super::{Client, Clients};

pub async fn client_connection(
    client_request_tx: tokio::sync::mpsc::UnboundedSender<ClientRequest>,
    ws: WebSocket,
    id: Uuid,
    state: Arc<RwLock<HyperdeckMonitorState>>,
    clients: Clients,
    mut client: Client,
) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = tokio::sync::broadcast::channel::<Message>(10);
    let client_rcv = BroadcastStream::new(client_rcv);

    tokio::task::spawn(
        client_rcv
            .filter(|msg| future::ready(msg.is_ok()))
            .map(|msg| Ok(msg.unwrap()))
            .forward(client_ws_sender),
    );

    let current_state = state.read().await.clone();
    let state_json =
        serde_json::to_string(&ServerEvent::HyperdeckMonitorState(current_state.into())).unwrap();
    client_sender.send(Message::Text(state_json.clone())).ok();

    client.sender = Some(client_sender);
    clients.lock().await.insert(id, client);

    info!("{} connected", id);

    while let Some(result) = client_ws_rcv.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                error!("error resolving ws message for id: {}: {}", id.clone(), e);
                break;
            }
        };
        client_msg(client_request_tx.clone(), &id, msg).await;
    }

    clients.lock().await.remove(&id);
    info!("{} disconnected", id);
}

async fn client_msg(
    client_request_tx: tokio::sync::mpsc::UnboundedSender<ClientRequest>,
    id: &Uuid,
    msg: Message,
) {
    debug!("received message from {}: {:?}", id, msg);
    let message = match msg.into_text() {
        Ok(v) => v,
        Err(err) => {
            error!("error: {:?}", err);
            return;
        }
    };

    if message == "ping" || message == "ping\n" {
        return;
    }

    let client_request: super::message::ClientRequest = match serde_json::from_str(&message) {
        Ok(v) => v,
        Err(_) => {
            return;
        }
    };

    let _ = client_request_tx.send(client_request);
}
