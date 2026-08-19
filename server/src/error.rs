use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("数据库错误")]
    Db(#[from] sqlx::Error),
    #[error("{0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrBody {
    code: u16,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, m.clone()),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m.clone()),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, m.clone()),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, m.clone()),
            AppError::Db(e) => {
                tracing::error!("db error: {e:?}");
                (StatusCode::INTERNAL_SERVER_ERROR, "数据库错误".to_string())
            }
            AppError::Internal(m) => {
                tracing::error!("internal: {m}");
                (StatusCode::INTERNAL_SERVER_ERROR, m.clone())
            }
        };
        (
            status,
            Json(ErrBody {
                code: status.as_u16(),
                message,
            }),
        )
            .into_response()
    }
}

#[derive(Serialize)]
pub struct ApiOk<T: Serialize> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

pub fn ok<T: Serialize>(data: T) -> Json<ApiOk<T>> {
    Json(ApiOk {
        code: 0,
        message: "ok".into(),
        data,
    })
}
