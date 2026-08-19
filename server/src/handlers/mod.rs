use axum::{
    routing::{delete, get, post},
    Json, Router,
};
use serde::Serialize;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{error::ok, state::AppState};

mod bill;
mod dev;
mod plan;
mod travel;
mod user;

pub fn router(state: AppState) -> Router {
    let public = Router::new()
        .route("/health", get(health))
        .route("/api/user/login", post(user::login))
        .route("/api/dev/seed", post(dev::seed));

    let api = Router::new()
        .route("/api/user/info", get(user::info).post(user::update))
        .route("/api/travel/create", post(travel::create))
        .route("/api/travel/list", get(travel::list))
        .route("/api/travel/detail", get(travel::detail))
        .route("/api/travel/join", post(travel::join))
        .route("/api/travel/member", get(travel::member))
        .route("/api/travel/lock", post(travel::lock))
        .route("/api/travel/quit", post(travel::quit))
        .route("/api/travel/archive", post(travel::archive))
        .route("/api/travel/remove", post(travel::remove))
        .route("/api/travel/perm", post(travel::set_perm))
        .route("/api/plan/save", post(plan::save))
        .route("/api/plan/list", get(plan::list))
        .route("/api/plan/del", delete(plan::del).post(plan::del))
        .route("/api/plan/sort", post(plan::sort))
        .route("/api/map/global", get(plan::map_global))
        .route("/api/map/day", get(plan::map_day))
        .route("/api/map/search", get(plan::map_search))
        .route("/api/map/regeo", get(plan::map_regeo))
        .route("/api/bill/save", post(bill::save))
        .route("/api/bill/list", get(bill::list))
        .route("/api/bill/del", delete(bill::del).post(bill::del))
        .route("/api/stat/total", get(bill::stat))
        .route("/api/settle/calc", get(bill::settle_calc));

    Router::new()
        .merge(public)
        .merge(api)
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}

#[derive(Serialize)]
struct Health {
    ok: bool,
}

async fn health() -> Json<crate::error::ApiOk<Health>> {
    ok(Health { ok: true })
}

