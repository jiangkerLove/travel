-- 入库时间（NOW() 写入的 TIMESTAMP）改为北京时间。
-- bill.consume_time 是用户填写的消费时间，不在此列。

DO $$
DECLARE
  tz text;
  delta interval;
BEGIN
  -- reset_val 是库/服务器默认时区，不受本次会话 SET TIME ZONE 影响。
  -- 历史 NOW() 就是按这个时区写成的钟面时间。
  SELECT reset_val INTO tz FROM pg_settings WHERE name = 'TimeZone';

  EXECUTE format(
    'SELECT (CURRENT_TIMESTAMP AT TIME ZONE %L) - (CURRENT_TIMESTAMP AT TIME ZONE %L)',
    'Asia/Shanghai',
    tz
  ) INTO delta;

  IF delta <> INTERVAL '0' THEN
    RAISE NOTICE '数据库默认时区为 %，将入库时间校正 %', tz, delta;
    UPDATE app_user SET create_time = create_time + delta;
    UPDATE travel SET create_time = create_time + delta;
    UPDATE travel_member SET join_time = join_time + delta;
    UPDATE bill SET create_time = create_time + delta;
    UPDATE route_cache SET updated_at = updated_at + delta;
    UPDATE ai_plan_log SET created_at = created_at + delta;
  END IF;

  BEGIN
    EXECUTE format(
      'ALTER DATABASE %I SET timezone TO %L',
      current_database(),
      'Asia/Shanghai'
    );
  EXCEPTION
    WHEN OTHERS THEN
      RAISE NOTICE '未能修改数据库默认时区，应用连接仍使用 Asia/Shanghai: %', SQLERRM;
  END;
END $$;
