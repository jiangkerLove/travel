use serde::Serialize;
use serde_json::Value;
use sqlx::PgPool;

#[derive(Debug)]
pub struct AiDraftLogCtx<'a> {
    pub pool: &'a PgPool,
    pub travel_id: i64,
    pub user_id: i64,
    pub mode: &'a str,
    pub fresh: bool,
    pub day_num: Option<i32>,
    pub user_prompt: String,
}

#[derive(Debug)]
pub struct AiPlanLogInsert {
    pub travel_id: i64,
    pub user_id: i64,
    pub mode: String,
    pub fresh: bool,
    pub day_num: Option<i32>,
    pub user_prompt: String,
    pub request_json: Value,
    pub model_response_raw: Option<String>,
    pub result_json: Option<Value>,
    pub error_message: Option<String>,
    pub duration_ms: i32,
}

pub async fn insert_ai_plan_log(pool: &PgPool, row: AiPlanLogInsert) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO ai_plan_log (
            travel_id, user_id, mode, fresh, day_num, user_prompt,
            request_json, model_response_raw, result_json, error_message, duration_ms
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)"#,
    )
    .bind(row.travel_id)
    .bind(row.user_id)
    .bind(row.mode)
    .bind(row.fresh)
    .bind(row.day_num)
    .bind(row.user_prompt)
    .bind(row.request_json)
    .bind(row.model_response_raw)
    .bind(row.result_json)
    .bind(row.error_message)
    .bind(row.duration_ms)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Serialize, sqlx::FromRow)]
pub struct AiPlanLogVo {
    pub id: i64,
    pub travel_id: i64,
    pub mode: String,
    pub fresh: bool,
    pub day_num: Option<i32>,
    pub user_prompt: String,
    pub request_json: Value,
    pub model_response_raw: Option<String>,
    pub result_json: Option<Value>,
    pub error_message: Option<String>,
    pub duration_ms: Option<i32>,
    pub created_at: chrono::NaiveDateTime,
}

pub async fn list_ai_plan_logs(
    pool: &PgPool,
    travel_id: i64,
    limit: i64,
) -> Result<Vec<AiPlanLogVo>, sqlx::Error> {
    sqlx::query_as::<_, AiPlanLogVo>(
        r#"SELECT id, travel_id, mode, fresh, day_num, user_prompt,
                  request_json, model_response_raw, result_json, error_message,
                  duration_ms, created_at
           FROM ai_plan_log
           WHERE travel_id = $1
           ORDER BY created_at DESC
           LIMIT $2"#,
    )
    .bind(travel_id)
    .bind(limit)
    .fetch_all(pool)
    .await
}
