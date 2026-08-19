use axum::{
    extract::{Query, State},
    Json,
};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{
    db::{find_travel, list_members, require_biller, require_member},
    error::{ok, ApiOk, AppError},
    settle::{calc_transfers, MemberBalance},
    state::{AppState, AuthUser},
    util::{dec_from_f64, dec_to_f64, default_cost_of_point, parse_datetime, split_amount, valid_cost_type},
};

#[derive(Deserialize)]
pub struct SaveReq {
    pub id: Option<i64>,
    pub travel_id: i64,
    pub day_plan_id: Option<i64>,
    pub bill_name: String,
    pub amount: f64,
    pub bill_type: i16,
    pub cost_type: String,
    pub pay_user_id: Option<i64>,
    pub consume_time: String,
    pub visible_all: Option<bool>,
    pub share_user_ids: Option<Vec<i64>>,
    pub remark: Option<String>,
}

#[derive(Deserialize)]
pub struct ListQ {
    pub travel_id: i64,
}

#[derive(Deserialize)]
pub struct DelReq {
    pub id: i64,
}

#[derive(sqlx::FromRow)]
struct BillRow {
    id: i64,
    travel_id: i64,
    day_plan_id: Option<i64>,
    bill_name: String,
    amount: Decimal,
    bill_type: i16,
    cost_type: String,
    pay_user_id: i64,
    consume_time: chrono::NaiveDateTime,
    visible_all: bool,
    remark: Option<String>,
    pay_nickname: String,
    pay_avatar: Option<String>,
    plan_place_name: Option<String>,
}

#[derive(Serialize)]
pub struct ShareVo {
    pub user_id: i64,
    pub nickname: String,
    pub share_amount: f64,
}

#[derive(Serialize)]
pub struct BillVo {
    pub id: i64,
    pub travel_id: i64,
    pub day_plan_id: Option<i64>,
    pub plan_place_name: Option<String>,
    pub bill_name: String,
    pub amount: f64,
    pub bill_type: i16,
    pub cost_type: String,
    pub pay_user_id: i64,
    pub pay_nickname: String,
    pub pay_avatar: Option<String>,
    pub consume_time: String,
    pub visible_all: bool,
    pub remark: Option<String>,
    pub shares: Vec<ShareVo>,
}

#[derive(Serialize)]
pub struct CatVo {
    pub cost_type: String,
    pub amount: f64,
}

#[derive(Serialize)]
pub struct StatVo {
    pub public_total: f64,
    pub private_total: f64,
    pub trip_total: f64,
    pub avg_public: f64,
    pub member_count: i64,
    pub categories: Vec<CatVo>,
}

fn to_vo(b: BillRow, shares: Vec<ShareVo>) -> BillVo {
    BillVo {
        id: b.id,
        travel_id: b.travel_id,
        day_plan_id: b.day_plan_id,
        plan_place_name: b.plan_place_name,
        bill_name: b.bill_name,
        amount: dec_to_f64(b.amount),
        bill_type: b.bill_type,
        cost_type: b.cost_type,
        pay_user_id: b.pay_user_id,
        pay_nickname: b.pay_nickname,
        pay_avatar: b.pay_avatar,
        consume_time: b.consume_time.format("%Y-%m-%d %H:%M").to_string(),
        visible_all: b.visible_all,
        remark: b.remark,
        shares,
    }
}

async fn load_shares(pool: &sqlx::PgPool, bill_id: i64) -> Result<Vec<ShareVo>, AppError> {
    let rows: Vec<(i64, String, Decimal)> = sqlx::query_as(
        r#"
        SELECT s.user_id, u.nickname, s.share_amount
        FROM bill_share s
        JOIN app_user u ON u.id = s.user_id
        WHERE s.bill_id = $1
        ORDER BY s.id
        "#,
    )
    .bind(bill_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(user_id, nickname, share_amount)| ShareVo {
            user_id,
            nickname,
            share_amount: dec_to_f64(share_amount),
        })
        .collect())
}

pub async fn save(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<SaveReq>,
) -> Result<Json<ApiOk<BillVo>>, AppError> {
    require_biller(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if t.is_lock {
        return Err(AppError::Forbidden("结算已锁定，不能修改账单".into()));
    }
    let name = req.bill_name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("消费名称不能为空".into()));
    }
    if req.amount <= 0.0 {
        return Err(AppError::BadRequest("金额必须大于 0".into()));
    }
    if req.bill_type != 1 && req.bill_type != 2 {
        return Err(AppError::BadRequest("账单类型不合法".into()));
    }
    let mut cost_type = req.cost_type.clone();
    if cost_type.is_empty() {
        cost_type = "other".into();
    }
    if !valid_cost_type(&cost_type) {
        return Err(AppError::BadRequest("费用类型不合法".into()));
    }
    let day_plan_id = req.day_plan_id.filter(|id| *id > 0);
    if let Some(plan_id) = day_plan_id {
        let pt: Option<String> =
            sqlx::query_scalar("SELECT point_type FROM day_plan WHERE id=$1 AND travel_id=$2")
                .bind(plan_id)
                .bind(req.travel_id)
                .fetch_optional(&state.pool)
                .await?;
        let pt = pt.ok_or_else(|| AppError::BadRequest("关联行程点位不存在".into()))?;
        if req.cost_type.is_empty() {
            cost_type = default_cost_of_point(&pt).into();
        }
    }
    let amount = dec_from_f64(req.amount)?.round_dp(2);
    let pay_user_id = req.pay_user_id.unwrap_or(user.id);
    require_member(&state.pool, req.travel_id, pay_user_id).await?;
    let consume_time = parse_datetime(&req.consume_time)?;
    let visible_all = if req.bill_type == 1 {
        true
    } else {
        req.visible_all.unwrap_or(false)
    };

    let members = list_members(&state.pool, req.travel_id).await?;
    let share_ids = if req.bill_type == 1 {
        let ids = req.share_user_ids.clone().unwrap_or_default();
        let ids = if ids.is_empty() {
            members.iter().map(|m| m.user_id).collect::<Vec<_>>()
        } else {
            ids
        };
        for id in &ids {
            if !members.iter().any(|m| m.user_id == *id) {
                return Err(AppError::BadRequest("分摊成员必须属于当前旅途".into()));
            }
        }
        ids
    } else {
        vec![]
    };
    let parts = split_amount(amount, share_ids.len());

    let mut tx = state.pool.begin().await?;
    let bill_id = if let Some(id) = req.id {
        let owner: Option<(i64, i16)> =
            sqlx::query_as("SELECT pay_user_id, bill_type FROM bill WHERE id=$1 AND travel_id=$2")
                .bind(id)
                .bind(req.travel_id)
                .fetch_optional(&mut *tx)
                .await?;
        let (owner_id, _) = owner.ok_or_else(|| AppError::NotFound("账单不存在".into()))?;
        if owner_id != user.id && t.creator_id != user.id {
            return Err(AppError::Forbidden("只能编辑自己的账单".into()));
        }
        sqlx::query(
            r#"
            UPDATE bill SET day_plan_id=$2, bill_name=$3, amount=$4, bill_type=$5, cost_type=$6,
                pay_user_id=$7, consume_time=$8, visible_all=$9, remark=$10
            WHERE id=$1
            "#,
        )
        .bind(id)
        .bind(day_plan_id)
        .bind(name)
        .bind(amount)
        .bind(req.bill_type)
        .bind(&cost_type)
        .bind(pay_user_id)
        .bind(consume_time)
        .bind(visible_all)
        .bind(req.remark.as_deref())
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM bill_share WHERE bill_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        sqlx::query_scalar(
            r#"
            INSERT INTO bill (
                travel_id, day_plan_id, bill_name, amount, bill_type, cost_type,
                pay_user_id, consume_time, visible_all, remark
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            RETURNING id
            "#,
        )
        .bind(req.travel_id)
        .bind(day_plan_id)
        .bind(name)
        .bind(amount)
        .bind(req.bill_type)
        .bind(&cost_type)
        .bind(pay_user_id)
        .bind(consume_time)
        .bind(visible_all)
        .bind(req.remark.as_deref())
        .fetch_one(&mut *tx)
        .await?
    };

    for (uid, part) in share_ids.iter().zip(parts.iter()) {
        sqlx::query("INSERT INTO bill_share (bill_id, user_id, share_amount) VALUES ($1,$2,$3)")
            .bind(bill_id)
            .bind(uid)
            .bind(*part)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;

    let row = fetch_bill(&state.pool, bill_id, user.id).await?
        .ok_or_else(|| AppError::NotFound("账单不存在".into()))?;
    let shares = load_shares(&state.pool, bill_id).await?;
    Ok(ok(to_vo(row, shares)))
}

async fn fetch_bill(pool: &sqlx::PgPool, id: i64, user_id: i64) -> Result<Option<BillRow>, AppError> {
    Ok(sqlx::query_as::<_, BillRow>(
        r#"
        SELECT b.id, b.travel_id, b.day_plan_id, b.bill_name, b.amount, b.bill_type, b.cost_type,
               b.pay_user_id, b.consume_time, b.visible_all, b.remark,
               u.nickname AS pay_nickname, u.avatar AS pay_avatar,
               p.place_name AS plan_place_name
        FROM bill b
        JOIN app_user u ON u.id = b.pay_user_id
        LEFT JOIN day_plan p ON p.id = b.day_plan_id
        WHERE b.id = $1 AND (
            b.bill_type = 1 OR b.pay_user_id = $2 OR b.visible_all = TRUE
        )
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<Vec<BillVo>>>, AppError> {
    require_member(&state.pool, q.travel_id, user.id).await?;
    let rows: Vec<BillRow> = sqlx::query_as(
        r#"
        SELECT b.id, b.travel_id, b.day_plan_id, b.bill_name, b.amount, b.bill_type, b.cost_type,
               b.pay_user_id, b.consume_time, b.visible_all, b.remark,
               u.nickname AS pay_nickname, u.avatar AS pay_avatar,
               p.place_name AS plan_place_name
        FROM bill b
        JOIN app_user u ON u.id = b.pay_user_id
        LEFT JOIN day_plan p ON p.id = b.day_plan_id
        WHERE b.travel_id = $1 AND (
            b.bill_type = 1 OR b.pay_user_id = $2 OR b.visible_all = TRUE
        )
        ORDER BY b.consume_time DESC, b.id DESC
        "#,
    )
    .bind(q.travel_id)
    .bind(user.id)
    .fetch_all(&state.pool)
    .await?;

    let mut list = Vec::new();
    for row in rows {
        let shares = load_shares(&state.pool, row.id).await?;
        list.push(to_vo(row, shares));
    }
    Ok(ok(list))
}

pub async fn del(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<DelReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    let row: Option<(i64, i64, bool)> = sqlx::query_as(
        r#"
        SELECT b.travel_id, b.pay_user_id, t.is_lock
        FROM bill b
        JOIN travel t ON t.id = b.travel_id
        WHERE b.id = $1
        "#,
    )
    .bind(req.id)
    .fetch_optional(&state.pool)
    .await?;
    let (travel_id, pay_user_id, is_lock) =
        row.ok_or_else(|| AppError::NotFound("账单不存在".into()))?;
    require_biller(&state.pool, travel_id, user.id).await?;
    if is_lock {
        return Err(AppError::Forbidden("结算已锁定，不能删除账单".into()));
    }
    let t = find_travel(&state.pool, travel_id).await?;
    if pay_user_id != user.id && t.creator_id != user.id {
        return Err(AppError::Forbidden("只能删除自己的账单".into()));
    }
    sqlx::query("DELETE FROM bill WHERE id=$1")
        .bind(req.id)
        .execute(&state.pool)
        .await?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

pub async fn stat(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<StatVo>>, AppError> {
    require_member(&state.pool, q.travel_id, user.id).await?;
    let member_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM travel_member WHERE travel_id=$1")
            .bind(q.travel_id)
            .fetch_one(&state.pool)
            .await?;
    let public_total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount),0) FROM bill WHERE travel_id=$1 AND bill_type=1",
    )
    .bind(q.travel_id)
    .fetch_one(&state.pool)
    .await?;
    let private_total: Decimal = sqlx::query_scalar(
        "SELECT COALESCE(SUM(amount),0) FROM bill WHERE travel_id=$1 AND bill_type=2 AND pay_user_id=$2",
    )
    .bind(q.travel_id)
    .bind(user.id)
    .fetch_one(&state.pool)
    .await?;
    let cats: Vec<(String, Decimal)> = sqlx::query_as(
        r#"
        SELECT cost_type, COALESCE(SUM(amount),0)
        FROM bill
        WHERE travel_id=$1 AND bill_type=1
        GROUP BY cost_type
        ORDER BY SUM(amount) DESC
        "#,
    )
    .bind(q.travel_id)
    .fetch_all(&state.pool)
    .await?;
    let avg = if member_count > 0 {
        dec_to_f64(public_total) / member_count as f64
    } else {
        0.0
    };
    Ok(ok(StatVo {
        public_total: dec_to_f64(public_total),
        private_total: dec_to_f64(private_total),
        trip_total: dec_to_f64(public_total + private_total),
        avg_public: (avg * 100.0).round() / 100.0,
        member_count,
        categories: cats
            .into_iter()
            .map(|(cost_type, amount)| CatVo {
                cost_type,
                amount: dec_to_f64(amount),
            })
            .collect(),
    }))
}

pub async fn settle_calc(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    require_member(&state.pool, q.travel_id, user.id).await?;
    let t = find_travel(&state.pool, q.travel_id).await?;
    let members = list_members(&state.pool, q.travel_id).await?;
    let paid_rows: Vec<(i64, Decimal)> = sqlx::query_as(
        r#"
        SELECT pay_user_id, COALESCE(SUM(amount),0)
        FROM bill WHERE travel_id=$1 AND bill_type=1
        GROUP BY pay_user_id
        "#,
    )
    .bind(q.travel_id)
    .fetch_all(&state.pool)
    .await?;
    let owed_rows: Vec<(i64, Decimal)> = sqlx::query_as(
        r#"
        SELECT s.user_id, COALESCE(SUM(s.share_amount),0)
        FROM bill_share s
        JOIN bill b ON b.id = s.bill_id
        WHERE b.travel_id=$1 AND b.bill_type=1
        GROUP BY s.user_id
        "#,
    )
    .bind(q.travel_id)
    .fetch_all(&state.pool)
    .await?;

    let balances: Vec<MemberBalance> = members
        .iter()
        .map(|m| {
            let paid = paid_rows
                .iter()
                .find(|(id, _)| *id == m.user_id)
                .map(|(_, v)| *v)
                .unwrap_or(Decimal::ZERO);
            let owed = owed_rows
                .iter()
                .find(|(id, _)| *id == m.user_id)
                .map(|(_, v)| *v)
                .unwrap_or(Decimal::ZERO);
            MemberBalance {
                user_id: m.user_id,
                nickname: m.nickname.clone(),
                avatar: m.avatar.clone(),
                paid,
                owed,
            }
        })
        .collect();
    let (users, transfers) = calc_transfers(&balances);
    Ok(ok(serde_json::json!({
        "is_lock": t.is_lock,
        "is_leader": t.creator_id == user.id,
        "users": users,
        "transfers": transfers,
    })))
}

