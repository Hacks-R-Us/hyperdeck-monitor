use std::{net::IpAddr, time::Duration};

use color_eyre::Report;
use futures_util::{
    pin_mut, select,
    stream::{SplitSink, SplitStream},
    FutureExt, SinkExt, StreamExt,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    setup_logging().expect("Failed to setup logging");
    tracing::info!("Hello, world!");

    let cancel = CancellationToken::new();
    let node_process = run_node_process(cancel.clone()).fuse();

    let (ws_message_tx, ws_message_rx) = tokio::sync::mpsc::unbounded_channel();
    let (commands_tx, commands_rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::default();
    let ws_process = talk_to_node_ws(state, ws_message_tx, commands_rx, cancel.clone()).fuse();

    pin_mut!(node_process);
    pin_mut!(ws_process);

    select! {
        _ = node_process => {},
        _ = ws_process => {},
        _ = cancel.cancelled().fuse() => {}
    };

    cancel.cancel();
}

async fn run_node_process(cancel: CancellationToken) {
    while !cancel.is_cancelled() {
        let result = tokio::process::Command::new("node")
            .arg("monitor/index.js")
            .output()
            .await;
        if let Ok(output) = result {
            if !output.status.success() {
                let err = String::from_utf8(output.stderr).unwrap_or("Unknown".to_string());
                tracing::error!("Node process exited with error: {}", err);
                // Back-off in case we are immediately crashing in a loop.
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[derive(Default)]
struct AppState {}
enum NodeWsCommand {
    Ping,
    AddHyperdeck(AddHyperdeckCommand),
    RemoveHyperdeck(RemoveHyperdeckCommand),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "event")]
enum NodeWsMessageReceived {
    Log { message: String },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddHyperdeckCommand {
    ip: IpAddr,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveHyperdeckCommand {
    ip: IpAddr,
}

async fn talk_to_node_ws(
    state: AppState,
    ws_message_tx: tokio::sync::mpsc::UnboundedSender<NodeWsMessageReceived>,
    commands_rx: tokio::sync::mpsc::UnboundedReceiver<NodeWsCommand>,
    cancel: CancellationToken,
) {
    let ws_stream = wait_for_connection().await;
    let (write, read) = ws_stream.split();

    let outgoing = handle_outbound_messages(commands_rx, write).fuse();
    let incoming = handle_inbound_messages(read, ws_message_tx).fuse();

    pin_mut!(outgoing);
    pin_mut!(incoming);

    select! {
        _ = outgoing => {},
        _ = incoming => {},
        _ = cancel.cancelled().fuse() => {},
    }
}

async fn wait_for_connection() -> WebSocketStream<MaybeTlsStream<TcpStream>> {
    loop {
        // Wait for Node to wake up...
        tokio::time::sleep(Duration::from_secs(1)).await;

        let ws_url = url::Url::parse("ws://127.0.0.1:7867").expect("Invalid websocket URL");
        match tokio_tungstenite::connect_async(ws_url.clone()).await {
            Ok((ws_stream, _)) => {
                tracing::info!("Connected to Node process on {ws_url}");
                return ws_stream;
            }
            Err(err) => {
                tracing::error!("Error connecting to Node process: {:?}", err)
            }
        }
    }
}

async fn handle_outbound_messages(
    mut commands_rx: tokio::sync::mpsc::UnboundedReceiver<NodeWsCommand>,
    mut socket_tx: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
) {
    while let Some(command) = commands_rx.recv().await {
        match command {
            NodeWsCommand::Ping => {
                let _ = socket_tx
                    .send(tokio_tungstenite::tungstenite::Message::Ping(vec![]))
                    .await;
            }
            NodeWsCommand::AddHyperdeck(command) => {
                let _ = socket_tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        serde_json::to_string(&command)
                            .expect("Could not serialize AddHyperdeck command"),
                    ))
                    .await;
            }
            NodeWsCommand::RemoveHyperdeck(command) => {
                let _ = socket_tx
                    .send(tokio_tungstenite::tungstenite::Message::Text(
                        serde_json::to_string(&command)
                            .expect("Could not serialize RemoveHyperdeck command"),
                    ))
                    .await;
            }
        }
    }
}

async fn handle_inbound_messages(
    socket_rx: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ws_message_tx: tokio::sync::mpsc::UnboundedSender<NodeWsMessageReceived>,
) {
    socket_rx
        .for_each(|message| async {
            match message {
                Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                    if let Ok(received) = serde_json::from_str::<NodeWsMessageReceived>(&text) {
                        match received {
                            NodeWsMessageReceived::Log { message } => {
                                tracing::info!("Message from Node process: {message}");
                            }
                        }
                    }
                }
                Ok(tokio_tungstenite::tungstenite::Message::Pong(_)) => {}
                _ => {}
            }
        })
        .await;
}

fn setup_logging() -> Result<(), Report> {
    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "1");
    }
    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "debug");
    }
    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(())
}
