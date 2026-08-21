use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use chrono::{Datelike, NaiveDate};

use crate::{
    auth::make_token,
    db::{find_user, USER_COLS},
    error::{ok, ApiOk, AppError},
    state::{AppState, AuthUser},
};

#[derive(Deserialize)]
pub struct LoginReq {
    pub code: Option<String>,
    pub open_id: Option<String>,
    pub nickname: Option<String>,
    pub avatar: Option<String>,
}

#[derive(Serialize)]
pub struct LoginVo {
    pub token: String,
    pub user: UserVo,
}

#[derive(Serialize)]
pub struct UserVo {
    pub id: i64,
    pub open_id: String,
    pub nickname: String,
    pub avatar: Option<String>,
    pub default_bill_visible: bool,
    pub birthday: Option<String>,
    pub gender: i16,
    pub female_role: i16,
    pub work_start_year: Option<i32>,
    pub work_life: Option<crate::worklife::WorkLifeVo>,
}

fn user_vo(u: &crate::db::UserRow) -> UserVo {
    UserVo {
        id: u.id,
        open_id: u.open_id.clone(),
        nickname: u.nickname.clone(),
        avatar: u.avatar.clone(),
        default_bill_visible: u.default_bill_visible,
        birthday: u.birthday.map(|d| d.format("%Y-%m-%d").to_string()),
        gender: u.gender,
        female_role: u.female_role,
        work_start_year: u.work_start_year,
        work_life: crate::worklife::build_work_life(
            u.birthday,
            u.gender,
            u.female_role,
            u.work_start_year,
        ),
    }
}

fn wechat_enabled(state: &AppState) -> bool {
    !state.wechat_appid.is_empty() && !state.wechat_secret.is_empty()
}

#[derive(Deserialize)]
pub struct UpdateUserReq {
    pub nickname: Option<String>,
    pub avatar: Option<String>,
    pub default_bill_visible: Option<bool>,
    pub birthday: Option<String>,
    #[serde(default, alias = "femaleRole")]
    pub female_role: Option<i16>,
    #[serde(default, alias = "workStartYear")]
    pub work_start_year: Option<i32>,
    pub gender: Option<i16>,
}

#[derive(Deserialize)]
struct WxSession {
    openid: Option<String>,
    errcode: Option<i32>,
    errmsg: Option<String>,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> Result<Json<ApiOk<LoginVo>>, AppError> {
    let open_id = resolve_open_id(&state, &req).await?;
    let nickname = req
        .nickname
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "旅行者".into());
    let avatar = req.avatar.clone();

    let existing = sqlx::query_as::<_, crate::db::UserRow>(&format!(
        "SELECT {USER_COLS} FROM app_user WHERE open_id = $1"
    ))
    .bind(&open_id)
    .fetch_optional(&state.pool)
    .await?;

    let user = if let Some(u) = existing {
        if req.nickname.is_some() || req.avatar.is_some() {
            sqlx::query_as::<_, crate::db::UserRow>(&format!(
                r#"
                UPDATE app_user
                SET nickname = COALESCE($2, nickname),
                    avatar = COALESCE($3, avatar)
                WHERE id = $1
                RETURNING {USER_COLS}
                "#
            ))
            .bind(u.id)
            .bind(req.nickname.as_deref())
            .bind(req.avatar.as_deref())
            .fetch_one(&state.pool)
            .await?
        } else {
            u
        }
    } else {
        sqlx::query_as::<_, crate::db::UserRow>(&format!(
            r#"
            INSERT INTO app_user (open_id, nickname, avatar)
            VALUES ($1, $2, $3)
            RETURNING {USER_COLS}
            "#
        ))
        .bind(&open_id)
        .bind(&nickname)
        .bind(&avatar)
        .fetch_one(&state.pool)
        .await?
    };

    if crate::sample::should_grant_sample(&open_id) {
        if let Err(e) = crate::sample::ensure_sample_travel(&state.pool, user.id).await {
            tracing::warn!("写入示例攻略失败: {e}");
        }
    }

    let token = make_token(user.id, &state.jwt_secret)?;
    Ok(ok(LoginVo {
        token,
        user: user_vo(&user),
    }))
}

async fn resolve_open_id(state: &AppState, req: &LoginReq) -> Result<String, AppError> {
    if state.dev_mode {
        if let Some(id) = req
            .open_id
            .clone()
            .filter(|s| s.starts_with("demo_"))
        {
            return Ok(id);
        }
    }

    if wechat_enabled(state) {
        let code = req
            .code
            .as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| AppError::BadRequest("缺少微信授权 code".into()))?;
        let url = format!(
            "https://api.weixin.qq.com/sns/jscode2session?appid={}&secret={}&js_code={}&grant_type=authorization_code",
            state.wechat_appid, state.wechat_secret, code
        );
        let wx: WxSession = reqwest::get(&url)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .json()
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(err) = wx.errcode {
            if err != 0 {
                return Err(AppError::BadRequest(
                    wx.errmsg.unwrap_or_else(|| format!("微信登录失败 {err}")),
                ));
            }
        }
        return wx
            .openid
            .ok_or_else(|| AppError::BadRequest("微信未返回 openid".into()));
    }

    if !state.dev_mode {
        return Err(AppError::BadRequest(
            "未配置 WECHAT_APPID / WECHAT_SECRET，无法用微信身份识别用户".into(),
        ));
    }

    if let Some(id) = req.open_id.clone().filter(|s| !s.is_empty()) {
        return Ok(id);
    }
    if let Some(name) = req.nickname.clone().filter(|s| !s.trim().is_empty()) {
        return Ok(format!("dev_{name}"));
    }
    if let Some(code) = req.code.clone().filter(|s| !s.is_empty()) {
        return Ok(format!("wx_{code}"));
    }
    Err(AppError::BadRequest("请提供登录信息".into()))
}

pub async fn info(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<ApiOk<UserVo>>, AppError> {
    let u = find_user(&state.pool, user.id).await?;
    Ok(ok(user_vo(&u)))
}

pub async fn update(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<UpdateUserReq>,
) -> Result<Json<ApiOk<UserVo>>, AppError> {
    if let Some(n) = &req.nickname {
        if n.trim().is_empty() || n.chars().count() > 50 {
            return Err(AppError::BadRequest("昵称不合法".into()));
        }
    }
    let birthday = parse_birthday(req.birthday.as_deref())?;
    if let Some(g) = req.gender {
        if g < 0 || g > 2 {
            return Err(AppError::BadRequest("性别不合法".into()));
        }
    }
    if let Some(r) = req.female_role {
        if r < 0 || r > 1 {
            return Err(AppError::BadRequest("岗位类型不合法".into()));
        }
    }
    let this_year = chrono::Local::now().year();
    if let Some(y) = req.work_start_year {
        if y < 1960 || y > this_year {
            return Err(AppError::BadRequest("参加工作年份不合法".into()));
        }
    }
    let sql = format!(
        r#"
        UPDATE app_user
        SET nickname = COALESCE($2::varchar, nickname),
            avatar = COALESCE($3::varchar, avatar),
            default_bill_visible = COALESCE($4::boolean, default_bill_visible),
            birthday = COALESCE($5::date, birthday),
            gender = COALESCE($6::smallint, gender),
            female_role = COALESCE($7::smallint, female_role),
            work_start_year = COALESCE($8::int, work_start_year)
        WHERE id = $1
        RETURNING {USER_COLS}
        "#
    );
    let u = sqlx::query_as::<_, crate::db::UserRow>(&sql)
        .bind(user.id)
        .bind(req.nickname.as_deref())
        .bind(req.avatar.as_deref())
        .bind(req.default_bill_visible)
        .bind(birthday)
        .bind(req.gender)
        .bind(req.female_role)
        .bind(req.work_start_year)
        .fetch_one(&state.pool)
        .await?;
    Ok(ok(user_vo(&u)))
}

fn parse_birthday(raw: Option<&str>) -> Result<Option<NaiveDate>, AppError> {
    let Some(s) = raw.map(str::trim).filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("出生日期不合法".into()))?;
    let today = chrono::Local::now().date_naive();
    if date.year() < 1920 || date > today {
        return Err(AppError::BadRequest("出生日期不合法".into()));
    }
    Ok(Some(date))
}
