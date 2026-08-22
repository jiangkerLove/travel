use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::Deserialize;

static WARNED_KEY: AtomicBool = AtomicBool::new(false);
static WARNED_QUOTA: AtomicBool = AtomicBool::new(false);
static SKIP_REMOTE: AtomicBool = AtomicBool::new(false);
static ROUTE_CACHE: OnceLock<Mutex<HashMap<String, Option<CachedRoute>>>> = OnceLock::new();

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct LatLng {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(Clone)]
struct CachedRoute {
    points: Vec<LatLng>,
    from_nav: bool,
    distance_m: i32,
    duration_s: i32,
}

#[derive(Clone)]
pub struct RouteResult {
    pub mode: String,
    pub points: Vec<LatLng>,
    pub from_nav: bool,
    pub distance_m: i32,
    pub duration_s: i32,
}

#[derive(Deserialize)]
struct AmapResp {
    status: Option<String>,
    infocode: Option<String>,
    info: Option<String>,
    route: Option<AmapRoute>,
}

#[derive(Deserialize)]
struct AmapRoute {
    paths: Option<Vec<AmapPath>>,
}

#[derive(Deserialize)]
struct AmapPath {
    distance: Option<serde_json::Value>,
    cost: Option<AmapCost>,
    polyline: Option<serde_json::Value>,
    steps: Option<Vec<AmapStep>>,
}

#[derive(Deserialize)]
struct AmapCost {
    duration: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct AmapStep {
    polyline: Option<serde_json::Value>,
    cost: Option<AmapCost>,
}

pub fn traffic_mode(traffic: Option<&str>) -> &'static str {
    match traffic.unwrap_or("drive") {
        "walk" => "walking",
        "drive" | "bus" => "driving",
        "highspeed" | "train" => "transit",
        "plane" => "air",
        _ => "driving",
    }
}

fn haversine_km(from_lat: f64, from_lng: f64, to_lat: f64, to_lng: f64) -> f64 {
    let r = 6371.0;
    let d_lat = (to_lat - from_lat).to_radians();
    let d_lng = (to_lng - from_lng).to_radians();
    let a = (d_lat / 2.0).sin().powi(2)
        + from_lat.to_radians().cos() * to_lat.to_radians().cos() * (d_lng / 2.0).sin().powi(2);
    2.0 * r * a.sqrt().min(1.0).asin()
}

fn estimate_cost(mode: &str, km: f64) -> (i32, i32) {
    let distance_m = (km * 1000.0).round().max(1.0) as i32;
    let hours = match mode {
        "walking" => km / 4.5,
        "driving" => km / 55.0,
        "air" => km / 750.0 + 0.7,
        "transit" => km / 250.0 + 0.3,
        _ => km / 55.0,
    };
    let duration_s = (hours * 3600.0).round().max(60.0) as i32;
    (distance_m, duration_s)
}

/// 路书概览：点到点连线。飞机画航线弧，其它交通画直线。
pub fn sketch_route(
    traffic: Option<&str>,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
) -> Vec<LatLng> {
    let mode = traffic_mode(traffic);
    let km = haversine_km(from_lat, from_lng, to_lat, to_lng);
    if km < 0.05 || mode != "air" {
        return vec![
            LatLng {
                latitude: from_lat,
                longitude: from_lng,
            },
            LatLng {
                latitude: to_lat,
                longitude: to_lng,
            },
        ];
    }
    let n = if km < 80.0 { 12 } else { 20 };
    let d_lat = to_lat - from_lat;
    let d_lng = to_lng - from_lng;
    let cos = ((from_lat + to_lat) / 2.0).to_radians().cos().max(0.2);
    let mut p_lng = -d_lat / cos;
    let mut p_lat = d_lng * cos;
    let plen = (p_lng * p_lng + p_lat * p_lat).sqrt().max(1e-9);
    p_lng /= plen;
    p_lat /= plen;
    let mag = 0.18 * (d_lat * d_lat + (d_lng * cos).powi(2)).sqrt();
    (0..=n)
        .map(|i| {
            let t = i as f64 / n as f64;
            let ease = (std::f64::consts::PI * t).sin();
            LatLng {
                latitude: from_lat + d_lat * t + p_lat * mag * ease,
                longitude: from_lng + d_lng * t + p_lng * mag * ease,
            }
        })
        .collect()
}

fn point_line_dist(p: &LatLng, a: &LatLng, b: &LatLng) -> f64 {
    let dx = b.longitude - a.longitude;
    let dy = b.latitude - a.latitude;
    if dx == 0.0 && dy == 0.0 {
        let ddx = p.longitude - a.longitude;
        let ddy = p.latitude - a.latitude;
        return (ddx * ddx + ddy * ddy).sqrt();
    }
    let t = ((p.longitude - a.longitude) * dx + (p.latitude - a.latitude) * dy) / (dx * dx + dy * dy);
    let t = t.clamp(0.0, 1.0);
    let cx = a.longitude + t * dx;
    let cy = a.latitude + t * dy;
    let ddx = p.longitude - cx;
    let ddy = p.latitude - cy;
    (ddx * ddx + ddy * ddy).sqrt()
}

fn rdp(pts: &[LatLng], eps: f64) -> Vec<LatLng> {
    if pts.len() <= 2 {
        return pts.to_vec();
    }
    let first = &pts[0];
    let last = &pts[pts.len() - 1];
    let mut max_d = 0.0;
    let mut idx = 0;
    for i in 1..pts.len() - 1 {
        let d = point_line_dist(&pts[i], first, last);
        if d > max_d {
            max_d = d;
            idx = i;
        }
    }
    if max_d > eps {
        let mut left = rdp(&pts[..=idx], eps);
        let right = rdp(&pts[idx..], eps);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![first.clone(), last.clone()]
    }
}

fn simplify_overview(pts: Vec<LatLng>) -> Vec<LatLng> {
    if pts.len() <= 64 {
        return pts;
    }
    rdp(&pts, 0.0015)
}

fn json_num(v: &Option<serde_json::Value>) -> Option<f64> {
    match v {
        Some(serde_json::Value::Number(n)) => n.as_f64(),
        Some(serde_json::Value::String(s)) if !s.is_empty() && s != "[]" => s.parse().ok(),
        _ => None,
    }
}

fn path_duration(path: &AmapPath) -> i32 {
    if let Some(n) = path.cost.as_ref().and_then(|c| json_num(&c.duration)) {
        if n > 0.0 {
            return n.round() as i32;
        }
    }
    path.steps
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|s| s.cost.as_ref().and_then(|c| json_num(&c.duration)))
        .sum::<f64>()
        .round() as i32
}

fn json_str(v: &Option<serde_json::Value>) -> Option<String> {
    match v {
        Some(serde_json::Value::String(s)) if !s.is_empty() && s != "[]" => Some(s.clone()),
        _ => None,
    }
}

fn parse_amap_polyline(raw: &str) -> Vec<LatLng> {
    let mut out = Vec::new();
    for part in raw.split(';') {
        let mut it = part.split(',');
        let (Some(lng_s), Some(lat_s)) = (it.next(), it.next()) else {
            continue;
        };
        let (Ok(lng), Ok(lat)) = (lng_s.trim().parse::<f64>(), lat_s.trim().parse::<f64>()) else {
            continue;
        };
        if lat.abs() <= 90.0 && lng.abs() <= 180.0 {
            out.push(LatLng {
                latitude: lat,
                longitude: lng,
            });
        }
    }
    out
}

fn collect_polyline(path: &AmapPath) -> Vec<LatLng> {
    if let Some(raw) = json_str(&path.polyline) {
        let pts = parse_amap_polyline(&raw);
        if pts.len() >= 2 {
            return pts;
        }
    }
    let mut pts = Vec::new();
    for step in path.steps.as_deref().unwrap_or(&[]) {
        if let Some(raw) = json_str(&step.polyline) {
            pts.extend(parse_amap_polyline(&raw));
        }
    }
    pts
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

fn amap_sig(query: &str, secret: &str) -> String {
    format!("{:x}", md5::compute(format!("{query}{secret}").as_bytes()))
}

fn cache() -> &'static Mutex<HashMap<String, Option<CachedRoute>>> {
    ROUTE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(mode: &str, from_lat: f64, from_lng: f64, to_lat: f64, to_lng: f64) -> String {
    format!("amap:{mode}:{from_lat:.5},{from_lng:.5}->{to_lat:.5},{to_lng:.5}")
}

fn fatal_amap(infocode: &str) -> bool {
    matches!(
        infocode,
        "10001" | "10003" | "10009" | "10029" | "10044" | "USERKEY_PLAT_NOMATCH"
    )
}

fn sketch_result(
    traffic: Option<&str>,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
) -> RouteResult {
    let mode = traffic_mode(traffic).to_string();
    let km = haversine_km(from_lat, from_lng, to_lat, to_lng);
    let (distance_m, duration_s) = estimate_cost(&mode, km);
    RouteResult {
        mode,
        points: sketch_route(traffic, from_lat, from_lng, to_lat, to_lng),
        from_nav: false,
        distance_m,
        duration_s,
    }
}

async fn fetch_amap(
    key: &str,
    secret: &str,
    mode: &str,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
) -> Option<CachedRoute> {
    let origin = format!("{from_lng:.6},{from_lat:.6}");
    let destination = format!("{to_lng:.6},{to_lat:.6}");
    let mut params: Vec<(&str, &str)> = vec![
        ("destination", destination.as_str()),
        ("key", key),
        ("origin", origin.as_str()),
        ("show_fields", "cost,polyline"),
    ];
    let strategy = "32";
    if mode == "driving" {
        params.push(("strategy", strategy));
    }
    let query = sorted_query(&params);
    let mut url = format!("https://restapi.amap.com/v5/direction/{mode}?{query}");
    if !secret.is_empty() {
        url.push_str("&sig=");
        url.push_str(&amap_sig(&query, secret));
    }
    crate::poi::throttle_amap(&format!("/v5/direction/{mode}")).await;
    let resp = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(4))
        .build()
        .ok()?
        .get(&url)
        .send()
        .await
        .ok()?
        .json::<AmapResp>()
        .await
        .ok()?;
    let status = resp.status.as_deref().unwrap_or("0");
    if status != "1" {
        let code = resp.infocode.unwrap_or_default();
        if fatal_amap(&code) {
            SKIP_REMOTE.store(true, Ordering::Relaxed);
            if code == "10001" || code == "10009" || code == "10044" {
                if !WARNED_KEY.swap(true, Ordering::Relaxed) {
                    tracing::warn!(
                        "高德路线 Key 不可用(infocode={code})，已停止继续请求。请确认 Web 服务 Key 已开通路径规划 2.0"
                    );
                }
            } else if !WARNED_QUOTA.swap(true, Ordering::Relaxed) {
                tracing::warn!(
                    "高德路线额度不足(infocode={code})。路书改用点到点连线并估算时间"
                );
            }
        } else {
            tracing::warn!(
                "amap direction status={} infocode={} mode={} msg={:?}",
                status,
                code,
                mode,
                resp.info
            );
        }
        return None;
    }
    let path = resp.route?.paths?.into_iter().next()?;
    let distance_m = json_num(&path.distance).unwrap_or(0.0).round() as i32;
    let duration_s = path_duration(&path);
    let pts = collect_polyline(&path);
    if distance_m <= 0 && duration_s <= 0 && pts.len() < 2 {
        return None;
    }
    let from_nav = pts.len() >= 2;
    Some(CachedRoute {
        points: if from_nav {
            simplify_overview(pts)
        } else {
            vec![]
        },
        from_nav,
        distance_m: distance_m.max(0),
        duration_s: duration_s.max(0),
    })
}

pub async fn plan_route(
    key: &str,
    secret: &str,
    traffic: Option<&str>,
    from_lat: f64,
    from_lng: f64,
    to_lat: f64,
    to_lng: f64,
) -> RouteResult {
    let mode = traffic_mode(traffic);
    let sketch = sketch_result(traffic, from_lat, from_lng, to_lat, to_lng);
    let key_s = cache_key(mode, from_lat, from_lng, to_lat, to_lng);
    if let Ok(guard) = cache().lock() {
        if let Some(hit) = guard.get(&key_s) {
            return match hit {
                Some(pts) => RouteResult {
                    mode: mode.to_string(),
                    points: if pts.points.len() >= 2 {
                        pts.points.clone()
                    } else {
                        sketch.points.clone()
                    },
                    from_nav: pts.from_nav,
                    distance_m: if pts.distance_m > 0 {
                        pts.distance_m
                    } else {
                        sketch.distance_m
                    },
                    duration_s: if pts.duration_s > 0 {
                        pts.duration_s
                    } else {
                        sketch.duration_s
                    },
                },
                None => sketch,
            };
        }
    }
    // 高铁/火车/飞机不调道路规划。步行过远高德会拒，改估算。
    let km = haversine_km(from_lat, from_lng, to_lat, to_lng);
    let skip_remote = mode == "air"
        || mode == "transit"
        || key.is_empty()
        || SKIP_REMOTE.load(Ordering::Relaxed)
        || (mode == "walking" && km > 80.0);
    if skip_remote {
        return sketch;
    }
    match fetch_amap(key, secret, mode, from_lat, from_lng, to_lat, to_lng).await {
        Some(hit) => {
            let points = if hit.points.len() >= 2 {
                hit.points.clone()
            } else {
                sketch.points.clone()
            };
            let distance_m = if hit.distance_m > 0 {
                hit.distance_m
            } else {
                sketch.distance_m
            };
            let duration_s = if hit.duration_s > 0 {
                hit.duration_s
            } else {
                sketch.duration_s
            };
            let stored = CachedRoute {
                points: points.clone(),
                from_nav: hit.from_nav,
                distance_m,
                duration_s,
            };
            if let Ok(mut guard) = cache().lock() {
                guard.insert(key_s, Some(stored.clone()));
            }
            RouteResult {
                mode: mode.to_string(),
                points,
                from_nav: stored.from_nav,
                distance_m,
                duration_s,
            }
        }
        None => {
            if let Ok(mut guard) = cache().lock() {
                guard.insert(key_s, None);
            }
            sketch
        }
    }
}
