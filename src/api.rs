use axum::extract::ws::Message;
use axum::extract::{State, WebSocketUpgrade};
use axum::response::Html;
use axum::Json;
use axum::{
    body::Bytes,
    http::{header, HeaderValue, Method},
    response::IntoResponse,
    routing::get,
    Router,
};
use message::{ClientRequest, HyperdeckMonitorState, ServerEvent};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Mutex, RwLock};
use tower::ServiceBuilder;
use tower_http::timeout::TimeoutLayer;
use tower_http::ServiceBuilderExt;
use tower_http::{
    cors::{Any, CorsLayer},
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
    LatencyUnit,
};
use tracing::info;
use uuid::Uuid;

pub mod message;
mod ws;

const FILE_NAME_INDEX: &str = env!("FILE_NAME_INDEX");
const FILE_NAME_WASM: &str = env!("FILE_NAME_WASM");
const FILE_NAME_JS: &str = env!("FILE_NAME_JS");
const FILE_NAME_MANIFEST: &str = env!("FILE_NAME_MANIFEST");
const FILE_NAME_SERVICE_WORKER: &str = env!("FILE_NAME_SERVICE_WORKER");

#[derive(Debug, Clone)]
pub struct Client {
    pub sender: Option<tokio::sync::broadcast::Sender<Message>>,
}

type Clients = Arc<Mutex<HashMap<Uuid, Client>>>;

pub async fn initialize_api(
    mut state_rx: tokio::sync::broadcast::Receiver<HyperdeckMonitorState>,
    client_request_tx: tokio::sync::mpsc::UnboundedSender<ClientRequest>,
) {
    info!("Initializing API");

    let clients: Clients = Default::default();

    let state = Arc::new(RwLock::new(state_rx.recv().await.unwrap()));

    let state_clients = clients.clone();
    let state_loop = state.clone();
    tokio::spawn(async move {
        loop {
            if let Ok(hyperdeck_monitor_state) = state_rx.recv().await {
                let mut state = state_loop.write().await;
                *state = hyperdeck_monitor_state.clone();

                let clients = state_clients.lock().await;
                let state_json = serde_json::to_string(&ServerEvent::HyperdeckMonitorState(
                    hyperdeck_monitor_state,
                ))
                .unwrap();
                for (_, client) in clients.iter() {
                    if let Some(sender) = &client.sender {
                        let message: Message = Message::Text(state_json.clone());
                        let _ = sender.send(message);
                    }
                }
            }
        }
    });

    let app_state = AppState {
        state,
        client_request_tx,
        clients,
        port: 9681,
    };

    let addr = SocketAddr::from((Ipv4Addr::UNSPECIFIED, app_state.port));
    info!("Listening on {}", addr);
    // TODO: This could fail, need to figure out how to get a result from this
    let _ = axum::Server::bind(&addr)
        .serve(app(app_state).into_make_service())
        .await;
}

#[derive(Clone)]
struct AppState {
    state: Arc<RwLock<HyperdeckMonitorState>>,
    client_request_tx: tokio::sync::mpsc::UnboundedSender<ClientRequest>,
    clients: Clients,
    port: u16,
}

fn app(state: AppState) -> Router {
    let sensitive_headers: Arc<[_]> = vec![header::AUTHORIZATION, header::COOKIE].into();
    let middleware = ServiceBuilder::new()
        // Mark the `Authorization` and `Cookie` headers as sensitive so it doesn't show in logs
        .sensitive_request_headers(sensitive_headers.clone())
        // Add high level tracing/logging to all requests
        .layer(
            TraceLayer::new_for_http()
                .on_body_chunk(|chunk: &Bytes, latency: Duration, _: &tracing::Span| {
                    tracing::trace!(size_bytes = chunk.len(), latency = ?latency, "sending body chunk")
                })
                .make_span_with(DefaultMakeSpan::new().include_headers(true))
                .on_response(DefaultOnResponse::new().include_headers(true).latency_unit(LatencyUnit::Micros)),
        )
        .sensitive_response_headers(sensitive_headers)
        // Set a timeout
        .layer(TimeoutLayer::new(Duration::from_secs(10)))
        // Box the response body so it implements `Default` which is required by axum
        .map_response_body(axum::body::boxed)
        // Compress responses
        .compression()
        // Set a `Content-Type` if there isn't one already.
        .insert_response_header_if_not_present(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/octet-stream"),
        );

    let cors = CorsLayer::new()
        .allow_methods(vec![
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(Any)
        .allow_origin(Any)
        .allow_credentials(false);

    Router::new()
        .route("/", get(get_index))
        .route(&format!("/{}", FILE_NAME_INDEX), get(get_index))
        .route(&format!("/{}", FILE_NAME_WASM), get(get_wasm))
        .route(&format!("/{}", FILE_NAME_JS), get(get_js))
        .route(&format!("/{}", FILE_NAME_MANIFEST), get(get_manifest))
        .route(
            &format!("/{}", FILE_NAME_SERVICE_WORKER),
            get(get_service_worker),
        )
        .route("/ws", get(upgrade_ws))
        .layer(middleware)
        .layer(cors)
        .with_state(state)
}

async fn get_index() -> Html<String> {
    Html(include_str!(env!("INCLUDE_PATH_INDEX")).to_string())
}

async fn get_wasm() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/wasm")],
        include_bytes!(env!("INCLUDE_PATH_WASM")),
    )
}

async fn get_js() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!(env!("INCLUDE_PATH_JS")),
    )
}

async fn get_manifest() -> Json<String> {
    Json(include_str!(env!("INCLUDE_PATH_MANIFEST")).to_string())
}

async fn get_service_worker() -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "text/javascript")],
        include_str!(env!("INCLUDE_PATH_SERVICE_WORKER")),
    )
}

#[axum::debug_handler]
async fn upgrade_ws(state: State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    info!("New client websocket connection");
    let client_id = uuid::Uuid::new_v4();
    state
        .clients
        .lock()
        .await
        .insert(client_id, Client { sender: None });
    let client = state.clients.lock().await.get(&client_id).cloned().unwrap();
    ws.on_upgrade(move |socket| {
        ws::client_connection(
            state.client_request_tx.clone(),
            socket,
            client_id,
            state.state.clone(),
            state.clients.clone(),
            client,
        )
    })
}
