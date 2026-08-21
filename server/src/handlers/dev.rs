use axum::{extract::State, Json};

use crate::{
    error::{ok, ApiOk, AppError},
    sample,
    state::AppState,
};

#[derive(serde::Serialize)]
pub struct SeedVo {
    pub invite_code: String,
    pub users: Vec<SeedUser>,
}

#[derive(serde::Serialize)]
pub struct SeedUser {
    pub open_id: String,
    pub nickname: String,
}

pub async fn seed(State(state): State<AppState>) -> Result<Json<ApiOk<SeedVo>>, AppError> {
    if !state.dev_mode {
        return Err(AppError::Forbidden("未开启开发模式".into()));
    }

    let users = [
        ("demo_deer", "小鹿"),
        ("demo_wei", "阿伟"),
        ("demo_lin", "小林"),
    ];
    let mut ids = Vec::new();
    for (open_id, nickname) in users {
        let id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO app_user (open_id, nickname)
            VALUES ($1, $2)
            ON CONFLICT (open_id) DO UPDATE SET nickname = EXCLUDED.nickname
            RETURNING id
            "#,
        )
        .bind(open_id)
        .bind(nickname)
        .fetch_one(&state.pool)
        .await?;
        ids.push(id);
    }

    let invite = "DEMO88";
    let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM travel WHERE invite_code=$1")
        .bind(invite)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        let end = chrono::Local::now().date_naive() - chrono::Duration::days(7);
        let start = end - chrono::Duration::days(3);
        sample::create_sample_travel_with_dates(
            &state.pool,
            ids[0],
            Some([ids[1], ids[2]]),
            Some(invite),
            start,
            end,
        )
        .await?;
    }

    Ok(ok(SeedVo {
        invite_code: invite.into(),
        users: vec![
            SeedUser {
                open_id: "demo_deer".into(),
                nickname: "小鹿".into(),
            },
            SeedUser {
                open_id: "demo_wei".into(),
                nickname: "阿伟".into(),
            },
            SeedUser {
                open_id: "demo_lin".into(),
                nickname: "小林".into(),
            },
        ],
    }))
}
