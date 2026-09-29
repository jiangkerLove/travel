mod ai;
mod ai_log;
mod auth;
mod client;
mod db;
mod error;
mod handlers;
mod moderation;
mod poi;
mod route;
mod route_thumb;
mod sample;
mod settle;
mod state;
mod util;
mod worklife;

use std::env;

use sqlx::postgres::PgPoolOptions;
use sqlx::Executor;
use tracing_subscriber::EnvFilter;

use crate::state::AppState;

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("server=debug,sqlx=warn,tower_http=info")),
        )
        .init();

    let db_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
        "postgres://travel:travel@127.0.0.1:5432/travel".into()
    });
    let jwt_secret = env::var("JWT_SECRET").unwrap_or_else(|_| "travel-dev-secret-change-me".into());
    let wechat_appid = env::var("WECHAT_APPID").unwrap_or_default().trim().to_string();
    let wechat_secret = env::var("WECHAT_SECRET").unwrap_or_default().trim().to_string();
    let amap_key = env::var("AMAP_KEY").unwrap_or_default().trim().to_string();
    let amap_secret = env::var("AMAP_SECRET").unwrap_or_default().trim().to_string();
    let deepseek_api_key = env::var("DEEPSEEK_API_KEY").unwrap_or_default().trim().to_string();
    if amap_key.is_empty() {
        tracing::info!("未配置 AMAP_KEY：路书用点到点连线并估算时间；填高德 Web 服务 Key 后按驾车/步行规划");
    }
    if deepseek_api_key.is_empty() {
        tracing::info!("未配置 DEEPSEEK_API_KEY：AI 排行程不可用");
    } else {
        tracing::info!("已启用 DeepSeek AI 排行程");
    }
    if wechat_appid.is_empty() || wechat_secret.is_empty() {
        tracing::info!("未配置 WECHAT_APPID/WECHAT_SECRET：开发模式用本地身份；真机请填写小程序 AppID 和 AppSecret");
    } else {
        tracing::info!("已启用微信 OpenID 静默识别");
    }
    let dev_mode = env::var("DEV_MODE").unwrap_or_else(|_| "1".into()) != "0";
    let port = env::var("PORT").unwrap_or_else(|_| "3000".into());
    let review_raw = env::var("REVIEW_VERSION").unwrap_or_default();
    let review_version = crate::client::parse_version(review_raw.trim());
    if review_raw.trim().is_empty() {
        tracing::info!("未配置 REVIEW_VERSION：邀请码对所有版本开放");
    } else if review_version.is_none() {
        tracing::warn!("REVIEW_VERSION 无法解析，已忽略：{}", review_raw.trim());
    } else {
        tracing::info!("审核版本 {}：该版本不提供邀请码", review_raw.trim());
    }

    tracing::info!("connecting database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                // TIMESTAMP 无时区，NOW() 会按会话时区落成钟面时间
                conn.execute("SET TIME ZONE 'Asia/Shanghai'").await?;
                Ok(())
            })
        })
        .connect(&db_url)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("数据库连接失败: {e}");
            tracing::error!("请先启动 PostgreSQL，并确认 DATABASE_URL={db_url}");
            std::process::exit(1);
        });

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations failed");

    let state = AppState {
        pool,
        jwt_secret,
        wechat_appid,
        wechat_secret,
        amap_key,
        amap_secret,
        deepseek_api_key,
        dev_mode,
        review_version,
    };
    let app = handlers::router(state);
    let addr = format!("0.0.0.0:{port}");
    tracing::info!("travel api listening on {addr}");
    let listener = tokio::net::TcpListener::bind(&addr).await.expect("bind port");
    axum::serve(listener, app).await.expect("server");
}
