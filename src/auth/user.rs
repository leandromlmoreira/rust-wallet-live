use std::convert::Infallible;

use axum::extract::FromRequestParts;
use axum_extra::extract::CookieJar;
use jwt_simple::{
    claims::Claims,
    prelude::{Duration, HS256Key, MACLike},
};
use password_auth::VerifyError;
use serde::{Deserialize, Serialize};

use crate::{app::AppState, error::AppError, repository::Repository};

/// Nome do cookie que guarda o JWT da sessão.
pub const TOKEN_COOKIE: &str = "token";
/// Validade da sessão.
pub const SESSION_HOURS: u64 = 8;

pub struct UnauthenticatedUser {
    username: String,
    password: String,
}

impl UnauthenticatedUser {
    pub fn new(username: String, password: String) -> Self {
        Self { username, password }
    }

    pub async fn authenticate(&self, repository: &Repository) -> Result<User, AppError> {
        let user_record = match repository.get_user_by_name(&self.username).await? {
            Some(user_record) => user_record,
            None => return Err(AppError::UserDoesNotExist),
        };

        match password_auth::verify_password(&self.password, &user_record.password_hash) {
            Ok(()) => Ok(User::new(user_record.id, user_record.username)),
            Err(VerifyError::PasswordInvalid) => Err(AppError::InvalidCredentials),
            // Um hash corrompido não deve derrubar o servidor: tratamos como credencial inválida.
            Err(VerifyError::Parse(_)) => Err(AppError::InvalidCredentials),
        }
    }

    pub async fn register(self, repository: &Repository) -> Result<User, AppError> {
        let password_hash = password_auth::generate_hash(self.password);
        let user_record = match repository.add_user(&self.username, &password_hash).await {
            Ok(user_record) => user_record,
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                return Err(AppError::UsernameTaken);
            }
            Err(err) => return Err(AppError::Database(err)),
        };

        Ok(User::new(user_record.id, user_record.username))
    }
}

pub struct User {
    id: i64,
    username: String,
}

impl User {
    fn new(id: i64, username: String) -> Self {
        Self { id, username }
    }

    pub const fn username(&self) -> &String {
        &self.username
    }

    pub const fn id(&self) -> i64 {
        self.id
    }

    pub fn auth_token(self, secret: &[u8]) -> Result<String, AppError> {
        let key = HS256Key::from_bytes(secret);
        let claims =
            Claims::with_custom_claims(UserClaims::from(self), Duration::from_hours(SESSION_HOURS));
        let token = key.authenticate(claims)?;
        Ok(token)
    }

    pub fn from_auth_token(token: &str, secret: &[u8]) -> Result<Self, AppError> {
        let key = HS256Key::from_bytes(secret);
        let claims: UserClaims = key.verify_token(token, None)?.custom;
        Ok(Self::new(claims.id, claims.username))
    }
}

impl FromRequestParts<AppState> for User {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);

        let token = match jar.get(TOKEN_COOKIE) {
            Some(token) => token.value(),
            None => return Err(AppError::MissingAuthorization),
        };

        User::from_auth_token(token, &state.config.jwt_secret)
    }
}

impl FromRequestParts<AppState> for Option<User> {
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(User::from_request_parts(parts, state).await.ok())
    }
}

#[derive(Serialize, Deserialize)]
struct UserClaims {
    id: i64,
    username: String,
}

impl From<User> for UserClaims {
    fn from(User { id, username }: User) -> Self {
        Self { id, username }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret-with-enough-bytes";

    #[test]
    fn token_round_trip_keeps_identity() {
        let token = User::new(7, "ana".to_string()).auth_token(SECRET).unwrap();
        let user = User::from_auth_token(&token, SECRET).unwrap();

        assert_eq!(user.id(), 7);
        assert_eq!(user.username(), "ana");
    }

    #[test]
    fn token_signed_with_another_secret_is_rejected() {
        let token = User::new(7, "ana".to_string())
            .auth_token(b"another-secret-entirely")
            .unwrap();

        assert!(User::from_auth_token(&token, SECRET).is_err());
    }
}
