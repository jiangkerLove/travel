use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap, HeaderName},
};

const VERSION_HEADER: HeaderName = HeaderName::from_static("x-app-version");
const ENV_HEADER: HeaderName = HeaderName::from_static("x-app-env");

/// 小程序随请求带上的版本。
/// 线上旧包和审核中的新包会同时打到同一套接口；没有 `X-App-Version` 的请求是更早的线上包。
/// 接口不兼容时在 handler 参数里加上 `ClientInfo`，用 [`ClientInfo::at_least`] 分流。
#[derive(Debug, Clone)]
pub struct ClientInfo {
    pub version: Option<(u32, u32, u32)>,
    pub raw: String,
    pub env: String,
}

impl ClientInfo {
    pub fn from_headers(headers: &HeaderMap) -> Self {
        let raw = header_value(headers, &VERSION_HEADER);
        let env = normalize_env(&header_value(headers, &ENV_HEADER));
        Self {
            version: parse_version(&raw),
            raw,
            env,
        }
    }

    pub fn label(&self) -> &str {
        if self.raw.is_empty() {
            "legacy"
        } else {
            &self.raw
        }
    }

    /// 未带版本号，或版本号无法解析时，视为低于任意目标版本。
    pub fn at_least(&self, version: &str) -> bool {
        match (self.version, parse_version(version)) {
            (Some(current), Some(min)) => current >= min,
            _ => false,
        }
    }
}

fn header_value(headers: &HeaderMap, name: &HeaderName) -> String {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .trim()
        .chars()
        .take(32)
        .collect()
}

fn normalize_env(raw: &str) -> String {
    match raw {
        "develop" | "trial" | "release" => raw.to_string(),
        _ => String::new(),
    }
}

fn parse_version(raw: &str) -> Option<(u32, u32, u32)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let mut parts = raw.split('.');
    let major = part_num(parts.next()?)?;
    let minor = part_num(parts.next().unwrap_or("0"))?;
    let patch = part_num(parts.next().unwrap_or("0"))?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn part_num(part: &str) -> Option<u32> {
    let part = part.split(['-', '+']).next().unwrap_or("");
    if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    part.parse().ok()
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for ClientInfo
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        Ok(Self::from_headers(&parts.headers))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(raw: &str) -> ClientInfo {
        ClientInfo {
            version: parse_version(raw),
            raw: raw.to_string(),
            env: String::new(),
        }
    }

    #[test]
    fn legacy_client_is_below_any_version() {
        let old = info("");
        assert!(!old.at_least("1.0.0"));
        assert_eq!(old.label(), "legacy");
    }

    #[test]
    fn compares_release_and_review_versions() {
        let live = info("1.0.0");
        let review = info("1.1.0");
        assert!(live.at_least("1.0.0"));
        assert!(!live.at_least("1.1.0"));
        assert!(review.at_least("1.1.0"));
        assert!(review.at_least("1.0.0"));
    }

    #[test]
    fn patch_compares_as_number() {
        let current = info("1.0.2");
        let later = info("1.0.102");
        assert!(current.at_least("1.0.2"));
        assert!(!current.at_least("1.0.102"));
        assert!(later.at_least("1.0.2"));
        assert!(later.at_least("1.0.102"));
        assert!(!info("1.0.9").at_least("1.0.10"));
    }
}
