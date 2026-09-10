use std::sync::Arc;

use sea_orm::DatabaseConnection;
use sea_orm_migration::MigratorTrait;
use tokio::time::{interval, Duration};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use vibeyeah_core::config::Config;
use vibeyeah_core::migration::Migrator;
use vibeyeah_core::{core_router, service, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 多个依赖同时引入了 ring 和 aws-lc-rs，需要显式指定 rustls 使用的 crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("安装 rustls CryptoProvider 失败");

    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vibeyeah_core=debug,tower_http=info".into()),
        )
        .init();

    // 仅 DATABASE_URL / BIND_ADDR 来自环境变量；其余配置在迁移后从 settings 表加载
    let mut config = Config::bootstrap()?;

    // SQLite：文件不存在则先创建（sqlx 不自动创建缺失文件）
    if config.database_url.starts_with("sqlite://") {
        ensure_sqlite_file(&config.database_url)?;
    }
    let db = sea_orm::Database::connect(&config.database_url).await?;
    Migrator::up(&db, None).await?;
    config.load_from_db(&db).await?;
    tracing::info!("数据库迁移完成，配置已从 settings 表加载");

    // 库中尚无任何组织时：交互引导创建首个组织并绑定飞书 Bot
    maybe_bootstrap_org(&db, &config.k8s_namespace).await;

    // 全局飞书登录应用（控制台「飞书登录」用，settings 表）未配置时不阻塞启动
    if config.lark_app_id.trim().is_empty() || config.lark_app_secret.trim().is_empty() {
        tracing::info!(
            "settings 表未配置全局飞书 OAuth 应用（lark_app_id/lark_app_secret），控制台「飞书登录」不可用；如需启用请写入后重启"
        );
    }

    // 自动从 in-cluster config 或 ~/.kube/config 初始化
    let k8s_client = kube::Client::try_default()
        .await
        .expect("无法初始化 K8s client，请确认 kubeconfig 或 in-cluster 权限");

    let state = AppState {
        db: db.clone(),
        config: config.clone(),
        k8s_client: k8s_client.clone(),
    };

    // 启动定时 Pod 状态同步任务
    spawn_pod_sync_task(
        db.clone(),
        k8s_client.clone(),
        config.pod_sync_interval_secs,
    );

    // 启动飞书 WebSocket 长连接 Bot（遍历所有配置了飞书应用的组织）
    service::lark_bot::spawn_all_lark_bots(db.clone(), k8s_client.clone(), config.clone()).await;

    let app = core_router(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&config.bind_addr).await?;
    tracing::info!("VibeYeah (开源版) 监听 {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}

/// 在后台 tokio 任务中定时同步所有 Pod 状态
fn spawn_pod_sync_task(db: DatabaseConnection, k8s_client: kube::Client, interval_secs: u64) {
    let db = Arc::new(db);
    let k8s_client = Arc::new(k8s_client);

    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            tracing::debug!("触发 Pod 状态同步");
            if let Err(e) = service::k8s::sync_all_pod_statuses(&db, &k8s_client).await {
                tracing::error!("Pod 状态同步失败: {}", e);
            }
        }
    });
}

/// 为 `sqlite://<file>` 连接确保数据库文件存在（sqlx 默认不创建缺失文件）。
/// `:memory:` 等非文件形式直接返回。
fn ensure_sqlite_file(url: &str) -> anyhow::Result<()> {
    let raw = url.trim_start_matches("sqlite://");
    let path = raw.split('?').next().unwrap_or(raw);
    if path.is_empty() || path == ":memory:" {
        return Ok(());
    }
    if !std::path::Path::new(path).exists() {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(path)
            .map_err(|e| anyhow::anyhow!("创建 SQLite 文件 {} 失败: {}", path, e))?;
    }
    Ok(())
}

/// 库中尚无组织时的交互初始化向导。
///
/// 数据库 organizations 表为空时（全新部署，已去掉自动 seed 的默认组织）：
///   1. 询问是否创建首个组织；
///   2. 输入组织显示名称（默认 "default"）；
///   3. 复用 lark_qr 打印 ASCII 二维码，为组织注册/绑定飞书应用作为 Bot 入口
///      （成员可直接对它发送 /add 创建智能体）；可按回车跳过。
/// 仅当 stdin 是终端（交互式运行）且组织数为 0 时才进入；非交互只打日志。
async fn maybe_bootstrap_org(db: &sea_orm::DatabaseConnection, k8s_namespace: &str) {
    use std::io::IsTerminal;

    let count = match service::organization::count_orgs(db).await {
        Ok(n) => n,
        Err(e) => {
            tracing::error!("查询组织数量失败，跳过初始化向导: {e}");
            return;
        }
    };
    if count > 0 {
        return; // 已有组织，无需初始化
    }

    if !std::io::stdin().is_terminal() {
        tracing::warn!(
            "数据库中还没有任何组织。当前为非交互环境，跳过初始化向导；\
             请登录控制台后创建首个组织，或带终端重启后端以运行向导"
        );
        return;
    }

    println!("\n[初始化] 数据库中还没有任何组织。");
    let ans = read_line_prompt("是否现在创建首个组织，并为它绑定飞书应用作为 Bot 入口？[Y/n] ")
        .await
        .unwrap_or_default();
    let a = ans.trim().to_ascii_lowercase();
    if !(a.is_empty() || a == "y" || a == "yes") {
        println!("已跳过。可稍后登录控制台创建组织；重启后仍可再次运行本向导。");
        return;
    }

    let raw = read_line_prompt("请输入组织显示名称 [default]: ")
        .await
        .unwrap_or_default();
    let name = if raw.trim().is_empty() {
        "default".to_string()
    } else {
        raw.trim().to_string()
    };
    let slug = slugify_org(&name);

    let org = match service::organization::create_initial_org(db, &name, &slug, k8s_namespace).await
    {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("创建首个组织失败: {e}");
            return;
        }
    };
    println!(
        "✅ 已创建首个组织：{}（slug={}，id={}）。",
        org.name, org.slug, org.id
    );

    bind_org_lark_app(db, org.id).await;
}

/// 为组织绑定飞书应用（组织的 Bot 入口）：复用 lark_qr 扫码注册，
/// 成功后把 app_id / app_secret 写入 organizations 表。可按回车跳过。
async fn bind_org_lark_app(db: &sea_orm::DatabaseConnection, org_id: uuid::Uuid) {
    let base = service::lark_qr::feishu_accounts_base_url().to_string();

    if service::lark_qr::init(&base).await.is_err() {
        tracing::warn!("无法连接飞书注册服务，跳过组织飞书应用绑定");
        println!(
            "⚠️ 无法连接飞书注册服务，未绑定飞书应用。之后可在飞书开放平台创建应用后，\
             把 App ID / Secret 写入 organizations.lark_app_id / lark_app_secret 并重启。"
        );
        return;
    }
    let begin = match service::lark_qr::begin(&base).await {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("获取飞书注册二维码失败: {e}");
            return;
        }
    };

    println!("\n[绑定] 正在为组织注册飞书应用（Bot 入口）。请用飞书扫描下方二维码注册/选择应用：");
    println!("{}", ascii_qr(&begin.qr_url));
    println!("无法扫码？请直接打开链接：{}", begin.qr_url);
    println!("注册完成后将自动写入本组织并继续；或输入任意内容后回车跳过绑定。\n");

    // 后台轮询扫码结果
    let (tx, mut rx) = tokio::sync::oneshot::channel::<
        std::result::Result<Option<service::lark_qr::PollResult>, anyhow::Error>,
    >();
    let poll_handle = {
        let base = base.clone();
        let device_code = begin.device_code.clone();
        tokio::spawn(async move {
            let res = service::lark_qr::poll(
                &base,
                &device_code,
                begin.interval_secs,
                begin.expire_in_secs,
            )
            .await;
            let _ = tx.send(res);
        })
    };

    // 任意键（回车）跳过
    let (ktx, mut krx) = tokio::sync::oneshot::channel::<()>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        let mut line = String::new();
        let _ = stdin.lock().read_line(&mut line);
        let _ = ktx.send(());
    });

    tokio::select! {
        _ = &mut krx => {
            poll_handle.abort();
            println!("已跳过飞书应用绑定。可在飞书开放平台创建应用后，把 App ID/Secret 写入 organizations 表并重启。");
        }
        res = &mut rx => match res {
            Ok(Ok(Some(p))) => {
                match service::organization::set_lark_app(db, org_id, &p.app_id, &p.app_secret).await {
                    Ok(_) => {
                        if let Some(bot) =
                            service::lark_qr::probe_bot(&p.app_id, &p.app_secret).await
                        {
                            tracing::info!(
                                "组织 Bot 探测成功: open_id={} name={}",
                                bot.open_id,
                                bot.name
                            );
                        }
                        if let Some(ref user_open_id) = p.user_open_id {
                            let owner_id: uuid::Uuid =
                                service::organization::DEFAULT_OWNER_ID.parse().unwrap();
                            match service::organization::set_member_lark_open_id(
                                db, org_id, owner_id, user_open_id,
                            )
                            .await
                            {
                                Ok(_) => tracing::info!(
                                    "已写入组织所有者的 open_id: {}",
                                    user_open_id
                                ),
                                Err(e) => tracing::warn!("写入所有者 open_id 失败: {e}"),
                            }
                        }
                        println!(
                            "✅ 组织飞书应用绑定成功（App ID: {}），成员现在可以向它发送 /add 创建智能体。",
                            p.app_id
                        );
                    }
                    Err(e) => tracing::warn!("写入组织飞书应用失败: {e}"),
                }
            }
            Ok(Ok(None)) => println!("注册超时或已被拒绝，未绑定。"),
            Ok(Err(e)) => tracing::warn!("飞书注册轮询失败: {e}"),
            Err(_) => {}
        }
    }
}

/// 终端交互读取一行（带提示）；非终端或读取失败返回 None。
async fn read_line_prompt(prompt: &str) -> Option<String> {
    use std::io::{BufRead, IsTerminal, Write};

    if !std::io::stdin().is_terminal() {
        return None;
    }
    if !prompt.is_empty() {
        print!("{prompt}");
        let _ = std::io::stdout().flush();
    }
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut line = String::new();
        let _ = stdin.lock().read_line(&mut line);
        let _ = tx.send(line);
    });
    rx.await.ok()
}

/// 生成终端 ASCII 二维码（Unicode 半块字符，可直接扫码）。
fn ascii_qr(url: &str) -> String {
    use image::Luma;
    use qrcode::{EcLevel, QrCode};

    let code = match QrCode::with_error_correction_level(url, EcLevel::M) {
        Ok(c) => c,
        Err(e) => return format!("(生成二维码失败: {e})"),
    };
    let img = code
        .render::<Luma<u8>>()
        .module_dimensions(1, 1)
        .quiet_zone(true)
        .build();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let dark = |x: usize, y: usize| img.get_pixel(x as u32, y as u32).0[0] < 128;

    let mut out = String::new();
    let mut y = 0;
    while y < h {
        let mut line = String::new();
        for x in 0..w {
            let top = dark(x, y);
            let bot = y + 1 < h && dark(x, y + 1);
            line.push(match (top, bot) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push_str(line.trim_end());
        out.push('\n');
        y += 2;
    }
    out
}

/// 组织 slug：仅保留 ASCII 字母/数字并转小写，其余收敛为 '-'，去首尾 '-'；空则回退 "org"。
fn slugify_org(name: &str) -> String {
    let mut prev_dash = true;
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.extend(ch.to_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "org".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue::Set};
    use sea_orm_migration::MigratorTrait;

    use vibeyeah_core::config::Config;
    use vibeyeah_core::entity::setting;
    use vibeyeah_core::migration::Migrator;

    use crate::{ascii_qr, slugify_org};

    /// 验证 SQLite：DATABASE_URL 为空时使用默认 SQLite 文件，全量迁移 + 从 settings 加载配置。
    #[tokio::test]
    async fn sqlite_migrations_and_config_load() {
        let file = format!("vibeyeah-sqlite-test-{}.db", uuid::Uuid::new_v4());
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file)
            .expect("创建 SQLite 文件失败");
        let db = sea_orm::Database::connect(&format!("sqlite://{}", file))
            .await
            .expect("连接 SQLite 失败");
        Migrator::up(&db, None).await.expect("SQLite 迁移失败");

        let row = setting::ActiveModel {
            key: Set("jwt_secret".to_string()),
            value: Set("test-secret".to_string()),
            ..Default::default()
        };
        row.update(&db)
            .await
            .expect("更新 settings.jwt_secret 失败");

        let mut config = Config::bootstrap().expect("bootstrap 失败");
        config
            .load_from_db(&db)
            .await
            .expect("从 settings 加载配置失败");
        assert_eq!(config.jwt_secret, "test-secret");

        let _ = std::fs::remove_file(&file);
    }

    /// 全新库迁移后不应再有自动 seed 的默认组织（由启动向导接管）；
    /// 向导创建的首个组织沿用固定 id、无需创建者/成员，并可绑定飞书应用。
    #[tokio::test]
    async fn fresh_db_empty_org_and_initial_org_bootstrap() {
        use sea_orm::EntityTrait;
        use vibeyeah_core::entity::{org_member, organization as org_entity, user};
        use vibeyeah_core::service::organization::{self, DEFAULT_ORG_ID, DEFAULT_OWNER_ID};

        let file = format!("vibeyeah-org-test-{}.db", uuid::Uuid::new_v4());
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file)
            .expect("创建 SQLite 文件失败");
        let db = sea_orm::Database::connect(&format!("sqlite://{}", file))
            .await
            .expect("连接 SQLite 失败");
        Migrator::up(&db, None).await.expect("SQLite 迁移失败");

        assert_eq!(
            organization::count_orgs(&db).await.expect("统计组织失败"),
            0,
            "全新库不应再有自动 seed 的默认组织"
        );

        let org = organization::create_initial_org(&db, "default", "default", "default")
            .await
            .expect("创建首个组织失败");
        assert_eq!(org.id.to_string(), DEFAULT_ORG_ID);

        use sea_orm::{ColumnTrait, QueryFilter};
        let owner_id: uuid::Uuid = DEFAULT_OWNER_ID.parse().unwrap();
        let owner = user::Entity::find_by_id(owner_id)
            .one(&db)
            .await
            .expect("查询所有者失败")
            .expect("所有者用户应存在");
        assert_eq!(owner.nickname.as_deref(), Some("admin"));
        assert_eq!(org.created_by, Some(owner_id));
        let member = org_member::Entity::find()
            .filter(org_member::Column::OrgId.eq(org.id))
            .filter(org_member::Column::UserId.eq(owner_id))
            .one(&db)
            .await
            .expect("查询成员失败")
            .expect("所有者应为组织成员");
        assert!(matches!(member.role, org_member::OrgRole::Owner));

        organization::set_member_lark_open_id(&db, org.id, owner_id, "ou_owner_open_id")
            .await
            .expect("写入所有者 open_id 失败");
        let member = org_member::Entity::find()
            .filter(org_member::Column::OrgId.eq(org.id))
            .filter(org_member::Column::UserId.eq(owner_id))
            .one(&db)
            .await
            .expect("查询成员失败")
            .expect("所有者应为组织成员");
        assert_eq!(member.lark_open_id.as_deref(), Some("ou_owner_open_id"));

        organization::set_lark_app(&db, org.id, "cli_xxx", "secret_yyy")
            .await
            .expect("写入组织飞书应用失败");
        let reloaded = org_entity::Entity::find_by_id(org.id)
            .one(&db)
            .await
            .expect("查询组织失败")
            .expect("组织应存在");
        assert_eq!(reloaded.lark_app_id.as_deref(), Some("cli_xxx"));
        assert_eq!(reloaded.lark_app_secret.as_deref(), Some("secret_yyy"));

        let _ = std::fs::remove_file(&file);
    }

    /// set_org_property：支持 openai_* 白名单，未知 key 报错，空值清除。
    #[tokio::test]
    async fn set_org_property_updates_openai_fields() {
        use sea_orm::EntityTrait;
        use vibeyeah_core::entity::organization as org_entity;
        use vibeyeah_core::service::organization::{self, DEFAULT_ORG_ID};

        let file = format!("vibeyeah-setprop-test-{}.db", uuid::Uuid::new_v4());
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&file)
            .expect("创建 SQLite 文件失败");
        let db = sea_orm::Database::connect(&format!("sqlite://{}", file))
            .await
            .expect("连接 SQLite 失败");
        Migrator::up(&db, None).await.expect("SQLite 迁移失败");

        let org = organization::create_initial_org(&db, "default", "default", "default")
            .await
            .expect("创建组织失败");
        let org_id: uuid::Uuid = DEFAULT_ORG_ID.parse().unwrap();
        assert_eq!(org.id, org_id);

        organization::set_org_property(&db, org_id, "openai_base_url", "https://x/v1")
            .await
            .expect("设置 openai_base_url 失败");
        organization::set_org_property(&db, org_id, "openai_api_key", "sk-abc")
            .await
            .expect("设置 openai_api_key 失败");
        organization::set_org_property(&db, org_id, "openai_model", "gpt-4o-mini")
            .await
            .expect("设置 openai_model 失败");

        let reloaded = org_entity::Entity::find_by_id(org_id)
            .one(&db)
            .await
            .expect("查询组织失败")
            .expect("组织应存在");
        assert_eq!(reloaded.openai_base_url.as_deref(), Some("https://x/v1"));
        assert_eq!(reloaded.openai_api_key.as_deref(), Some("sk-abc"));
        assert_eq!(reloaded.openai_model.as_deref(), Some("gpt-4o-mini"));

        assert!(organization::set_org_property(&db, org_id, "nope", "x")
            .await
            .is_err());
        organization::set_org_property(&db, org_id, "openai_api_key", "")
            .await
            .expect("清除 openai_api_key 失败");
        let reloaded = org_entity::Entity::find_by_id(org_id)
            .one(&db)
            .await
            .expect("查询组织失败")
            .expect("组织应存在");
        assert!(reloaded.openai_api_key.is_none());

        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn slugify_org_normalizes() {
        let s = slugify_org;
        assert_eq!(s("Default"), "default");
        assert_eq!(s("  My  Org! "), "my-org");
        assert_eq!(s("VibeYeah"), "vibeyeah");
        assert_eq!(s("某某科技"), "org");
    }

    #[test]
    fn ascii_qr_renders_blocks() {
        let out = ascii_qr("https://open.feishu.cn/app");
        assert!(!out.is_empty(), "ASCII QR 不应为空");
        assert!(
            out.contains('█') || out.contains('▀') || out.contains('▄'),
            "ASCII QR 应包含实心/半块字符"
        );
    }
}
