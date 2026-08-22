use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::{
    error::AppError,
    poi::{geocode_address, pick_best_poi, search_places, PoiVo},
    util::{parse_date, valid_point_type},
};

const DEEPSEEK_URL: &str = "https://api.deepseek.com/chat/completions";
/// DeepSeek-V4-Flash-0731。旧名 deepseek-chat 已下线。
const DEEPSEEK_MODEL: &str = "deepseek-v4-flash";

#[derive(Serialize, Deserialize, Clone)]
pub struct AiPoint {
    pub place_name: String,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    pub point_type: String,
    pub stay_minutes: Option<i32>,
    pub arrive: Option<String>,
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_return: Option<bool>,
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
struct ModelSelfCheck {
    direction_consistent: Option<bool>,
    excluded_omitted: Option<bool>,
    hotel_specific: Option<bool>,
    no_repeated_segment: Option<bool>,
    no_radial_pattern: Option<bool>,
}

#[derive(Deserialize)]
struct ModelOut {
    summary: Option<String>,
    self_check: Option<ModelSelfCheck>,
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
    location: Option<String>,
    point_type: Option<String>,
    stay_minutes: Option<i32>,
    arrive: Option<String>,
    note: Option<String>,
    is_return: Option<bool>,
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

const SYSTEM_PROMPT: &str = r#"你是旅游环线规划引擎，专精中国自驾/自由行。只输出 JSON，不要 markdown，不要正文解释。

【0. 数据源优先级】
唯一数据源 = 用户本次输入。用户输入中的天数、日期、景点、方向为最终依据。
如果用户输入中缺少某字段（如无方向偏好），则由AI按地理合理性自动补全。
不存在"系统参数"这一外部数据源。

【1. 环线方向锁定】
先确定主线方向（顺时针或逆时针，以用户指定为准），全程保持一致。
每日 theme 开头标注方向，格式如「逆时针·D2」或「顺时针·D3」。
禁止放射式绕行：反例「宜宾→乐山→自贡→眉山→又回到宜宾另一侧景点」；
正例：沿环边界单向前进，每天住宿点沿主线推进，最终闭合回出发方向。

用户未指定方向时，按以下规则自动判定：
- 若起点经度 < 终点经度（起点偏西），选顺时针
- 若起点经度 > 终点经度（起点偏东），选逆时针
- 若起点=终点，先向北再向南为逆时针，先向南再向北为顺时针

【2. 住宿向前推进】
每天 points 数组中，最后一个元素必须是 point_type=hotel。
place_name 必须是具体镇/县/片区地名，如「乐山市区」「九寨沟沟口」「日隆镇」。
禁止「途中休息」「路上」「附近」等模糊词。
住宿须沿主线方向前移；禁止连续两天住同一地却往相反方向跑远（基地模式）。
例外：同一城市连住多日时，每天游览不同片区，且不重复昨日已走的主路段。

【3. 折返管控】
允许当日短线支线折返（如进峡谷景区后原路返回主线），但须当日完成、不跨日。
折返路段在 points 中须有标识："is_return": true（起点和终点两个点都标）。
禁止连续两天及以上重复同一段折返路。
默认每日纯驾车约不超过4小时（用户另有说明从其要求）。

【4. 排除过滤】
用户标注不去的景点，任何情况下不得出现在 place_name、query、note 中，
也不得安排为途经点。

【5. 输出格式】
只输出一个 JSON 对象，不要 markdown 代码块，不要正文解释。结构如下：

{
  "summary": "全程逆时针环线，12天，长春出发经阿尔山、满洲里、根河返回长春。",
  "self_check": {
    "direction_consistent": true,
    "excluded_omitted": true,
    "hotel_specific": true,
    "no_repeated_segment": true,
    "no_radial_pattern": true
  },
  "days": [
    {
      "day_num": 1,
      "theme": "逆时针·D1",
      "points": [
        {
          "place_name": "长春市区",
          "query": "长春 出发",
          "location": "吉林省长春市",
          "point_type": "transport",
          "stay_minutes": 0,
          "arrive": "08:00",
          "note": "出发",
          "is_return": false
        },
        {
          "place_name": "成吉思汗庙",
          "query": "乌兰浩特 成吉思汗庙",
          "location": "内蒙古兴安盟乌兰浩特市成吉思汗庙",
          "point_type": "sight",
          "stay_minutes": 90,
          "arrive": "13:30",
          "note": "停车方便，门票30元",
          "is_return": false
        },
        {
          "place_name": "乌兰浩特市区",
          "query": "乌兰浩特 住宿",
          "location": "内蒙古兴安盟乌兰浩特市",
          "point_type": "hotel",
          "stay_minutes": 0,
          "arrive": "17:00",
          "note": "入住，次日前往阿尔山",
          "is_return": false
        }
      ]
    }
  ]
}

【6. 字段说明】
summary：一句话概括全程（不含自检，自检另放）。
self_check：5项布尔值自检结果，逐项核对：
  - direction_consistent：方向是否全程一致
  - excluded_omitted：是否不含排除景点
  - hotel_specific：每晚住宿是否具体地名
  - no_repeated_segment：是否无连续两天重复路段
  - no_radial_pattern：是否非放射式绕行
days：数组，每项对应一天。
day_num：第几天，从1开始。
theme：当天主题，须含环线方向，如「逆时针·D2」。
points：当天地点列表，按游览顺序排列，每天3-6个（含过夜点）。
place_name：具体景区/地标/镇村，不写单独地级市名。
query：「城市 具体地点」，作备用检索。
location：该地点的完整地址描述（省+市+区+具体名称），用于后端调用高德地理编码API反查坐标。
示例：「内蒙古兴安盟乌兰浩特市成吉思汗庙」「四川省阿坝州九寨沟县九寨沟景区入口」
point_type：sight / hotel / food / gas / transport 五选一。
stay_minutes：停留分钟数，整数。
arrive：到达时间 HH:MM，随游览顺序递增。
note：一句实用提醒（门票/预约/路况/午餐点），无则空字符串。
is_return：标识该点是否为折返路段端点，true/false。

【7. 去重】
同一地点当天只出现一次。禁止相邻两个相同地名。古镇与同名镇算一地。

【8. 生成后自检】
生成完整路线后，必须逐项核对 self_check 中5项指标，如实填写 true/false。
有任何一项为 false，须在 points 中重新调整后再输出最终版本。"#;

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
        "temperature": 0.5,
        "thinking": { "type": "disabled" },
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": SYSTEM_PROMPT
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
    let cache_key = format!(
        "{}|{}|{}",
        p.place_name.trim(),
        p.query.trim(),
        p.location.as_deref().unwrap_or("").trim()
    );
    if let Some((lng, lat)) = cache.get(&cache_key) {
        p.longitude = Some(*lng);
        p.latitude = Some(*lat);
        p.found = true;
        return;
    }
    if try_geocode(key, secret, city, p).await {
        if let (Some(lng), Some(lat)) = (p.longitude, p.latitude) {
            cache.insert(cache_key, (lng, lat));
        }
    }
}

async fn try_geocode(key: &str, secret: &str, city: &str, p: &mut AiPoint) -> bool {
    let city_opt = city.trim();
    let city_opt = if city_opt.is_empty() { None } else { Some(city_opt) };
    let name = p.place_name.trim();
    let mut queries = Vec::new();
    if let Some(loc) = p.location.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        queries.push(loc.to_string());
    }
    let q = p.query.trim();
    if !q.is_empty() && !queries.iter().any(|x| x == q) {
        queries.push(q.to_string());
    }
    if !name.is_empty() && !queries.iter().any(|x| x == name) {
        queries.push(name.to_string());
    }
    for kw in queries {
        if let Some(hit) = geocode_address(key, secret, &kw, city_opt).await {
            apply_poi(p, &hit);
            return true;
        }
        let Ok(list) = search_places(key, secret, &kw, None, None, city_opt).await else {
            continue;
        };
        if let Some(hit) = pick_best_poi(&list, name, &kw) {
            apply_poi(p, hit);
            return true;
        }
    }
    false
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
    let user_request = if prompt.is_empty() {
        "经典环线，节奏适中，每日驾车约4小时内。".into()
    } else {
        prompt.to_string()
    };
    let (existing_line, scope) = if fresh {
        (
            String::new(),
            match focus_day {
                Some(d) => format!("请从零规划第 {d} 天，输出 day_num={d}。"),
                None => format!("请从零规划全部 {days} 天，day_num 从 1 到 {days}。"),
            },
        )
    } else if recommend {
        (
            format!("当前行程：{existing}\n"),
            match focus_day {
                Some(d) => format!("沿途推荐：保留第 {d} 天已有顺序，顺路加 1-2 个点。只输出 day_num={d}。"),
                None => format!("沿途推荐：保留每天已有顺序，顺路加点。day_num 从 1 到 {days}。"),
            },
        )
    } else {
        (
            format!("当前行程：{existing}\n"),
            match focus_day {
                Some(d) => format!("在现有基础上按新要求改第 {d} 天，未提及的尽量保留。只输出 day_num={d}。"),
                None => format!("在现有基础上按新要求调整全部 {days} 天，未提及的尽量保留。"),
            },
        )
    };
    let day_dates = trip_day_dates(start, days);
    let day_dates_line = if day_dates.is_empty() {
        String::new()
    } else {
        format!("（{day_dates}）")
    };
    let user_content = format!(
        "目的地：{destination}\n\
出行：{start} 至 {end}，共 {days} 天{day_dates_line}\n\
{existing_line}\
{scope}\n\n\
{user_request}{links}"
    );
    let model = chat_json(api_key, user_content).await?;
    let mut summary = model.summary.unwrap_or_else(|| "已排好一版行程".into());
    if let Some(check) = model.self_check {
        let failed = [
            (!check.direction_consistent.unwrap_or(true), "方向"),
            (!check.excluded_omitted.unwrap_or(true), "排除"),
            (!check.hotel_specific.unwrap_or(true), "住宿"),
            (!check.no_repeated_segment.unwrap_or(true), "重复路段"),
            (!check.no_radial_pattern.unwrap_or(true), "放射绕行"),
        ]
        .into_iter()
        .filter_map(|(bad, label)| bad.then_some(label))
        .collect::<Vec<_>>();
        if !failed.is_empty() {
            summary = format!("{summary}（自检未通过：{}）", failed.join("、"));
        }
    }
    let mut draft = AiDraft {
        summary,
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
                location: p.location.filter(|s| !s.trim().is_empty()),
                point_type: map_point_type(&p.point_type.unwrap_or_default()),
                stay_minutes: p.stay_minutes.filter(|n| *n > 0 && *n < 24 * 60),
                arrive: p.arrive.filter(|s| s.len() >= 4 && s.len() <= 8),
                note: p.note.filter(|s| !s.trim().is_empty()),
                is_return: p.is_return,
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
