use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::{
    db::{
        display_status, find_travel, list_members, require_leader, require_member, status_text, MemberRow,
        TravelRow,
    },
    error::{ok, ApiOk, AppError},
    state::{AppState, AuthUser},
    util::{day_count, gen_invite_code, parse_date},
};

#[derive(Deserialize)]
pub struct CreateReq {
    pub travel_name: String,
    pub destination: String,
    pub start_date: String,
    pub end_date: String,
    pub remark: Option<String>,
}

#[derive(Serialize)]
pub struct TravelVo {
    pub id: i64,
    pub travel_name: String,
    pub destination: String,
    pub start_date: String,
    pub end_date: String,
    pub invite_code: String,
    pub status: i16,
    pub status_text: String,
    pub creator_id: i64,
    pub is_lock: bool,
    pub remark: Option<String>,
    pub is_sample: bool,
    pub member_count: i64,
    pub role: i16,
    pub can_edit: bool,
    pub can_bill: bool,
    pub day_count: i32,
}

#[derive(Deserialize)]
pub struct ListQ {
    pub archived: Option<bool>,
}

#[derive(Deserialize)]
pub struct DetailQ {
    pub id: i64,
}

#[derive(Deserialize)]
pub struct JoinReq {
    pub invite_code: String,
}

#[derive(Deserialize)]
pub struct TravelIdReq {
    pub travel_id: i64,
}

#[derive(Deserialize)]
pub struct LockReq {
    pub travel_id: i64,
    pub is_lock: Option<bool>,
}

#[derive(Deserialize)]
pub struct RemoveReq {
    pub travel_id: i64,
    pub user_id: i64,
}

#[derive(Serialize)]
pub struct MemberVo {
    pub id: i64,
    pub user_id: i64,
    pub nickname: String,
    pub avatar: Option<String>,
    pub role: i16,
    pub role_text: String,
    pub can_edit: bool,
    pub can_bill: bool,
    pub perm_text: String,
    pub group_name: Option<String>,
    pub is_guest: bool,
}

#[derive(sqlx::FromRow)]
struct TravelListRow {
    #[sqlx(flatten)]
    travel: TravelRow,
    member_count: i64,
    role: i16,
    can_edit: bool,
    can_bill: bool,
}

fn to_vo(t: &TravelRow, member_count: i64, role: i16, can_edit: bool, can_bill: bool) -> TravelVo {
    let is_sample = crate::sample::is_sample_remark(&t.remark);
    let read_only = is_sample || t.status == 2;
    TravelVo {
        id: t.id,
        travel_name: t.travel_name.clone(),
        destination: t.destination.clone(),
        start_date: t.start_date.to_string(),
        end_date: t.end_date.to_string(),
        invite_code: t.invite_code.clone(),
        status: display_status(t.status, t.end_date),
        status_text: status_text(t.status, t.end_date).into(),
        creator_id: t.creator_id,
        is_lock: t.is_lock || is_sample,
        remark: t.remark.clone(),
        is_sample,
        member_count,
        role,
        can_edit: !read_only && (role == 1 || can_edit),
        can_bill: !read_only && (role == 1 || can_bill),
        day_count: day_count(t.start_date, t.end_date),
    }
}

fn member_vo(m: &MemberRow) -> MemberVo {
    let can_edit = m.role == 1 || m.can_edit;
    let can_bill = m.role == 1 || m.can_bill;
    let is_guest = m.open_id.starts_with("guest_");
    let group = m
        .group_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    let perm_text = if m.role == 1 {
        "团长".into()
    } else if is_guest {
        "随行成员".into()
    } else {
        let mut bits = vec!["可看路线"];
        if can_edit {
            bits.push("可改行程");
        }
        if can_bill {
            bits.push("可记账");
        }
        bits.join(" · ")
    };
    MemberVo {
        id: m.id,
        user_id: m.user_id,
        nickname: m.nickname.clone(),
        avatar: m.avatar.clone(),
        role: m.role,
        role_text: if m.role == 1 { "团长" } else { "成员" }.into(),
        can_edit,
        can_bill,
        perm_text,
        group_name: group,
        is_guest,
    }
}

pub async fn create(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<CreateReq>,
) -> Result<Json<ApiOk<TravelVo>>, AppError> {
    let name = req.travel_name.trim();
    let dest = req.destination.trim();
    if name.is_empty() || dest.is_empty() {
        return Err(AppError::BadRequest("旅途名称和目的地不能为空".into()));
    }
    let start = parse_date(&req.start_date)?;
    let end = parse_date(&req.end_date)?;
    if end < start {
        return Err(AppError::BadRequest("结束日期不能早于开始日期".into()));
    }

    let mut tx = state.pool.begin().await?;
    let mut invite = gen_invite_code();
    for _ in 0..8 {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM travel WHERE invite_code = $1)")
                .bind(&invite)
                .fetch_one(&mut *tx)
                .await?;
        if !exists {
            break;
        }
        invite = gen_invite_code();
    }

    let travel: TravelRow = sqlx::query_as(
        r#"
        INSERT INTO travel (travel_name, destination, start_date, end_date, invite_code, creator_id, remark)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, travel_name, destination, start_date, end_date, invite_code,
                  status, creator_id, is_lock, remark
        "#,
    )
    .bind(name)
    .bind(dest)
    .bind(start)
    .bind(end)
    .bind(&invite)
    .bind(user.id)
    .bind(req.remark.as_deref())
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("INSERT INTO travel_member (travel_id, user_id, role, can_edit, can_bill) VALUES ($1, $2, 1, TRUE, TRUE)")
        .bind(travel.id)
        .bind(user.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    Ok(ok(to_vo(&travel, 1, 1, true, true)))
}

#[derive(Deserialize)]
pub struct UpdateReq {
    pub travel_id: i64,
    pub travel_name: Option<String>,
    pub destination: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub remark: Option<String>,
}

/// 团长修改旅途信息 / 日期；缩短日期时删除超出天数的行程点
pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<UpdateReq>,
) -> Result<Json<ApiOk<TravelVo>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if crate::sample::is_sample_remark(&t.remark) {
        return Err(AppError::BadRequest("示例旅途不可修改".into()));
    }
    if t.status == 2 {
        return Err(AppError::BadRequest("已归档旅途不可修改".into()));
    }
    if t.is_lock {
        return Err(AppError::BadRequest("已锁定，不可修改日期".into()));
    }

    let name = req
        .travel_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(t.travel_name.as_str());
    let dest = req
        .destination
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(t.destination.as_str());
    if name.chars().count() > 100 || dest.chars().count() > 100 {
        return Err(AppError::BadRequest("名称或目的地过长".into()));
    }

    let start = match &req.start_date {
        Some(s) => parse_date(s)?,
        None => t.start_date,
    };
    let end = match &req.end_date {
        Some(s) => parse_date(s)?,
        None => t.end_date,
    };
    if end < start {
        return Err(AppError::BadRequest("结束日期不能早于开始日期".into()));
    }
    let days = day_count(start, end);
    if days > 60 {
        return Err(AppError::BadRequest("行程请控制在 60 天以内".into()));
    }

    let mut tx = state.pool.begin().await?;
    // 缩短行程：清掉超出天数的点位（账单上的绑定会置空）
    sqlx::query("DELETE FROM day_plan WHERE travel_id = $1 AND day_num > $2")
        .bind(req.travel_id)
        .bind(days)
        .execute(&mut *tx)
        .await?;

    let remark = match &req.remark {
        Some(s) => Some(s.trim()).filter(|x| !x.is_empty()).map(|s| s.to_string()),
        None => t.remark.clone(),
    };

    let travel: TravelRow = sqlx::query_as(
        r#"
        UPDATE travel
        SET travel_name = $2,
            destination = $3,
            start_date = $4,
            end_date = $5,
            remark = $6
        WHERE id = $1
        RETURNING id, travel_name, destination, start_date, end_date, invite_code,
                  status, creator_id, is_lock, remark
        "#,
    )
    .bind(req.travel_id)
    .bind(name)
    .bind(dest)
    .bind(start)
    .bind(end)
    .bind(remark.as_deref())
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM travel_member WHERE travel_id = $1")
        .bind(travel.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(ok(to_vo(&travel, count, 1, true, true)))
}

pub async fn list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ListQ>,
) -> Result<Json<ApiOk<Vec<TravelVo>>>, AppError> {
    let archived = q.archived.unwrap_or(false);
    let rows: Vec<TravelListRow> = if archived {
        // 归档列表：含已结束的示例攻略
        sqlx::query_as(
            r#"
            SELECT t.id, t.travel_name, t.destination, t.start_date, t.end_date, t.invite_code,
                   t.status, t.creator_id, t.is_lock, t.remark,
                   (SELECT COUNT(*) FROM travel_member m2 WHERE m2.travel_id = t.id) AS member_count,
                   m.role, m.can_edit, m.can_bill
            FROM travel t
            JOIN travel_member m ON m.travel_id = t.id
            WHERE m.user_id = $1
              AND t.status = 2
            ORDER BY
              CASE WHEN t.remark LIKE '【示例攻略】%' THEN 0 ELSE 1 END,
              t.create_time DESC
            "#,
        )
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?
    } else {
        // 进行中：真实行程；无真实行程时额外展示示例（示例本身已归档）
        sqlx::query_as(
            r#"
            SELECT t.id, t.travel_name, t.destination, t.start_date, t.end_date, t.invite_code,
                   t.status, t.creator_id, t.is_lock, t.remark,
                   (SELECT COUNT(*) FROM travel_member m2 WHERE m2.travel_id = t.id) AS member_count,
                   m.role, m.can_edit, m.can_bill
            FROM travel t
            JOIN travel_member m ON m.travel_id = t.id
            WHERE m.user_id = $1
              AND (
                (
                  t.status <> 2
                  AND (t.remark IS NULL OR t.remark NOT LIKE '【示例攻略】%')
                )
                OR (
                  t.remark LIKE '【示例攻略】%'
                  AND NOT EXISTS (
                    SELECT 1
                    FROM travel t2
                    JOIN travel_member m2 ON m2.travel_id = t2.id
                    WHERE m2.user_id = $1
                      AND t2.status <> 2
                      AND (t2.remark IS NULL OR t2.remark NOT LIKE '【示例攻略】%')
                  )
                )
              )
            ORDER BY
              CASE WHEN t.remark LIKE '【示例攻略】%' THEN 1 ELSE 0 END,
              t.create_time DESC
            "#,
        )
        .bind(user.id)
        .fetch_all(&state.pool)
        .await?
    };

    Ok(ok(rows
        .into_iter()
        .map(|r| to_vo(&r.travel, r.member_count, r.role, r.can_edit, r.can_bill))
        .collect()))
}

pub async fn detail(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<DetailQ>,
) -> Result<Json<ApiOk<TravelVo>>, AppError> {
    let m = require_member(&state.pool, q.id, user.id).await?;
    let t = find_travel(&state.pool, q.id).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM travel_member WHERE travel_id = $1")
        .bind(q.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(ok(to_vo(&t, count, m.role, m.can_edit, m.can_bill)))
}

pub async fn join(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<JoinReq>,
) -> Result<Json<ApiOk<TravelVo>>, AppError> {
    let code = req.invite_code.trim().to_uppercase();
    if code.is_empty() {
        return Err(AppError::BadRequest("请输入邀请码".into()));
    }
    let t: TravelRow = sqlx::query_as(
        r#"
        SELECT id, travel_name, destination, start_date, end_date, invite_code,
               status, creator_id, is_lock, remark
        FROM travel WHERE UPPER(invite_code) = $1
        "#,
    )
    .bind(&code)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("邀请码无效".into()))?;

    if t.status == 2 {
        return Err(AppError::BadRequest("该旅途已归档，无法加入".into()));
    }

    sqlx::query(
        r#"
        INSERT INTO travel_member (travel_id, user_id, role)
        VALUES ($1, $2, 0)
        ON CONFLICT (travel_id, user_id) DO NOTHING
        "#,
    )
    .bind(t.id)
    .bind(user.id)
    .execute(&state.pool)
    .await?;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM travel_member WHERE travel_id = $1")
        .bind(t.id)
        .fetch_one(&state.pool)
        .await?;
    let m = require_member(&state.pool, t.id, user.id).await?;
    Ok(ok(to_vo(&t, count, m.role, m.can_edit, m.can_bill)))
}

pub async fn member(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<TravelIdReq>,
) -> Result<Json<ApiOk<Vec<MemberVo>>>, AppError> {
    require_member(&state.pool, q.travel_id, user.id).await?;
    let list = list_members(&state.pool, q.travel_id).await?;
    Ok(ok(list.iter().map(member_vo).collect()))
}

pub async fn lock(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<LockReq>,
) -> Result<Json<ApiOk<TravelVo>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if crate::sample::is_sample_remark(&t.remark) {
        return Err(AppError::BadRequest("示例旅途不可修改锁定状态".into()));
    }
    if t.status == 2 {
        return Err(AppError::BadRequest("已归档旅途不可修改锁定状态".into()));
    }
    let next = req.is_lock.unwrap_or(!t.is_lock);
    sqlx::query("UPDATE travel SET is_lock = $2 WHERE id = $1")
        .bind(req.travel_id)
        .bind(next)
        .execute(&state.pool)
        .await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM travel_member WHERE travel_id = $1")
        .bind(t.id)
        .fetch_one(&state.pool)
        .await?;
    Ok(ok(to_vo(&t, count, 1, true, true)))
}

pub async fn quit(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<TravelIdReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    let m = require_member(&state.pool, req.travel_id, user.id).await?;
    if m.role == 1 {
        return Err(AppError::BadRequest("团长不能退出，请先归档旅途".into()));
    }
    let t = find_travel(&state.pool, req.travel_id).await?;
    if !t.is_lock {
        let involved: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM bill b
                WHERE b.travel_id = $1 AND b.bill_type = 1 AND (
                    b.pay_user_id = $2 OR EXISTS (
                        SELECT 1 FROM bill_share s WHERE s.bill_id = b.id AND s.user_id = $2
                    )
                )
            )
            "#,
        )
        .bind(req.travel_id)
        .bind(user.id)
        .fetch_one(&state.pool)
        .await?;
        if involved {
            return Err(AppError::BadRequest("存在未结算账单，暂不能退出".into()));
        }
    }
    sqlx::query("DELETE FROM travel_member WHERE travel_id = $1 AND user_id = $2")
        .bind(req.travel_id)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

pub async fn archive(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<TravelIdReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if t.creator_id != user.id {
        return Err(AppError::Forbidden("仅旅途创建人可归档".into()));
    }
    if crate::sample::is_sample_remark(&t.remark) {
        return Err(AppError::BadRequest("示例旅途不可归档".into()));
    }
    if t.status == 2 {
        return Err(AppError::BadRequest("旅途已归档".into()));
    }
    sqlx::query("UPDATE travel SET status = 2, is_lock = TRUE WHERE id = $1")
        .bind(req.travel_id)
        .execute(&state.pool)
        .await?;
    Ok(ok(serde_json::json!({ "ok": true })))
}

pub async fn remove(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<RemoveReq>,
) -> Result<Json<ApiOk<serde_json::Value>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    if req.user_id == user.id {
        return Err(AppError::BadRequest("不能移除自己".into()));
    }
    let t = find_travel(&state.pool, req.travel_id).await?;
    if !t.is_lock {
        let involved: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM bill b
                WHERE b.travel_id = $1 AND b.bill_type = 1 AND (
                    b.pay_user_id = $2 OR EXISTS (
                        SELECT 1 FROM bill_share s WHERE s.bill_id = b.id AND s.user_id = $2
                    )
                )
            )
            "#,
        )
        .bind(req.travel_id)
        .bind(req.user_id)
        .fetch_one(&state.pool)
        .await?;
        if involved {
            return Err(AppError::BadRequest("该成员有未结算账单，无法移除".into()));
        }
    }
    let r = sqlx::query("DELETE FROM travel_member WHERE travel_id = $1 AND user_id = $2 AND role = 0")
        .bind(req.travel_id)
        .bind(req.user_id)
        .execute(&state.pool)
        .await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound("成员不存在".into()));
    }
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct PermReq {
    pub travel_id: i64,
    pub user_id: i64,
    pub can_edit: Option<bool>,
    pub can_bill: Option<bool>,
}

pub async fn set_perm(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<PermReq>,
) -> Result<Json<ApiOk<MemberVo>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if crate::sample::is_sample_remark(&t.remark) || t.status == 2 {
        return Err(AppError::BadRequest("已归档旅途不可改权限".into()));
    }
    if req.user_id == user.id {
        return Err(AppError::BadRequest("不用给自己改权限".into()));
    }
    let target = require_member(&state.pool, req.travel_id, req.user_id).await?;
    if target.role == 1 {
        return Err(AppError::BadRequest("不能修改团长权限".into()));
    }
    let can_edit = req.can_edit.unwrap_or(target.can_edit);
    let can_bill = req.can_bill.unwrap_or(target.can_bill);
    sqlx::query("UPDATE travel_member SET can_edit=$3, can_bill=$4 WHERE travel_id=$1 AND user_id=$2 AND role=0")
        .bind(req.travel_id)
        .bind(req.user_id)
        .bind(can_edit)
        .bind(can_bill)
        .execute(&state.pool)
        .await?;
    let updated = require_member(&state.pool, req.travel_id, req.user_id).await?;
    Ok(ok(member_vo(&updated)))
}

#[derive(Deserialize)]
pub struct AddCompanionReq {
    pub travel_id: i64,
    pub nickname: String,
    pub group_name: Option<String>,
}

/// 团长添加随行成员（可无微信账号，用于按人头分账、按团体汇总）
pub async fn add_companion(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<AddCompanionReq>,
) -> Result<Json<ApiOk<MemberVo>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if crate::sample::is_sample_remark(&t.remark) || t.status == 2 {
        return Err(AppError::BadRequest("已归档/示例旅途不可加人".into()));
    }
    let nickname = req.nickname.trim();
    if nickname.is_empty() || nickname.chars().count() > 20 {
        return Err(AppError::BadRequest("请填写 1–20 字昵称".into()));
    }
    let group = req
        .group_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(20).collect::<String>());

    let open_id = format!("guest_{}_{}", req.travel_id, gen_invite_code().to_lowercase());
    let mut tx = state.pool.begin().await?;
    let uid: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO app_user (open_id, nickname, avatar)
        VALUES ($1, $2, NULL)
        RETURNING id
        "#,
    )
    .bind(&open_id)
    .bind(nickname)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO travel_member (travel_id, user_id, role, can_edit, can_bill, group_name)
        VALUES ($1, $2, 0, FALSE, FALSE, $3)
        "#,
    )
    .bind(req.travel_id)
    .bind(uid)
    .bind(group.as_deref())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    let m = require_member(&state.pool, req.travel_id, uid).await?;
    Ok(ok(member_vo(&m)))
}

#[derive(Deserialize)]
pub struct SetGroupReq {
    pub travel_id: i64,
    pub user_id: i64,
    pub group_name: Option<String>,
}

pub async fn set_group(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<SetGroupReq>,
) -> Result<Json<ApiOk<MemberVo>>, AppError> {
    require_leader(&state.pool, req.travel_id, user.id).await?;
    let t = find_travel(&state.pool, req.travel_id).await?;
    if crate::sample::is_sample_remark(&t.remark) || t.status == 2 {
        return Err(AppError::BadRequest("已归档/示例旅途不可改团体".into()));
    }
    require_member(&state.pool, req.travel_id, req.user_id).await?;
    let group = req
        .group_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.chars().take(20).collect::<String>());
    sqlx::query("UPDATE travel_member SET group_name=$3 WHERE travel_id=$1 AND user_id=$2")
        .bind(req.travel_id)
        .bind(req.user_id)
        .bind(group.as_deref())
        .execute(&state.pool)
        .await?;
    let updated = require_member(&state.pool, req.travel_id, req.user_id).await?;
    Ok(ok(member_vo(&updated)))
}
