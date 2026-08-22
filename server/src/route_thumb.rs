/// 把旅途途经点收成一张小路线图 SVG，给首页卡片左侧用。

const SIZE: f64 = 80.0;
const PAD: f64 = 14.0;

pub fn from_coords(coords: &[(f64, f64)]) -> Option<String> {
    let pts = unique_stops(coords);
    if pts.is_empty() {
        return None;
    }
    let drawn = project(&pts);
    Some(render(&drawn))
}

fn unique_stops(coords: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for &(lat, lng) in coords {
        if !lat.is_finite() || !lng.is_finite() {
            continue;
        }
        if lat.abs() > 90.0 || lng.abs() > 180.0 {
            continue;
        }
        if let Some(&(plat, plng)) = out.last() {
            let plat: f64 = plat;
            let plng: f64 = plng;
            if (lat - plat).abs() < 1e-4 && (lng - plng).abs() < 1e-4 {
                continue;
            }
        }
        out.push((lat, lng));
    }
    if out.len() > 10 {
        let last = out.len() - 1;
        let mut picked = vec![out[0]];
        for i in 1..10 {
            let idx = (i * last) / 9;
            if picked.last() != Some(&out[idx]) {
                picked.push(out[idx]);
            }
        }
        if picked.last() != Some(&out[last]) {
            picked.push(out[last]);
        }
        return picked;
    }
    out
}

fn project(pts: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mid_lat = pts.iter().map(|p| p.0).sum::<f64>() / pts.len() as f64;
    let cos = mid_lat.to_radians().cos().max(0.25);
    let xs: Vec<f64> = pts.iter().map(|p| p.1 * cos).collect();
    let ys: Vec<f64> = pts.iter().map(|p| p.0).collect();
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

fn render(pts: &[(f64, f64)]) -> String {
    let mut out = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 80 80" fill="none">"##,
    );
    out.push_str(r##"<path d="M12 20 H68 M12 40 H68 M12 60 H68 M20 12 V68 M40 12 V68 M60 12 V68" stroke="#d7ebe3" stroke-width="1"/>"##);
    if pts.len() == 1 {
        let (x, y) = pts[0];
        out.push_str(&format!(
            r##"<circle cx="{x}" cy="{y}" r="10" stroke="#7ccab4" stroke-width="2"/><circle cx="{x}" cy="{y}" r="3.5" fill="#4f9f8a"/>"##
        ));
    } else {
        out.push_str(&format!(
            r##"<path d="{}" stroke="#4f9f8a" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round"/>"##,
            path_d(pts)
        ));
        let (sx, sy) = pts[0];
        let (ex, ey) = pts[pts.len() - 1];
        out.push_str(&format!(
            r##"<circle cx="{sx}" cy="{sy}" r="3.2" fill="#7ccab4"/><circle cx="{ex}" cy="{ey}" r="3.8" fill="#2c6758"/>"##
        ));
    }
    out.push_str("</svg>");
    out
}

fn path_d(pts: &[(f64, f64)]) -> String {
    if pts.len() == 2 {
        let (ax, ay) = pts[0];
        let (bx, by) = pts[1];
        let mx = (ax + bx) / 2.0;
        let my = (ay + by) / 2.0;
        let dx = bx - ax;
        let dy = by - ay;
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let cx = round2(mx - dy / len * 8.0);
        let cy = round2(my + dx / len * 8.0);
        return format!("M{ax} {ay} Q{cx} {cy} {bx} {by}");
    }
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
        assert!(svg.contains("Q"));
    }
}
