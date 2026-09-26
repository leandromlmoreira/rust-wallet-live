use axum::{extract::FromRequestParts, http::header::AUTHORIZATION};

use crate::{app::AppState, error::AppError};

pub struct Admin;

impl FromRequestParts<AppState> for Admin {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(auth) = parts.headers.get(AUTHORIZATION) else {
            return Err(AppError::MissingAuthorization);
        };

        if auth.as_bytes() == state.config.admin_key.as_bytes() {
            Ok(Admin)
        } else {
            Err(AppError::InvalidCredentials)
        }
    }
}
