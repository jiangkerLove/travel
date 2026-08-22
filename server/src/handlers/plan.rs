use axum::{
    extract::{Query, State},
    Json,
};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    db::{
        clear_travel_route_cache, find_travel, invalidate_route_cache, list_plans, load_route_cache,
        require_editor, require_member, save_route_cache, PlanRow,
    },
    error::{ok, ApiOk, AppError},
    route::{plan_route, LatLng},
    poi::{reverse_geocode, search_places},
    state::{AppState, AuthUser},
    util::{
        day_count, opt_coord_to_f64, parse_time, valid_point_type, valid_traffic_type,
    },
};

#[derive(Deserialize)]
pub struct SaveReq {
    pub id: Option<i64>,
    pub travel_id: i64,
    pub day_num: i32,
    pub point_type: String,
    pub place_name: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub arrive_time: Option<String>,
    pub leave_time: Option<String>,
    pub stay_duration: Option<i32>,
    pub traffic_type: Option<String>,
    pub traffic_duration: Option<i32>,
    pub remark: Option<String>,
    pub after_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct ListQ {
    pub travel_id: i64,
    pub day_num: Option<i32>,
    /// 为 0 时跳过路书计算，仅返回排程点位（编辑态用）
    pub routes: Option<i16>,
    /// 为 1 时忽略已有路书缓存，重新向高德要当天路线
    pub fresh: Option<i16>,
    /// 为 1 时只读数据库路书缓存，缺失段不请求高德
    pub cache_only: Option<i16>,
}

#[derive(Deserialize)]
pub struct SearchQ {
    pub q: String,
    pub lng: Option<f64>,
    pub lat: Option<f64>,
}

#[derive(Deserialize)]
pub struct RegeoQ {
    pub lng: f64,
    pub lat: f64,
}

#[derive(Deserialize)]
pub struct DelReq {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct SortReq {
    pub travel_id: i64,
    /// 目标天：ids 会按顺序排到这一天（可跨天挪入）
    pub day_num: i32,
    pub ids: Vec<i64>,
}

#[derive(Deserialize)]
pub struct MoveReq {
    pub travel_id: i64,
    pub id: i64,
    pub day_num: i32,
    /// 插到该点之后；不传则放到当天末尾
    pub after_id: Option<i64>,
}

#[derive(Serialize, Clone)]
pub struct PlanVo {
    pub id: i64,
    pub travel_id: i64,
    pub day_num: i32,
    pub point_type: String,
    pub place_name: String,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub arrive_time: Option<String>,
    pub leave_time: Option<String>,
    pub stay_duration: Option<i32>,
    pub traffic_type: Option<String>,
    pub traffic_duration: Option<i32>,
    pub sort: i32,
    pub remark: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_distance_m: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_duration_s: Option<i32>,
}

#[derive(Serialize, Clone)]
pub struct StartFromVo {
    pub id: i64,
    pub place_name: String,
    pub day_num: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub point_type: Option<String>,
}

#[derive(Serialize)]
pub struct DayVo {
    pub day_num: i32,
    pub date: String,
    pub plans: Vec<PlanVo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_from: Option<StartFromVo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_distance_m: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_duration_s: Option<i32>,
}

#[derive(Serialize)]
pub struct PlanListVo {
    pub day_count: i32,
    pub start_date: String,
    pub days: Vec<DayVo>,
}

#[derive(Serialize, Clone)]
pub struct MapLineVo {
    pub from_id: i64,
    pub to_id: i64,
    pub traffic_type: Option<String>,
    pub mode: String,
    pub color: String,
    pub dotted: bool,
    pub from_nav: bool,
    pub distance_m: i32,
    pub duration_s: i32,
    pub points: Vec<LatLng>,
}

#[derive(Serialize)]
pub struct MapVo {
    pub points: Vec<PlanVo>,
    pub lines: Vec<MapLineVo>,
}

fn fmt_time(t: Option<chrono::NaiveTime>) -> Option<String> {
    t.map(|v| v.format("%H:%M").to_string())
}

pub fn to_vo(p: &PlanRow) -> PlanVo {
    PlanVo {
        id: p.id,
        travel_id: p.travel_id,
        day_num: p.day_num,
        point_type: p.point_type.clone(),
        place_name: p.place_name.clone(),
        longitude: opt_coord_to_f64(p.longitude),
        latitude: opt_coord_to_f64(p.latitude),
        arrive_time: fmt_time(p.arrive_time),
        leave_time: fmt_time(p.leave_time),
        stay_duration: p.stay_duration,
        traffic_type: p.traffic_type.clone(),
        traffic_duration: p.traffic_duration,
        sort: p.sort,
        remark: p.remark.clone(),
        next_distance_m: None,
        next_duration_s: None,
    }
}

fn same_stay(a: &PlanVo, b: &PlanVo) -> bool {
    a.place_name.trim() == b.place_name.trim()
}

fn traffic_style(t: Option<&str>) -> (String, bool) {
    match t.unwrap_or("drive") {
        "walk" => ("#8B8B8B".into(), true),
        "drive" => ("#0052D9".into(), false),
        "highspeed" => ("#7C3AED".into(), true),
        "train" => ("#008858".into(), true),
        "plane" => ("#E37318".into(), true),
        "bus" => ("#BE5A00".into(), false),
        _ => ("#366EF4".into(), false),
    }
}

fn build_lines(points: &[PlanVo]) -> Vec<MapLineVo> {
    let mut lines = Vec::new();
    for i in 0..points.len().saturating_sub(1) {
        let a = &points[i];
        let b = &points[i + 1];
        if a.latitude.is_none() || a.longitude.is_none() || b.latitude.is_none() || b.longitude.is_none()
        {
            continue;
        }
        let (color, dotted) = traffic_style(b.traffic_type.as_deref());
        lines.push(MapLineVo {
            from_id: a.id,
            to_id: b.id,
            traffic_type: b.traffic_type.clone(),
            mode: String::new(),
            color,
            dotted,
            from_nav: false,
            distance_m: 0,
            duration_s: 0,
            points: vec![],
        });
    }
    lines
}

async fn with_routes(
    pool: &sqlx::PgPool,
    key: &str,
    sk: &str,
    points: &[PlanVo],
    force: bool,
    cache_only: bool,
) -> Vec<MapLineVo> {
    let mut lines = build_lines(points);
    for line in &mut lines {
        let Some(a) = points.iter().find(|p| p.id == line.from_id) else {
            continue;
        };
        let Some(b) = points.iter().find(|p| p.id == line.to_id) else {
            continue;
        };
        let from_lat = a.latitude.unwrap_or(0.0);
        let from_lng = a.longitude.unwrap_or(0.0);
        let to_lat = b.latitude.unwrap_or(0.0);
        let to_lng = b.longitude.unwrap_or(0.0);
        let traffic = b.traffic_type.as_deref();
        if !force {
            if let Some((mode, pts, from_nav, distance_m, duration_s)) = load_route_cache(
                pool, a.id, b.id, traffic, from_lat, from_lng, to_lat, to_lng,
            )
            .await
            {
                line.mode = mode;
                line.points = pts;
                line.from_nav = from_nav;
                line.distance_m = distance_m;
                line.duration_s = duration_s;
                continue;
            }
        }
        if cache_only {
            continue;
        }
        let result = plan_route(key, sk, traffic, from_lat, from_lng, to_lat, to_lng).await;
        line.mode = result.mode;
        line.points = result.points;
        line.from_nav = result.from_nav;
        line.distance_m = result.distance_m;
        line.duration_s = result.duration_s;
        if result.from_nav {
            save_route_cache(
                pool,
                a.id,
                b.id,
                traffic,
                from_lat,
                from_lng,
                to_lat,
                to_lng,
                &line.mode,
                line.from_nav,
                line.distance_m,
                line.duration_s,
                &line.points,
            )
            .await;
        }
    }
    lines
}

pub async fn save(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<SaveReq>,
) -> Result<Json<ApiOk<PlanVo>>, AppError> {
    require_editor(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    let days = day_count(t.start_date, t.end_date);
    if req.day_num < 1 || req.day_num > days {
        return Err(AppError::BadRequest("天数不在旅途范围内".into()));
    }
    if !valid_point_type(&req.point_type) {
        return Err(AppError::BadRequest("点位类型不合法".into()));
    }
    let name = req.place_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("地点名称不能为空".into()));
    }
    if let Some(tr) = &req.traffic_type {
        if !tr.is_empty() && !valid_traffic_type(tr) {
            return Err(AppError::BadRequest("交通方式不合法".into()));
        }
    }
    let lng = req.longitude.and_then(Decimal::from_f64);
    let lat = req.latitude.and_then(Decimal::from_f64);
    let arrive = parse_time(&req.arrive_time)?;
    let leave = parse_time(&req.leave_time)?;
    let traffic = req
        .traffic_type
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let traffic_duration = match req.traffic_duration {
        Some(n) if n <= 0 => None,
        Some(n) if n > 10000 => {
            return Err(AppError::BadRequest("交通时间过长".into()));
        }
        other => other,
    };

    let insert_after = req.after_id;
    let row = if let Some(id) = req.id {
        sqlx::query_as::<_, PlanRow>(
            r#"
            UPDATE day_plan SET
                day_num=$2, point_type=$3, place_name=$4, longitude=$5, latitude=$6,
                arrive_time=$7, leave_time=$8, stay_duration=$9, traffic_type=$10,
                traffic_duration=$11, remark=$12
            WHERE id=$1 AND travel_id=$13
            RETURNING id, travel_id, day_num, point_type, place_name, longitude, latitude,
                      arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
            "#,
        )
        .bind(id)
        .bind(req.day_num)
        .bind(&req.point_type)
        .bind(name)
        .bind(lng)
        .bind(lat)
        .bind(arrive)
        .bind(leave)
        .bind(req.stay_duration)
        .bind(&traffic)
        .bind(traffic_duration)
        .bind(req.remark.as_deref())
        .bind(req.travel_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("行程点位不存在".into()))?
    } else {
        let mut tx = state.pool.begin().await?;
        let sort = if let Some(after_id) = insert_after {
            #[derive(sqlx::FromRow)]
            struct AfterRow {
                sort: i32,
                day_num: i32,
            }
            let after = sqlx::query_as::<_, AfterRow>(
                "SELECT sort, day_num FROM day_plan WHERE id=$1 AND travel_id=$2",
            )
            .bind(after_id)
            .bind(req.travel_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| AppError::BadRequest("插入位置不存在".into()))?;
            if after.day_num != req.day_num {
                return Err(AppError::BadRequest("只能插在同一天".into()));
            }
            sqlx::query(
                "UPDATE day_plan SET sort = sort + 1 WHERE travel_id=$1 AND day_num=$2 AND sort > $3",
            )
            .bind(req.travel_id)
            .bind(req.day_num)
            .bind(after.sort)
            .execute(&mut *tx)
            .await?;
            after.sort + 1
        } else {
            sqlx::query_scalar::<_, i32>(
                "SELECT COALESCE(MAX(sort), -1) FROM day_plan WHERE travel_id=$1 AND day_num=$2",
            )
            .bind(req.travel_id)
            .bind(req.day_num)
            .fetch_one(&mut *tx)
            .await?
                + 1
        };
        let row = sqlx::query_as::<_, PlanRow>(
            r#"
            INSERT INTO day_plan (
                travel_id, day_num, point_type, place_name, longitude, latitude,
                arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
            RETURNING id, travel_id, day_num, point_type, place_name, longitude, latitude,
                      arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
            "#,
        )
        .bind(req.travel_id)
        .bind(req.day_num)
        .bind(&req.point_type)
        .bind(name)
        .bind(lng)
        .bind(lat)
        .bind(arrive)
        .bind(leave)
        .bind(req.stay_duration)
        .bind(&traffic)
        .bind(traffic_duration)
        .bind(sort)
        .bind(req.remark.as_deref())
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        row
    };
    let mut drop_ids = vec![row.id];
    if let Some(after_id) = insert_after {
        drop_ids.push(after_id);
    }
    invalidate_route_cache(&state.pool, &drop_ids).await;
    Ok(ok(to_vo(&row)))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<PlanListVo>>, AppError> {
    require_member(&state.pool, q.travel_id, user.id).await?;
    let t = find_travel(&state.pool, q.travel_id).await?;
    let total_days = day_count(t.start_date, t.end_date);
    let plans = list_plans(&state.pool, q.travel_id, None).await?;
    let range: Vec<i32> = if let Some(d) = q.day_num {
        vec![d]
    } else {
        (1..=total_days).collect()
    };
    let want_routes = q.routes.unwrap_or(1) != 0;
    let cache_only = q.cache_only.unwrap_or(0) != 0;
    let mut days = Vec::new();
    let mut prev_last: Option<PlanVo> = None;
    for day_num in 1..=total_days {
        let date = t.start_date + chrono::Duration::days((day_num - 1) as i64);
        let mut day_plans: Vec<PlanVo> = plans
            .iter()
            .filter(|p| p.day_num == day_num)
            .map(to_vo)
            .collect();
        let in_range = range.contains(&day_num);
        if want_routes && in_range {
            let lines = with_routes(
                &state.pool,
                &state.amap_key,
                &state.amap_secret,
                &day_plans,
                false,
                cache_only,
            )
            .await;
            for p in &mut day_plans {
                if let Some(line) = lines.iter().find(|l| l.from_id == p.id) {
                    p.next_distance_m = Some(line.distance_m);
                    p.next_duration_s = Some(line.duration_s);
                }
            }
        }
        let duplicated = match (day_plans.first(), prev_last.as_ref()) {
            (Some(first), Some(prev)) => same_stay(first, prev),
            _ => false,
        };
        let (start_from, start_distance_m, start_duration_s) =
            if !in_range || duplicated || prev_last.is_none() {
                (None, None, None)
            } else {
                let start = prev_last.as_ref().map(|p| StartFromVo {
                    id: p.id,
                    place_name: p.place_name.clone(),
                    day_num: p.day_num,
                    longitude: p.longitude,
                    latitude: p.latitude,
                    point_type: Some(p.point_type.clone()),
                });
                let mut dist = None;
                let mut dur = None;
                if want_routes {
                    if let (Some(a), Some(b)) = (prev_last.as_ref(), day_plans.first()) {
                        let pair = vec![a.clone(), b.clone()];
                        let cross = with_routes(
                            &state.pool,
                            &state.amap_key,
                            &state.amap_secret,
                            &pair,
                            false,
                            cache_only,
                        )
                        .await;
                        if let Some(line) = cross.first() {
                            dist = Some(line.distance_m);
                            dur = Some(line.duration_s);
                        }
                    }
                }
                (start, dist, dur)
            };
        if range.contains(&day_num) {
            days.push(DayVo {
                day_num,
                date: date.to_string(),
                plans: day_plans.clone(),
                start_from,
                start_distance_m,
                start_duration_s,
            });
        }
        prev_last = day_plans.last().cloned();
    }
    Ok(ok(PlanListVo {
        day_count: total_days,
        start_date: t.start_date.to_string(),
        days,
    }))
}

pub async fn del(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<DelReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    let travel_id: Option<i64> = sqlx::query_scalar("SELECT travel_id FROM day_plan WHERE id=$1")
        .bind(req.id)
        .fetch_optional(&state.pool)
        .await?;
    let travel_id = travel_id.ok_or_else(|| AppError::NotFound("行程点位不存在".into()))?;
    require_editor(&state.pool, travel_id, user.id).await?;
    sqlx::query("DELETE FROM day_plan WHERE id=$1")
        .bind(req.id)
        .execute(&state.pool)
        .await?;
    invalidate_route_cache(&state.pool, &[req.id]).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

pub async fn sort(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<SortReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    require_editor(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    let days = day_count(t.start_date, t.end_date);
    if req.day_num < 1 || req.day_num > days {
        return Err(AppError::BadRequest("天数不在旅途范围内".into()));
    }
    if req.ids.is_empty() {
        return Ok(ok(serde_json::json!({ "ok": true })));
    }

    let mut tx = state.pool.begin().await?;
    // 校验 ids 都属于本旅途，允许从其他天挪入
    for id in &req.ids {
        let ok: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM day_plan WHERE id=$1 AND travel_id=$2)",
        )
        .bind(id)
        .bind(req.travel_id)
        .fetch_one(&mut *tx)
        .await?;
        if !ok {
            return Err(AppError::NotFound("行程点位不存在".into()));
        }
    }

    for (i, id) in req.ids.iter().enumerate() {
        if i == 0 {
            // 当天第一个点：清空「怎么来」，跨天挪过来也一样
            sqlx::query(
                r#"
                UPDATE day_plan
                SET day_num=$1, sort=$2, traffic_type=NULL, traffic_duration=NULL
                WHERE id=$3 AND travel_id=$4
                "#,
            )
            .bind(req.day_num)
            .bind(i as i32)
            .bind(id)
            .bind(req.travel_id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE day_plan
                SET day_num=$1, sort=$2, traffic_duration=NULL,
                    traffic_type = COALESCE(NULLIF(traffic_type, ''), 'drive')
                WHERE id=$3 AND travel_id=$4
                "#,
            )
            .bind(req.day_num)
            .bind(i as i32)
            .bind(id)
            .bind(req.travel_id)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    invalidate_route_cache(&state.pool, &req.ids).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

pub async fn move_plan(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<MoveReq>,
) -> Result<Json<ApiOk<PlanVo>>, AppError> {
    require_editor(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    let days = day_count(t.start_date, t.end_date);
    if req.day_num < 1 || req.day_num > days {
        return Err(AppError::BadRequest("天数不在旅途范围内".into()));
    }

    let mut tx = state.pool.begin().await?;
    let row = sqlx::query_as::<_, PlanRow>(
        r#"
        SELECT id, travel_id, day_num, point_type, place_name, longitude, latitude,
               arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
        FROM day_plan WHERE id=$1 AND travel_id=$2
        "#,
    )
    .bind(req.id)
    .bind(req.travel_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("行程点位不存在".into()))?;

    let sort = if let Some(after_id) = req.after_id {
        #[derive(sqlx::FromRow)]
        struct AfterRow {
            sort: i32,
            day_num: i32,
        }
        let after = sqlx::query_as::<_, AfterRow>(
            "SELECT sort, day_num FROM day_plan WHERE id=$1 AND travel_id=$2",
        )
        .bind(after_id)
        .bind(req.travel_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| AppError::BadRequest("插入位置不存在".into()))?;
        if after.day_num != req.day_num {
            return Err(AppError::BadRequest("目标位置不在指定天".into()));
        }
        sqlx::query(
            "UPDATE day_plan SET sort = sort + 1 WHERE travel_id=$1 AND day_num=$2 AND sort > $3",
        )
        .bind(req.travel_id)
        .bind(req.day_num)
        .bind(after.sort)
        .execute(&mut *tx)
        .await?;
        after.sort + 1
    } else {
        sqlx::query_scalar::<_, i32>(
            "SELECT COALESCE(MAX(sort), -1) FROM day_plan WHERE travel_id=$1 AND day_num=$2 AND id<>$3",
        )
        .bind(req.travel_id)
        .bind(req.day_num)
        .bind(req.id)
        .fetch_one(&mut *tx)
        .await?
            + 1
    };

    // 从原天抽出后，当天其余点重新紧排
    let old_day = row.day_num;
    sqlx::query(
        r#"
        UPDATE day_plan
        SET day_num=$1, sort=$2, traffic_duration=NULL,
            traffic_type = CASE
                WHEN $2 = 0 THEN NULL
                ELSE COALESCE(NULLIF(traffic_type, ''), 'drive')
            END
        WHERE id=$3 AND travel_id=$4
        "#,
    )
    .bind(req.day_num)
    .bind(sort)
    .bind(req.id)
    .bind(req.travel_id)
    .execute(&mut *tx)
    .await?;

    if old_day != req.day_num {
        let remain: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM day_plan WHERE travel_id=$1 AND day_num=$2 ORDER BY sort ASC, id ASC",
        )
        .bind(req.travel_id)
        .bind(old_day)
        .fetch_all(&mut *tx)
        .await?;
        for (i, id) in remain.iter().enumerate() {
            if i == 0 {
                sqlx::query(
                    "UPDATE day_plan SET sort=$1, traffic_type=NULL, traffic_duration=NULL WHERE id=$2",
                )
                .bind(i as i32)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            } else {
                sqlx::query("UPDATE day_plan SET sort=$1 WHERE id=$2")
                    .bind(i as i32)
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }

    // 目标天按 sort 紧排，避免空洞
    let target: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM day_plan WHERE travel_id=$1 AND day_num=$2 ORDER BY sort ASC, id ASC",
    )
    .bind(req.travel_id)
    .bind(req.day_num)
    .fetch_all(&mut *tx)
    .await?;
    for (i, id) in target.iter().enumerate() {
        if i == 0 {
            sqlx::query(
                "UPDATE day_plan SET sort=$1, traffic_type=NULL, traffic_duration=NULL WHERE id=$2",
            )
            .bind(i as i32)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                r#"
                UPDATE day_plan SET sort=$1,
                    traffic_type = COALESCE(NULLIF(traffic_type, ''), 'drive')
                WHERE id=$2
                "#,
            )
            .bind(i as i32)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
    }

    let updated = sqlx::query_as::<_, PlanRow>(
        r#"
        SELECT id, travel_id, day_num, point_type, place_name, longitude, latitude,
               arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
        FROM day_plan WHERE id=$1
        "#,
    )
    .bind(req.id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut drop_ids = vec![req.id];
    drop_ids.extend(target);
    invalidate_route_cache(&state.pool, &drop_ids).await;
    Ok(ok(to_vo(&updated)))
}

pub async fn map_global(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<MapVo>>, AppError> {
    require_member(&state.pool, q.travel_id, user.id).await?;
    let plans = list_plans(&state.pool, q.travel_id, None).await?;
    let points: Vec<PlanVo> = plans.iter().map(to_vo).collect();
    let cache_only = q.cache_only.unwrap_or(0) != 0;
    let lines = with_routes(
        &state.pool,
        &state.amap_key,
        &state.amap_secret,
        &points,
        false,
        cache_only,
    )
    .await;
    Ok(ok(MapVo { points, lines }))
}

pub async fn map_day(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<MapVo>>, AppError> {
    let day = q
        .day_num
        .ok_or_else(|| AppError::BadRequest("缺少 day_num".into()))?;
    require_member(&state.pool, q.travel_id, user.id).await?;
    let plans = list_plans(&state.pool, q.travel_id, Some(day)).await?;
    let mut points: Vec<PlanVo> = plans.iter().map(to_vo).collect();

    // 把「昨天最后停留」接到当天地图最前面，否则早上的出发起点看不见
    if day > 1 {
        let prev = list_plans(&state.pool, q.travel_id, Some(day - 1)).await?;
        if let Some(prev_last) = prev.last() {
            let start = to_vo(prev_last);
            let need_start = match points.first() {
                Some(first) => !same_stay(first, &start),
                None => start.latitude.is_some() && start.longitude.is_some(),
            };
            if need_start {
                points.insert(0, start);
            }
        }
    }

    let force = q.fresh.unwrap_or(0) != 0;
    let cache_only = q.cache_only.unwrap_or(0) != 0;
    if force {
        let ids: Vec<i64> = points.iter().map(|p| p.id).collect();
        invalidate_route_cache(&state.pool, &ids).await;
    }
    let lines = with_routes(
        &state.pool,
        &state.amap_key,
        &state.amap_secret,
        &points,
        force,
        cache_only,
    )
    .await;
    Ok(ok(MapVo { points, lines }))
}

pub async fn map_search(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(q): Query<SearchQ>,
) -> Result<Json<ApiOk<Vec<crate::poi::PoiVo>>>, AppError> {
    let list = search_places(
        &state.amap_key,
        &state.amap_secret,
        &q.q,
        q.lng,
        q.lat,
        None,
    )
    .await?;
    Ok(ok(list))
}

pub async fn map_regeo(
    State(state): State<AppState>,
    _user: AuthUser,
    Query(q): Query<RegeoQ>,
) -> Result<Json<ApiOk<crate::poi::PoiVo>>, AppError> {
    let poi = reverse_geocode(&state.amap_key, &state.amap_secret, q.lng, q.lat).await?;
    Ok(ok(poi))
}

#[derive(Deserialize)]
pub struct AiDraftReq {
    pub travel_id: i64,
    pub prompt: String,
    pub day_num: Option<i32>,
    pub mode: Option<String>,
    pub fresh: Option<bool>,
}

#[derive(Deserialize)]
pub struct AiApplyReq {
    pub travel_id: i64,
    pub day_num: Option<i32>,
    pub days: Vec<crate::ai::AiDay>,
}

fn existing_plan_brief(plans: &[PlanRow], focus_day: Option<i32>) -> String {
    if plans.is_empty() {
        return "暂无行程".into();
    }
    let mut by_day: Vec<(i32, Vec<String>)> = Vec::new();
    for p in plans {
        if let Some((_, names)) = by_day.iter_mut().find(|(d, _)| *d == p.day_num) {
            names.push(p.place_name.clone());
        } else {
            by_day.push((p.day_num, vec![p.place_name.clone()]));
        }
    }
    let all = by_day
        .iter()
        .map(|(d, names)| format!("D{d}: {}", names.join("、")))
        .collect::<Vec<_>>()
        .join("；");
    if let Some(d) = focus_day {
        let names = by_day
            .into_iter()
            .find(|(n, _)| *n == d)
            .map(|(_, names)| names.join("、"))
            .unwrap_or_else(|| "这一天还空着".into());
        format!("全程：{all}\n正在改 D{d}，现有点：{names}")
    } else {
        all
    }
}

pub async fn ai_draft(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<AiDraftReq>,
) -> Result<Json<ApiOk<crate::ai::AiDraft>>, AppError> {
    require_editor(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    let days = day_count(t.start_date, t.end_date);
    let focus_day = match req.day_num {
        Some(d) if d >= 1 && d <= days => Some(d),
        Some(_) => return Err(AppError::BadRequest("天数不在旅途范围内".into())),
        None => None,
    };
    let plans = list_plans(&state.pool, req.travel_id, None).await?;
    let fresh = req.fresh.unwrap_or(false);
    let recommend = !fresh && req.mode.as_deref() == Some("recommend");
    if recommend && plans.is_empty() {
        return Err(AppError::BadRequest("先排几个地点，再沿途推荐".into()));
    }
    let draft = crate::ai::draft_itinerary(
        &state.deepseek_api_key,
        &state.amap_key,
        &state.amap_secret,
        &t.destination,
        &t.start_date.to_string(),
        &t.end_date.to_string(),
        days,
        &existing_plan_brief(&plans, focus_day),
        &req.prompt,
        focus_day,
        recommend,
        fresh,
    )
    .await?;
    Ok(ok(draft))
}

pub async fn ai_apply(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<AiApplyReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    require_editor(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    let max_days = day_count(t.start_date, t.end_date);
    if req.days.is_empty() {
        return Err(AppError::BadRequest("没有可保存的行程".into()));
    }
    let focus_day = match req.day_num {
        Some(d) if d >= 1 && d <= max_days => Some(d),
        Some(_) => return Err(AppError::BadRequest("天数不在旅途范围内".into())),
        None => None,
    };
    let mut rows: Vec<(i32, crate::ai::AiPoint)> = Vec::new();
    for day in &req.days {
        if day.day_num < 1 || day.day_num > max_days {
            return Err(AppError::BadRequest("天数不在旅途范围内".into()));
        }
        if focus_day.is_some() && focus_day != Some(day.day_num) {
            continue;
        }
        for p in &day.points {
            let name = p.place_name.trim();
            if name.is_empty() {
                continue;
            }
            if !valid_point_type(&p.point_type) {
                return Err(AppError::BadRequest("点位类型不合法".into()));
            }
            rows.push((day.day_num, p.clone()));
            if rows.len() > 56 {
                return Err(AppError::BadRequest("地点太多，精简后再保存".into()));
            }
        }
    }
    let old_ids: Vec<i64> = if let Some(day) = focus_day {
        sqlx::query_scalar("SELECT id FROM day_plan WHERE travel_id=$1 AND day_num=$2")
            .bind(req.travel_id)
            .bind(day)
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_scalar("SELECT id FROM day_plan WHERE travel_id=$1")
            .bind(req.travel_id)
            .fetch_all(&state.pool)
            .await?
    };

    if focus_day.is_none() {
        clear_travel_route_cache(&state.pool, req.travel_id).await;
    }
    let mut tx = state.pool.begin().await?;
    if let Some(day) = focus_day {
        sqlx::query("DELETE FROM day_plan WHERE travel_id=$1 AND day_num=$2")
            .bind(req.travel_id)
            .bind(day)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query("DELETE FROM day_plan WHERE travel_id=$1")
            .bind(req.travel_id)
            .execute(&mut *tx)
            .await?;
    }

    let mut sort_by_day: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    for (day_num, p) in rows {
        let sort = {
            let n = sort_by_day.entry(day_num).or_insert(0);
            let cur = *n;
            *n += 1;
            cur
        };
        let lng = p.longitude.and_then(Decimal::from_f64);
        let lat = p.latitude.and_then(Decimal::from_f64);
        let arrive = parse_time(&p.arrive)?;
        let remark = p.note.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let traffic = if sort == 0 {
            None
        } else {
            Some("drive".to_string())
        };
        sqlx::query(
            r#"
            INSERT INTO day_plan (
                travel_id, day_num, point_type, place_name, longitude, latitude,
                arrive_time, leave_time, stay_duration, traffic_type, traffic_duration, sort, remark
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,NULL,$8,$9,NULL,$10,$11)
            "#,
        )
        .bind(req.travel_id)
        .bind(day_num)
        .bind(&p.point_type)
        .bind(p.place_name.trim())
        .bind(lng)
        .bind(lat)
        .bind(arrive)
        .bind(p.stay_minutes)
        .bind(&traffic)
        .bind(sort)
        .bind(remark)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    invalidate_route_cache(&state.pool, &old_ids).await;
    Ok(ok(serde_json::json!({ "ok": true })))
}
