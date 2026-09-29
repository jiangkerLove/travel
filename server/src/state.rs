use sqlx::PgPool;

use crate::client::ClientInfo;

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub jwt_secret: String,
    pub wechat_appid: String,
    pub wechat_secret: String,
    pub amap_key: String,
    pub amap_secret: String,
    pub deepseek_api_key: String,
    pub dev_mode: bool,
    /// `REVIEW_VERSION`。客户端版本与它按段数值一致时，视为审核包。空则没有任何版本处于审核。
    pub review_version: Option<(u32, u32, u32)>,
}

impl AppState {
    pub fn is_review(&self, client: &ClientInfo) -> bool {
        client.matches(self.review_version)
    }
}

#[derive(Clone)]
pub struct AuthUser {
    pub id: i64,
}
