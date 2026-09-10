/// /add 多用户开通：准备用户在 NAS 上的 hermes home。
///
/// 将共享模板 `{nas_root}/vibeyeah/configs/.hermes` 拷贝到
/// `{nas_root}/vibeyeah/agents/{agent_dir}/users/{user_id}/home/.hermes`，
/// 跳过运行期状态（给新用户一个干净的起点）。拷贝完成后，把该用户的飞书网关凭据
/// 与组织的 OpenAI 配置（`openai_base_url`/`openai_api_key`/`openai_model`）渲染进
/// `.env` / `config.yaml`（见 `prepare_user_home`）。
///
/// 说明：`.claude.json` / `.claude/settings.json` 不在此处（按用户）拷贝——它们由
/// `entrypoint.sh` 从 `AGENT_CONFIG_DIR` 软链进每个用户的 home；其中
/// `.claude/settings.json` 的组织级 OpenAI 渲染见 `prepare_agent_claude_settings`。
use std::path::Path;

use anyhow::{anyhow, Context, Result};

/// 新用户不应继承的运行期状态文件/目录
const SKIP_NAMES: &[&str] = &[
    "gateway_state.json",
    "channel_directory.json",
    "sessions",
    "logs",
    "backups",
    ".DS_Store",
];

/// 组织级 OpenAI 兼容配置。
///
/// `openai_base_url` 不含 `/v1` 尾缀（模板里按各自用途自行拼接 `/v1`）。
/// 三项均非空才视为「已配置」，此时才会做占位符替换。
#[derive(Clone, Debug, Default)]
pub struct OpenAiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl OpenAiConfig {
    pub fn is_complete(&self) -> bool {
        !self.base_url.trim().is_empty()
            && !self.api_key.trim().is_empty()
            && !self.model.trim().is_empty()
    }
}

/// 把 OpenAI 占位符替换为组织实际值（仅在配置完整时调用）。
fn render_openai(content: &str, cfg: &OpenAiConfig) -> String {
    content
        .replace("__OPENAI_BASE_URL__", &cfg.base_url)
        .replace("__OPENAI_API_KEY__", &cfg.api_key)
        .replace("__OPENAI_MODEL__", &cfg.model)
}

/// 同步（阻塞）准备用户 hermes home。调用方应放在 `spawn_blocking` 中。
///
/// 拷贝完成后，把该用户的飞书网关凭据与组织的 OpenAI 配置渲染进
/// `.env` / `config.yaml`（见模板中的 `__LARK_*__` / `__OPENAI_*__` 占位符）。
pub fn prepare_user_home(
    nas_root: &str,
    agent_dir: &str,
    user_id: &str,
    lark_app_id: Option<&str>,
    lark_app_secret: Option<&str>,
    openai: &OpenAiConfig,
) -> Result<()> {
    let template = Path::new(nas_root)
        .join("vibeyeah")
        .join("configs")
        .join(".hermes");
    if !template.is_dir() {
        return Err(anyhow!("hermes 模板目录不存在: {}", template.display()));
    }

    let dest = Path::new(nas_root)
        .join("vibeyeah")
        .join("agents")
        .join(agent_dir)
        .join("users")
        .join(user_id)
        .join("home")
        .join(".hermes");
    std::fs::create_dir_all(&dest)
        .with_context(|| format!("创建用户 hermes 目录失败: {}", dest.display()))?;

    copy_hermes_tree(&template, &dest)?;

    // 占位符替换：先收集飞书网关凭据，再收集组织级 OpenAI 配置；
    // 对模板文件逐一遍历替换（无对应占位符时内容不变，不写回）。
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    if let (Some(app_id), Some(app_secret)) = (lark_app_id, lark_app_secret) {
        pairs.push(("__LARK_APP_ID__", app_id));
        pairs.push(("__LARK_APP_SECRET__", app_secret));
    }
    if openai.is_complete() {
        pairs.push(("__OPENAI_BASE_URL__", openai.base_url.as_str()));
        pairs.push(("__OPENAI_API_KEY__", openai.api_key.as_str()));
        pairs.push(("__OPENAI_MODEL__", openai.model.as_str()));
    }
    if !pairs.is_empty() {
        for file_name in [".env", "config.yaml"] {
            let file = dest.join(file_name);
            if !file.is_file() {
                continue;
            }
            let content = std::fs::read_to_string(&file)
                .with_context(|| format!("读取 {} 失败", file.display()))?;
            let rendered = pairs
                .iter()
                .fold(content.clone(), |acc, (k, v)| acc.replace(k, v));
            if rendered != content {
                std::fs::write(&file, rendered)
                    .with_context(|| format!("写入 {} 失败", file.display()))?;
            }
        }
    }

    tracing::info!(
        "[user_home] 已准备用户 home agent={} user={} -> {}",
        agent_dir,
        user_id,
        dest.display()
    );
    Ok(())
}

/// 把组织的 OpenAI 配置渲染进该 agent 的 Claude Code 共享配置
/// `{nas}/vibeyeah/agents/{agent_dir}/configs/.claude/settings.json`。
///
/// Pod 的 init 容器启动时会以 `cp -a -n` 把共享模板复制到该目录，`-n` 不会覆盖
/// 已存在的文件，因此这里在创建 Pod 前预生成渲染结果即可在运行时生效
/// （entrypoint.sh 再把该文件软链进每个用户的 `.claude/settings.json`）。
///
/// OpenAI 未配齐或共享模板缺失时静默跳过（回退到模板原文/默认），不阻塞建 agent。
pub fn prepare_agent_claude_settings(
    nas_root: &str,
    agent_dir: &str,
    openai: &OpenAiConfig,
) -> Result<()> {
    if !openai.is_complete() {
        return Ok(());
    }
    let template = Path::new(nas_root)
        .join("vibeyeah")
        .join("configs")
        .join(".claude")
        .join("settings.json");
    if !template.is_file() {
        tracing::warn!(
            "[user_home] Claude Code 共享模板缺失，跳过渲染: {}",
            template.display()
        );
        return Ok(());
    }
    let content = std::fs::read_to_string(&template)
        .with_context(|| format!("读取 {} 失败", template.display()))?;
    let rendered = render_openai(&content, openai);

    let dest = Path::new(nas_root)
        .join("vibeyeah")
        .join("agents")
        .join(agent_dir)
        .join("configs")
        .join(".claude")
        .join("settings.json");
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("创建目录失败: {}", dir.display()))?;
    }
    std::fs::write(&dest, rendered).with_context(|| format!("写入 {} 失败", dest.display()))?;

    tracing::info!(
        "[user_home] 已渲染 agent Claude Code 配置 agent={} -> {}",
        agent_dir,
        dest.display()
    );
    Ok(())
}

/// 是否跳过（运行期状态 / state.db*）
fn skip_entry(name: &str) -> bool {
    SKIP_NAMES.iter().any(|s| *s == name) || name.starts_with("state.db")
}

/// 递归拷贝 `src` 到 `dst`，跳过运行期状态项。
fn copy_hermes_tree(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in
        std::fs::read_dir(src).with_context(|| format!("读取模板目录失败: {}", src.display()))?
    {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if skip_entry(&name) {
            continue;
        }
        let s = entry.path();
        let d = dst.join(&name);
        if s.is_dir() {
            copy_hermes_tree(&s, &d)?;
        } else {
            std::fs::copy(&s, &d)
                .with_context(|| format!("拷贝失败: {} -> {}", s.display(), d.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn openai_cfg() -> OpenAiConfig {
        OpenAiConfig {
            base_url: "https://dashscope.aliyuncs.com/compatible-mode".to_string(),
            api_key: "sk-abc123".to_string(),
            model: "qwen3.8-max".to_string(),
        }
    }

    fn setup_template(tmp: &std::path::Path) -> std::path::PathBuf {
        // 模板
        let tpl = tmp.join("vibeyeah").join("configs").join(".hermes");
        fs::create_dir_all(tpl.join("skills").join("demo")).unwrap();
        fs::create_dir_all(tpl.join("sessions")).unwrap();
        fs::write(
            tpl.join("config.yaml"),
            "default: __OPENAI_MODEL__\nbase_url: __OPENAI_BASE_URL__/v1\napi_key: __OPENAI_API_KEY__\n",
        )
        .unwrap();
        fs::write(tpl.join("skills").join("demo").join("SKILL.md"), "# demo").unwrap();
        fs::write(tpl.join("state.db"), "binary-state").unwrap();
        fs::write(tpl.join("gateway_state.json"), "{}").unwrap();
        fs::write(
            tpl.join(".env"),
            "FEISHU_APP_ID=__LARK_APP_ID__\nFEISHU_APP_SECRET=__LARK_APP_SECRET__\n\
             DASHSCOPE_API_KEY=__OPENAI_API_KEY__\nDASHSCOPE_BASE_URL=__OPENAI_BASE_URL__/v1\n",
        )
        .unwrap();
        tpl
    }

    #[test]
    fn test_prepare_user_home() {
        let tmp = std::env::temp_dir().join(format!("user-home-test-{}", uuid::Uuid::new_v4()));
        setup_template(&tmp);

        let nas_root = tmp.to_str().unwrap();
        prepare_user_home(
            nas_root,
            "agent-abc",
            "u1",
            Some("cli_id_1"),
            Some("sec_1"),
            &openai_cfg(),
        )
        .unwrap();

        let dest = tmp
            .join("vibeyeah")
            .join("agents")
            .join("agent-abc")
            .join("users")
            .join("u1")
            .join("home")
            .join(".hermes");
        // 配置与技能被拷贝
        assert!(dest.join("config.yaml").is_file());
        assert!(dest.join("skills").join("demo").join("SKILL.md").is_file());
        // 运行期状态被跳过
        assert!(!dest.join("state.db").exists());
        assert!(!dest.join("gateway_state.json").exists());
        assert!(!dest.join("sessions").exists());
        // 凭据占位符被替换
        let env = fs::read_to_string(dest.join(".env")).unwrap();
        assert!(env.contains("FEISHU_APP_ID=cli_id_1"));
        assert!(env.contains("FEISHU_APP_SECRET=sec_1"));
        assert!(env.contains("DASHSCOPE_API_KEY=sk-abc123"));
        assert!(
            env.contains("DASHSCOPE_BASE_URL=https://dashscope.aliyuncs.com/compatible-mode/v1")
        );
        assert!(!env.contains("__LARK_APP_ID__"));
        assert!(!env.contains("__OPENAI_API_KEY__"));
        // config.yaml 的 OpenAI 占位符被替换
        let cfg = fs::read_to_string(dest.join("config.yaml")).unwrap();
        assert!(cfg.contains("default: qwen3.8-max"));
        assert!(cfg.contains("api_key: sk-abc123"));
        assert!(!cfg.contains("__OPENAI_"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_prepare_user_home_skips_openai_when_incomplete() {
        let tmp = std::env::temp_dir().join(format!("user-home-test-{}", uuid::Uuid::new_v4()));
        setup_template(&tmp);

        let nas_root = tmp.to_str().unwrap();
        prepare_user_home(
            nas_root,
            "agent-abc",
            "u1",
            Some("cli_id_1"),
            Some("sec_1"),
            &OpenAiConfig::default(),
        )
        .unwrap();

        let dest = tmp
            .join("vibeyeah")
            .join("agents")
            .join("agent-abc")
            .join("users")
            .join("u1")
            .join("home")
            .join(".hermes");
        let env = fs::read_to_string(dest.join(".env")).unwrap();
        // 飞书凭据仍替换，OpenAI 占位符保留
        assert!(env.contains("FEISHU_APP_ID=cli_id_1"));
        assert!(env.contains("__OPENAI_API_KEY__"));

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_prepare_agent_claude_settings() {
        let tmp = std::env::temp_dir().join(format!("user-home-test-{}", uuid::Uuid::new_v4()));
        // Claude Code 共享模板
        let claude_tpl = tmp.join("vibeyeah").join("configs").join(".claude");
        fs::create_dir_all(&claude_tpl).unwrap();
        fs::write(
            claude_tpl.join("settings.json"),
            r#"{
    "env": {
        "ANTHROPIC_AUTH_TOKEN": "__OPENAI_API_KEY__",
        "ANTHROPIC_BASE_URL": "__OPENAI_BASE_URL__",
        "ANTHROPIC_MODEL": "__OPENAI_MODEL__"
    }
}"#,
        )
        .unwrap();

        let nas_root = tmp.to_str().unwrap();
        prepare_agent_claude_settings(nas_root, "agent-abc", &openai_cfg()).unwrap();

        let rendered = fs::read_to_string(
            tmp.join("vibeyeah")
                .join("agents")
                .join("agent-abc")
                .join("configs")
                .join(".claude")
                .join("settings.json"),
        )
        .unwrap();
        assert!(rendered.contains("ANTHROPIC_AUTH_TOKEN\": \"sk-abc123\""));
        assert!(rendered
            .contains("ANTHROPIC_BASE_URL\": \"https://dashscope.aliyuncs.com/compatible-mode\""));
        assert!(rendered.contains("ANTHROPIC_MODEL\": \"qwen3.8-max\""));
        assert!(!rendered.contains("__OPENAI_"));

        // OpenAI 未配齐时不渲染（不产生文件）
        prepare_agent_claude_settings(nas_root, "agent-xyz", &OpenAiConfig::default()).unwrap();
        assert!(!tmp
            .join("vibeyeah")
            .join("agents")
            .join("agent-xyz")
            .join("configs")
            .join(".claude")
            .join("settings.json")
            .exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
