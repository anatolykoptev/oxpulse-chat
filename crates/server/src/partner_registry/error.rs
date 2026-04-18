//! Registration error taxonomy and HTTP status mapping.
//!
//! Kept in its own file so SQL logic, HTTP handler, and docs can depend
//! on the error surface without pulling in transaction internals.

use axum::http::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("token not found")]
    TokenNotFound,
    #[error("token already used")]
    TokenAlreadyUsed,
    #[error("token revoked")]
    TokenRevoked,
    #[error("token expired")]
    TokenExpired,
    #[error("partner mismatch")]
    PartnerMismatch,
    #[error("reality credentials not configured")]
    RealityNotConfigured,
    #[error("turn secret not configured")]
    TurnNotConfigured,
    #[error("partner backend endpoint not configured")]
    BackendEndpointNotConfigured,
    #[error("internal error: {0}")]
    Internal(String),
}

impl RegistrationError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::TokenNotFound => "token_not_found",
            Self::TokenAlreadyUsed => "token_already_used",
            Self::TokenRevoked => "token_revoked",
            Self::TokenExpired => "token_expired",
            Self::PartnerMismatch => "partner_mismatch",
            Self::RealityNotConfigured => "reality_not_configured",
            Self::TurnNotConfigured => "turn_not_configured",
            Self::BackendEndpointNotConfigured => "backend_endpoint_not_configured",
            Self::Internal(_) => "internal_error",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::TokenAlreadyUsed => StatusCode::CONFLICT,
            Self::RealityNotConfigured | Self::TurnNotConfigured | Self::BackendEndpointNotConfigured | Self::Internal(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
            _ => StatusCode::FORBIDDEN,
        }
    }
}

impl From<sqlx::Error> for RegistrationError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_status_mapping() {
        assert_eq!(
            RegistrationError::TokenNotFound.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            RegistrationError::TokenAlreadyUsed.status(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            RegistrationError::TokenExpired.status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            RegistrationError::RealityNotConfigured.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(
            RegistrationError::BackendEndpointNotConfigured.status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn error_code_strings_are_snake_case() {
        assert_eq!(RegistrationError::TokenNotFound.code(), "token_not_found");
        assert_eq!(
            RegistrationError::PartnerMismatch.code(),
            "partner_mismatch"
        );
        assert_eq!(
            RegistrationError::BackendEndpointNotConfigured.code(),
            "backend_endpoint_not_configured"
        );
    }
}
