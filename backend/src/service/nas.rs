/// 组织级 NAS 根目录。
///
/// 根目录存于 `organizations.nas_mount_root`（创建组织时按后端进程 cwd 生成
/// `<cwd>/data/nas`），布局与 `service::user_home` / `service::k8s` 的约定一致：
/// `{root}/vibeyeah/configs` 为共享配置模板，`{root}/vibeyeah/agents/...` 为各 agent 数据。
///
/// 创建组织时会确保该目录存在，并在缺少 `{root}/vibeyeah/configs` 时从仓库
/// `docker/desktop/configs` 播种（等价于运维手工执行的
/// `cp -a docker/desktop/configs/. <nas>/vibeyeah/configs/`）。
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::entity::organization;

/// 历史组织（`nas_mount_root` 为 NULL）回退的默认根目录。
pub const LEGACY_DEFAULT_MOUNT_ROOT: &str = "/data/nas";

/// 根目录下承载平台数据的子目录（与 user_home / k8s 的路径约定一致）
const VIBEYEAH_DIR: &str = "vibeyeah";

/// 共享配置模板在根目录下的子目录名
const CONFIGS_DIR: &str = "configs";

/// 共享配置模板在仓库内的相对位置（相对后端进程 cwd）
const TEMPLATE_SRC: &str = "docker/desktop/configs";

/// 新建组织时使用的默认 NAS 根目录：后端进程 cwd 下的 `data/nas`。
///
/// 容器内 cwd 为 `/` 时结果即 `/data/nas`（与既有部署一致）；
/// 本地从仓库根目录启动时得到 `<仓库根>/data/nas`，无需预先挂载 NAS。
pub fn default_mount_root() -> PathBuf {
    match std::env::current_dir() {
        Ok(cwd) => cwd.join("data").join("nas"),
        Err(e) => {
            tracing::warn!("获取当前目录失败，NAS 根目录回退 {LEGACY_DEFAULT_MOUNT_ROOT}: {e}");
            PathBuf::from(LEGACY_DEFAULT_MOUNT_ROOT)
        }
    }
}

/// 解析组织实际使用的 NAS 根目录；为空（历史组织）时回退默认值。
pub fn resolve(org: &organization::Model) -> String {
    org.nas_mount_root
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .unwrap_or(LEGACY_DEFAULT_MOUNT_ROOT)
        .to_string()
}

/// 同步（阻塞）初始化组织的 NAS 根目录，调用方应放在 `spawn_blocking` 中。
///
/// 1. 创建根目录（含父级）；
/// 2. `{root}/vibeyeah/configs` 不存在时从仓库模板目录播种。
pub fn ensure_mount_root(root: &Path) -> Result<()> {
    std::fs::create_dir_all(root)
        .with_context(|| format!("创建 NAS 根目录失败: {}", root.display()))?;

    let src = match std::env::current_dir() {
        Ok(cwd) => cwd.join(TEMPLATE_SRC),
        Err(_) => PathBuf::from(TEMPLATE_SRC),
    };
    ensure_mount_root_with_src(root, &src)?;

    tracing::info!("[nas] NAS 根目录就绪: {}", root.display());
    Ok(())
}

/// `ensure_mount_root` 的实现体：显式指定模板源目录以便单测。
///
/// 目标模板目录已存在则原样保留（不覆盖使用中的模板）；源目录不存在只记 warning
/// 并跳过播种，不视为失败（此时仍会保障根目录本身存在）。
fn ensure_mount_root_with_src(root: &Path, src: &Path) -> Result<()> {
    let configs = root.join(VIBEYEAH_DIR).join(CONFIGS_DIR);
    if configs.is_dir() {
        tracing::info!("[nas] 共享配置模板已存在，跳过播种: {}", configs.display());
        return Ok(());
    }
    if !src.is_dir() {
        tracing::warn!(
            "[nas] 未找到共享配置模板源目录 {}，跳过播种（可手工从仓库 docker/desktop/configs 拷贝）",
            src.display()
        );
        return Ok(());
    }

    copy_dir_all(src, &configs)?;
    tracing::info!(
        "[nas] 已播种共享配置模板: {} -> {}",
        src.display(),
        configs.display()
    );
    Ok(())
}

/// 递归拷贝目录内容（等价 `cp -a src/. dst/`，不跳过任何文件）。
///
/// 与 `user_home::copy_hermes_tree` 的区别：后者面向「给新用户一个干净起点」，
/// 会跳过 `state.db*` / `sessions` 等运行期状态；播种共享模板需要完整拷贝。
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst).with_context(|| format!("创建目录失败: {}", dst.display()))?;
    for entry in
        std::fs::read_dir(src).with_context(|| format!("读取目录失败: {}", src.display()))?
    {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry
            .file_type()
            .with_context(|| format!("读取文件类型失败: {}", from.display()))?
            .is_dir()
        {
            copy_dir_all(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .with_context(|| format!("拷贝失败: {} -> {}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// 临时目录（进程退出不清理，与仓库其他测试一致）
    fn tmp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nas-{tag}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// 构造模板源目录：{src}/.hermes/config.yaml 与 {src}/.claude/settings.json
    fn template_src(dir: &Path) -> PathBuf {
        let src = dir.join("docker").join("desktop").join("configs");
        fs::create_dir_all(src.join(".hermes")).unwrap();
        fs::create_dir_all(src.join(".claude")).unwrap();
        fs::write(src.join(".hermes").join("config.yaml"), "model: x").unwrap();
        fs::write(src.join(".claude").join("settings.json"), "{}").unwrap();
        src
    }

    /// 目标目录不存在时创建根目录，并把模板递归拷贝到 {root}/vibeyeah/configs
    #[test]
    fn test_ensure_mount_root_creates_and_seeds() {
        let dir = tmp_dir("seed");
        let src = template_src(&dir);
        let root = dir.join("data").join("nas");

        ensure_mount_root_with_src(&root, &src).unwrap();

        assert!(root
            .join("vibeyeah")
            .join("configs")
            .join(".hermes")
            .join("config.yaml")
            .is_file());
        assert!(root
            .join("vibeyeah")
            .join("configs")
            .join(".claude")
            .join("settings.json")
            .is_file());

        let _ = fs::remove_dir_all(&dir);
    }

    /// 模板目录已存在时不覆盖（保留运行中模板的改动）
    #[test]
    fn test_ensure_mount_root_keeps_existing_configs() {
        let dir = tmp_dir("keep");
        let src = template_src(&dir);
        let root = dir.join("data").join("nas");
        let configs = root.join("vibeyeah").join("configs");
        fs::create_dir_all(&configs).unwrap();
        fs::write(configs.join("marker.txt"), "keep me").unwrap();

        ensure_mount_root_with_src(&root, &src).unwrap();

        assert_eq!(
            fs::read_to_string(configs.join("marker.txt")).unwrap(),
            "keep me"
        );
        assert!(!configs.join(".hermes").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    /// 模板源目录缺失时只跳过播种，不报错
    #[test]
    fn test_ensure_mount_root_tolerates_missing_src() {
        let dir = tmp_dir("nosrc");
        let root = dir.join("data").join("nas");
        let absent = dir.join("docker").join("desktop").join("configs");

        ensure_mount_root_with_src(&root, &absent).unwrap();

        assert!(!root.join("vibeyeah").join("configs").exists());

        let _ = fs::remove_dir_all(&dir);
    }

    /// resolve：组织值非空则用它，NULL / 空白回退默认
    #[test]
    fn test_resolve_falls_back_for_empty_org_value() {
        let now = chrono::Utc::now();
        let mut org = organization::Model {
            id: uuid::Uuid::nil(),
            name: "o".into(),
            slug: "o".into(),
            k8s_namespace: "default".into(),
            k8s_kubeconfig: None,
            image_pull_secret: None,
            lark_tenant_key: None,
            lark_app_id: None,
            lark_app_secret: None,
            git_url: None,
            git_username: None,
            git_password: None,
            git_ssh_private_key: None,
            openai_base_url: None,
            openai_api_key: None,
            openai_model: None,
            nas_mount_root: None,
            created_by: None,
            created_at: now.into(),
            updated_at: now.into(),
        };

        assert_eq!(resolve(&org), LEGACY_DEFAULT_MOUNT_ROOT);

        org.nas_mount_root = Some("  ".into());
        assert_eq!(resolve(&org), LEGACY_DEFAULT_MOUNT_ROOT);

        org.nas_mount_root = Some("/mnt/org-a".into());
        assert_eq!(resolve(&org), "/mnt/org-a");
    }
}
