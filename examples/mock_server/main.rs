//! ONVIF mock server — stateful, handles Get and Set operations.
//!
//! ```sh
//! cargo run --example mock_server
//! # Then point OxDM at http://127.0.0.1:18080/onvif/device
//! ```

mod dispatch;
mod helpers;
mod services;
mod snapshot;
mod state;
pub mod xml_parse;

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

use state::{DeviceState, SharedState};

pub struct MockState {
    pub base: String,
    pub device: SharedState,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(18080);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let base = format!("http://{addr}");
    let state = Arc::new(MockState {
        base: base.clone(),
        device: SharedState::new(DeviceState::default()),
    });

    let app = Router::new()
        .route("/mock/snapshot.jpg", get(handle_snapshot))
        .route("/{*path}", post(handle_soap))
        .with_state(state);

    let listener = TcpListener::bind(addr).await.expect("bind failed");
    println!("ONVIF mock server listening on {base}");
    println!("  ONVIF_URL={base}/onvif/device");
    println!("  Stateful mode — Set operations persist in memory");
    println!();

    axum::serve(listener, app).await.expect("serve failed");
}

async fn handle_soap(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let action = helpers::extract_action(&headers).unwrap_or_default();
    let body_str = String::from_utf8_lossy(&body);
    eprintln!("  → {action}");

    let xml = dispatch::dispatch(&action, &state.base, &state.device, &body_str);

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")],
        xml,
    )
}

async fn handle_snapshot() -> impl IntoResponse {
    let bmp = snapshot::generate_test_bmp();
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/bmp"),
            (header::CACHE_CONTROL, "no-cache, no-store"),
        ],
        bmp,
    )
}
