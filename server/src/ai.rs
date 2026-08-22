use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    error::AppError,
    poi::{search_places, PoiVo},
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
                "content": "你是旅行行程规划助手。只输出 JSON，不要解释。按用户天数把行程排到每天，地点用中国大陆可搜索的真实地名。避开用户明确不想去的地方。每天 2-5 个点，不要安排过多。point_type 只能是 sight/hotel/food/gas/transport。query 写成便于高德搜索的词，带上城市。"
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

async fn geocode_point(key: &str, secret: &str, city: &str, p: &mut AiPoint) {
    if key.is_empty() {
        return;
    }
    let q = if p.query.trim().is_empty() {
        format!("{} {}", p.place_name, city)
    } else {
        p.query.clone()
    };
    if let Ok(list) = search_places(key, secret, &q, None, None).await {
        if let Some(hit) = pick_poi(&list, &p.place_name) {
            p.longitude = Some(hit.longitude);
            p.latitude = Some(hit.latitude);
            if p.place_name.chars().count() < 2 {
                p.place_name = hit.name.clone();
            }
            p.found = true;
            return;
        }
    }
    if q != p.place_name {
        if let Ok(list) = search_places(key, secret, &p.place_name, None, None).await {
            if let Some(hit) = pick_poi(&list, &p.place_name) {
                p.longitude = Some(hit.longitude);
                p.latitude = Some(hit.latitude);
                p.found = true;
            }
        }
    }
}

fn pick_poi<'a>(list: &'a [PoiVo], name: &str) -> Option<&'a PoiVo> {
    if list.is_empty() {
        return None;
    }
    list.iter()
        .find(|p| p.name.contains(name) || name.contains(&p.name))
        .or_else(|| list.first())
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
) -> Result<AiDraft, AppError> {
    if api_key.is_empty() {
        return Err(AppError::BadRequest("未配置 DEEPSEEK_API_KEY".into()));
    }
    let prompt = prompt.trim();
    if prompt.chars().count() < 2 {
        return Err(AppError::BadRequest("请写要去哪里，或粘贴攻略链接".into()));
    }
    if prompt.chars().count() > 2000 {
        return Err(AppError::BadRequest("描述太长，精简一下再试".into()));
    }
    let links = collect_link_notes(prompt).await;
    let scope = match focus_day {
        Some(d) => format!(
            "只改第 {d} 天，days 里只输出 day_num={d} 的一天。在该天现有点上按用户要求增加、删掉或微调；用户没说去掉的地点尽量保留。"
        ),
        None => format!("可重排全部 {days} 天。day_num 必须在 1 到 {days} 之间。"),
    };
    let user_content = format!(
        "目的地：{destination}\n日期：{start} 至 {end}，共 {days} 天\n现有行程：{existing}\n{scope}\n用户要求：{prompt}\n{links}\n请输出 JSON：{{\"summary\":\"一句话\",\"days\":[{{\"day_num\":1,\"theme\":\"\",\"points\":[{{\"place_name\":\"\",\"query\":\"城市 地点\",\"point_type\":\"sight\",\"stay_minutes\":90,\"arrive\":\"10:00\",\"note\":\"\"}}]}}]}}"
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
            if points.len() >= 6 {
                break;
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
    let mut geocoded = 0;
    for day in &mut draft.days {
        for p in &mut day.points {
            if geocoded >= 16 {
                break;
            }
            geocode_point(amap_key, amap_secret, destination, p).await;
            geocoded += 1;
        }
    }
    Ok(draft)
}
