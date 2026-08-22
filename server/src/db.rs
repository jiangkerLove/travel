use chrono::NaiveDate;
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::error::AppError;
use crate::route::LatLng;

pub const USER_COLS: &str = "id, open_id, nickname, avatar, default_bill_visible, birthday, gender, female_role, work_start_year, work_start_month";

#[derive(sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct UserRow {
    pub id: i64,
    pub open_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub default_bill_visible: bool,
    pub birthday: Option<NaiveDate>,
    pub gender: i16,
    pub female_role: i16,
    pub work_start_year: Option<i32>,
    pub work_start_month: Option<i16>,
}

#[derive(sqlx::FromRow, Clone)]
pub struct TravelRow {
    pub id: i64,
    pub travel_name: String,
    pub destination: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub invite_code: String,
    pub status: i16,
    pub creator_id: i64,
    pub is_lock: bool,
    pub remark: Option<String>,
}

#[derive(sqlx::FromRow, Clone)]
#[allow(dead_code)]
pub struct MemberRow {
    pub id: i64,
    pub travel_id: i64,
    pub user_id: i64,
    pub role: i16,
    pub can_edit: bool,
    pub can_bill: bool,
    pub group_name: Option<String>,
    pub nickname: String,
    pub avatar: Option<String>,
    pub open_id: String,
}

#[derive(sqlx::FromRow, Clone)]
pub struct PlanRow {
    pub id: i64,
    pub travel_id: i64,
    pub day_num: i32,
    pub point_type: String,
    pub place_name: String,
    pub longitude: Option<Decimal>,
    pub latitude: Option<Decimal>,
    pub arrive_time: Option<chrono::NaiveTime>,
    pub leave_time: Option<chrono::NaiveTime>,
    pub stay_duration: Option<i32>,
    pub traffic_type: Option<String>,
    pub traffic_duration: Option<i32>,
    pub sort: i32,
    pub remark: Option<String>,
}

pub async fn find_user(pool: &PgPool, id: i64) -> Result<UserRow, AppError> {
    sqlx::query_as::<_, UserRow>(&format!(
        "SELECT {USER_COLS} FROM app_user WHERE id = $1"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Unauthorized("用户不存在".into()))
}

pub async fn require_member(pool: &PgPool, travel_id: i64, user_id: i64) -> Result<MemberRow, AppError> {
    sqlx::query_as::<_, MemberRow>(
        r#"
        SELECT m.id, m.travel_id, m.user_id, m.role, m.can_edit, m.can_bill, m.group_name,
               u.nickname, u.avatar, u.open_id
        FROM travel_member m
        JOIN app_user u ON u.id = m.user_id
        WHERE m.travel_id = $1 AND m.user_id = $2
        "#,
    )
    .bind(travel_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::Forbidden("你不是该旅途成员".into()))
}

pub async fn require_leader(pool: &PgPool, travel_id: i64, user_id: i64) -> Result<MemberRow, AppError> {
    let m = require_member(pool, travel_id, user_id).await?;
    if m.role != 1 {
        return Err(AppError::Forbidden("仅团长可执行该操作".into()));
    }
    Ok(m)
}

pub fn can_edit_plan(m: &MemberRow) -> bool {
    m.role == 1 || m.can_edit
}

pub fn can_write_bill(m: &MemberRow) -> bool {
    m.role == 1 || m.can_bill
}

pub async fn require_editor(pool: &PgPool, travel_id: i64, user_id: i64) -> Result<MemberRow, AppError> {
    let m = require_member(pool, travel_id, user_id).await?;
    let t = find_travel(pool, travel_id).await?;
    if t.status == 2 || crate::sample::is_sample_remark(&t.remark) {
        return Err(AppError::Forbidden("已归档旅途仅可查看".into()));
    }
    if !can_edit_plan(&m) {
        return Err(AppError::Forbidden("没有改行程权限".into()));
    }
    Ok(m)
}

pub async fn require_biller(pool: &PgPool, travel_id: i64, user_id: i64) -> Result<MemberRow, AppError> {
    let m = require_member(pool, travel_id, user_id).await?;
    let t = find_travel(pool, travel_id).await?;
    if t.status == 2 || crate::sample::is_sample_remark(&t.remark) {
        return Err(AppError::Forbidden("已归档旅途仅可查看".into()));
    }
    if !can_write_bill(&m) {
        return Err(AppError::Forbidden("没有记账权限".into()));
    }
    Ok(m)
}

pub async fn find_travel(pool: &PgPool, id: i64) -> Result<TravelRow, AppError> {
    sqlx::query_as::<_, TravelRow>(
        r#"
        SELECT id, travel_name, destination, start_date, end_date, invite_code,
               status, creator_id, is_lock, remark
        FROM travel WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("旅途不存在".into()))
}

pub async fn list_members(pool: &PgPool, travel_id: i64) -> Result<Vec<MemberRow>, AppError> {
    Ok(sqlx::query_as::<_, MemberRow>(
        r#"
        SELECT m.id, m.travel_id, m.user_id, m.role, m.can_edit, m.can_bill, m.group_name,
               u.nickname, u.avatar, u.open_id
        FROM travel_member m
        JOIN app_user u ON u.id = m.user_id
        WHERE m.travel_id = $1
        ORDER BY
          CASE WHEN NULLIF(TRIM(m.group_name), '') IS NULL THEN 1 ELSE 0 END,
          m.group_name NULLS LAST,
          m.role DESC,
          m.join_time ASC
        "#,
    )
    .bind(travel_id)
    .fetch_all(pool)
    .await?)
}

pub async fn list_plans(pool: &PgPool, travel_id: i64, day_num: Option<i32>) -> Result<Vec<PlanRow>, AppError> {
    if let Some(day) = day_num {
        Ok(sqlx::query_as::<_, PlanRow>(
            r#"
            SELECT id, travel_id, day_num, point_type, place_name, longitude, latitude,
                   arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
            FROM day_plan WHERE travel_id = $1 AND day_num = $2
            ORDER BY sort ASC, id ASC
            "#,
        )
        .bind(travel_id)
        .bind(day)
        .fetch_all(pool)
        .await?)
    } else {
        Ok(sqlx::query_as::<_, PlanRow>(
            r#"
            SELECT id, travel_id, day_num, point_type, place_name, longitude, latitude,
                   arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
            FROM day_plan WHERE travel_id = $1
            ORDER BY day_num ASC, sort ASC, id ASC
            "#,
        )
        .bind(travel_id)
        .fetch_all(pool)
        .await?)
    }
}

pub fn status_text(status: i16, end_date: NaiveDate) -> &'static str {
    if status == 2 {
        "已归档"
    } else if end_date < crate::util::shanghai_today() {
        "已结束"
    } else {
        "进行中"
    }
}

pub fn display_status(status: i16, end_date: NaiveDate) -> i16 {
    if status == 2 {
        2
    } else if end_date < crate::util::shanghai_today() {
        1
    } else {
        0
    }
}

#[derive(sqlx::FromRow)]
struct RouteCacheRow {
    traffic_type: Option<String>,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
    mode: String,
    from_nav: bool,
    distance_m: i32,
    duration_s: i32,
    provider: String,
    points: sqlx::types::Json<Vec<LatLng>>,
}

fn coord_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5
}

pub async fn load_route_cache(
    pool: &PgPool,
    from_id: i64,
    to_id: i64,
    traffic: Option<&str>,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
) -> Option<(String, Vec<LatLng>, bool, i32, i32)> {
    let row = sqlx::query_as::<_, RouteCacheRow>(
        r#"
        SELECT traffic_type, from_lat, from_lng, to_lat, to_lng, mode, from_nav,
               distance_m, duration_s, provider, points
        FROM route_cache
        WHERE from_plan_id = $1 AND to_plan_id = $2
        "#,
    )
    .bind(from_id)
    .bind(to_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;
    let traffic_ok = row.traffic_type.as_deref() == traffic;
    if traffic_ok
        && row.provider == "amap"
        && coord_eq(row.from_lat, from_lat)
        && coord_eq(row.from_lng, from_lng)
        && coord_eq(row.to_lat, to_lat)
        && coord_eq(row.to_lng, to_lng)
        && row.points.0.len() >= 2
    {
        Some((
            row.mode,
            row.points.0,
            row.from_nav,
            row.distance_m,
            row.duration_s,
        ))
    } else {
        None
    }
}

pub async fn save_route_cache(
    pool: &PgPool,
    from_id: i64,
    to_id: i64,
    traffic: Option<&str>,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
    mode: &str,
    from_nav: bool,
    distance_m: i32,
    duration_s: i32,
    points: &[LatLng],
) {
    let _ = sqlx::query(
        r#"
        INSERT INTO route_cache (
            from_plan_id, to_plan_id, traffic_type, from_lat, from_lng, to_lat, to_lng,
            mode, from_nav, distance_m, duration_s, provider, points, updated_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'amap',$12, NOW())
        ON CONFLICT (from_plan_id, to_plan_id) DO UPDATE SET
            traffic_type = EXCLUDED.traffic_type,
            from_lat = EXCLUDED.from_lat,
            from_lng = EXCLUDED.from_lng,
            to_lat = EXCLUDED.to_lat,
            to_lng = EXCLUDED.to_lng,
            mode = EXCLUDED.mode,
            from_nav = EXCLUDED.from_nav,
            distance_m = EXCLUDED.distance_m,
            duration_s = EXCLUDED.duration_s,
            provider = EXCLUDED.provider,
            points = EXCLUDED.points,
            updated_at = NOW()
        "#,
    )
    .bind(from_id)
    .bind(to_id)
    .bind(traffic)
    .bind(from_lat)
    .bind(from_lng)
    .bind(to_lat)
    .bind(to_lng)
    .bind(mode)
    .bind(from_nav)
    .bind(distance_m)
    .bind(duration_s)
    .bind(sqlx::types::Json(points.to_vec()))
    .execute(pool)
    .await;
}

pub async fn invalidate_route_cache(pool: &PgPool, plan_ids: &[i64]) {
    if plan_ids.is_empty() {
        return;
    }
    let _ = sqlx::query(
        "DELETE FROM route_cache WHERE from_plan_id = ANY($1) OR to_plan_id = ANY($1)",
    )
    .bind(plan_ids)
    .execute(pool)
    .await;
}
