use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/** Standard success response wrapper. */
#[derive(Debug, Serialize)]
pub struct ApiResponse<T: Serialize> {
    pub data: T,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

/** Standard error response. */
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ErrorBody {
                code: code.into(),
                message: message.into(),
            },
        }
    }

    pub fn not_found(resource: &str) -> (StatusCode, Self) {
        (
            StatusCode::NOT_FOUND,
            Self::new(
                format!("{}_NOT_FOUND", resource.to_uppercase()),
                format!("{} not found", resource),
            ),
        )
    }

    pub fn bad_request(message: impl Into<String>) -> (StatusCode, Self) {
        (StatusCode::BAD_REQUEST, Self::new("BAD_REQUEST", message))
    }

    pub fn internal(message: impl Into<String>) -> (StatusCode, Self) {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Self::new("INTERNAL_ERROR", message),
        )
    }

    pub fn forbidden(message: impl Into<String>) -> (StatusCode, Self) {
        (StatusCode::FORBIDDEN, Self::new("FORBIDDEN", message))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}
