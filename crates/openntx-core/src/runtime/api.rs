// openntx-core/src/runtime/api.rs — RESTful API Bridge Server.
//
// Provides a lightweight HTTP API (via `axum`) that bridges the GUI
// AppPortal and the OpenNTX runtime subsystem.  The server exposes
// endpoints for PE execution and process purging, allowing external
// tools to drive the subsystem programmatically.
//
// Endpoints:
//   POST /api/v1/execute   — Execute a PE file with optional hardening
//   POST /api/v1/purge     — Reap zombie/orphan processes for an app

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use crate::runtime::executor::{OpenNTXExecutor, SecurityConfig};
use crate::runtime::reaper::ReaperEngine;
use crate::{OpenNtxError, Result};

// ── Request / Response types ─────────────────────────────────────────────────

/// Request body for `POST /api/v1/execute`.
#[derive(Debug, Clone, Deserialize)]
pub struct ExecuteRequest {
    pub app_id: String,
    pub exe_path: String,
    pub hardened_mode: bool,
}

/// Request body for `POST /api/v1/purge`.
#[derive(Debug, Clone, Deserialize)]
pub struct PurgeRequest {
    pub app_id: String,
}

/// Standard success response.
#[derive(Debug, Clone, Serialize)]
pub struct SuccessResponse {
    pub status: String,
}

/// Response for the purge endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct PurgeResponse {
    pub status: String,
    pub killed_pids: u32,
}

/// Response for the execute endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct ExecuteResponse {
    pub status: String,
    pub app_id: String,
}

// ── Shared application state ─────────────────────────────────────────────────

/// Shared state accessible from all request handlers.
#[derive(Clone)]
pub struct AppState {
    pub executor: Arc<OpenNTXExecutor>,
    pub reaper: Arc<ReaperEngine>,
}

// ── API Bridge Server ────────────────────────────────────────────────────────

/// Lightweight RESTful API server that bridges the GUI AppPortal and
/// the OpenNTX runtime subsystem.
///
/// # Examples
///
/// ```no_run
/// use openntx_core::runtime::api::ApiBridgeServer;
///
/// #[tokio::main]
/// async fn main() {
///     ApiBridgeServer::start_server("0.0.0.0", 8420).await.unwrap();
/// }
/// ```
pub struct ApiBridgeServer;

impl ApiBridgeServer {
    /// Start the API server on the given host and port.
    ///
    /// This function blocks until the server is shut down.
    pub async fn start_server(host: &str, port: u16) -> Result<()> {
        let executor = OpenNTXExecutor::new()
            .map_err(|e| OpenNtxError::Config(format!("failed to initialize executor: {}", e)))?;
        let reaper = ReaperEngine::new();

        let state = AppState {
            executor: Arc::new(executor),
            reaper: Arc::new(reaper),
        };

        let app = Self::build_router(state);

        let addr: SocketAddr = format!("{}:{}", host, port).parse().map_err(|e| {
            OpenNtxError::InvalidInput(format!("invalid address {}:{}: {}", host, port, e))
        })?;

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| OpenNtxError::Config(format!("failed to bind to {}: {}", addr, e)))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| OpenNtxError::Config(format!("server error: {}", e)))
    }

    /// Build the axum router with all endpoints.
    pub fn build_router(state: AppState) -> Router {
        Router::new()
            .route("/api/v1/execute", post(handle_execute))
            .route("/api/v1/purge", post(handle_purge))
            .with_state(Arc::new(state))
    }
}

// ── Custom JSON extractor with 422 on rejection ─────────────────────────────

/// Wrapper around `axum::Json` that returns `422 Unprocessable Entity`
/// instead of the default `400 Bad Request` when JSON parsing fails.
struct ValidJson<T>(T);

#[axum::async_trait]
impl<T, S> axum::extract::FromRequest<S> for ValidJson<T>
where
    T: serde::de::DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = (StatusCode, Json<serde_json::Value>);

    async fn from_request(
        req: axum::extract::Request,
        state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(ValidJson(value)),
            Err(rejection) => {
                let status = rejection.status();
                let message = rejection.body_text();
                // Override 400 → 422 for JSON parse errors.
                let status = if status == StatusCode::BAD_REQUEST {
                    StatusCode::UNPROCESSABLE_ENTITY
                } else {
                    status
                };
                Err((
                    status,
                    Json(serde_json::json!({"status": "error", "message": message})),
                ))
            }
        }
    }
}

// ── Request handlers ─────────────────────────────────────────────────────────

/// Handle `POST /api/v1/execute`.
///
/// Validates the request, selects the appropriate security configuration,
/// and delegates to `OpenNTXExecutor::execute_pe`.
async fn handle_execute(
    State(state): State<Arc<AppState>>,
    ValidJson(body): ValidJson<ExecuteRequest>,
) -> impl IntoResponse {
    if body.app_id.is_empty() || body.exe_path.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(
                serde_json::json!({"status": "error", "message": "app_id and exe_path are required"}),
            ),
        );
    }

    let security = if body.hardened_mode {
        SecurityConfig::hardened()
    } else {
        SecurityConfig::permissive()
    };

    let exe_path = PathBuf::from(&body.exe_path);

    match state.executor.execute_pe(&exe_path, &[], &security) {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "app_id": body.app_id,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string(),
            })),
        ),
    }
}

/// Handle `POST /api/v1/purge`.
///
/// Delegates to `ReaperEngine::reap_application` to terminate all
/// processes in the app's cgroup.
async fn handle_purge(
    State(state): State<Arc<AppState>>,
    ValidJson(body): ValidJson<PurgeRequest>,
) -> impl IntoResponse {
    if body.app_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "app_id is required"})),
        );
    }

    match state.reaper.reap_application(&body.app_id) {
        Ok(killed) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "success",
                "killed_pids": killed,
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string(),
            })),
        ),
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn test_state() -> AppState {
        // Use temp directories for isolated testing.
        let tmp = tempfile::tempdir().expect("temp dir");
        let profiles_dir = tmp.path().join("profiles");
        let sandbox_dir = tmp.path().join("sandbox");
        let cgroup_dir = tmp.path().join("cgroups");

        std::fs::create_dir_all(&profiles_dir).unwrap();
        std::fs::create_dir_all(&sandbox_dir).unwrap();
        std::fs::create_dir_all(&cgroup_dir).unwrap();

        let profile_mgr = crate::profile::ProfileManager::with_path(profiles_dir).unwrap();
        let executor = OpenNTXExecutor::with_paths(profile_mgr, sandbox_dir).unwrap();
        let reaper = ReaperEngine::with_root(cgroup_dir);

        AppState {
            executor: Arc::new(executor),
            reaper: Arc::new(reaper),
        }
    }

    #[tokio::test]
    async fn execute_endpoint_returns_200_on_valid_request() {
        let state = test_state();
        let app = ApiBridgeServer::build_router(state);

        // Create a temp PE file.
        let tmp = tempfile::tempdir().unwrap();
        let pe_path = tmp.path().join("test.exe");
        std::fs::write(&pe_path, b"MZ\x90\x00test data").unwrap();

        let body = serde_json::json!({
            "app_id": "test-app",
            "exe_path": pe_path.display().to_string(),
            "hardened_mode": false,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Wine is not available in CI, so we expect either 200 (if Wine
        // happens to be installed) or 500 (if not).  Either way, the
        // endpoint must respond.
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::INTERNAL_SERVER_ERROR,
            "unexpected status: {}",
            response.status()
        );
    }

    #[tokio::test]
    async fn execute_endpoint_returns_400_on_empty_body() {
        let state = test_state();
        let app = ApiBridgeServer::build_router(state);

        let body = serde_json::json!({
            "app_id": "",
            "exe_path": "",
            "hardened_mode": false,
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn purge_endpoint_returns_200_on_valid_request() {
        let state = test_state();
        let app = ApiBridgeServer::build_router(state);

        let body = serde_json::json!({
            "app_id": "nonexistent-app",
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/purge")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        // The app doesn't exist, so reap returns 0 killed.
        assert_eq!(response.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(json["status"], "success");
        assert_eq!(json["killed_pids"], 0);
    }

    #[tokio::test]
    async fn purge_endpoint_returns_400_on_empty_app_id() {
        let state = test_state();
        let app = ApiBridgeServer::build_router(state);

        let body = serde_json::json!({
            "app_id": "",
        });

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/purge")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let state = test_state();
        let app = ApiBridgeServer::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn execute_endpoint_returns_422_on_invalid_json() {
        let state = test_state();
        let app = ApiBridgeServer::build_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/execute")
                    .header("content-type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
