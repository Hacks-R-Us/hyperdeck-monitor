use std::{process::Stdio, time::Duration};

use api::message::{
    AddHyperdeckRequest, ClientRequest, HyperdeckConnectionState, HyperdeckMonitorState,
    HyperdeckState, RemoveHyperdeckRequest,
};
use color_eyre::Report;
use futures_util::{
    pin_mut, select,
    stream::{SplitSink, SplitStream},
    FutureExt, SinkExt, StreamExt,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tokio_util::{
    codec::{FramedRead, LinesCodec},
    sync::CancellationToken,
};
use tracing_subscriber::EnvFilter;

mod api;

#[tokio::main]
async fn main() {
    setup_logging().expect("Failed to setup logging");
    tracing::info!("Hello, world!");

    let cancel = CancellationToken::new();
    let node_process = run_node_process(cancel.clone()).fuse();

    let (node_ws_message_tx, node_ws_message_rx) = tokio::sync::mpsc::unbounded_channel();
    let (node_commands_tx, node_commands_rx) = tokio::sync::mpsc::unbounded_channel();
    let state = AppState::default();
    let node_ws_communication =
        talk_to_node_ws(state, node_ws_message_tx, node_commands_rx, cancel.clone()).fuse();

    let (state_tx, state_rx) = tokio::sync::broadcast::channel(1);
    let (client_request_tx, client_request_rx) = tokio::sync::mpsc::unbounded_channel();
    let api = api::initialize_api(state_rx, client_request_tx).fuse();

    let hyperdeck_monitor = run(
        node_commands_tx,
        node_ws_message_rx,
        state_tx,
        client_request_rx,
        cancel.clone(),
    )
    .fuse();

    pin_mut!(node_process);
    pin_mut!(node_ws_communication);
    pin_mut!(api);
    pin_mut!(hyperdeck_monitor);

    select! {
        _ = node_process => {},
        _ = node_ws_communication => {},
        _ = api => {},
        _ = hyperdeck_monitor => {},
        _ = cancel.cancelled().fuse() => {}
    };

    cancel.cancel();
}

async fn run(
    mut node_commands_tx: tokio::sync::mpsc::UnboundedSender<NodeWsCommand>,
    mut node_ws_message_rx: tokio::sync::mpsc::UnboundedReceiver<NodeWsMessageReceived>,
    mut state_tx: tokio::sync::broadcast::Sender<HyperdeckMonitorState>,
    mut client_request_rx: tokio::sync::mpsc::UnboundedReceiver<ClientRequest>,
    cancel: CancellationToken,
) {
    let mut state = HyperdeckMonitorState::default();
    let _ = state_tx.send(state.clone());

    while !cancel.is_cancelled() {
        let state_modified = select! {
            message_from_node = node_ws_message_rx.recv().fuse() => {
                if let Some(msg) = message_from_node {
                    handle_message_from_node(msg, &mut node_commands_tx, &mut state).await
                } else {
                    false
                }
            },
            message_from_client = client_request_rx.recv().fuse() => {
                if let Some(msg) = message_from_client {
                    handle_message_from_client(msg, &mut node_commands_tx, &mut state).await
                } else {
                    false
                }
            }
        };

        if state_modified {
            let _ = state_tx.send(state.clone());
        }
    }
}

async fn handle_message_from_node(
    msg: NodeWsMessageReceived,
    node_commands_tx: &mut tokio::sync::mpsc::UnboundedSender<NodeWsCommand>,
    state: &mut HyperdeckMonitorState,
) -> bool {
    match msg {
        NodeWsMessageReceived::Log { message } => {
            tracing::info!("[NODE] {message}");
            false
        }
        NodeWsMessageReceived::HyperdeckConnected { id } => {
            state.hyperdecks.entry(id).and_modify(|hyperdeck| {
                hyperdeck.connection_state = HyperdeckConnectionState::Connected
            });
            true
        }
        NodeWsMessageReceived::HypderdeckDisconnected { id } => {
            state.hyperdecks.entry(id).and_modify(|hyperdeck| {
                hyperdeck.connection_state = HyperdeckConnectionState::Disconnected
            });
            true
        }
    }
}

async fn handle_message_from_client(
    msg: ClientRequest,
    node_commands_tx: &mut tokio::sync::mpsc::UnboundedSender<NodeWsCommand>,
    state: &mut HyperdeckMonitorState,
) -> bool {
    match msg {
        ClientRequest::AddHyperdeck(AddHyperdeckRequest { name, ip, port }) => {
            tracing::info!("Adding hyperdeck");
            let id = uuid::Uuid::new_v4();
            state.hyperdecks.insert(
                id.to_string(),
                HyperdeckState {
                    name,
                    ip: ip.clone(),
                    port,
                    connection_state: api::message::HyperdeckConnectionState::Disconnected,
                },
            );
            let _ = node_commands_tx.send(NodeWsCommand::AddHyperdeck(AddHyperdeckCommand {
                id: id.to_string(),
                ip,
                port,
            }));
            true
        }
        ClientRequest::RemoveHyperdeck(RemoveHyperdeckRequest { id }) => {
            let _ = state.hyperdecks.remove(&id);
            let _ = node_commands_tx.send(NodeWsCommand::RemoveHyperdeck(RemoveHyperdeckCommand {
                id,
            }));
            true
        }
    }
}

async fn run_node_process(cancel: CancellationToken) {
    while !cancel.is_cancelled() {
        // Back-off in case we are immediately crashing in a loop.
        tokio::time::sleep(Duration::from_secs(1)).await;

        let result = tokio::process::Command::new("node")
            .arg("./index.js")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();
        match result {
            Ok(mut child_process) => {
                let Some(raw_stdout) = child_process.stdout.take() else {
                    let _ = child_process.kill().await;
                    continue;
                };

                let Some(raw_stderr) = child_process.stderr.take() else {
                    let _ = child_process.kill().await;
                    continue;
                };

                let mut stdout = FramedRead::new(raw_stdout, LinesCodec::new())
                    .map(|data| data.expect("Could not read stdout"));
                let mut stderr = FramedRead::new(raw_stderr, LinesCodec::new())
                    .map(|data| data.expect("Could not read stderr"));

                while !cancel.is_cancelled() {
                    select! {
                        line = stdout.next().fuse() =>  {
                            if let Some(line) = line {
                                tracing::info!("[NODE] {line}");
                            }
                        }
                        line = stderr.next().fuse() =>  {
                            if let Some(line) = line {
                                tracing::error!("[NODE] {line}");
                            }
                        }
                    }
                }

                let _ = child_process.kill().await;
            }
            Err(err) => {
                tracing::error!("Error running Node child process: {err}");
            }
        }
    }
}

#[derive(Default)]
struct AppState {}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
enum NodeWsCommand {
    #[serde(rename = "add_hyperdeck")]
    AddHyperdeck(AddHyperdeckCommand),
    #[serde(rename = "remove_hyperdeck")]
    RemoveHyperdeck(RemoveHyperdeckCommand),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "event")]
enum NodeWsMessageReceived {
    Log { message: String },
    HyperdeckConnected { id: String },
    HypderdeckDisconnected { id: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddHyperdeckCommand {
    id: String,
    ip: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoveHyperdeckCommand {
    id: String,
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
        if let Err(err) = socket_tx
            .send(tokio_tungstenite::tungstenite::Message::Text(
                serde_json::to_string(&command).expect("Could not serialize command"),
            ))
            .await
        {
            tracing::error!("Error sending command to Node proccess: {err}");
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
                        let _ = ws_message_tx.send(received);
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
