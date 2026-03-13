// br-daemon/src/api/auth.rs
use crate::api::LocalOnlyMode;
use crate::types::UserRole;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // username
    pub role: UserRole,
    pub exp: usize, // expiry timestamp
}

/** Detailed token validation errors for refresh flow. */
#[derive(Debug)]
pub enum TokenError {
    /** Token has expired but claims were successfully decoded (for refresh within grace period). */
    Expired { claims: Claims },
    /** Token signature is invalid or other validation error. */
    Invalid(String),
    /** Token is malformed and cannot be parsed. */
    Malformed,
}

/** Error codes for auth responses. */
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthErrorCode {
    TokenExpired,
    TokenInvalid,
    TokenMissing,
    Unauthorized,
    Forbidden,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub role: UserRole,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
pub struct AuthError {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<AuthErrorCode>,
}

impl AuthError {
    pub fn new(error: impl Into<String>) -> Self {
        Self {
            error: error.into(),
            code: None,
        }
    }

    pub fn with_code(error: impl Into<String>, code: AuthErrorCode) -> Self {
        Self {
            error: error.into(),
            code: Some(code),
        }
    }

    pub fn token_expired() -> Self {
        Self::with_code("Token has expired", AuthErrorCode::TokenExpired)
    }

    pub fn token_invalid() -> Self {
        Self::with_code("Invalid token", AuthErrorCode::TokenInvalid)
    }

    pub fn token_missing() -> Self {
        Self::with_code("Missing Authorization header", AuthErrorCode::TokenMissing)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

pub fn create_token(
    username: &str,
    role: UserRole,
    secret: &str,
    duration_hours: u64,
) -> Result<(String, chrono::DateTime<chrono::Utc>), jsonwebtoken::errors::Error> {
    let expiry = chrono::Utc::now() + chrono::Duration::hours(duration_hours as i64);
    let claims = Claims {
        sub: username.to_string(),
        role,
        exp: expiry.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;
    Ok((token, expiry))
}

pub fn verify_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/** Decode a token without validating expiry (for refresh flow). */
pub fn decode_token_unvalidated(
    token: &str,
    secret: &str,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = false;

    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(token_data.claims)
}

/** Verify token with detailed error types for refresh handling. */
pub fn verify_token_detailed(token: &str, secret: &str) -> Result<Claims, TokenError> {
    match verify_token(token, secret) {
        Ok(claims) => Ok(claims),
        Err(e) => {
            if matches!(e.kind(), jsonwebtoken::errors::ErrorKind::ExpiredSignature) {
                // Token expired - try to decode without expiry validation to get claims
                match decode_token_unvalidated(token, secret) {
                    Ok(claims) => Err(TokenError::Expired { claims }),
                    Err(_) => Err(TokenError::Invalid(e.to_string())),
                }
            } else if matches!(e.kind(), jsonwebtoken::errors::ErrorKind::InvalidToken) {
                Err(TokenError::Malformed)
            } else {
                Err(TokenError::Invalid(e.to_string()))
            }
        }
    }
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    bcrypt::verify(password, hash).unwrap_or(false)
}

pub fn hash_password(password: &str) -> Result<String, bcrypt::BcryptError> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST)
}

/** Extractor for authenticated requests. */
pub struct AuthUser {
    pub username: String,
    pub role: UserRole,
}

/** Extractor for admin-only requests. */
pub struct AdminUser {
    pub username: String,
}

#[derive(Clone)]
pub struct JwtSecret(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Check if running in local-only mode (skip auth)
        if let Some(LocalOnlyMode(true)) = parts.extensions.get::<LocalOnlyMode>() {
            return Ok(AuthUser {
                username: "local".to_string(),
                role: UserRole::Admin,
            });
        }

        let secret = parts
            .extensions
            .get::<JwtSecret>()
            .ok_or_else(|| AuthError::new("Server configuration error"))?;

        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| AuthError::token_missing())?;

        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            AuthError::with_code(
                "Invalid Authorization header format",
                AuthErrorCode::TokenInvalid,
            )
        })?;

        let claims = verify_token_detailed(token, &secret.0).map_err(|e| match e {
            TokenError::Expired { .. } => AuthError::token_expired(),
            TokenError::Invalid(_) | TokenError::Malformed => AuthError::token_invalid(),
        })?;

        Ok(AuthUser {
            username: claims.sub,
            role: claims.role,
        })
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for AdminUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state).await?;
        if user.role != UserRole::Admin {
            return Err(AuthError::with_code(
                "Admin access required",
                AuthErrorCode::Forbidden,
            ));
        }
        Ok(AdminUser {
            username: user.username,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-jwt-tokens";

    // JWT Tests
    #[test]
    fn test_create_token_valid() {
        let result = create_token("admin", UserRole::Admin, TEST_SECRET, 24);
        assert!(result.is_ok());
        let (token, _) = result.unwrap();
        assert!(!token.is_empty());
    }

    #[test]
    fn test_create_token_includes_claims() {
        let (token, expiry) = create_token("testuser", UserRole::Viewer, TEST_SECRET, 1).unwrap();

        // Verify by decoding
        let claims = verify_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.role, UserRole::Viewer);

        // Expiry should be approximately 1 hour from now
        let expected_exp = expiry.timestamp() as usize;
        assert_eq!(claims.exp, expected_exp);
    }

    #[test]
    fn test_verify_token_valid() {
        let (token, _) = create_token("admin", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let result = verify_token(&token, TEST_SECRET);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_token_wrong_secret() {
        let (token, _) = create_token("admin", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let result = verify_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_malformed() {
        let result = verify_token("not-a-valid-jwt", TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_token_empty() {
        let result = verify_token("", TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_roundtrip_admin_role() {
        let (token, _) = create_token("admin_user", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let claims = verify_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "admin_user");
        assert_eq!(claims.role, UserRole::Admin);
    }

    #[test]
    fn test_token_roundtrip_viewer_role() {
        let (token, _) = create_token("viewer_user", UserRole::Viewer, TEST_SECRET, 24).unwrap();
        let claims = verify_token(&token, TEST_SECRET).unwrap();
        assert_eq!(claims.sub, "viewer_user");
        assert_eq!(claims.role, UserRole::Viewer);
    }

    #[test]
    fn test_token_different_users_different_tokens() {
        let (token1, _) = create_token("user1", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let (token2, _) = create_token("user2", UserRole::Admin, TEST_SECRET, 24).unwrap();
        assert_ne!(token1, token2);
    }

    // Password Tests
    #[test]
    fn test_hash_password_returns_hash() {
        let result = hash_password("secret123");
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert!(!hash.is_empty());
        // bcrypt hashes start with $2b$ or $2a$
        assert!(hash.starts_with("$2"));
    }

    #[test]
    fn test_verify_password_correct() {
        let hash = hash_password("mypassword").unwrap();
        assert!(verify_password("mypassword", &hash));
    }

    #[test]
    fn test_verify_password_incorrect() {
        let hash = hash_password("correctpassword").unwrap();
        assert!(!verify_password("wrongpassword", &hash));
    }

    #[test]
    fn test_hash_password_different_each_time() {
        let hash1 = hash_password("samepassword").unwrap();
        let hash2 = hash_password("samepassword").unwrap();
        // Bcrypt uses random salt, so hashes should differ
        assert_ne!(hash1, hash2);
        // But both should verify correctly
        assert!(verify_password("samepassword", &hash1));
        assert!(verify_password("samepassword", &hash2));
    }

    #[test]
    fn test_verify_password_invalid_hash() {
        // Should return false for invalid hash format
        assert!(!verify_password("password", "not-a-valid-hash"));
    }

    #[test]
    fn test_verify_password_empty_hash() {
        assert!(!verify_password("password", ""));
    }

    #[test]
    fn test_hash_password_empty_input() {
        // bcrypt should handle empty passwords
        let result = hash_password("");
        assert!(result.is_ok());
        let hash = result.unwrap();
        assert!(verify_password("", &hash));
    }

    #[test]
    fn test_hash_password_unicode() {
        let password = "密码🔐";
        let hash = hash_password(password).unwrap();
        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn test_verify_password_case_sensitive() {
        let hash = hash_password("Password").unwrap();
        assert!(verify_password("Password", &hash));
        assert!(!verify_password("password", &hash));
        assert!(!verify_password("PASSWORD", &hash));
    }

    // Token detailed verification tests
    #[test]
    fn test_verify_token_detailed_valid() {
        let (token, _) = create_token("admin", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let result = verify_token_detailed(&token, TEST_SECRET);
        assert!(result.is_ok());
        let claims = result.unwrap();
        assert_eq!(claims.sub, "admin");
        assert_eq!(claims.role, UserRole::Admin);
    }

    #[test]
    fn test_verify_token_detailed_wrong_secret() {
        let (token, _) = create_token("admin", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let result = verify_token_detailed(&token, "wrong-secret");
        assert!(matches!(result, Err(TokenError::Invalid(_))));
    }

    #[test]
    fn test_verify_token_detailed_malformed() {
        let result = verify_token_detailed("not-a-valid-jwt", TEST_SECRET);
        assert!(matches!(
            result,
            Err(TokenError::Invalid(_)) | Err(TokenError::Malformed)
        ));
    }

    #[test]
    fn test_decode_token_unvalidated() {
        let (token, _) = create_token("testuser", UserRole::Viewer, TEST_SECRET, 24).unwrap();
        let result = decode_token_unvalidated(&token, TEST_SECRET);
        assert!(result.is_ok());
        let claims = result.unwrap();
        assert_eq!(claims.sub, "testuser");
        assert_eq!(claims.role, UserRole::Viewer);
    }

    #[test]
    fn test_decode_token_unvalidated_wrong_secret() {
        let (token, _) = create_token("admin", UserRole::Admin, TEST_SECRET, 24).unwrap();
        let result = decode_token_unvalidated(&token, "wrong-secret");
        assert!(result.is_err());
    }

    // AuthError tests
    #[test]
    fn test_auth_error_new() {
        let err = AuthError::new("Test error");
        assert_eq!(err.error, "Test error");
        assert!(err.code.is_none());
    }

    #[test]
    fn test_auth_error_with_code() {
        let err = AuthError::with_code("Token expired", AuthErrorCode::TokenExpired);
        assert_eq!(err.error, "Token expired");
        assert_eq!(err.code, Some(AuthErrorCode::TokenExpired));
    }

    #[test]
    fn test_auth_error_helpers() {
        let expired = AuthError::token_expired();
        assert_eq!(expired.code, Some(AuthErrorCode::TokenExpired));

        let invalid = AuthError::token_invalid();
        assert_eq!(invalid.code, Some(AuthErrorCode::TokenInvalid));

        let missing = AuthError::token_missing();
        assert_eq!(missing.code, Some(AuthErrorCode::TokenMissing));
    }
}
