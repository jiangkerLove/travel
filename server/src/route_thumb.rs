/// 把旅途途经点和路段收成一张小路线图 SVG，给首页卡片左侧用。

use crate::route::LatLng;

const SIZE: f64 = 80.0;
const PAD: f64 = 10.0;
const MAX_STOPS: usize = 24;
const MAX_PATH: usize = 72;
const PER_LEG: usize = 16;

pub fn from_coords(coords: &[(f64, f64)]) -> Option<String> {
    from_trip(coords, &[])
}

pub fn from_trip(stops: &[(f64, f64)], path: &[(f64, f64)]) -> Option<String> {
    let stops = prepare_stops(stops);
    if stops.is_empty() {
        return None;
    }
    let path = if path.len() >= 2 {
        prepare_path(path, MAX_PATH)
    } else {
        stops.clone()
    };
    let mut frame = path.clone();
    frame.extend(stops.iter().copied());
    let drawn_path = project(&path, &frame);
    let drawn_stops = project(&stops, &frame);
    Some(render(&drawn_path, &drawn_stops))
}

pub fn stitch_path(stops: &[(i64, f64, f64)], legs: &[(i64, i64, Vec<LatLng>)]) -> Vec<(f64, f64)> {
    if stops.is_empty() {
        return vec![];
    }
    let mut lookup = std::collections::HashMap::new();
    for (from_id, to_id, pts) in legs {
        lookup.insert((*from_id, *to_id), pts);
    }
    let mut path = vec![(stops[0].1, stops[0].2)];
    for w in stops.windows(2) {
        let (a_id, _, _) = w[0];
        let (b_id, b_lat, b_lng) = w[1];
        if let Some(pts) = lookup.get(&(a_id, b_id)) {
            let slim = prepare_path(
                &pts.iter().map(|p| (p.latitude, p.longitude)).collect::<Vec<_>>(),
                PER_LEG,
            );
            for (lat, lng) in slim.into_iter().skip(1) {
                path.push((lat, lng));
            }
        } else {
            path.push((b_lat, b_lng));
        }
    }
    path
}

fn prepare_stops(coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(lat, lng) in coords {
        if !usable(lat, lng) {
            continue;
        }
        if let Some(&(plat, plng)) = out.last() {
            if almost_same(lat, lng, plat, plng) {
                continue;
            }
        }
        out.push((lat, lng));
    }
    downsample(&out, MAX_STOPS)
}

fn prepare_path(coords: &[(f64, f64)], max_n: usize) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(lat, lng) in coords {
        if !usable(lat, lng) {
            continue;
        }
        if let Some(&(plat, plng)) = out.last() {
            if almost_same(lat, lng, plat, plng) {
                continue;
            }
        }
        out.push((lat, lng));
    }
    downsample(&out, max_n)
}

fn usable(lat: f64, lng: f64) -> bool {
    lat.is_finite() && lng.is_finite() && lat.abs() <= 90.0 && lng.abs() <= 180.0
}

fn almost_same(lat: f64, lng: f64, plat: f64, plng: f64) -> bool {
    (lat - plat).abs() < 1e-4 && (lng - plng).abs() < 1e-4
}

fn downsample(pts: &[(f64, f64)], max_n: usize) -> Vec<(f64, f64)> {
    if pts.len() <= max_n || max_n < 2 {
        return pts.to_vec();
    }
    let last = pts.len() - 1;
    let mut out = vec![pts[0]];
    for i in 1..max_n - 1 {
        let idx = i * last / (max_n - 1);
        if out.last() != Some(&pts[idx]) {
            out.push(pts[idx]);
        }
    }
    if out.last() != Some(&pts[last]) {
        out.push(pts[last]);
    }
    out
}

fn project(pts: &[(f64, f64)], frame: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mid_lat = frame.iter().map(|p| p.0).sum::<f64>() / frame.len().max(1) as f64;
    let cos = mid_lat.to_radians().cos().max(0.25);
    let xs: Vec<f64> = frame.iter().map(|p| p.1 * cos).collect();
    let ys: Vec<f64> = frame.iter().map(|p| p.0).collect();
    let min_x = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_x = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let min_y = ys.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_y = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let mut w = (max_x - min_x).max(1e-6);
    let mut h = (max_y - min_y).max(1e-6);
    if w < 0.02 && h < 0.02 {
        w = 0.02;
        h = 0.02;
    }
    let box_s = SIZE - PAD * 2.0;
    let scale = (box_s / w).min(box_s / h);
    let ox = (SIZE - w * scale) / 2.0;
    let oy = (SIZE - h * scale) / 2.0;
    pts.iter()
        .map(|&(lat, lng)| {
            let x = ox + (lng * cos - min_x) * scale;
            let y = SIZE - (oy + (lat - min_y) * scale);
            (round2(x), round2(y))
        })
        .collect()
}

fn round2(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn render(path: &[(f64, f64)], stops: &[(f64, f64)]) -> String {
    let mut out = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80" fill="none">"##,
    );
    out.push_str(
        r##"<path d="M12 20 H68 M12 40 H68 M12 60 H68 M20 12 V68 M40 12 V68 M60 12 V68" stroke="#e4f0ea" stroke-width="0.8"/>"##,
    );
    if path.len() >= 2 {
        out.push_str(&format!(
            r##"<path d="{}" stroke="#4f9f8a" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"##,
            path_d(path)
        ));
    }
    if stops.len() == 1 {
        let (x, y) = stops[0];
        out.push_str(&format!(
            r##"<circle cx="{x}" cy="{y}" r="8" stroke="#7ccab4" stroke-width="1.4"/><circle cx="{x}" cy="{y}" r="2.8" fill="#4f9f8a"/>"##
        ));
    } else {
        for (i, &(x, y)) in stops.iter().enumerate() {
            if i == 0 {
                out.push_str(&format!(
                    r##"<circle cx="{x}" cy="{y}" r="2.6" fill="#7ccab4"/>"##
                ));
            } else if i + 1 == stops.len() {
                out.push_str(&format!(
                    r##"<circle cx="{x}" cy="{y}" r="3" fill="#2c6758"/>"##
                ));
            } else {
                out.push_str(&format!(
                    r##"<circle cx="{x}" cy="{y}" r="1.7" fill="#56ab92"/>"##
                ));
            }
        }
    }
    out.push_str("</svg>");
    out
}

fn path_d(pts: &[(f64, f64)]) -> String {
    let mut d = format!("M{} {}", pts[0].0, pts[0].1);
    for w in pts.windows(2) {
        d.push_str(&format!(" L{} {}", w[1].0, w[1].1));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_none() {
        assert!(from_coords(&[]).is_none());
    }

    #[test]
    fn two_points_has_path() {
        let svg = from_coords(&[(30.67, 104.06), (31.05, 103.48)]).unwrap();
        assert!(svg.contains("<path"));
        assert!(svg.contains("L"));
    }

    #[test]
    fn keeps_mid_stops() {
        let svg = from_coords(&[(30.0, 104.0), (30.2, 103.8), (30.4, 103.5)]).unwrap();
        let n = svg.matches("<circle").count();
        assert!(n >= 3);
    }
}
