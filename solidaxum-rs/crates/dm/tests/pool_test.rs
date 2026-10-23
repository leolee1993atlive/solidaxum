//! 需要真实达梦。`cargo test -p dm --test pool_test -- --ignored`

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn connection_survives_across_multiple_execs() {
    let pool = dm::pool::DmPool::new(
        std::env::var("DM_ODBC").unwrap(),
        4,
        std::time::Duration::from_secs(30),
        None,
    );
    // 拿到拥有式连接,跨三次调用使用 —— 事务就是这么用的
    let mut conn = pool.acquire().await.expect("acquire 失败");
    for _ in 0..3 {
        conn.exec(|c| {
            c.execute("SELECT 1", (), None)?;
            Ok(())
        })
        .await
        .expect("exec 失败");
    }
}

/// 配置了 schema 后,裸表名必须在该 schema 下解析。
///
/// 达梦是单实例单库,MySQL 的 database 对应它的 schema,而 schema 即用户 ——
/// 连接后的默认 schema 是登录账号自己的那个。装表账号与跑应用的账号不同一个时
/// (交付常态:建 `RUOYI` 用户装表、另一账号跑应用),不指定 schema 就报
/// `无效的表或视图名[sys_user]`(SQLSTATE 42S02)。
///
/// 本测试依赖实例上存在 `DM_SCHEMA` 指向的 schema 且其中有 `sys_user`
/// —— 即 `sql/dm/solidaxum_init.sql` 装过的那个库。未设 `DM_SCHEMA` 时跳过。
#[tokio::test]
#[ignore = "需要真实达梦"]
async fn resolves_bare_table_names_in_configured_schema() {
    let Ok(schema) = std::env::var("DM_SCHEMA") else {
        eprintln!("未设 DM_SCHEMA,跳过");
        return;
    };
    let conn_str = std::env::var("DM_ODBC").unwrap();

    // 不指定 schema:落在登录用户自己的 schema,裸表名解析不到
    let bare = dm::pool::DmPool::new(
        conn_str.clone(),
        1,
        std::time::Duration::from_secs(30),
        None,
    );
    let mut c = bare.acquire().await.unwrap();
    let without = c
        .exec(|c| {
            c.execute("SELECT COUNT(*) FROM sys_user", (), None)?;
            Ok(())
        })
        .await;
    assert!(
        without.is_err(),
        "未指定 schema 时本应解析不到 sys_user —— 若这里通过,说明登录账号恰好就是装表账号,\
         该用例失去意义,请换一个账号或 schema 再验"
    );

    // 指定 schema:同样的裸表名应当解析成功
    let scoped = dm::pool::DmPool::new(
        conn_str,
        1,
        std::time::Duration::from_secs(30),
        Some(schema.clone()),
    );
    let mut c = scoped.acquire().await.unwrap();
    c.exec(|c| {
        c.execute("SELECT COUNT(*) FROM sys_user", (), None)?;
        Ok(())
    })
    .await
    .unwrap_or_else(|e| {
        panic!("指定 schema={schema} 后仍解析不到 sys_user: {e}")
    });
}

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn returns_connection_to_pool_on_drop() {
    let pool = dm::pool::DmPool::new(
        std::env::var("DM_ODBC").unwrap(),
        1,
        std::time::Duration::from_secs(30),
        None,
    );
    // 池容量为 1:若 drop 未归还,第二次 acquire 会永久阻塞
    {
        let mut c = pool.acquire().await.unwrap();
        c.exec(|c| {
            c.execute("SELECT 1", (), None)?;
            Ok(())
        })
        .await
        .unwrap();
    }
    let second =
        tokio::time::timeout(std::time::Duration::from_secs(5), pool.acquire())
            .await;
    assert!(second.is_ok(), "drop 后连接未归还池");
}

#[tokio::test]
#[ignore = "需要真实达梦"]
async fn runs_queries_concurrently() {
    let pool = dm::pool::DmPool::new(
        std::env::var("DM_ODBC").unwrap(),
        4,
        std::time::Duration::from_secs(30),
        None,
    );
    let mut handles = Vec::new();
    for _ in 0..8 {
        let p = pool.clone();
        handles.push(tokio::spawn(async move {
            let mut c = p.acquire().await?;
            c.exec(|c| {
                c.execute("SELECT 1", (), None)?;
                Ok(())
            })
            .await
        }));
    }
    for h in handles {
        h.await.unwrap().expect("并发查询失败");
    }
}
