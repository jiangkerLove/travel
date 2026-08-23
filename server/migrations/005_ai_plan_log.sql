CREATE TABLE IF NOT EXISTS ai_plan_log (
    id                  BIGSERIAL PRIMARY KEY,
    travel_id           BIGINT NOT NULL REFERENCES travel(id) ON DELETE CASCADE,
    user_id             BIGINT NOT NULL REFERENCES app_user(id),
    mode                VARCHAR(20) NOT NULL DEFAULT 'plan',
    fresh               BOOLEAN NOT NULL DEFAULT FALSE,
    day_num             INT,
    user_prompt         TEXT NOT NULL DEFAULT '',
    request_json        JSONB NOT NULL,
    model_response_raw  TEXT,
    result_json         JSONB,
    error_message       TEXT,
    duration_ms         INT,
    created_at          TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_ai_plan_log_travel_created
    ON ai_plan_log(travel_id, created_at DESC);
