use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::AppError;

/// 高德同一接口 QPS ≤ 3。按 path 滑动窗口排队。
pub async fn throttle_amap(path: &str) {
    const MAX: usize = 3;
    const WINDOW: Duration = Duration::from_millis(1050);
    static LIMITER: OnceLock<Mutex<HashMap<String, VecDeque<Instant>>>> = OnceLock::new();
    let slots = LIMITER.get_or_init(|| Mutex::new(HashMap::new()));
    loop {
        let mut map = slots.lock().await;
        let now = Instant::now();
        let q = map.entry(path.to_string()).or_default();
        while q.front().is_some_and(|t| now.duration_since(*t) >= WINDOW) {
            q.pop_front();
        }
        if q.len() < MAX {
            q.push_back(now);
            return;
        }
        let wait = q
            .front()
            .map(|t| WINDOW.saturating_sub(now.duration_since(*t)) + Duration::from_millis(20))
            .unwrap_or(Duration::from_millis(350));
        drop(map);
        tokio::time::sleep(wait).await;
    }
}

fn amap_qps_exceeded(v: &serde_json::Value) -> bool {
    let info = v.get("info").and_then(|x| x.as_str()).unwrap_or("");
    let code = v.get("infocode").and_then(|x| x.as_str()).unwrap_or("");
    code == "10021" || info.contains("CUQPS") || info.contains("QPS")
}

#[derive(serde::Serialize, Clone)]
pub struct PoiVo {
    pub name: String,
    pub address: String,
    pub longitude: f64,
    pub latitude: f64,
}

#[derive(Deserialize)]
struct AmapSearch {
    status: Option<String>,
    info: Option<String>,
    pois: Option<Vec<AmapPoi>>,
    tips: Option<Vec<AmapPoi>>,
    geocodes: Option<Vec<AmapGeo>>,
}

#[derive(Deserialize)]
struct AmapPoi {
    name: Option<serde_json::Value>,
    address: Option<serde_json::Value>,
    location: Option<serde_json::Value>,
    district: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct AmapGeo {
    formatted_address: Option<serde_json::Value>,
    location: Option<serde_json::Value>,
}

fn amap_text(v: &Option<serde_json::Value>) -> String {
    match v {
        Some(serde_json::Value::String(s)) if !s.is_empty() && s != "[]" => s.clone(),
        _ => String::new(),
    }
}

fn parse_lng_lat(raw: &str) -> Option<(f64, f64)> {
    let mut it = raw.split(',');
    let lng: f64 = it.next()?.trim().parse().ok()?;
    let lat: f64 = it.next()?.trim().parse().ok()?;
    if lat.abs() <= 90.0 && lng.abs() <= 180.0 && (lat != 0.0 || lng != 0.0) {
        Some((lng, lat))
    } else {
        None
    }
}

fn encode_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn sorted_query(params: &[(&str, &str)]) -> String {
    let mut pairs: Vec<(&str, &str)> = params.to_vec();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn encoded_query(params: &[(&str, &str)]) -> String {
    let mut pairs: Vec<(&str, &str)> = params.to_vec();
    pairs.sort_by(|a, b| a.0.cmp(b.0));
    pairs
        .iter()
        .map(|(k, v)| format!("{k}={}", encode_component(v)))
        .collect::<Vec<_>>()
        .join("&")
}

async fn amap_json(key: &str, secret: &str, path: &str, params: &[(&str, &str)]) -> Option<serde_json::Value> {
    let mut all: Vec<(&str, &str)> = params.to_vec();
    all.push(("key", key));
    let query_raw = sorted_query(&all);
    let mut url = format!("https://restapi.amap.com{path}?{}", encoded_query(&all));
    if !secret.is_empty() {
        url.push_str("&sig=");
        url.push_str(&format!("{:x}", md5::compute(format!("{query_raw}{secret}").as_bytes())));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .ok()?;
    for attempt in 0..3 {
        throttle_amap(path).await;
        let v = client.get(&url).send().await.ok()?.json::<serde_json::Value>().await.ok()?;
        if amap_qps_exceeded(&v) && attempt < 2 {
            tracing::warn!("amap qps limited path={path} attempt={}", attempt + 1);
            tokio::time::sleep(Duration::from_millis(400)).await;
            continue;
        }
        return Some(v);
    }
    None
}

async fn amap_get(key: &str, secret: &str, path: &str, params: &[(&str, &str)]) -> Option<AmapSearch> {
    let v = amap_json(key, secret, path, params).await?;
    serde_json::from_value(v).ok()
}

fn pois_from_search(resp: AmapSearch) -> Vec<PoiVo> {
    if resp.status.as_deref() != Some("1") {
        tracing::warn!("amap search fail: {:?}", resp.info);
        return vec![];
    }
    let mut out = Vec::new();
    let mut push = |name: String, address: String, loc: &str| {
        if name.is_empty() {
            return;
        }
        let Some((lng, lat)) = parse_lng_lat(loc) else {
            return;
        };
        if out.iter().any(|p: &PoiVo| {
            (p.longitude - lng).abs() < 1e-5 && (p.latitude - lat).abs() < 1e-5 && p.name == name
        }) {
            return;
        }
        out.push(PoiVo {
            name,
            address,
            longitude: lng,
            latitude: lat,
        });
    };
    for p in resp.pois.unwrap_or_default() {
        let name = amap_text(&p.name);
        let mut address = amap_text(&p.address);
        let district = amap_text(&p.district);
        if address.is_empty() {
            address = district;
        }
        push(name, address, &amap_text(&p.location));
    }
    for p in resp.tips.unwrap_or_default() {
        let name = amap_text(&p.name);
        let address = amap_text(&p.address);
        push(name, address, &amap_text(&p.location));
    }
    for g in resp.geocodes.unwrap_or_default() {
        let name = amap_text(&g.formatted_address);
        push(name.clone(), name, &amap_text(&g.location));
    }
    out
}

pub async fn search_places(
    key: &str,
    secret: &str,
    keyword: &str,
    lng: Option<f64>,
    lat: Option<f64>,
    city: Option<&str>,
) -> Result<Vec<PoiVo>, AppError> {
    if key.is_empty() {
        return Err(AppError::BadRequest("未配置 AMAP_KEY，无法搜索地点".into()));
    }
    let kw = keyword.trim();
    if kw.chars().count() < 2 {
        return Ok(vec![]);
    }
    let loc = match (lng, lat) {
        (Some(lng), Some(lat)) => Some(format!("{lng:.6},{lat:.6}")),
        _ => None,
    };
    let city = city.map(str::trim).filter(|s| !s.is_empty());
    let mut params: Vec<(&str, &str)> = vec![
        ("keywords", kw),
        ("offset", "8"),
        ("page", "1"),
        ("extensions", "base"),
    ];
    if let Some(ref loc) = loc {
        params.push(("location", loc.as_str()));
    }
    if let Some(c) = city {
        params.push(("city", c));
    }
    let mut list = pois_from_search(
        amap_get(key, secret, "/v3/place/text", &params)
            .await
            .unwrap_or(AmapSearch {
                status: None,
                info: None,
                pois: None,
                tips: None,
                geocodes: None,
            }),
    );
    if list.is_empty() {
        let mut tip_params: Vec<(&str, &str)> = vec![("keywords", kw)];
        if let Some(ref loc) = loc {
            tip_params.push(("location", loc.as_str()));
        }
        if let Some(c) = city {
            tip_params.push(("city", c));
        }
        list = pois_from_search(
            amap_get(key, secret, "/v3/assistant/inputtips", &tip_params)
                .await
                .unwrap_or(AmapSearch {
                    status: None,
                    info: None,
                    pois: None,
                    tips: None,
                    geocodes: None,
                }),
        );
    }
    if list.is_empty() {
        let mut geo_params: Vec<(&str, &str)> = vec![("address", kw)];
        if let Some(c) = city {
            geo_params.push(("city", c));
        }
        list = pois_from_search(
            amap_get(key, secret, "/v3/geocode/geo", &geo_params)
                .await
                .unwrap_or(AmapSearch {
                    status: None,
                    info: None,
                    pois: None,
                    tips: None,
                    geocodes: None,
                }),
        );
    }
    Ok(list)
}

#[derive(Deserialize)]
struct RegeoResp {
    status: Option<String>,
    regeocode: Option<RegeoBody>,
}

#[derive(Deserialize)]
struct RegeoBody {
    formatted_address: Option<serde_json::Value>,
    pois: Option<Vec<AmapPoi>>,
}

pub async fn reverse_geocode(
    key: &str,
    secret: &str,
    lng: f64,
    lat: f64,
) -> Result<PoiVo, AppError> {
    if key.is_empty() {
        return Ok(PoiVo {
            name: "地图选点".into(),
            address: String::new(),
            longitude: lng,
            latitude: lat,
        });
    }
    let loc = format!("{lng:.6},{lat:.6}");
    let raw = amap_json(
        key,
        secret,
        "/v3/geocode/regeo",
        &[("extensions", "all"), ("location", loc.as_str()), ("radius", "300")],
    )
    .await
    .unwrap_or(serde_json::Value::Null);
    let parsed: RegeoResp = serde_json::from_value(raw).unwrap_or(RegeoResp {
        status: None,
        regeocode: None,
    });
    let mut name = String::new();
    let mut address = String::new();
    if parsed.status.as_deref() == Some("1") {
        if let Some(body) = parsed.regeocode {
            address = amap_text(&body.formatted_address);
            if let Some(pois) = body.pois {
                for p in pois {
                    let n = amap_text(&p.name);
                    if !n.is_empty() {
                        name = n;
                        break;
                    }
                }
            }
        }
    }
    if name.is_empty() {
        name = if address.is_empty() {
            "地图选点".into()
        } else {
            address.clone()
        };
    }
    Ok(PoiVo {
        name,
        address,
        longitude: lng,
        latitude: lat,
    })
}
