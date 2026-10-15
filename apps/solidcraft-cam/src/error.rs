//! Typed HTTP error → axum response mapping.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

/// All errors that can occur inside solidcraft-cam request handlers.
#[derive(Debug, thiserror::Error)]
pub enum CamError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found")]
    NotFound,
    #[error("conflict: job is not in a cancellable state")]
    Conflict,
    #[error("request body too large")]
    TooLarge,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("job store is full")]
    StoreFull,
    #[error("path outside permitted workspace")]
    PathJailEscape,
    #[error("backend unavailable: {0}")]
    BackendUnavailable(String),
    #[error("job was cancelled")]
    Cancelled,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
}

impl IntoResponse for CamError {
    fn into_response(self) -> Response {
        let status = match &self {
            CamError::Unauthorized => StatusCode::UNAUTHORIZED,
            CamError::NotFound => StatusCode::NOT_FOUND,
            CamError::Conflict => StatusCode::CONFLICT,
            CamError::TooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            CamError::BadRequest(_) => StatusCode::BAD_REQUEST,
            CamError::StoreFull => StatusCode::SERVICE_UNAVAILABLE,
            CamError::PathJailEscape => StatusCode::FORBIDDEN,
            CamError::BackendUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            CamError::Cancelled => StatusCode::GONE,
            CamError::Io(_) | CamError::Json(_) | CamError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let msg = self.to_string();
        (status, Json(ErrorBody { error: &msg })).into_response()
    }
}
