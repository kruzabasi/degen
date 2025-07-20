use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;
use std::fmt;
use thiserror::Error;
use tracing::{error, instrument};

/// A set of errors that can occur during request handling
#[derive(Debug, Error)]
pub enum AppError {
    /// Return `400 Bad Request`
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Return `404 Not Found`
    #[error("Not found: {0}")]
    NotFound(String),

    /// Return `409 Conflict`
    #[error("Conflict: {0}")]
    Conflict(String),

    /// Return `422 Unprocessable Entity`
    #[error("Unprocessable entity: {0}")]
    UnprocessableEntity(String),

    /// Return `500 Internal Server Error`
    #[error("Internal server error: {0}")]
    InternalServerError(String),

    /// Return `503 Service Unavailable`
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
}

/// Error response payload
#[derive(Serialize)]
pub struct ErrorResponse {
    /// Error message
    pub error: String,
    /// Optional error code for programmatic handling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<&'static str>,
    /// Optional additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AppError {
    /// Get the HTTP status code for this error
    pub fn status_code(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::UnprocessableEntity(_) => StatusCode::UNPROCESSABLE_ENTITY,
            Self::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// Get the error code for programmatic handling
    pub fn code(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "bad_request",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::UnprocessableEntity(_) => "unprocessable_entity",
            Self::InternalServerError(_) => "internal_server_error",
            Self::ServiceUnavailable(_) => "service_unavailable",
        }
    }
}

// Display is derived via #[derive(Error)]

impl IntoResponse for AppError {
    #[instrument]
    fn into_response(self) -> Response {
        let status = self.status_code();
        let code = self.code();
        let message = self.to_string();

        // Log internal server errors
        if status.is_server_error() {
            error!(
                error = &message,
                error_code = code,
                status = status.as_str(),
                "Request failed with server error"
            );
        }

        let body = Json(ErrorResponse {
            error: message,
            code: Some(code),
            details: None,
        });

        (status, body).into_response()
    }
}

// Convert database errors to our AppError
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(db_err) => {
                // Handle unique constraint violations
                if db_err.code().map(|c| c == "23505").unwrap_or(false) {
                    return Self::Conflict("A record with these values already exists".to_string());
                }

                // Handle foreign key violations
                if let Some(constraint) = db_err.constraint() {
                    if constraint.ends_with("_fkey") {
                        return Self::BadRequest(format!("Invalid reference: {constraint}"));
                    }
                }

                Self::InternalServerError(format!("Database error: {db_err}"))
            }
            sqlx::Error::RowNotFound => Self::NotFound("Requested data not found".to_string()),
            _ => Self::InternalServerError(format!("Database error: {err}")),
        }
    }
}

// Convert (StatusCode, String) to AppError
impl From<(StatusCode, String)> for AppError {
    fn from((status, message): (StatusCode, String)) -> Self {
        match status {
            StatusCode::BAD_REQUEST => Self::BadRequest(message),
            StatusCode::NOT_FOUND => Self::NotFound(message),
            StatusCode::CONFLICT => Self::Conflict(message),
            StatusCode::UNPROCESSABLE_ENTITY => Self::UnprocessableEntity(message),
            StatusCode::SERVICE_UNAVAILABLE => Self::ServiceUnavailable(message),
            _ => Self::InternalServerError(message),
        }
    }
}

// Helper function to convert any error to AppError
/// Creates a new internal server error with the given error message
///
/// # Arguments
/// * `err` - The error that occurred, which will be converted to a string
///
/// # Returns
/// An `AppError` with status code 500 (Internal Server Error)
pub fn internal_error<E: fmt::Display>(err: E) -> AppError {
    AppError::InternalServerError(err.to_string())
}

// Helper function for validation errors
/// Creates a new validation error with the given message
///
/// # Arguments
/// * `message` - A description of what validation failed
///
/// # Returns
/// An `AppError` with status code 422 (Unprocessable Entity)
pub fn validation_error(message: &str) -> AppError {
    AppError::UnprocessableEntity(message.to_string())
}

// Helper function for not found errors
/// Creates a new not found error for the specified resource
///
/// # Arguments
/// * `resource` - The type of resource that wasn't found (e.g., "wallet", "transaction")
/// * `id` - The ID that was being searched for
///
/// # Returns
/// An `AppError` with status code 404 (Not Found)
pub fn not_found_error(resource: &str, id: &str) -> AppError {
    AppError::NotFound(format!("{resource} with ID {id} not found"))
}

// Helper function for conflict errors
/// Creates a new conflict error with the given message
///
/// # Arguments
/// * `message` - A description of the conflict
///
/// # Returns
/// An `AppError` with status code 409 (Conflict)
pub fn conflict_error(message: &str) -> AppError {
    AppError::Conflict(message.to_string())
}
