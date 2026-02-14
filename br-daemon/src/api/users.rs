// br-daemon/src/api/users.rs
use crate::api::auth::{hash_password, AdminUser};
use crate::api::response::{ApiError, ApiResponse};
use crate::api::AppState;
use crate::config::UserConfig;
use crate::types::UserRole;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

/** Session information for a user. */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: usize,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

/** In-memory session store (shared across requests). */
#[derive(Debug, Default)]
pub struct SessionStore {
    sessions: parking_lot::RwLock<HashMap<Uuid, Session>>,
    user_last_login: parking_lot::RwLock<HashMap<usize, DateTime<Utc>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_session(
        &self,
        user_id: usize,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Session {
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            user_id,
            ip_address,
            user_agent,
            created_at: now,
            last_active: now,
        };

        // Update last login
        self.user_last_login.write().insert(user_id, now);

        // Store session
        self.sessions.write().insert(session.id, session.clone());

        session
    }

    pub fn get_sessions_for_user(&self, user_id: usize) -> Vec<Session> {
        self.sessions
            .read()
            .values()
            .filter(|s| s.user_id == user_id)
            .cloned()
            .collect()
    }

    pub fn get_session(&self, session_id: Uuid) -> Option<Session> {
        self.sessions.read().get(&session_id).cloned()
    }

    pub fn update_last_active(&self, session_id: Uuid) {
        if let Some(session) = self.sessions.write().get_mut(&session_id) {
            session.last_active = Utc::now();
        }
    }

    pub fn revoke_session(&self, session_id: Uuid) -> bool {
        self.sessions.write().remove(&session_id).is_some()
    }

    pub fn revoke_all_sessions_for_user(&self, user_id: usize) -> usize {
        let mut sessions = self.sessions.write();
        let to_remove: Vec<Uuid> = sessions
            .values()
            .filter(|s| s.user_id == user_id)
            .map(|s| s.id)
            .collect();
        let count = to_remove.len();
        for id in to_remove {
            sessions.remove(&id);
        }
        count
    }

    pub fn get_last_login(&self, user_id: usize) -> Option<DateTime<Utc>> {
        self.user_last_login.read().get(&user_id).copied()
    }

    pub fn has_active_sessions(&self, user_id: usize) -> bool {
        self.sessions.read().values().any(|s| s.user_id == user_id)
    }
}

/** User view without password hash. */
#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: usize,
    pub username: String,
    pub role: UserRole,
    pub last_login: Option<DateTime<Utc>>,
    pub is_online: bool,
}

#[derive(Debug, Serialize)]
pub struct UsersResponse {
    pub users: Vec<UserView>,
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub role: UserRole,
}

#[derive(Debug, Serialize)]
pub struct DeleteUserResponse {
    pub deleted: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateUserRequest {
    pub role: Option<UserRole>,
    pub password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionsResponse {
    pub sessions: Vec<Session>,
}

#[derive(Debug, Serialize)]
pub struct RevokeSessionResponse {
    pub revoked: bool,
}

#[derive(Debug, Serialize)]
pub struct RevokeAllSessionsResponse {
    pub revoked_count: usize,
}

/** List all users (admin only). */
pub async fn list_users(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<UsersResponse>> {
    let config = state.config.read();
    let users: Vec<UserView> = config
        .users
        .iter()
        .enumerate()
        .map(|(i, u)| UserView {
            id: i,
            username: u.username.clone(),
            role: u.role,
            last_login: state.session_store.get_last_login(i),
            is_online: state.session_store.has_active_sessions(i),
        })
        .collect();

    Json(ApiResponse::new(UsersResponse { users }))
}

/** Create a new user (admin only). */
pub async fn create_user(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<ApiResponse<UserView>>), (StatusCode, ApiError)> {
    // Validate username
    if request.username.is_empty() {
        return Err(ApiError::bad_request("Username cannot be empty"));
    }
    if request.password.len() < 8 {
        return Err(ApiError::bad_request(
            "Password must be at least 8 characters",
        ));
    }

    let password_hash = hash_password(&request.password)
        .map_err(|_| ApiError::internal("Failed to hash password"))?;

    let mut config = state.config.write();

    // Check if username already exists
    if config.users.iter().any(|u| u.username == request.username) {
        return Err(ApiError::bad_request("Username already exists"));
    }

    let user_config = UserConfig {
        username: request.username.clone(),
        password_hash,
        role: request.role,
    };

    config.users.push(user_config);
    let id = config.users.len() - 1;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::new(UserView {
            id,
            username: request.username,
            role: request.role,
            last_login: None,
            is_online: false,
        })),
    ))
}

/** Delete a user (admin only). */
pub async fn delete_user(
    admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
) -> Result<Json<ApiResponse<DeleteUserResponse>>, (StatusCode, ApiError)> {
    let mut config = state.config.write();

    if id >= config.users.len() {
        return Err(ApiError::not_found("User"));
    }

    // Cannot delete yourself
    if config.users[id].username == admin_user.username {
        return Err(ApiError::bad_request("Cannot delete yourself"));
    }

    // Revoke all sessions for this user
    state.session_store.revoke_all_sessions_for_user(id);

    config.users.remove(id);

    Ok(Json(ApiResponse::new(DeleteUserResponse { deleted: true })))
}

/** Update a user (admin only). */
pub async fn update_user(
    admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
    Json(request): Json<UpdateUserRequest>,
) -> Result<Json<ApiResponse<UserView>>, (StatusCode, ApiError)> {
    let mut config = state.config.write();

    if id >= config.users.len() {
        return Err(ApiError::not_found("User"));
    }

    // Cannot demote yourself from admin
    if let Some(new_role) = request.role {
        if config.users[id].username == admin_user.username && new_role != UserRole::Admin {
            return Err(ApiError::bad_request("Cannot demote yourself from admin"));
        }
        config.users[id].role = new_role;
    }

    // Update password if provided
    if let Some(password) = request.password {
        if password.len() < 8 {
            return Err(ApiError::bad_request(
                "Password must be at least 8 characters",
            ));
        }
        let password_hash = hash_password(&password)
            .map_err(|_| ApiError::internal("Failed to hash password"))?;
        config.users[id].password_hash = password_hash;
    }

    let user = &config.users[id];
    let view = UserView {
        id,
        username: user.username.clone(),
        role: user.role,
        last_login: state.session_store.get_last_login(id),
        is_online: state.session_store.has_active_sessions(id),
    };

    Ok(Json(ApiResponse::new(view)))
}

/** Get sessions for a user (admin only). */
pub async fn get_user_sessions(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
) -> Result<Json<ApiResponse<SessionsResponse>>, (StatusCode, ApiError)> {
    let config = state.config.read();

    if id >= config.users.len() {
        return Err(ApiError::not_found("User"));
    }

    let sessions = state.session_store.get_sessions_for_user(id);

    Ok(Json(ApiResponse::new(SessionsResponse { sessions })))
}

/** Revoke all sessions for a user (admin only). */
pub async fn revoke_all_user_sessions(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<usize>,
) -> Result<Json<ApiResponse<RevokeAllSessionsResponse>>, (StatusCode, ApiError)> {
    let config = state.config.read();

    if id >= config.users.len() {
        return Err(ApiError::not_found("User"));
    }

    let count = state.session_store.revoke_all_sessions_for_user(id);

    Ok(Json(ApiResponse::new(RevokeAllSessionsResponse {
        revoked_count: count,
    })))
}

/** Revoke a specific session for a user (admin only). */
pub async fn revoke_user_session(
    _admin_user: AdminUser,
    State(state): State<Arc<AppState>>,
    Path((user_id, session_id)): Path<(usize, Uuid)>,
) -> Result<Json<ApiResponse<RevokeSessionResponse>>, (StatusCode, ApiError)> {
    let config = state.config.read();

    if user_id >= config.users.len() {
        return Err(ApiError::not_found("User"));
    }

    // Verify the session belongs to this user
    let session = state
        .session_store
        .get_session(session_id)
        .ok_or_else(|| ApiError::not_found("Session"))?;

    if session.user_id != user_id {
        return Err(ApiError::not_found("Session"));
    }

    let revoked = state.session_store.revoke_session(session_id);

    Ok(Json(ApiResponse::new(RevokeSessionResponse { revoked })))
}
