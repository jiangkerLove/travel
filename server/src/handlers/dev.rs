use axum::{extract::State, Json};
use rust_decimal::Decimal;

use crate::{
    error::{ok, ApiOk, AppError},
    state::AppState,
};

#[derive(serde::Serialize)]
pub struct SeedVo {
    pub invite_code: String,
    pub users: Vec<SeedUser>,
}

#[derive(serde::Serialize)]
pub struct SeedUser {
    pub open_id: String,
    pub nickname: String,
}

pub async fn seed(State(state): State<AppState>) -> Result<Json<ApiOk<SeedVo>>, AppError> {
    if !state.dev_mode {
        return Err(AppError::Forbidden("未开启开发模式".into()));
    }

    let users = [
        ("demo_deer", "小鹿", None::<String>),
        ("demo_wei", "阿伟", None::<String>),
        ("demo_lin", "小林", None::<String>),
    ];
    let mut ids = Vec::new();
    for (open_id, nickname, avatar) in users {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO app_user (open_id, nickname, avatar)
            VALUES ($1, $2, $3)
            ON CONFLICT (open_id) DO UPDATE SET nickname = EXCLUDED.nickname
            RETURNING id
            "#,
        )
        .bind(open_id)
        .bind(nickname)
        .bind(avatar)
        .fetch_one(&state.pool)
        .await?;
        ids.push(id);
    }

    let invite = "DEMO88";
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM travel WHERE invite_code=$1")
        .bind(invite)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_some() {
        return Ok(ok(SeedVo {
            invite_code: invite.into(),
            users: vec![
                SeedUser { open_id: "demo_deer".into(), nickname: "小鹿".into() },
                SeedUser { open_id: "demo_wei".into(), nickname: "阿伟".into() },
                SeedUser { open_id: "demo_lin".into(), nickname: "小林".into() },
            ],
        }));
    }

    let mut tx = state.pool.begin().await?;
    let start: chrono::NaiveDate = "2026-08-20".parse().unwrap();
    let end: chrono::NaiveDate = "2026-08-23".parse().unwrap();
    let travel_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO travel (travel_name, destination, start_date, end_date, invite_code, creator_id, remark)
        VALUES ('川西小环线', '四姑娘山', $1, $2, $3, $4, '自驾看山，账单AA')
        RETURNING id
        "#,
    )
    .bind(start)
    .bind(end)
    .bind(invite)
    .bind(ids[0])
    .fetch_one(&mut *tx)
    .await?;

    for (i, uid) in ids.iter().enumerate() {
        sqlx::query("INSERT INTO travel_member (travel_id, user_id, role, can_edit, can_bill) VALUES ($1,$2,$3,$4,$5)")
            .bind(travel_id)
            .bind(uid)
            .bind(if i == 0 { 1_i16 } else { 0_i16 })
            .bind(i == 0)
            .bind(i == 0)
            .execute(&mut *tx)
            .await?;
    }

    let plans: Vec<(i32, &str, &str, f64, f64, &str, &str, &str, i32)> = vec![
        (1, "transport", "成都双流机场", 103.947, 30.578, "09:00", "10:00", "plane", 0),
        (1, "food", "宽窄巷子火锅", 104.054, 30.672, "12:00", "13:30", "drive", 1),
        (1, "sight", "春熙路", 104.082, 30.657, "15:00", "17:00", "walk", 2),
        (1, "hotel", "成都太古里酒店", 104.083, 30.655, "18:30", "08:00", "drive", 3),
        (2, "sight", "都江堰景区", 103.610, 31.004, "10:00", "13:00", "drive", 0),
        (2, "sight", "青城山", 103.570, 30.900, "14:30", "17:30", "drive", 1),
        (2, "hotel", "都江堰民宿", 103.620, 31.010, "19:00", "08:00", "drive", 2),
        (3, "gas", "映秀加油站", 103.484, 31.048, "09:00", "09:20", "drive", 0),
        (3, "sight", "四姑娘山双桥沟", 102.900, 31.108, "12:00", "16:30", "drive", 1),
        (3, "food", "日隆藏式餐厅", 102.830, 30.993, "18:00", "19:00", "walk", 2),
        (3, "hotel", "日隆镇住宿", 102.828, 30.990, "19:30", "08:00", "walk", 3),
    ];
    let mut plan_ids = Vec::new();
    for (day, ptype, name, lng, lat, arrive, leave, traffic, sort) in plans {
        let arrive_t = chrono::NaiveTime::parse_from_str(&format!("{arrive}:00"), "%H:%M:%S").ok();
        let leave_t = chrono::NaiveTime::parse_from_str(&format!("{leave}:00"), "%H:%M:%S").ok();
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO day_plan (travel_id, day_num, point_type, place_name, longitude, latitude,
                arrive_time, leave_time, stay_duration, traffic_type, sort)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
            RETURNING id
            "#,
        )
        .bind(travel_id)
        .bind(day)
        .bind(ptype)
        .bind(name)
        .bind(lng)
        .bind(lat)
        .bind(arrive_t)
        .bind(leave_t)
        .bind(90_i32)
        .bind(traffic)
        .bind(sort)
        .fetch_one(&mut *tx)
        .await?;
        plan_ids.push(id);
    }

    // 集体账单
    let bills: Vec<(&str, &str, &str, i64, i64, Option<i64>)> = vec![
        ("机票拼车到市区", "1800.00", "transport", ids[0], plan_ids[0], None),
        ("宽窄巷子火锅", "328.00", "food", ids[1], plan_ids[1], None),
        ("春熙路门票小食", "96.00", "sight", ids[2], plan_ids[2], None),
        ("成都酒店两晚", "900.00", "hotel", ids[2], plan_ids[3], None),
        ("都江堰门票", "240.00", "sight", ids[0], plan_ids[4], None),
        ("加油", "420.00", "gas", ids[1], plan_ids[7], None),
    ];
    for (name, amt, cost, payer, plan_id, _) in bills {
        let amount: Decimal = amt.parse().unwrap();
        let bill_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO bill (travel_id, day_plan_id, bill_name, amount, bill_type, cost_type,
                pay_user_id, consume_time, visible_all)
            VALUES ($1,$2,$3,$4,1,$5,$6,$7,TRUE)
            RETURNING id
            "#,
        )
        .bind(travel_id)
        .bind(plan_id)
        .bind(name)
        .bind(amount)
        .bind(cost)
        .bind(payer)
        .bind(chrono::NaiveDateTime::parse_from_str("2026-08-20 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
        .fetch_one(&mut *tx)
        .await?;
        let share = (amount / Decimal::from(3)).round_dp(2);
        let mut parts = [share, share, share];
        parts[2] = amount - share - share;
        for (i, uid) in ids.iter().enumerate() {
            sqlx::query("INSERT INTO bill_share (bill_id, user_id, share_amount) VALUES ($1,$2,$3)")
                .bind(bill_id)
                .bind(uid)
                .bind(parts[i])
                .execute(&mut *tx)
                .await?;
        }
    }

    // 小鹿私人账单
    sqlx::query(
        r#"
        INSERT INTO bill (travel_id, bill_name, amount, bill_type, cost_type, pay_user_id, consume_time, visible_all, remark)
        VALUES ($1, '纪念品冰箱贴', 68.00, 2, 'other', $2, $3, FALSE, '仅自己可见')
        "#,
    )
    .bind(travel_id)
    .bind(ids[0])
    .bind(chrono::NaiveDateTime::parse_from_str("2026-08-21 16:00:00", "%Y-%m-%d %H:%M:%S").unwrap())
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(ok(SeedVo {
        invite_code: invite.into(),
        users: vec![
            SeedUser { open_id: "demo_deer".into(), nickname: "小鹿".into() },
            SeedUser { open_id: "demo_wei".into(), nickname: "阿伟".into() },
            SeedUser { open_id: "demo_lin".into(), nickname: "小林".into() },
        ],
    }))
}
