use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    error::AppError,
    poi::{geocode_address, looks_like_admin_place, pick_best_poi, search_places, PoiVo},
    util::valid_point_type,
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

const SYSTEM_ROLE: &str = "你是资深自驾游与自由行行程规划师，熟悉中国路况与景区分布。只输出 JSON，不要 markdown，不要解释。";

const OUTPUT_FORMAT: &str = "\
【输出格式】place_name 必须是具体景区、地标、镇村名，不要只写地级市/省会名（错误：松原市当景点；正确：查干湖、乾安泥林；过夜写前郭县、日隆镇等）。query 写成「城市 具体地点」（如「松原 查干湖」），不要只写城市名，避免定位偏到机场。不要输出 longitude/latitude，坐标由系统查询。point_type 仅 sight/hotel/food/gas/transport；每天 3-6 个点（含过夜点）；arrive 用 HH:MM，随路程先后递增；stay_minutes 合理估算游览时长；note 只写一句必要提醒；summary 用一两句话说明整趟或当天怎么走；theme 写当天一句话主题。";

const PLANNING_METHOD: &str = "\
【规划步骤】先按天划定活动范围并确定当晚住宿镇/县 → 再选当天顺路可达的景区 → 按动线从早到晚排点 → 自检是否折返或重复地名 → 最后填 arrive 与 stay_minutes。全程住宿地尽量少换，整体路线向前推进。";

const OVERNIGHT_RULE: &str = "\
【过夜】每天最后一个点必须是能过夜的镇、乡、村或县城，place_name 只写地名（如日隆镇、塔河县），point_type=hotel。不写具体酒店/民宿名。不用景区、山顶、观景台收尾。最后一天若返程，末点可为机场或车站（point_type=transport）。";

const PLACE_DEDUP_RULE: &str = "\
【去重】每个地点在当天只出现一次。相邻两点不能是同一地方，禁止连续两个完全相同地名（如两个「塔河县」）。古镇/镇/古城算同一地，不要「日隆古镇」后又排「日隆镇」。若当天在某镇活动，该镇只作最后过夜点，前面不把该镇当景点重复列出。";

const ROUTE_ORDER_RULE: &str = "\
【顺路】每天的点按实际驾车动线从早到晚串联，一路走向当晚住宿地，避免折返和走回头路。排点前在脑中画路线：每去下一个点应比折回去更近住宿地方向，不要出现「东→西→再东」「A→B→又回到 A 附近」的走法。路上风景、观景台也可作游览点，不必套用固定时段模板，早中晚按路程与景点特点合理安排即可。summary 可简要说明当天动线方向（如「一路向北」「沿国道 318 西行」）。";

const CROSS_DAY_RULE: &str = "\
【跨天】今天在哪过夜，明天默认从那里出发；下一天第一个点应是新的游览目的地，不要重复昨夜住宿地名。相邻两天的活动区域宜向前推进，不要把后一天的点安排在需要折返回昨天路过区域的地方。";

fn system_prompt() -> String {
    [
        SYSTEM_ROLE,
        OUTPUT_FORMAT,
        PLANNING_METHOD,
        OVERNIGHT_RULE,
        PLACE_DEDUP_RULE,
        ROUTE_ORDER_RULE,
        CROSS_DAY_RULE,
    ]
    .concat()
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
        "temperature": 0.3,
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
    let prefer = if prompt.is_empty() {
        "无特别偏好：选经典顺路景点，节奏适中，每天车程不宜过长。".into()
    } else {
        prompt.to_string()
    };
    let (existing_line, scope) = if fresh {
        (
            "无（全新规划，不得沿用或模仿任何旧地点）。".into(),
            match focus_day {
                Some(d) => format!(
                    "【任务】从零规划第 {d} 天。结合目的地与总天数，安排顺路景点并以镇/县过夜收尾。只输出第 {d} 天。"
                ),
                None => format!(
                    "【任务】从零规划全部 {days} 天。day_num 从 1 到 {days}，每天顺路串联、以镇/县过夜，整体路线连贯向前。避开用户不想去的地方。"
                ),
            },
        )
    } else if recommend {
        (
            existing.to_string(),
            match focus_day {
                Some(d) => format!(
                    "【任务】沿途推荐，只改第 {d} 天。必须原样保留该天已有地点及先后顺序，不得删点、换序、改线。只在相邻两点之间的顺路方向上插入 1-2 个新景区，插在当天过夜点之前。新点须与已有点不同地、不同镇，不插同名或同镇点（如已有日隆镇则不加日隆古镇）。只输出第 {d} 天。"
                ),
                None => format!(
                    "【任务】沿途推荐。必须原样保留每天已有地点及先后顺序，不得删点、换序、重排。只在现有路线顺路方向每天插入 1-2 个新景区，插在过夜点之前。新点不得与已有点同名或同镇。day_num 从 1 到 {days}。"
                ),
            },
        )
    } else {
        (
            existing.to_string(),
            match focus_day {
                Some(d) => format!(
                    "【任务】在上一版基础上改第 {d} 天。按用户要求增删或调整；未提及的地点尽量保留。改完后当天仍须顺路、以镇/县过夜，并自检无重复地名与折返。只输出第 {d} 天。"
                ),
                None => format!(
                    "【任务】在上一版基础上调整全部 {days} 天。按用户要求修改；未提及的可保留。改完后每天须顺路、以镇/县过夜，并自检无重复地名与折返。day_num 从 1 到 {days}。"
                ),
            },
        )
    };
    let user_content = format!(
        "目的地：{destination}\n出行日期：{start} 至 {end}，共 {days} 天\n现有行程（按顺序）：{existing_line}\n{scope}\n用户要求：{prefer}\n{links}\
请严格遵守系统规则（过夜、去重、顺路、跨天）。输出 JSON：{{\"summary\":\"\",\"days\":[{{\"day_num\":1,\"theme\":\"\",\"points\":[{{\"place_name\":\"\",\"query\":\"城市 地点\",\"point_type\":\"sight\",\"stay_minutes\":90,\"arrive\":\"09:00\",\"note\":\"\"}}]}}]}}"
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
