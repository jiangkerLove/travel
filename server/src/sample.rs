use chrono::{Duration, NaiveDate};
use rust_decimal::Decimal;
use sqlx::PgPool;

use crate::{error::AppError, util::split_amount};

pub const SAMPLE_REMARK: &str =
    "【示例攻略】成都出发，四姑娘山—丹巴—新都桥小环线。已结束示例，可看行程、账单与分账，不可编辑。";

pub fn is_sample_remark(remark: &Option<String>) -> bool {
    remark
        .as_deref()
        .is_some_and(|s| s.starts_with("【示例攻略】"))
}

pub fn should_grant_sample(open_id: &str) -> bool {
    !open_id.starts_with("demo_") && !open_id.starts_with("sys_guide_")
}

/// 无真实行程时准备一份已归档的只读示例；有进行中的真实行程后首页不再展示。
pub async fn ensure_sample_travel(pool: &PgPool, user_id: i64) -> Result<(), AppError> {
    // 统一为已归档只读，便于查看智能分账
    sqlx::query(
        r#"
        UPDATE travel t
        SET status = 2,
            is_lock = TRUE,
            remark = $2
        FROM travel_member m
        WHERE m.travel_id = t.id
          AND m.user_id = $1
          AND t.remark LIKE '【示例攻略】%'
        "#,
    )
    .bind(user_id)
    .bind(SAMPLE_REMARK)
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        UPDATE travel_member m
        SET can_edit = FALSE, can_bill = FALSE
        FROM travel t
        WHERE m.travel_id = t.id
          AND t.remark LIKE '【示例攻略】%'
          AND EXISTS (
              SELECT 1 FROM travel_member x
              WHERE x.travel_id = t.id AND x.user_id = $1
          )
        "#,
    )
    .bind(user_id)
    .execute(pool)
    .await?;

    let has_sample: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM travel t
            JOIN travel_member m ON m.travel_id = t.id
            WHERE m.user_id = $1 AND t.remark LIKE '【示例攻略】%'
        )
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if has_sample {
        return Ok(());
    }

    let active_real: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM travel t
        JOIN travel_member m ON m.travel_id = t.id
        WHERE m.user_id = $1
          AND t.status <> 2
          AND (t.remark IS NULL OR t.remark NOT LIKE '【示例攻略】%')
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if active_real > 0 {
        return Ok(());
    }
    create_sample_travel(pool, user_id, None, None).await?;
    Ok(())
}

pub async fn create_sample_travel(
    pool: &PgPool,
    creator_id: i64,
    companion_ids: Option<[i64; 2]>,
    invite_code: Option<&str>,
) -> Result<i64, AppError> {
    // 已结束的四天行程，便于直接看账单与分账
    let end = crate::util::shanghai_today() - Duration::days(7);
    let start = end - Duration::days(3);
    create_sample_travel_with_dates(pool, creator_id, companion_ids, invite_code, start, end).await
}

pub async fn create_sample_travel_with_dates(
    pool: &PgPool,
    creator_id: i64,
    companion_ids: Option<[i64; 2]>,
    invite_code: Option<&str>,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<i64, AppError> {
    let mut tx = pool.begin().await?;
    let mates = match companion_ids {
        Some(ids) => ids,
        None => [
            upsert_user(&mut tx, "sys_guide_wei", "阿伟").await?,
            upsert_user(&mut tx, "sys_guide_lin", "小林").await?,
        ],
    };
    let ids = [creator_id, mates[0], mates[1]];

    let invite = unique_invite(&mut tx, invite_code).await?;
    let travel_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO travel (travel_name, destination, start_date, end_date, invite_code, creator_id, remark, status, is_lock)
        VALUES ('川西小环线', '四姑娘山 · 丹巴', $1, $2, $3, $4, $5, 2, TRUE)
        RETURNING id
        "#,
    )
    .bind(start)
    .bind(end)
    .bind(&invite)
    .bind(creator_id)
    .bind(SAMPLE_REMARK)
    .fetch_one(&mut *tx)
    .await?;

    for (i, uid) in ids.iter().enumerate() {
        sqlx::query(
            "INSERT INTO travel_member (travel_id, user_id, role, can_edit, can_bill) VALUES ($1,$2,$3,FALSE,FALSE)",
        )
        .bind(travel_id)
        .bind(uid)
        .bind(if i == 0 { 1_i16 } else { 0_i16 })
        .execute(&mut *tx)
        .await?;
    }

    let plans: [PlanSeed; 12] = [
        PlanSeed {
            day: 1,
            ptype: "food",
            name: "宽窄巷子集合",
            lng: 104.054,
            lat: 30.672,
            arrive: "08:00",
            leave: "09:00",
            traffic: "walk",
            sort: 0,
            stay: 60,
            remark: Some("成都出发前吃个早饭，车上备点零食和水"),
        },
        PlanSeed {
            day: 1,
            ptype: "sight",
            name: "映秀镇",
            lng: 103.484,
            lat: 31.048,
            arrive: "11:00",
            leave: "11:20",
            traffic: "drive",
            sort: 1,
            stay: 20,
            remark: Some("都汶高速下来歇脚，之后盘山路注意会车"),
        },
        PlanSeed {
            day: 1,
            ptype: "gas",
            name: "映秀加油站",
            lng: 103.484,
            lat: 31.048,
            arrive: "11:20",
            leave: "11:40",
            traffic: "drive",
            sort: 2,
            stay: 20,
            remark: Some("进山前加满油，日隆油价更高"),
        },
        PlanSeed {
            day: 1,
            ptype: "hotel",
            name: "日隆镇住宿",
            lng: 102.828,
            lat: 30.990,
            arrive: "16:30",
            leave: "08:00",
            traffic: "drive",
            sort: 3,
            stay: 0,
            remark: Some("四姑娘山脚下，明早进双桥沟更近"),
        },
        PlanSeed {
            day: 2,
            ptype: "sight",
            name: "四姑娘山双桥沟",
            lng: 102.900,
            lat: 31.108,
            arrive: "08:30",
            leave: "16:00",
            traffic: "drive",
            sort: 0,
            stay: 420,
            remark: Some("沟内观光车往返，建议早点进沟，下午原路回日隆"),
        },
        PlanSeed {
            day: 2,
            ptype: "food",
            name: "日隆藏式火锅",
            lng: 102.830,
            lat: 30.993,
            arrive: "18:00",
            leave: "19:30",
            traffic: "walk",
            sort: 1,
            stay: 90,
            remark: Some("牦牛肉火锅，三人可拼一锅"),
        },
        PlanSeed {
            day: 2,
            ptype: "hotel",
            name: "日隆镇住宿",
            lng: 102.828,
            lat: 30.990,
            arrive: "20:00",
            leave: "08:00",
            traffic: "walk",
            sort: 2,
            stay: 0,
            remark: None,
        },
        PlanSeed {
            day: 3,
            ptype: "sight",
            name: "甲居藏寨",
            lng: 101.963,
            lat: 30.951,
            arrive: "12:30",
            leave: "16:00",
            traffic: "drive",
            sort: 0,
            stay: 180,
            remark: Some("嘉绒民居，停车后步行进寨，别错过碉楼观景台"),
        },
        PlanSeed {
            day: 3,
            ptype: "food",
            name: "丹巴县城晚餐",
            lng: 101.891,
            lat: 30.877,
            arrive: "18:00",
            leave: "19:30",
            traffic: "drive",
            sort: 1,
            stay: 90,
            remark: None,
        },
        PlanSeed {
            day: 3,
            ptype: "hotel",
            name: "丹巴住宿",
            lng: 101.891,
            lat: 30.877,
            arrive: "20:00",
            leave: "08:30",
            traffic: "walk",
            sort: 2,
            stay: 0,
            remark: None,
        },
        PlanSeed {
            day: 4,
            ptype: "sight",
            name: "新都桥光影小镇",
            lng: 101.491,
            lat: 30.043,
            arrive: "12:00",
            leave: "14:00",
            traffic: "drive",
            sort: 0,
            stay: 120,
            remark: Some("摄影小镇，下午光线更好，可在这转一圈再返程"),
        },
        PlanSeed {
            day: 4,
            ptype: "sight",
            name: "泸定桥",
            lng: 102.234,
            lat: 29.914,
            arrive: "16:30",
            leave: "17:30",
            traffic: "drive",
            sort: 1,
            stay: 60,
            remark: Some("铁索桥步行过河，之后沿国道回成都"),
        },
    ];

    let mut plan_ids = Vec::with_capacity(plans.len());
    for p in &plans {
        let arrive_t = parse_hm(p.arrive);
        let leave_t = parse_hm(p.leave);
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO day_plan (travel_id, day_num, point_type, place_name, longitude, latitude,
                arrive_time, leave_time, stay_duration, traffic_type, sort, remark)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
            RETURNING id
            "#,
        )
        .bind(travel_id)
        .bind(p.day)
        .bind(p.ptype)
        .bind(p.name)
        .bind(p.lng)
        .bind(p.lat)
        .bind(arrive_t)
        .bind(leave_t)
        .bind(p.stay)
        .bind(p.traffic)
        .bind(p.sort)
        .bind(p.remark)
        .fetch_one(&mut *tx)
        .await?;
        plan_ids.push(id);
    }

    let bills: [BillSeed; 6] = [
        BillSeed {
            name: "成都租车四天",
            amount: "1600.00",
            cost: "transport",
            payer: 0,
            plan_idx: Some(0),
            visible_all: true,
            remark: Some("含基本险，油费另计"),
            day_offset: 0,
        },
        BillSeed {
            name: "映秀加油",
            amount: "420.00",
            cost: "gas",
            payer: 1,
            plan_idx: Some(2),
            visible_all: true,
            remark: None,
            day_offset: 0,
        },
        BillSeed {
            name: "日隆民宿两晚",
            amount: "760.00",
            cost: "hotel",
            payer: 0,
            plan_idx: Some(3),
            visible_all: true,
            remark: None,
            day_offset: 0,
        },
        BillSeed {
            name: "双桥沟门票+观光车",
            amount: "360.00",
            cost: "sight",
            payer: 2,
            plan_idx: Some(4),
            visible_all: true,
            remark: Some("三人票"),
            day_offset: 1,
        },
        BillSeed {
            name: "甲居藏寨门票",
            amount: "150.00",
            cost: "sight",
            payer: 0,
            plan_idx: Some(7),
            visible_all: true,
            remark: None,
            day_offset: 2,
        },
        BillSeed {
            name: "四姑娘山冰箱贴",
            amount: "36.00",
            cost: "shop",
            payer: 0,
            plan_idx: None,
            visible_all: false,
            remark: Some("仅自己可见的个人开销"),
            day_offset: 1,
        },
    ];

    for b in &bills {
        let amount: Decimal = b.amount.parse().unwrap();
        let consume = (start + Duration::days(b.day_offset))
            .and_hms_opt(12, 0, 0)
            .unwrap();
        let plan_id = b.plan_idx.map(|i| plan_ids[i]);
        let bill_type: i16 = if b.visible_all { 1 } else { 2 };
        let bill_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO bill (travel_id, day_plan_id, bill_name, amount, bill_type, cost_type,
                pay_user_id, consume_time, visible_all, remark)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)
            RETURNING id
            "#,
        )
        .bind(travel_id)
        .bind(plan_id)
        .bind(b.name)
        .bind(amount)
        .bind(bill_type)
        .bind(b.cost)
        .bind(ids[b.payer])
        .bind(consume)
        .bind(b.visible_all)
        .bind(b.remark)
        .fetch_one(&mut *tx)
        .await?;

        if b.visible_all {
            let parts = split_amount(amount, ids.len());
            for (i, uid) in ids.iter().enumerate() {
                sqlx::query("INSERT INTO bill_share (bill_id, user_id, share_amount) VALUES ($1,$2,$3)")
                    .bind(bill_id)
                    .bind(uid)
                    .bind(parts[i])
                    .execute(&mut *tx)
                    .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(travel_id)
}

struct PlanSeed {
    day: i32,
    ptype: &'static str,
    name: &'static str,
    lng: f64,
    lat: f64,
    arrive: &'static str,
    leave: &'static str,
    traffic: &'static str,
    sort: i32,
    stay: i32,
    remark: Option<&'static str>,
}

struct BillSeed {
    name: &'static str,
    amount: &'static str,
    cost: &'static str,
    payer: usize,
    plan_idx: Option<usize>,
    visible_all: bool,
    remark: Option<&'static str>,
    day_offset: i64,
}

fn parse_hm(s: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(&format!("{s}:00"), "%H:%M:%S").ok()
}

async fn upsert_user(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    open_id: &str,
    nickname: &str,
) -> Result<i64, AppError> {
    Ok(sqlx::query_scalar(
        r#"
        INSERT INTO app_user (open_id, nickname)
        VALUES ($1, $2)
        ON CONFLICT (open_id) DO UPDATE SET nickname = EXCLUDED.nickname
        RETURNING id
        "#,
    )
    .bind(open_id)
    .bind(nickname)
    .fetch_one(&mut **tx)
    .await?)
}

async fn unique_invite(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    preferred: Option<&str>,
) -> Result<String, AppError> {
    if let Some(code) = preferred {
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM travel WHERE invite_code = $1)")
                .bind(code)
                .fetch_one(&mut **tx)
                .await?;
        if !exists {
            return Ok(code.to_string());
        }
    }
    for _ in 0..8 {
        let invite = crate::util::gen_invite_code();
        let exists: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM travel WHERE invite_code = $1)")
                .bind(&invite)
                .fetch_one(&mut **tx)
                .await?;
        if !exists {
            return Ok(invite);
        }
    }
    Ok(crate::util::gen_invite_code())
}
