use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};
use serde::Serialize;
use std::collections::HashSet;

use crate::util::shanghai_now;

const LUNAR: [u32; 201] = [
    0x04bd8, 0x04ae0, 0x0a570, 0x054d5, 0x0d260, 0x0d950, 0x16554, 0x056a0, 0x09ad0, 0x055d2,
    0x04ae0, 0x0a5b6, 0x0a4d0, 0x0d250, 0x1d255, 0x0b540, 0x0d6a0, 0x0ada2, 0x095b0, 0x14977,
    0x04970, 0x0a4b0, 0x0b4b5, 0x06a50, 0x06d40, 0x1ab54, 0x02b60, 0x09570, 0x052f2, 0x04970,
    0x06566, 0x0d4a0, 0x0ea50, 0x06e95, 0x05ad0, 0x02b60, 0x186e3, 0x092e0, 0x1c8d7, 0x0c950,
    0x0d4a0, 0x1d8a6, 0x0b550, 0x056a0, 0x1a5b4, 0x025d0, 0x092d0, 0x0d2b2, 0x0a950, 0x0b557,
    0x06ca0, 0x0b550, 0x15355, 0x04da0, 0x0a5d0, 0x14573, 0x052d0, 0x0a9a8, 0x0e950, 0x06aa0,
    0x0aea6, 0x0ab50, 0x04b60, 0x0aae4, 0x0a570, 0x05260, 0x0f263, 0x0d950, 0x05b57, 0x056a0,
    0x096d0, 0x04dd5, 0x04ad0, 0x0a4d0, 0x0d4d4, 0x0d250, 0x0d558, 0x0b540, 0x0b5a0, 0x195a6,
    0x095b0, 0x049b0, 0x0a974, 0x0a4b0, 0x0b27a, 0x06a50, 0x06d40, 0x0af46, 0x0ab60, 0x09570,
    0x04af5, 0x04970, 0x064b0, 0x074a3, 0x0ea50, 0x06b58, 0x055c0, 0x0ab60, 0x096d5, 0x092e0,
    0x0c960, 0x0d954, 0x0d4a0, 0x0da50, 0x07552, 0x056a0, 0x0abb7, 0x025d0, 0x092d0, 0x0cab5,
    0x0a950, 0x0b4a0, 0x0baa4, 0x0ad50, 0x055d9, 0x04ba0, 0x0a5b0, 0x15176, 0x052b0, 0x0a930,
    0x07954, 0x06aa0, 0x0ad50, 0x05b52, 0x04b60, 0x0a6e6, 0x0a4e0, 0x0d260, 0x0ea65, 0x0d530,
    0x05aa0, 0x076a3, 0x096d0, 0x04bd7, 0x04ad0, 0x0a4d0, 0x1d0b6, 0x0d250, 0x0d520, 0x0dd45,
    0x0b5a0, 0x056d0, 0x055b2, 0x049b0, 0x0a577, 0x0a4b0, 0x0aa50, 0x1b255, 0x06d20, 0x0ada0,
    0x14b63, 0x09370, 0x049f8, 0x04970, 0x064b0, 0x168a6, 0x0ea50, 0x06b20, 0x1a6c4, 0x0aae0,
    0x0a2e0, 0x0d2e3, 0x0c960, 0x0d557, 0x0d4a0, 0x0da50, 0x05d55, 0x056a0, 0x0a6d0, 0x055d4,
    0x052d0, 0x0a9b8, 0x0a950, 0x0b4a0, 0x0b6a6, 0x0ad50, 0x055a0, 0x0aba4, 0x0a5b0, 0x052b0,
    0x0b273, 0x06930, 0x07337, 0x06aa0, 0x0ad50, 0x14b55, 0x04b60, 0x0a570, 0x054e4, 0x0d160,
    0x0e968, 0x0d520, 0x0daa0, 0x16aa6, 0x056d0, 0x04ae0, 0x0a9d4, 0x0a2d0, 0x0d150, 0x0f252,
    0x0d520,
];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HolidayVo {
    pub name: String,
    pub start: String,
    pub end: String,
    pub start_text: String,
    pub until: i64,
    pub ongoing: bool,
    pub hint: String,
}

#[derive(Serialize, Clone)]
pub struct ExtraRow {
    pub k: String,
    pub v: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WorkLifeVo {
    pub ready: bool,
    pub retired: bool,
    pub age: i32,
    pub gender_text: String,
    pub holiday: Option<HolidayVo>,
    pub extras: Vec<ExtraRow>,
    pub retire_age_text: String,
    pub retire_date_text: String,
    pub rest_text: String,
    pub work_text: String,
    pub rest_hint: String,
    pub work_hint: String,
}

#[derive(Clone, Copy)]
struct Span {
    work: i64,
    rest: i64,
}

fn now() -> NaiveDateTime {
    shanghai_now()
}

fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap_or_else(|| NaiveDate::from_ymd_opt(y, m, 1).unwrap())
}

fn add_days(d: NaiveDate, n: i64) -> NaiveDate {
    d + Duration::days(n)
}

fn days_in_month(y: i32, m: u32) -> u32 {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    (ymd(ny, nm, 1) - Duration::days(1)).day()
}

fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let mut y = d.year();
    let mut m = d.month() as i32 + months;
    while m > 12 {
        m -= 12;
        y += 1;
    }
    while m < 1 {
        m += 12;
        y -= 1;
    }
    let last = days_in_month(y, m as u32);
    ymd(y, m as u32, d.day().min(last))
}

fn diff_days(a: NaiveDate, b: NaiveDate) -> i64 {
    (b - a).num_days()
}

fn js_weekday(d: NaiveDate) -> u32 {
    match d.weekday() {
        Weekday::Sun => 0,
        Weekday::Mon => 1,
        Weekday::Tue => 2,
        Weekday::Wed => 3,
        Weekday::Thu => 4,
        Weekday::Fri => 5,
        Weekday::Sat => 6,
    }
}

fn is_weekend(d: NaiveDate) -> bool {
    matches!(d.weekday(), Weekday::Sat | Weekday::Sun)
}

fn leap_month(y: i32) -> u32 {
    LUNAR[(y - 1900) as usize] & 0xf
}

fn leap_days(y: i32) -> u32 {
    if leap_month(y) == 0 {
        0
    } else if LUNAR[(y - 1900) as usize] & 0x10000 != 0 {
        30
    } else {
        29
    }
}

fn month_days(y: i32, m: u32) -> u32 {
    if LUNAR[(y - 1900) as usize] & (0x10000 >> m) != 0 {
        30
    } else {
        29
    }
}

fn l_year_days(y: i32) -> u32 {
    let mut sum = 348u32;
    let mut i = 0x8000u32;
    while i > 0x8 {
        if LUNAR[(y - 1900) as usize] & i != 0 {
            sum += 1;
        }
        i >>= 1;
    }
    sum + leap_days(y)
}

fn lunar_to_solar(ly: i32, lm: u32, ld: u32) -> NaiveDate {
    let mut offset = 0i64;
    for y in 1900..ly {
        offset += l_year_days(y) as i64;
    }
    let leap = leap_month(ly);
    for m in 1..lm {
        offset += month_days(ly, m) as i64;
        if leap != 0 && m == leap {
            offset += leap_days(ly) as i64;
        }
    }
    offset += ld as i64 - 1;
    add_days(ymd(1900, 1, 31), offset)
}

fn qingming(year: i32) -> NaiveDate {
    let c = year % 100;
    let day = ((c as f64) * 0.2422 + 4.81).floor() as i32 - c / 4;
    ymd(year, 4, day as u32)
}

fn three_day_break(day: NaiveDate) -> (NaiveDate, NaiveDate) {
    match js_weekday(day) {
        3 => (day, day),
        4 | 5 => (day, add_days(day, 2)),
        6 | 0 => (add_days(day, -1), add_days(day, 1)),
        1 | 2 => (add_days(day, -2), day),
        _ => (day, day),
    }
}

struct Break {
    name: &'static str,
    start: NaiveDate,
    end: NaiveDate,
}

fn generate_year_breaks(year: i32) -> Vec<Break> {
    let cny = lunar_to_solar(year, 1, 1);
    let eve = add_days(cny, -1);
    let duanwu = lunar_to_solar(year, 5, 5);
    let mid = lunar_to_solar(year, 8, 15);
    let qing = qingming(year);
    let labor = ymd(year, 5, 1);
    let nat_start = ymd(year, 10, 1);
    let (ny, ny_e) = three_day_break(ymd(year, 1, 1));
    let (qy, qy_e) = three_day_break(qing);
    let (dy, dy_e) = three_day_break(duanwu);
    let (my, my_e) = three_day_break(mid);
    let mut items = vec![
        Break { name: "元旦", start: ny, end: ny_e },
        Break { name: "春节", start: eve, end: add_days(cny, 6) },
        Break { name: "清明", start: qy, end: qy_e },
        Break { name: "劳动节", start: labor, end: add_days(labor, 4) },
        Break { name: "端午", start: dy, end: dy_e },
        Break { name: "中秋", start: my, end: my_e },
        Break { name: "国庆", start: nat_start, end: ymd(year, 10, 7) },
    ];
    let mid_i = items.iter().position(|x| x.name == "中秋");
    let nat_i = items.iter().position(|x| x.name == "国庆");
    if let (Some(mi), Some(ni)) = (mid_i, nat_i) {
        let a = &items[mi];
        let b = &items[ni];
        if a.start <= b.end && b.start <= a.end {
            items[ni].name = "中秋·国庆";
            items[ni].end = add_days(nat_start, 7);
            items.remove(mi);
        }
    }
    items.sort_by_key(|x| x.start);
    items
}

fn statutory_days(year: i32) -> Vec<NaiveDate> {
    let cny = lunar_to_solar(year, 1, 1);
    vec![
        ymd(year, 1, 1),
        add_days(cny, -1),
        cny,
        add_days(cny, 1),
        add_days(cny, 2),
        qingming(year),
        ymd(year, 5, 1),
        ymd(year, 5, 2),
        lunar_to_solar(year, 5, 5),
        lunar_to_solar(year, 8, 15),
        ymd(year, 10, 1),
        ymd(year, 10, 2),
        ymd(year, 10, 3),
    ]
}

fn next_holiday(now: NaiveDateTime) -> Option<HolidayVo> {
    let from = now.date();
    for y in from.year()..=from.year() + 2 {
        for h in generate_year_breaks(y) {
            if h.end >= from {
                let until = diff_days(from, h.start).max(0);
                let ongoing = until == 0 && h.start <= from;
                return Some(HolidayVo {
                    name: h.name.into(),
                    start: h.start.to_string(),
                    end: h.end.to_string(),
                    start_text: format!("{}月{}日", h.start.month(), h.start.day()),
                    until,
                    ongoing,
                    hint: span_hint(now, h.start, h.end, h.name),
                });
            }
        }
    }
    None
}

const OFF_HOUR: u32 = 18;

struct RestSpan {
    name: String,
    start: NaiveDate,
    end: NaiveDate,
}

fn named_holiday(d: NaiveDate) -> Option<String> {
    for y in (d.year() - 1)..=(d.year() + 1) {
        for h in generate_year_breaks(y) {
            if d >= h.start && d <= h.end {
                return Some(h.name.to_string());
            }
        }
    }
    None
}

fn is_rest_day(d: NaiveDate) -> bool {
    is_weekend(d) || named_holiday(d).is_some()
}

fn expand_rest(d: NaiveDate) -> RestSpan {
    let mut start = d;
    let mut end = d;
    while is_rest_day(add_days(start, -1)) {
        start = add_days(start, -1);
    }
    while is_rest_day(add_days(end, 1)) {
        end = add_days(end, 1);
    }
    let mut name = "周末".to_string();
    let mut cur = start;
    while cur <= end {
        if let Some(n) = named_holiday(cur) {
            name = n;
            break;
        }
        cur = add_days(cur, 1);
    }
    RestSpan { name, start, end }
}

fn rest_span_at(d: NaiveDate) -> Option<RestSpan> {
    if is_rest_day(d) {
        Some(expand_rest(d))
    } else {
        None
    }
}

fn next_rest_after(d: NaiveDate) -> Option<RestSpan> {
    for i in 1..16 {
        let n = add_days(d, i);
        if is_rest_day(n) {
            return Some(expand_rest(n));
        }
    }
    None
}

fn hours_left(now: NaiveDateTime, target: NaiveDateTime) -> i64 {
    if now >= target {
        return 0;
    }
    let secs = (target - now).num_seconds();
    (secs + 3599) / 3600
}

fn at_hour(d: NaiveDate, hour: u32) -> NaiveDateTime {
    d.and_hms_opt(hour, 0, 0).unwrap()
}

fn midnight_after(d: NaiveDate) -> NaiveDateTime {
    at_hour(add_days(d, 1), 0)
}

fn is_named_holiday(name: &str) -> bool {
    name != "周末"
}

fn break_countdown(now: NaiveDateTime) -> ExtraRow {
    let today = now.date();
    if let Some(span) = rest_span_at(today) {
        let holiday = is_named_holiday(&span.name);
        let k = if holiday { "假期中" } else { "周末" };
        if today == span.end {
            let h = hours_left(now, midnight_after(span.end)).max(1);
            ExtraRow {
                k: k.into(),
                v: format!("还有 {h} 小时结束"),
            }
        } else {
            let days = diff_days(today, span.end).max(1);
            ExtraRow {
                k: k.into(),
                v: format!("还有 {days} 天结束"),
            }
        }
    } else if let Some(span) = next_rest_after(today) {
        let holiday = is_named_holiday(&span.name);
        let k = if holiday { "距离假期" } else { "距离周末" };
        let label = span.name.clone();
        let last_work = add_days(span.start, -1);
        if today == last_work {
            let h = hours_left(now, at_hour(today, OFF_HOUR));
            let v = if h == 0 {
                let days = (diff_days(span.start, span.end) + 1).max(1);
                if holiday {
                    format!("假期还有 {days} 天")
                } else {
                    format!("周末还有 {days} 天")
                }
            } else {
                format!("还有 {h} 小时开始{label}")
            };
            ExtraRow { k: k.into(), v }
        } else {
            let days = diff_days(today, span.start).max(1);
            ExtraRow {
                k: k.into(),
                v: format!("还有 {days} 天"),
            }
        }
    } else {
        ExtraRow {
            k: "距离周末".into(),
            v: "—".into(),
        }
    }
}

fn span_hint(now: NaiveDateTime, start: NaiveDate, end: NaiveDate, name: &str) -> String {
    let today = now.date();
    if today >= start && today <= end {
        if today == end {
            format!("还有 {} 小时结束", hours_left(now, midnight_after(end)).max(1))
        } else {
            format!("还有 {} 天结束", diff_days(today, end).max(1))
        }
    } else if today + Duration::days(1) == start {
        let h = hours_left(now, at_hour(today, OFF_HOUR));
        if h == 0 {
            let days = (diff_days(start, end) + 1).max(1);
            if is_named_holiday(name) {
                format!("假期还有 {days} 天")
            } else {
                format!("周末还有 {days} 天")
            }
        } else {
            format!("还有 {h} 小时开始{name}")
        }
    } else {
        format!("还有 {} 天", diff_days(today, start).max(1))
    }
}

fn delay_months(birth: NaiveDate, orig_age: i32, step_months: i32, max_delay: i32) -> i32 {
    let anchor = match orig_age {
        50 => 1975,
        55 => 1970,
        _ => 1965,
    };
    let i = (birth.year() - anchor) * 12 + birth.month() as i32 - 1;
    if i < 0 {
        0
    } else {
        ((i / step_months) + 1).min(max_delay)
    }
}

struct Retire {
    date: NaiveDate,
    date_text: String,
    age_text: String,
}

fn retirement_of(birth: NaiveDate, gender: i16, female_role: i16) -> Retire {
    let male = gender == 1;
    let cadre = female_role == 1;
    let orig_age = if male { 60 } else if cadre { 55 } else { 50 };
    let step = if male || cadre { 4 } else { 2 };
    let max = if male || cadre { 36 } else { 60 };
    let delay = delay_months(birth, orig_age, step, max);
    let date = add_months(add_months(birth, orig_age * 12), delay);
    let extra = delay % 12;
    let years = orig_age + delay / 12;
    let age_text = if extra > 0 {
        format!("{years}岁{extra}个月")
    } else {
        format!("{years}岁")
    };
    Retire {
        date,
        date_text: format!("{}年{}月", date.year(), date.month()),
        age_text,
    }
}

fn clamp_range(
    from: NaiveDate,
    to: NaiveDate,
    start: NaiveDate,
    end: NaiveDate,
) -> Option<(NaiveDate, NaiveDate)> {
    let a = from.max(start);
    let b = to.min(end);
    if a < b {
        Some((a, b))
    } else {
        None
    }
}

fn holiday_weekdays(from: NaiveDate, to: NaiveDate) -> HashSet<NaiveDate> {
    let mut keys = HashSet::new();
    for y in (from.year() - 1)..=to.year() {
        for d in statutory_days(y) {
            let mut rest = d;
            if is_weekend(d) {
                rest = add_days(d, 1);
                while is_weekend(rest) || keys.contains(&rest) {
                    rest = add_days(rest, 1);
                }
            }
            if rest >= from && rest < to {
                keys.insert(rest);
            }
        }
    }
    keys
}

fn count_span(from: NaiveDate, to: NaiveDate) -> Span {
    if from >= to {
        return Span { work: 0, rest: 0 };
    }
    let extra = holiday_weekdays(from, to);
    let mut weekends = 0i64;
    let mut holiday = 0i64;
    let mut work = 0i64;
    let mut cur = from;
    while cur < to {
        if is_weekend(cur) {
            weekends += 1;
        } else if extra.contains(&cur) {
            holiday += 1;
        } else {
            work += 1;
        }
        cur = add_days(cur, 1);
    }
    Span {
        work,
        rest: weekends + holiday,
    }
}

fn age_on(birth: NaiveDate, on: NaiveDate) -> i32 {
    let mut age = on.year() - birth.year();
    if on.month() < birth.month() || (on.month() == birth.month() && on.day() < birth.day()) {
        age -= 1;
    }
    age
}

fn duration_ym(from: NaiveDate, to: NaiveDate) -> String {
    let mut y = to.year() - from.year();
    let mut m = to.month() as i32 - from.month() as i32;
    if to.day() < from.day() {
        m -= 1;
    }
    if m < 0 {
        m += 12;
        y -= 1;
    }
    if y > 0 {
        format!("{y}年{m}月")
    } else {
        format!("{m}月")
    }
}

fn birthday_on(birth: NaiveDate, year: i32) -> NaiveDate {
    let last = days_in_month(year, birth.month());
    ymd(year, birth.month(), birth.day().min(last))
}

fn next_birthday_days(birth: NaiveDate, today: NaiveDate) -> i64 {
    let mut next = birthday_on(birth, today.year());
    if next <= today {
        next = birthday_on(birth, today.year() + 1);
    }
    diff_days(today, next)
}

fn duration_text(from: NaiveDate, to: NaiveDate) -> String {
    let mut y = to.year() - from.year();
    let mut m = to.month() as i32 - from.month() as i32;
    let mut d = to.day() as i32 - from.day() as i32;
    if d < 0 {
        let prev = add_months(ymd(to.year(), to.month(), 1), -1);
        d += days_in_month(prev.year(), prev.month()) as i32;
        m -= 1;
    }
    if m < 0 {
        m += 12;
        y -= 1;
    }
    let mut parts = Vec::new();
    if y > 0 {
        parts.push(format!("{y}年"));
    }
    if m > 0 {
        parts.push(format!("{m}个月"));
    }
    if d > 0 || parts.is_empty() {
        parts.push(format!("{d}天"));
    }
    parts.join("")
}

fn format_days(n: i64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn years_hint(days: i64) -> String {
    if days <= 0 {
        String::new()
    } else if days < 30 {
        format!("{days} 天")
    } else if days < 365 {
        format!("约 {} 个月", ((days as f64) / 30.4).round() as i64)
    } else {
        format!("约 {:.1} 年", days as f64 / 365.25)
    }
}

pub fn trip_countdown(start: NaiveDate, end: NaiveDate, now: NaiveDate) -> String {
    if end < now {
        return "已结束".into();
    }
    if start > now {
        let span = count_span(now, start);
        if span.work > 0 {
            format!("还有 {} 天出发 · 含 {} 个工作日", span.work + span.rest, span.work)
        } else {
            format!("还有 {} 天出发", span.work + span.rest)
        }
    } else {
        let left = diff_days(now, end) + 1;
        if left <= 1 {
            "今天是最后一天".into()
        } else {
            format!("旅途中 · 还剩 {left} 天")
        }
    }
}

pub fn build_work_life(
    birthday: Option<NaiveDate>,
    gender: i16,
    female_role: i16,
    work_start_year: Option<i32>,
    work_start_month: Option<i16>,
) -> Option<WorkLifeVo> {
    let now = now();
    let today = now.date();
    let birth = birthday?;
    if gender != 1 && gender != 2 {
        return None;
    }
    let age = age_on(birth, today);
    let gender_text = if gender == 1 { "男" } else { "女" }.to_string();
    let holiday = next_holiday(now);
    let retire = retirement_of(birth, gender, female_role);
    if today >= retire.date {
        return Some(WorkLifeVo {
            ready: true,
            retired: true,
            age,
            gender_text,
            holiday,
            extras: vec![],
            retire_age_text: retire.age_text,
            retire_date_text: retire.date_text,
            rest_text: "0 天".into(),
            work_text: "0 天".into(),
            rest_hint: "已退休".into(),
            work_hint: "已退休".into(),
        });
    }
    let work_year = work_start_year.unwrap_or(birth.year() + 22);
    let work_month = work_start_month
        .filter(|m| (1..=12).contains(m))
        .unwrap_or(1) as u32;
    let work_from = ymd(work_year, work_month, 1);
    let all = count_span(today, retire.date);
    let same_year = today.year() == retire.date.year();
    let mut extras = Vec::new();
    if let Some((a, b)) = clamp_range(today, retire.date, ymd(today.year(), 1, 1), ymd(today.year() + 1, 1, 1))
    {
        let span = count_span(a, b);
        extras.push(ExtraRow {
            k: if same_year {
                "今年 · 退休年".into()
            } else {
                "今年剩下".into()
            },
            v: format!(
                "上班 {} 天 · 放假 {} 天",
                format_days(span.work),
                format_days(span.rest)
            ),
        });
    }
    if !same_year {
        if let Some((a, b)) =
            clamp_range(today, retire.date, ymd(retire.date.year(), 1, 1), ymd(retire.date.year() + 1, 1, 1))
        {
            let span = count_span(a, b);
            extras.push(ExtraRow {
                k: format!("{} 年退休前", retire.date.year()),
                v: format!(
                    "上班 {} 天 · 放假 {} 天",
                    format_days(span.work),
                    format_days(span.rest)
                ),
            });
        }
    }
    extras.push(ExtraRow {
        k: "距离退休".into(),
        v: duration_text(today, retire.date),
    });
    let bday = next_birthday_days(birth, today);
    extras.push(ExtraRow {
        k: "下个生日".into(),
        v: if bday > 0 {
            format!("还有 {bday} 天")
        } else {
            "就是今天".into()
        },
    });
    extras.push(break_countdown(now));
    if today > work_from {
        extras.push(ExtraRow {
            k: "已经工作".into(),
            v: duration_ym(work_from, today),
        });
    }
    Some(WorkLifeVo {
        ready: true,
        retired: false,
        age,
        gender_text,
        holiday,
        extras,
        retire_age_text: retire.age_text,
        retire_date_text: retire.date_text,
        rest_text: format!("{} 天", format_days(all.rest)),
        work_text: format!("{} 天", format_days(all.work)),
        rest_hint: years_hint(all.rest),
        work_hint: years_hint(all.work),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lunar_2025() {
        assert_eq!(lunar_to_solar(2025, 1, 1), ymd(2025, 1, 29));
        assert_eq!(lunar_to_solar(2025, 5, 5), ymd(2025, 5, 31));
        assert_eq!(lunar_to_solar(2025, 8, 15), ymd(2025, 10, 6));
    }

    #[test]
    fn hours_after_midnight_until_off() {
        let now = at_hour(ymd(2026, 8, 21), 0) + Duration::seconds(1);
        assert_eq!(hours_left(now, at_hour(ymd(2026, 8, 21), 18)), 18);
    }

    #[test]
    fn worked_as_year_month() {
        assert_eq!(duration_ym(ymd(2016, 1, 1), ymd(2026, 8, 21)), "10年7月");
    }
}
