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
mod fault_injection;
mod font;
mod helpers;
mod services;
mod snapshot;
mod state;
pub mod xml_parse;

use axum::{
    Router,
    extract::{Query, State},
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
    routing::{get, post},
};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;

use fault_injection::{FaultInjector, PendingFault};
use state::PersistentState;

pub struct MockState {
    pub base: String,
    pub device: PersistentState,
    pub fault_injector: FaultInjector,
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
        fault_injector: FaultInjector::new(),
    });

    let app = Router::new()
        .route("/mock/snapshot.jpg", get(handle_snapshot))
        .route("/admin/inject_fault", post(handle_inject_fault))
        .route("/admin/clear_faults", post(handle_clear_faults))
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

    // Test-only fault injection. Checked BEFORE auth so tests can short-
    // circuit a specific action regardless of WS-Security state. Normal
    // requests (no fault armed) fall through to the auth check below and
    // see the full production-like validation.
    if let Some(f) = state.fault_injector.take_for_action(&action) {
        eprintln!("    [INJECTED FAULT] {} -> {}", action, f.code);
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")],
            helpers::resp_soap_fault(&f.code, &f.reason),
        );
    }

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

    let xml = dispatch::dispatch(&action, &state.base, &state.device, &body_str).await;

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/soap+xml; charset=utf-8")],
        xml,
    )
}

async fn handle_snapshot(State(state): State<Arc<MockState>>) -> impl IntoResponse {
    let bmp = snapshot::generate_test_bmp(&state.device);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/bmp"),
            (header::CACHE_CONTROL, "no-cache, no-store"),
        ],
        bmp,
    )
}

/// `POST /admin/inject_fault?action=<suffix>&code=<faultcode>&reason=<text>`
///
/// Arm a single-shot SOAP Fault for the next request whose action URI
/// ends with `action`. Test-only helper — there is no auth on the
/// admin endpoints since the mock server binds to 127.0.0.1.
async fn handle_inject_fault(
    State(state): State<Arc<MockState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let action_suffix = params.get("action").cloned().unwrap_or_default();
    if action_suffix.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "missing required 'action' query parameter\n".to_string(),
        );
    }
    let code = params
        .get("code")
        .cloned()
        .unwrap_or_else(|| "s:Receiver".to_string());
    let reason = params
        .get("reason")
        .cloned()
        .unwrap_or_else(|| "Injected fault".to_string());

    eprintln!("  [ADMIN] inject fault: action_suffix='{action_suffix}' code='{code}'");
    state.fault_injector.inject(PendingFault {
        action_suffix,
        code,
        reason,
    });
    (StatusCode::OK, "fault injected\n".to_string())
}

/// `POST /admin/clear_faults` — drop every queued fault.
async fn handle_clear_faults(State(state): State<Arc<MockState>>) -> impl IntoResponse {
    eprintln!("  [ADMIN] clear all faults");
    state.fault_injector.clear_all();
    (StatusCode::OK, "faults cleared\n".to_string())
}
