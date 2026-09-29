use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::{error::AppError, state::AppState};

pub const REJECT_MSG: &str = "内容包含敏感信息，请修改后再保存";

const WORDS: &[&str] = &[
    "法轮功",
    "法轮大法",
    "全能神",
    "藏独",
    "台独",
    "疆独",
    "港独",
    "六四事件",
    "色情",
    "裸聊",
    "约炮",
    "一夜情",
    "援交",
    "嫖娼",
    "淫秽",
    "赌博",
    "博彩",
    "六合彩",
    "网赌",
    "赌场",
    "冰毒",
    "海洛因",
    "大麻",
    "摇头丸",
    "吸毒",
    "恐怖组织",
    "恐怖袭击",
    "傻逼",
    "操你妈",
    "草泥马",
];

struct CachedToken {
    token: String,
    expire_at: Instant,
}

static TOKEN: Mutex<Option<CachedToken>> = Mutex::new(None);

/// 保存用户输入前调用。本地词库必查；配置了小程序密钥且是真实 openid 时再走微信内容安全接口。
pub async fn ensure_user_text(
    state: &AppState,
    user_id: i64,
    texts: &[Option<&str>],
) -> Result<(), AppError> {
    let open_id: Option<String> =
        sqlx::query_scalar("SELECT open_id FROM app_user WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;
    ensure_text(state, open_id.as_deref(), texts).await
}

pub async fn ensure_text(
    state: &AppState,
    open_id: Option<&str>,
    texts: &[Option<&str>],
) -> Result<(), AppError> {
    let parts: Vec<&str> = texts
        .iter()
        .filter_map(|s| s.map(str::trim).filter(|s| !s.is_empty()))
        .collect();
    if parts.is_empty() {
        return Ok(());
    }
    let joined = parts.join("\n");
    if local_blocked(&joined) {
        return Err(AppError::BadRequest(REJECT_MSG.into()));
    }
    if state.wechat_appid.is_empty() || state.wechat_secret.is_empty() {
        return Ok(());
    }
    let Some(open_id) = open_id.filter(|id| real_openid(id)) else {
        return Ok(());
    };
    match wx_blocked(state, open_id, &joined).await {
        Ok(true) => Err(AppError::BadRequest(REJECT_MSG.into())),
        Ok(false) => Ok(()),
        Err(e) => {
            tracing::warn!("微信内容安全接口不可用，已仅使用本地词库: {e}");
            Ok(())
        }
    }
}

fn real_openid(id: &str) -> bool {
    let id = id.trim();
    id.len() >= 16
        && !id.starts_with("guest_")
        && !id.starts_with("demo_")
        && !id.starts_with("sys_")
        && !id.starts_with("mp_")
        && !id.starts_with("dev_")
        && !id.starts_with("wx_")
}

fn local_blocked(text: &str) -> bool {
    let normalized = normalize(text);
    if normalized.is_empty() {
        return false;
    }
    lexicon().iter().any(|word| normalized.contains(word))
}

fn lexicon() -> &'static [String] {
    static LEX: OnceLock<Vec<String>> = OnceLock::new();
    LEX.get_or_init(|| {
        WORDS
            .iter()
            .map(|w| normalize(w))
            .filter(|s| !s.is_empty())
            .collect()
    })
}

fn normalize(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .filter(|c| !matches!(c, '*' | '.' | '-' | '_' | '|' | '/' | '\\' | '·' | '。' | '，' | ','))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

async fn wx_blocked(state: &AppState, open_id: &str, text: &str) -> Result<bool, AppError> {
    let token = access_token(state).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;
    for chunk in utf8_chunks(text, 2000) {
        let url = format!("https://api.weixin.qq.com/wxa/msg_sec_check?access_token={token}");
        let resp = client
            .post(url)
            .json(&serde_json::json!({
                "openid": open_id,
                "scene": 4,
                "version": 2,
                "content": chunk,
            }))
            .send()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let body: WxSecResp = resp
            .json()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if body.errcode == 87014 {
            return Ok(true);
        }
        if body.errcode != 0 {
            return Err(AppError::Internal(format!(
                "msgSecCheck {} {}",
                body.errcode,
                body.errmsg.unwrap_or_default()
            )));
        }
        let suggest = body
            .result
            .as_ref()
            .and_then(|r| r.suggest.as_deref())
            .unwrap_or("pass");
        if suggest != "pass" {
            tracing::info!(suggest, label = body.result.as_ref().and_then(|r| r.label), "内容安全未通过");
            return Ok(true);
        }
    }
    Ok(false)
}

fn utf8_chunks(text: &str, max_bytes: usize) -> Vec<&str> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if rest.len() <= max_bytes {
            out.push(rest);
            break;
        }
        let mut end = max_bytes;
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        out.push(&rest[..end]);
        rest = &rest[end..];
    }
    out
}

async fn access_token(state: &AppState) -> Result<String, AppError> {
    if let Some(token) = cached_token() {
        return Ok(token);
    }
    let url = format!(
        "https://api.weixin.qq.com/cgi-bin/token?grant_type=client_credential&appid={}&secret={}",
        state.wechat_appid, state.wechat_secret
    );
    let body: WxTokenResp = reqwest::get(url)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let token = body
        .access_token
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            AppError::Internal(format!(
                "获取微信 access_token 失败 {}",
                body.errmsg.unwrap_or_default()
            ))
        })?;
    let ttl = body.expires_in.unwrap_or(7200).saturating_sub(120).max(60);
    let mut guard = TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(CachedToken {
        token: token.clone(),
        expire_at: Instant::now() + Duration::from_secs(ttl),
    });
    Ok(token)
}

fn cached_token() -> Option<String> {
    let guard = TOKEN.lock().unwrap_or_else(|e| e.into_inner());
    guard
        .as_ref()
        .filter(|c| c.expire_at > Instant::now())
        .map(|c| c.token.clone())
}

#[derive(Deserialize)]
struct WxTokenResp {
    access_token: Option<String>,
    expires_in: Option<u64>,
    errmsg: Option<String>,
}

#[derive(Deserialize)]
struct WxSecResp {
    #[serde(default)]
    errcode: i32,
    errmsg: Option<String>,
    result: Option<WxSecResult>,
}

#[derive(Deserialize)]
struct WxSecResult {
    suggest: Option<String>,
    label: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_word_with_separators() {
        assert!(!local_blocked("正常行程"));
        assert!(!local_blocked("四姑娘山环线"));
        assert!(local_blocked("去 赌*博"));
        assert!(local_blocked("法轮功"));
    }

    #[test]
    fn splits_on_char_boundary() {
        let text = "成都出发四姑娘山";
        let parts = utf8_chunks(text, 7);
        assert!(parts.len() > 1);
        assert_eq!(parts.concat(), text);
    }
}
