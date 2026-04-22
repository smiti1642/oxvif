//! ONVIF mock server — stateful, with WS-Security auth and file persistence.
//!
//! ```sh
//! # Default: state saved to ~/.oxvif/mock_device.toml
//! cargo run --example mock_server
//!
//! # Custom port + config file
//! cargo run --example mock_server -- 19090 --config /path/to/state.toml
//!
//! # Credentials: admin / admin
//! ```

mod auth;
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

use state::PersistentState;

pub struct MockState {
    pub base: String,
    pub device: PersistentState,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(18080);

    let state_path = state::resolve_state_path();
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let base = format!("http://{addr}");

    let state = Arc::new(MockState {
        base: base.clone(),
        device: PersistentState::load(state_path.clone()),
    });

    let app = Router::new()
        .route("/mock/snapshot.jpg", get(handle_snapshot))
        .route("/{*path}", post(handle_soap))
        .with_state(state);

    let listener = TcpListener::bind(addr).await.expect("bind failed");
    println!("ONVIF mock server listening on {base}");
    println!("  ONVIF_URL={base}/onvif/device");
    println!("  Credentials: admin / admin");
    println!("  State file: {}", state_path.display());
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

    if auth::requires_auth(&action) {
        if let Err(reason) = auth::validate_ws_security(&body_str, &state.device) {
            eprintln!("    [AUTH FAIL] {reason}");
            return (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")],
                auth::auth_fault(&reason),
            );
        }
    }

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
