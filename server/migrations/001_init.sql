-- 结伴出行 完整建表。删库后启动服务会自动执行。

CREATE TABLE IF NOT EXISTS app_user (
    id              BIGSERIAL PRIMARY KEY,
    open_id         VARCHAR(100) UNIQUE NOT NULL,
    nickname        VARCHAR(50) NOT NULL,
    avatar          VARCHAR(255),
    default_bill_visible BOOLEAN NOT NULL DEFAULT FALSE,
    create_time     TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS travel (
    id              BIGSERIAL PRIMARY KEY,
    travel_name     VARCHAR(100) NOT NULL,
    destination     VARCHAR(100) NOT NULL,
    start_date      DATE NOT NULL,
    end_date        DATE NOT NULL,
    invite_code     VARCHAR(20) UNIQUE NOT NULL,
    status          SMALLINT NOT NULL DEFAULT 0,
    creator_id      BIGINT NOT NULL REFERENCES app_user(id),
    is_lock         BOOLEAN NOT NULL DEFAULT FALSE,
    remark          TEXT,
    create_time     TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS travel_member (
    id              BIGSERIAL PRIMARY KEY,
    travel_id       BIGINT NOT NULL REFERENCES travel(id) ON DELETE CASCADE,
    user_id         BIGINT NOT NULL REFERENCES app_user(id),
    role            SMALLINT NOT NULL DEFAULT 0,
    can_edit        BOOLEAN NOT NULL DEFAULT FALSE,
    can_bill        BOOLEAN NOT NULL DEFAULT FALSE,
    -- 结算团体名（如同「我这边」）；空则按个人计
    group_name      VARCHAR(50),
    join_time       TIMESTAMP NOT NULL DEFAULT NOW(),
    UNIQUE (travel_id, user_id)
);

CREATE TABLE IF NOT EXISTS day_plan (
    id              BIGSERIAL PRIMARY KEY,
    travel_id       BIGINT NOT NULL REFERENCES travel(id) ON DELETE CASCADE,
    day_num         INT NOT NULL,
    point_type      VARCHAR(20) NOT NULL,
    place_name      VARCHAR(100) NOT NULL,
    longitude       NUMERIC(12,6),
    latitude        NUMERIC(12,6),
    arrive_time     TIME,
    leave_time      TIME,
    stay_duration   INT,
    traffic_type    VARCHAR(20),
    traffic_duration INT,
    sort            INT NOT NULL DEFAULT 0,
    remark          TEXT
);

CREATE TABLE IF NOT EXISTS bill (
    id              BIGSERIAL PRIMARY KEY,
    travel_id       BIGINT NOT NULL REFERENCES travel(id) ON DELETE CASCADE,
    day_plan_id     BIGINT REFERENCES day_plan(id) ON DELETE SET NULL,
    bill_name       VARCHAR(100) NOT NULL,
    amount          NUMERIC(10,2) NOT NULL,
    bill_type       SMALLINT NOT NULL,
    cost_type       VARCHAR(20) NOT NULL,
    pay_user_id     BIGINT NOT NULL REFERENCES app_user(id),
    consume_time    TIMESTAMP NOT NULL,
    visible_all     BOOLEAN NOT NULL DEFAULT FALSE,
    remark          TEXT,
    create_time     TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS bill_share (
    id              BIGSERIAL PRIMARY KEY,
    bill_id         BIGINT NOT NULL REFERENCES bill(id) ON DELETE CASCADE,
    user_id         BIGINT NOT NULL REFERENCES app_user(id),
    share_amount    NUMERIC(10,2) NOT NULL,
    UNIQUE (bill_id, user_id)
);

CREATE TABLE IF NOT EXISTS route_cache (
    from_plan_id    BIGINT NOT NULL REFERENCES day_plan(id) ON DELETE CASCADE,
    to_plan_id      BIGINT NOT NULL REFERENCES day_plan(id) ON DELETE CASCADE,
    traffic_type    VARCHAR(20),
    from_lat        DOUBLE PRECISION NOT NULL,
    from_lng        DOUBLE PRECISION NOT NULL,
    to_lat          DOUBLE PRECISION NOT NULL,
    to_lng          DOUBLE PRECISION NOT NULL,
    mode            VARCHAR(20) NOT NULL,
    from_nav        BOOLEAN NOT NULL DEFAULT FALSE,
    distance_m      INTEGER NOT NULL DEFAULT 0,
    duration_s      INTEGER NOT NULL DEFAULT 0,
    provider        VARCHAR(20) NOT NULL DEFAULT 'amap',
    points          JSONB NOT NULL,
    updated_at      TIMESTAMP NOT NULL DEFAULT NOW(),
    PRIMARY KEY (from_plan_id, to_plan_id)
);

CREATE INDEX IF NOT EXISTS idx_member_user ON travel_member(user_id);
CREATE INDEX IF NOT EXISTS idx_plan_travel_day ON day_plan(travel_id, day_num, sort);
CREATE INDEX IF NOT EXISTS idx_bill_travel ON bill(travel_id);
CREATE INDEX IF NOT EXISTS idx_share_bill ON bill_share(bill_id);
CREATE INDEX IF NOT EXISTS idx_route_cache_to ON route_cache(to_plan_id);
