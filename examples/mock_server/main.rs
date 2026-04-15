//! ONVIF mock server — handles every operation exercised by `full-workflow`.
//!
//! ```sh
//! # Terminal 1 — start the mock server (default port 18080)
//! cargo run --example mock_server
//!
//! # Terminal 2 — run the full workflow against it
//! ONVIF_URL=http://127.0.0.1:18080/onvif/device \
//! ONVIF_USERNAME=admin ONVIF_PASSWORD=password \
//! cargo run --example camera -- full-workflow
//! ```

mod dispatch;
mod helpers;
mod services;
mod snapshot;

use axum::{
    Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

pub struct MockState {
    pub base: String,
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(18080);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let base = format!("http://{addr}");
    let state = Arc::new(MockState { base: base.clone() });

    let app = Router::new()
        .route("/mock/snapshot.jpg", get(handle_snapshot))
        .route("/{*path}", post(handle_soap))
        .with_state(state);

    let listener = TcpListener::bind(addr).await.expect("bind failed");
    println!("ONVIF mock server listening on {base}");
    println!("  ONVIF_URL={base}/onvif/device");
    println!();

    axum::serve(listener, app).await.expect("serve failed");
}

async fn handle_soap(
    State(state): State<Arc<MockState>>,
    headers: HeaderMap,
    _body: axum::body::Bytes,
) -> impl IntoResponse {
    let action = helpers::extract_action(&headers).unwrap_or_default();
    eprintln!("  → {action}");

    let xml = dispatch::dispatch(&action, &state.base);

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
