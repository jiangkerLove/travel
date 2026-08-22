use chrono::{FixedOffset, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rand::Rng;
use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
use rust_decimal::Decimal;

use crate::error::AppError;

/// 上海时区（中国标准时间，无夏令时）
pub fn shanghai_offset() -> FixedOffset {
    FixedOffset::east_opt(8 * 3600).expect("UTC+8")
}

pub fn shanghai_now() -> NaiveDateTime {
    Utc::now().with_timezone(&shanghai_offset()).naive_local()
}

pub fn shanghai_today() -> NaiveDate {
    shanghai_now().date()
}

pub fn gen_nickname() -> String {
    const LEFT: &[&str] = &[
        "晚风", "北岛", "青野", "雾岛", "星河", "南风", "青石", "远山", "薄暮", "霜叶",
        "云隙", "潮声", "松风", "月渚", "荒原", "青岚", "暮色", "白露", "秋水", "凌川",
        "野径", "寒江", "晴空", "夜航", "山海", "银河", "晓雾", "金风", "翠微", "孤舟",
    ];
    const RIGHT: &[&str] = &[
        "行者", "旅人", "过客", "漫游", "拾光", "踏青", "远航", "听风", "问山", "追云",
        "渡河", "寻路", "观海", "栖野", "拾贝", "停云", "踏雪", "乘风", "望月", "揽星",
    ];
    let mut rng = rand::thread_rng();
    format!(
        "{}{}",
        LEFT[rng.gen_range(0..LEFT.len())],
        RIGHT[rng.gen_range(0..RIGHT.len())]
    )
}

pub fn gen_invite_code() -> String {
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..6)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

pub fn parse_date(s: &str) -> Result<NaiveDate, AppError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("日期格式应为 YYYY-MM-DD".into()))
}

pub fn parse_time(s: &Option<String>) -> Result<Option<NaiveTime>, AppError> {
    match s {
        None => Ok(None),
        Some(x) if x.trim().is_empty() => Ok(None),
        Some(x) => {
            let padded = if x.len() == 5 {
                format!("{x}:00")
            } else {
                x.clone()
            };
            NaiveTime::parse_from_str(&padded, "%H:%M:%S")
                .map(Some)
                .map_err(|_| AppError::BadRequest("时间格式应为 HH:MM".into()))
        }
    }
}

pub fn parse_datetime(s: &str) -> Result<NaiveDateTime, AppError> {
    let formats = [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%d",
    ];
    for f in formats {
        if let Ok(v) = NaiveDateTime::parse_from_str(s, f) {
            return Ok(v);
        }
        if f == "%Y-%m-%d" {
            if let Ok(d) = NaiveDate::parse_from_str(s, f) {
                return Ok(d.and_hms_opt(12, 0, 0).unwrap());
            }
        }
    }
    Err(AppError::BadRequest("消费时间格式错误".into()))
}

pub fn dec_from_f64(v: f64) -> Result<Decimal, AppError> {
    Decimal::from_f64(v).ok_or_else(|| AppError::BadRequest("金额不合法".into()))
}

pub fn dec_to_f64(v: Decimal) -> f64 {
    v.round_dp(2).to_f64().unwrap_or(0.0)
}

pub fn opt_coord_to_f64(v: Option<Decimal>) -> Option<f64> {
    v.and_then(|d| d.round_dp(6).to_f64())
}

pub fn split_amount(total: Decimal, n: usize) -> Vec<Decimal> {
    if n == 0 {
        return vec![];
    }
    let unit = (total / Decimal::from(n as i64)).round_dp(2);
    let mut parts = vec![unit; n];
    let sum: Decimal = parts.iter().copied().sum();
    if let Some(last) = parts.last_mut() {
        *last += total - sum;
    }
    parts
}

pub fn valid_point_type(v: &str) -> bool {
    matches!(v, "sight" | "hotel" | "food" | "gas" | "transport")
}

pub fn valid_traffic_type(v: &str) -> bool {
    matches!(v, "walk" | "drive" | "highspeed" | "train" | "plane" | "bus")
}

pub fn valid_cost_type(v: &str) -> bool {
    matches!(v, "hotel" | "food" | "sight" | "gas" | "transport" | "shop" | "other")
}

pub fn default_cost_of_point(point_type: &str) -> &'static str {
    match point_type {
        "sight" => "sight",
        "hotel" => "hotel",
        "food" => "food",
        "gas" => "gas",
        "transport" => "transport",
        _ => "other",
    }
}

pub fn day_count(start: NaiveDate, end: NaiveDate) -> i32 {
    (end - start).num_days() as i32 + 1
}
