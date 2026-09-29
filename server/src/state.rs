use sqlx::PgPool;

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
}

#[derive(Clone)]
pub struct AuthUser {
    pub id: i64,
}
