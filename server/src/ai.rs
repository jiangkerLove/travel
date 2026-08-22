use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    error::AppError,
    poi::{geocode_address, looks_like_admin_place, pick_best_poi, search_places, PoiVo},
    util::{parse_date, valid_point_type},
};

const DEEPSEEK_URL: &str = "https://api.deepseek.com/chat/completions";
/// DeepSeek-V4-Flash-0731。旧名 deepseek-chat 已下线。
const DEEPSEEK_MODEL: &str = "deepseek-v4-flash";

#[derive(Serialize, Deserialize, Clone)]
pub struct AiPoint {
    pub place_name: String,
    pub query: String,
    pub point_type: String,
    pub stay_minutes: Option<i32>,
    pub arrive: Option<String>,
    pub note: Option<String>,
    pub longitude: Option<f64>,
    pub latitude: Option<f64>,
    pub found: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AiDay {
    pub day_num: i32,
    pub theme: Option<String>,
    pub points: Vec<AiPoint>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AiDraft {
    pub summary: String,
    pub days: Vec<AiDay>,
}

#[derive(Deserialize)]
struct ChatResp {
    choices: Option<Vec<ChatChoice>>,
    error: Option<ChatErr>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: Option<ChatMsg>,
}

#[derive(Deserialize)]
struct ChatMsg {
    content: Option<String>,
}

#[derive(Deserialize)]
struct ChatErr {
    message: Option<String>,
}

#[derive(Deserialize)]
struct ModelOut {
    summary: Option<String>,
    days: Option<Vec<ModelDay>>,
}

#[derive(Deserialize)]
struct ModelDay {
    day_num: Option<i32>,
    theme: Option<String>,
    points: Option<Vec<ModelPoint>>,
}

#[derive(Deserialize)]
struct ModelPoint {
    place_name: Option<String>,
    query: Option<String>,
    point_type: Option<String>,
    stay_minutes: Option<i32>,
    arrive: Option<String>,
    note: Option<String>,
}

fn map_point_type(raw: &str) -> String {
    let s = raw.trim().to_ascii_lowercase();
    let t = if s.contains("hotel") || s.contains("stay") || raw.contains("住") || raw.contains("酒店")
    {
        "hotel"
    } else if s.contains("food") || s.contains("eat") || raw.contains("餐") || raw.contains("吃") {
        "food"
    } else if s.contains("gas") || raw.contains("加油") {
        "gas"
    } else if s.contains("transport") || s.contains("station") || raw.contains("车站") || raw.contains("机场")
    {
        "transport"
    } else {
        "sight"
    };
    if valid_point_type(t) {
        t.into()
    } else {
        "sight".into()
    }
}

fn looks_like_lodging(name: &str) -> bool {
    ["酒店", "宾馆", "民宿", "客栈", "旅馆", "旅社", "青旅", "度假村"]
        .iter()
        .any(|k| name.contains(k))
}

fn looks_like_settlement(name: &str) -> bool {
    name.contains("镇")
        || name.contains("乡")
        || name.contains("村")
        || name.contains("县城")
        || name.contains("街道")
        || name.ends_with("县")
        || name.ends_with("市")
}

fn looks_like_transport(name: &str) -> bool {
    name.contains("机场") || name.contains("火车站") || name.contains("高铁站") || name.contains("汽车站")
}

fn strip_lodging_brand(name: &str) -> Option<String> {
    const MARKS: &[&str] = &[
        "精品民宿",
        "特色民宿",
        "民宿",
        "精品酒店",
        "大酒店",
        "酒店",
        "宾馆",
        "客栈",
        "旅馆",
        "旅社",
        "青旅",
        "度假村",
    ];
    for mark in MARKS {
        if let Some(i) = name.find(mark) {
            let prefix = name[..i]
                .trim_end_matches(['的', '·', '-', '—', ' '])
                .trim();
            if looks_like_settlement(prefix) {
                return Some(prefix.to_string());
            }
        }
    }
    None
}

/// 每天收尾落到能过夜的镇村，不推荐具体住宿，也不用景点收尾。
fn finish_day_end(p: &mut AiPoint, is_trip_last_day: bool) {
    if looks_like_lodging(&p.place_name) {
        if let Some(town) = strip_lodging_brand(&p.place_name) {
            p.place_name = town;
            if looks_like_lodging(&p.query) {
                p.query.clear();
            }
        }
    }
    if p.note.as_ref().is_some_and(|n| looks_like_lodging(n)) {
        p.note = None;
    }
    if looks_like_transport(&p.place_name) && is_trip_last_day {
        p.point_type = "transport".into();
        return;
    }
    if looks_like_settlement(&p.place_name) || looks_like_lodging(&p.place_name) {
        p.point_type = "hotel".into();
    }
}

const SYSTEM_ROLE: &str = "你是旅游环线规划引擎，专精中国自驾/自由行。只输出 JSON，不要 markdown，不要正文解释。";

const USER_FIRST: &str = "\
【0. 最高优先级】用户输入中的约束高于一切默认规则。\
「不去/排除/别去」的景点：不得出现，也不得作为必经路段。\
「顺时针/逆时针」：全程锁定该方向，不得中途反向。\
「必去」：必须安排；「可选」：有余力再排。用户未指定方向时，选地理上闭合、少折返的环线即可。";

const TRIP_FACTS_RULE: &str = "\
【行程事实】默认按系统已创建的旅途日期与天数规划（起止日期、总天数、D1/D2 对应日历均由系统自动带入），不得自行改天数或日期。\
仅当用户明确要求调整出行时间、延长/缩短行程、改第几天安排时，才可偏离系统日期。\
用户在需求里随口写的日期/天数若与系统不一致，一律忽略。\
用户需求默认只写：景点、必去/不去、顺逆时针、车程节奏、游玩风格。";

const LOOP_DIRECTION_RULE: &str = "\
【1. 环线方向锁定】\
先确定主线方向（顺时针或逆时针，以用户为准），全程保持一致。\
每日 theme 开头标注方向，格式如「逆时针·D2」或「顺时针·D3」。\
禁止放射式绕行：反例「宜宾→乐山→自贡→眉山→又回到宜宾另一侧景点」；\
正例：沿环边界单向前进，每天住宿点沿主线推进，最终闭合回出发方向。";

const LODGING_RULE: &str = "\
【2. 住宿向前推进】\
每天最后一个点（point_type=hotel）必须是具体镇/县/片区地名，如「乐山市区」「九寨沟沟口」「日隆镇」。\
禁止「途中休息」「路上」「附近」等模糊词。\
住宿须沿主线方向前移；禁止连续两天住同一地却往相反方向跑远（基地模式）。\
例外：同一城市连住多日时，每天游览不同片区，且不重复昨日已走的主路段。";

const DETOUR_RULE: &str = "\
【3. 折返管控】\
允许当日短线支线折返（如进峡谷景区后原路返回主线），但须当日完成、不跨日。\
禁止连续两天及以上重复同一段折返路。\
避免大面积来回折返；整体折返路段宜少。默认每日纯驾车约不超过4小时（用户另有说明从其要求）。";

const EXCLUDE_RULE: &str = "\
【4. 排除过滤】\
用户标注不去的景点，任何情况下不得出现在 place_name、query、note 中，也不得安排为途经点。";

const OUTPUT_FORMAT: &str = "\
【5. 输出字段】\
place_name：具体景区/地标/镇村，不写单独地级市名。\
query：「城市 具体地点」，辅助高德定位。\
point_type：sight/hotel/food/gas/transport。\
每天 3-6 点（含过夜点）。arrive 用 HH:MM，随游览顺序递增。\
note：一句实用提醒（门票/预约/路况）。\
theme：当天主题，须含环线方向标注。\
summary：一两句话概括全程；末尾附路线健康度自检，格式：「自检：方向✓/排除✓/住宿具体✓/无连续重复路段✓」（某项有问题则写✗并简述）。\
不要输出 longitude/latitude。";

const DEDUP_RULE: &str = "\
【6. 去重】同一地点当天只出现一次；禁止相邻两个相同地名。古镇与同名镇算一地。";

const SELF_CHECK_RULE: &str = "\
【7. 生成后自检（写入 summary）】逐项核对：①方向是否全程一致；②是否含排除景点；③每晚住宿是否具体地名；④是否有连续两天重复路段；⑤是否呈放射式绕行。有问题在 summary 自检中标注✗。";

fn system_prompt() -> String {
    [
        SYSTEM_ROLE,
        USER_FIRST,
        TRIP_FACTS_RULE,
        LOOP_DIRECTION_RULE,
        LODGING_RULE,
        DETOUR_RULE,
        EXCLUDE_RULE,
        OUTPUT_FORMAT,
        DEDUP_RULE,
        SELF_CHECK_RULE,
    ]
    .concat()
}

/// 从用户文字里提取关键约束，再强调一遍给模型
fn constraint_reminder(prompt: &str) -> String {
    let p = prompt.trim();
    if p.is_empty() {
        return String::new();
    }
    let mut lines = Vec::new();
    if ["不去", "不要", "别去", "排除", "不想去", "不去的"]
        .iter()
        .any(|k| p.contains(k))
    {
        lines.push("→ 排除过滤：用户写明不去的景点一律不得出现或途经。");
    }
    if p.contains("逆时针") {
        lines.push("→ 环线方向锁定：全程逆时针，禁止按顺时针排。");
    } else if p.contains("顺时针") {
        lines.push("→ 环线方向锁定：全程顺时针，禁止按逆时针排。");
    }
    if p.contains("必去") || p.contains("一定要去") {
        lines.push("→ 标注「必去」的景点必须安排进路线。");
    }
    if p.contains("小时") || p.contains("车程") {
        lines.push("→ 遵守用户写的每日车程上限。");
    } else {
        lines.push("→ 默认每日纯驾车约不超过4小时。");
    }
    if ["放射", "折返", "重复路"]
        .iter()
        .any(|k| p.contains(k))
    {
        lines.push("→ 按用户纠偏要求调整：消除放射式绕行或连续重复路段。");
    }
    format!("\n【约束复核（必须遵守）】\n{}", lines.join("\n"))
}

/// 根据旅途开始日期列出 D1、D2… 对应公历
fn trip_day_dates(start: &str, days: i32) -> String {
    let Ok(base) = parse_date(start) else {
        return String::new();
    };
    (0..days)
        .map(|i| {
            let d = base + chrono::Duration::days(i as i64);
            format!("D{}={}", i + 1, d)
        })
        .collect::<Vec<_>>()
        .join("，")
}

/// 地名完全相同，或高德查到的坐标几乎重合
fn same_geocoded_stop(a: &AiPoint, b: &AiPoint) -> bool {
    if a.place_name.trim() == b.place_name.trim() {
        return true;
    }
    match (a.latitude, a.longitude, b.latitude, b.longitude) {
        (Some(lat1), Some(lng1), Some(lat2), Some(lng2)) => {
            let key = |lat: f64, lng: f64| ((lat * 1e5).round() as i64, (lng * 1e5).round() as i64);
            key(lat1, lng1) == key(lat2, lng2)
        }
        _ => false,
    }
}

/// 合并备注，去重相同内容
fn merge_notes(a: Option<&str>, b: Option<&str>) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for n in [a, b] {
        if let Some(s) = n.map(str::trim).filter(|s| !s.is_empty()) {
            if !parts.iter().any(|p| *p == s) {
                parts.push(s);
            }
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("；"))
    }
}

/// 高德查点之后，去掉紧挨着的完全重复节点
fn dedupe_adjacent_after_geocode(points: &mut Vec<AiPoint>) {
    let mut i = 0;
    while i + 1 < points.len() {
        if !same_geocoded_stop(&points[i], &points[i + 1]) {
            i += 1;
            continue;
        }
        let remove = if points[i + 1].point_type == "hotel" {
            i
        } else if points[i].point_type == "hotel" {
            i + 1
        } else {
            i
        };
        let keep = if remove == i { i + 1 } else { i };
        let keep_note = points[keep].note.clone();
        let drop_note = points[remove].note.clone();
        points[keep].note = merge_notes(keep_note.as_deref(), drop_note.as_deref());
        points.remove(remove);
        i = i.saturating_sub(1);
    }
}

fn extract_urls(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search_from = 0;
    while search_from < text.len() {
        let slice = &text[search_from..];
        let rel = ["http://", "https://"]
            .iter()
            .filter_map(|p| slice.find(p))
            .min();
        let Some(rel) = rel else {
            break;
        };
        let start = search_from + rel;
        let rest = &text[start..];
        let end_rel = rest
            .char_indices()
            .find(|(_, c)| c.is_whitespace() || "，。,!！?？;；\"'<>[]（）".contains(*c))
            .map(|(i, _)| i)
            .unwrap_or(rest.len());
        let url = rest[..end_rel].trim_end_matches(|c: char| matches!(c, '.' | ',' | '。'));
        if (url.starts_with("http://") || url.starts_with("https://"))
            && !out.iter().any(|x| x == url)
        {
            out.push(url.to_string());
        }
        if out.len() >= 2 {
            break;
        }
        search_from = start + end_rel.max(1);
    }
    out
}

fn strip_html(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    let mut in_tag = false;
    while let Some(c) = chars.next() {
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c == '>' {
            in_tag = false;
            out.push(' ');
            continue;
        }
        if !in_tag {
            out.push(c);
        }
    }
    out.split_whitespace().take(900).collect::<Vec<_>>().join(" ")
}

async fn fetch_link_text(url: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::limited(4))
        .build()
        .ok()?;
    let resp = client
        .get(url)
        .header(
            USER_AGENT,
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15",
        )
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let body = resp.text().await.ok()?;
    let text = strip_html(&body);
    if text.chars().count() < 20 {
        None
    } else {
        Some(text.chars().take(2400).collect())
    }
}

fn format_link_note(url: &str, text: Option<String>) -> String {
    match text {
        Some(text) => format!("\n链接 {url} 抓到的正文摘录：\n{text}\n"),
        None => format!(
            "\n用户分享了链接 {url}，页面未能抓取（小红书等常需登录）。请结合常见攻略与用户文字安排。\n"
        ),
    }
}

async fn collect_link_notes(prompt: &str) -> String {
    let urls = extract_urls(prompt);
    match urls.as_slice() {
        [] => String::new(),
        [u1] => format_link_note(u1, fetch_link_text(u1).await),
        [u1, u2, ..] => {
            let (t1, t2) = tokio::join!(fetch_link_text(u1), fetch_link_text(u2));
            format!("{}{}", format_link_note(u1, t1), format_link_note(u2, t2))
        }
    }
}

fn parse_model_json(raw: &str) -> Result<ModelOut, AppError> {
    let trimmed = raw.trim();
    let json_str = if let Some(start) = trimmed.find('{') {
        let end = trimmed.rfind('}').ok_or_else(|| AppError::Internal("模型返回无法解析".into()))?;
        &trimmed[start..=end]
    } else {
        return Err(AppError::Internal("模型没有返回行程".into()));
    };
    serde_json::from_str(json_str).map_err(|_| AppError::Internal("模型返回格式不对".into()))
}

async fn chat_json(api_key: &str, user_content: String) -> Result<ModelOut, AppError> {
    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| AppError::Internal("DeepSeek Key 不合法".into()))?,
    );
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    let body = json!({
        "model": DEEPSEEK_MODEL,
        "temperature": 0.2,
        "thinking": { "type": "disabled" },
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": system_prompt()
            },
            { "role": "user", "content": user_content }
        ]
    });
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(40))
        .build()
        .map_err(|e| AppError::Internal(format!("无法请求模型: {e}")))?;
    let resp = client
        .post(DEEPSEEK_URL)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("DeepSeek 请求失败: {e}")))?;
    let status = resp.status();
    let v: Value = resp
        .json()
        .await
        .map_err(|_| AppError::Internal("DeepSeek 响应不是 JSON".into()))?;
    let parsed: ChatResp = serde_json::from_value(v).unwrap_or(ChatResp {
        choices: None,
        error: None,
    });
    if !status.is_success() {
        let msg = parsed
            .error
            .and_then(|e| e.message)
            .unwrap_or_else(|| format!("DeepSeek 错误 {status}"));
        return Err(AppError::BadRequest(msg));
    }
    let content = parsed
        .choices
        .and_then(|c| c.into_iter().next())
        .and_then(|c| c.message)
        .and_then(|m| m.content)
        .ok_or_else(|| AppError::Internal("DeepSeek 没有返回内容".into()))?;
    parse_model_json(&content)
}

async fn geocode_point(
    key: &str,
    secret: &str,
    city: &str,
    p: &mut AiPoint,
    cache: &mut std::collections::HashMap<String, (f64, f64)>,
) {
    if key.is_empty() {
        return;
    }
    let name_key = p.place_name.trim().to_string();
    if let Some((lng, lat)) = cache.get(&name_key) {
        p.longitude = Some(*lng);
        p.latitude = Some(*lat);
        p.found = true;
        return;
    }
    let q = if p.query.trim().is_empty() {
        p.place_name.clone()
    } else {
        p.query.trim().to_string()
    };
    if try_geocode(key, secret, city, p, &q).await {
        if let (Some(lng), Some(lat)) = (p.longitude, p.latitude) {
            cache.insert(name_key, (lng, lat));
        }
        return;
    }
    let name = p.place_name.clone();
    if q != name && try_geocode(key, secret, city, p, &name).await {
        if let (Some(lng), Some(lat)) = (p.longitude, p.latitude) {
            cache.insert(name_key, (lng, lat));
        }
    }
}

async fn try_geocode(key: &str, secret: &str, city: &str, p: &mut AiPoint, q: &str) -> bool {
    let city_opt = city.trim();
    let city_opt = if city_opt.is_empty() { None } else { Some(city_opt) };
    if looks_like_admin_place(&p.place_name) || looks_like_admin_place(q) {
        if let Some(hit) = geocode_address(key, secret, q, city_opt).await {
            apply_poi(p, &hit);
            return true;
        }
    }
    let Ok(list) = search_places(key, secret, q, None, None, city_opt).await else {
        return false;
    };
    let Some(hit) = pick_best_poi(&list, &p.place_name, q) else {
        return false;
    };
    apply_poi(p, hit);
    true
}

fn apply_poi(p: &mut AiPoint, hit: &PoiVo) {
    p.longitude = Some(hit.longitude);
    p.latitude = Some(hit.latitude);
    if p.place_name.chars().count() < 2 {
        p.place_name = hit.name.clone();
    }
    p.found = true;
}

pub async fn draft_itinerary(
    api_key: &str,
    amap_key: &str,
    amap_secret: &str,
    destination: &str,
    start: &str,
    end: &str,
    days: i32,
    existing: &str,
    prompt: &str,
    focus_day: Option<i32>,
    recommend: bool,
    fresh: bool,
) -> Result<AiDraft, AppError> {
    if api_key.is_empty() {
        return Err(AppError::BadRequest("未配置 DEEPSEEK_API_KEY".into()));
    }
    let prompt = prompt.trim();
    if !recommend && prompt.chars().count() < 2 {
        return Err(AppError::BadRequest("请写要去哪里，或粘贴攻略链接".into()));
    }
    if prompt.chars().count() > 2000 {
        return Err(AppError::BadRequest("描述太长，精简一下再试".into()));
    }
    let links = collect_link_notes(prompt).await;
    let constraint = constraint_reminder(prompt);
    let prefer = if prompt.is_empty() {
        "按系统已定的出行日期和天数安排，经典环线，节奏适中，每日驾车约4小时内。".into()
    } else {
        prompt.to_string()
    };
    let (existing_line, scope) = if fresh {
        (
            "无。".into(),
            match focus_day {
                Some(d) => format!(
                    "任务：从零规划第 {d} 天环线片段，融入全程 {days} 天布局，遵守环线方向与住宿推进规则。只输出 day_num={d}。"
                ),
                None => format!(
                    "任务：从零规划 {days} 天完整环线。先定顺/逆时针方向，再按日推进住宿与景点，避免放射式绕行。day_num 从 1 到 {days}。"
                ),
            },
        )
    } else if recommend {
        (
            existing.to_string(),
            match focus_day {
                Some(d) => format!(
                    "任务：沿途推荐，只改第 {d} 天。保留已有地点和顺序，沿当前环线方向顺路插入 1-2 个新景区（过夜点之前）。不插排除景点。只输出 day_num={d}。"
                ),
                None => format!(
                    "任务：沿途推荐。保留每天已有地点和顺序，沿环线方向顺路插入新景区（每天最多 1-2 个，过夜点之前）。不插排除景点。day_num 从 1 到 {days}。"
                ),
            },
        )
    } else {
        (
            existing.to_string(),
            match focus_day {
                Some(d) => format!(
                    "任务：纠偏/改第 {d} 天。用户要求的排除、方向、增删必须执行；若用户指出放射式绕行或重复路段，须重排为环线推进。仅未提及的尽量保留。只输出 day_num={d}。"
                ),
                None => format!(
                    "任务：纠偏/改全部 {days} 天。用户要求的排除、方向、增删必须执行；若用户指出放射式绕行或重复路段，须重排为环线推进。仅未提及的尽量保留。day_num 从 1 到 {days}。"
                ),
            },
        )
    };
    let day_dates = trip_day_dates(start, days);
    let day_dates_line = if day_dates.is_empty() {
        String::new()
    } else {
        format!("\n- 每日日期：{day_dates}")
    };
    let user_content = format!(
        "\
## 行程参数（系统已定，以此为准）
- 目的地/区域：{destination}
- 出行日期：{start} 至 {end}（共 {days} 天）{day_dates_line}
- 现有行程：{existing_line}
- {scope}

## 用户需求（只写景点、方向、排除、节奏等，不要写日期和天数）
{prefer}{constraint}{links}

## 规划要求
1. 天数与日期以「行程参数」为准，勿采用用户口述的日期/天数
2. 先确定环线方向（以用户指定为准），全程锁定
3. 住宿点沿主线向前，用具体地名，禁止模糊词
4. 排除景点不得出现
5. 避免放射式绕行与连续重复路段
6. 生成后在 summary 附路线健康度自检

输出 JSON：{{\"summary\":\"含自检\",\"days\":[{{\"day_num\":1,\"theme\":\"逆时针·D1\",\"points\":[{{\"place_name\":\"\",\"query\":\"城市 地点\",\"point_type\":\"sight\",\"stay_minutes\":90,\"arrive\":\"09:00\",\"note\":\"\"}}]}}]}}"
    );
    let model = chat_json(api_key, user_content).await?;
    let mut draft = AiDraft {
        summary: model.summary.unwrap_or_else(|| "已排好一版行程".into()),
        days: vec![],
    };
    for d in model.days.unwrap_or_default() {
        let day_num = d.day_num.unwrap_or(0);
        if day_num < 1 || day_num > days {
            continue;
        }
        let mut points = Vec::new();
        for p in d.points.unwrap_or_default() {
            let name = p.place_name.unwrap_or_default().trim().to_string();
            if name.is_empty() {
                continue;
            }
            points.push(AiPoint {
                place_name: name.clone(),
                query: p.query.unwrap_or_default(),
                point_type: map_point_type(&p.point_type.unwrap_or_default()),
                stay_minutes: p.stay_minutes.filter(|n| *n > 0 && *n < 24 * 60),
                arrive: p.arrive.filter(|s| s.len() >= 4 && s.len() <= 8),
                note: p.note.filter(|s| !s.trim().is_empty()),
                longitude: None,
                latitude: None,
                found: false,
            });
            if points.len() >= if recommend { 8 } else { 6 } {
                break;
            }
        }
        if !recommend {
            if let Some(last) = points.last_mut() {
                finish_day_end(last, day_num == days);
            }
        }
        if !points.is_empty() {
            draft.days.push(AiDay {
                day_num,
                theme: d.theme.filter(|s| !s.trim().is_empty()),
                points,
            });
        }
    }
    if let Some(d) = focus_day {
        if draft.days.len() == 1 {
            draft.days[0].day_num = d;
        } else {
            draft.days.retain(|x| x.day_num == d);
        }
    }
    draft.days.sort_by_key(|d| d.day_num);
    if draft.days.is_empty() {
        return Err(AppError::BadRequest("没能排出地点，换种说法再试".into()));
    }
    let mut geo_cache: std::collections::HashMap<String, (f64, f64)> = std::collections::HashMap::new();
    for day in &mut draft.days {
        for p in &mut day.points {
            geocode_point(amap_key, amap_secret, destination, p, &mut geo_cache).await;
        }
        dedupe_adjacent_after_geocode(&mut day.points);
    }
    Ok(draft)
}
