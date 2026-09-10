use anyhow::{Context, Result};
use k8s_openapi::{
    api::{
        apps::v1::{Deployment, DeploymentSpec},
        core::v1::{
            Container, ContainerPort, EnvVar, LocalObjectReference,
            PersistentVolumeClaimVolumeSource, Pod, PodSpec, PodTemplateSpec, ResourceRequirements,
            SecurityContext, Volume, VolumeMount,
        },
    },
    apimachinery::pkg::{api::resource::Quantity, apis::meta::v1::LabelSelector},
};
use kube::{
    api::{Api, ListParams, ObjectMeta, PostParams},
    Client,
};
use std::collections::BTreeMap;
use uuid::Uuid;

use crate::entity::agent::PodStatus;

#[derive(Debug, Clone)]
pub struct WechatEnv {
    pub account_id: String,
    pub token: String,
    pub base_url: String,
    pub user_id: String,
}

#[derive(Debug, Clone)]
pub struct LarkEnv {
    pub app_id: String,
    pub app_secret: String,
    pub bot_open_id: String,
}

/// 从 base64 编码的 kubeconfig 内容构建 K8s Client。
/// `kubeconfig_b64` 为 None 时回退到全局默认（in-cluster 或 ~/.kube/config）。
pub async fn client_from_kubeconfig(kubeconfig_b64: Option<&str>) -> Result<Client> {
    let b64 = match kubeconfig_b64 {
        None | Some("") => return Ok(Client::try_default().await?),
        Some(b) => b,
    };

    use base64::{engine::general_purpose::STANDARD, Engine as _};
    let raw = STANDARD
        .decode(b64)
        .context("解码组织 kubeconfig 失败（base64）")?;

    let kubeconfig = kube::config::Kubeconfig::from_yaml(
        std::str::from_utf8(&raw).context("kubeconfig 不是合法 UTF-8")?,
    )
    .context("解析组织 kubeconfig YAML 失败")?;

    let opts = kube::config::KubeConfigOptions::default();
    let config = kube::Config::from_custom_kubeconfig(kubeconfig, &opts)
        .await
        .context("从 kubeconfig 构建 K8s Config 失败")?;

    Client::try_from(config).context("构建 K8s Client 失败")
}

/// NAS 在容器内的挂载点
const NAS_MOUNT_PATH: &str = "/data/nas";

/// agent 共享配置模板在 NAS 上的路径
const NAS_CONFIGS_SRC: &str = "/data/nas/vibeyeah/configs";

/// 在 K8s 创建 agent 的 Deployment（单副本 desktop Pod）
/// 返回 (deployment_name, namespace)
pub async fn create_agent_deployment(
    client: &Client,
    agent_id: &Uuid,
    namespace: &str,
    image_pull_secret: Option<String>,
    desktop_image: &str,
    sidecar_image: &str,
    nas_pvc_name: &str,
    lark_env: Option<LarkEnv>,
    wechat_env: Option<WechatEnv>,
) -> Result<(String, String)> {
    let name = agent_workload_name(agent_id);
    let api: Api<Deployment> = Api::namespaced(client.clone(), namespace);

    // 该 agent 在 NAS 上的专属配置目录（由 init 容器从模板复制而来）
    let agent_config_dir = format!("/data/nas/vibeyeah/agents/{name}/configs");

    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), name.clone());
    labels.insert("agent-id".to_string(), agent_id.to_string());

    let mut desktop_env = vec![EnvVar {
        name: "RESOLUTION".to_string(),
        value: Some("1920x1080".to_string()),
        ..Default::default()
    }];

    if let Some(ref lk) = lark_env {
        desktop_env.push(EnvVar {
            name: "LARK_APP_ID".to_string(),
            value: Some(lk.app_id.clone()),
            ..Default::default()
        });
        desktop_env.push(EnvVar {
            name: "LARK_APP_SECRET".to_string(),
            value: Some(lk.app_secret.clone()),
            ..Default::default()
        });
        desktop_env.push(EnvVar {
            name: "LARK_OPEN_ID".to_string(),
            value: Some(lk.bot_open_id.clone()),
            ..Default::default()
        });
    }

    if let Some(ref wc) = wechat_env {
        desktop_env.push(EnvVar {
            name: "WEIXIN_ACCOUNT_ID".to_string(),
            value: Some(wc.account_id.clone()),
            ..Default::default()
        });
        desktop_env.push(EnvVar {
            name: "WEIXIN_TOKEN".to_string(),
            value: Some(wc.token.clone()),
            ..Default::default()
        });
        desktop_env.push(EnvVar {
            name: "WEIXIN_BASE_URL".to_string(),
            value: Some(wc.base_url.clone()),
            ..Default::default()
        });
        desktop_env.push(EnvVar {
            name: "WEIXIN_USER_ID".to_string(),
            value: Some(wc.user_id.clone()),
            ..Default::default()
        });
    }

    // entrypoint.sh 依据该目录建立 /home/agent 下的软链
    desktop_env.push(EnvVar {
        name: "AGENT_NAME".to_string(),
        value: Some(name.clone()),
        ..Default::default()
    });
    desktop_env.push(EnvVar {
        name: "AGENT_CONFIG_DIR".to_string(),
        value: Some(agent_config_dir.clone()),
        ..Default::default()
    });

    // desktop 容器
    let desktop = Container {
        name: "desktop".to_string(),
        image: Some(desktop_image.to_string()),
        image_pull_policy: Some("IfNotPresent".to_string()),
        ports: None, // 无需对外暴露端口，sidecar 通过共享 X socket 采集画面
        env: Some(desktop_env),
        // // 与 sidecar 共享 X11 unix socket
        // volume_mounts: Some(vec![
        //     VolumeMount {
        //         name: "x11-socket".to_string(),
        //         mount_path: "/tmp/.X11-unix".to_string(),
        //         ..Default::default()
        //     },
        //     VolumeMount {
        //         name: "home".to_string(),
        //         mount_path: "/home/agent".to_string(),
        //         ..Default::default()
        //     },
        // ]),
        volume_mounts: Some(vec![VolumeMount {
            name: "nas".to_string(),
            mount_path: NAS_MOUNT_PATH.to_string(),
            ..Default::default()
        }]),
        resources: Some(ResourceRequirements {
            requests: Some({
                let mut m = BTreeMap::new();
                m.insert("cpu".to_string(), Quantity("500m".to_string()));
                m.insert("memory".to_string(), Quantity("1Gi".to_string()));
                m
            }),
            limits: Some({
                let mut m = BTreeMap::new();
                m.insert("cpu".to_string(), Quantity("2".to_string()));
                m.insert("memory".to_string(), Quantity("4Gi".to_string()));
                m
            }),
            ..Default::default()
        }),
        security_context: Some(SecurityContext {
            run_as_non_root: Some(true),
            run_as_user: Some(1000),
            ..Default::default()
        }),
        ..Default::default()
    };

    // sidecar 容器：mediamtx 常驻，ffmpeg 默认关闭
    let sidecar = Container {
        name: "webrtc-sidecar".to_string(),
        image: Some(sidecar_image.to_string()),
        ports: Some(vec![
            ContainerPort {
                name: Some("rtsp".to_string()),
                container_port: 8554,
                ..Default::default()
            },
            ContainerPort {
                name: Some("webrtc".to_string()),
                container_port: 8889,
                ..Default::default()
            },
            ContainerPort {
                name: Some("mtx-api".to_string()),
                container_port: 9997,
                ..Default::default()
            },
            ContainerPort {
                name: Some("stream-ctrl".to_string()),
                container_port: 9998,
                ..Default::default()
            },
        ]),
        env: Some(vec![
            EnvVar {
                name: "RESOLUTION".to_string(),
                value: Some("1920x1080".to_string()),
                ..Default::default()
            },
            EnvVar {
                name: "MEDIAMTX_RTSP".to_string(),
                value: Some("rtsp://localhost:8554/desktop".to_string()),
                ..Default::default()
            },
        ]),
        // volume_mounts: Some(vec![VolumeMount {
        //     name: "x11-socket".to_string(),
        //     mount_path: "/tmp/.X11-unix".to_string(),
        //     ..Default::default()
        // }]),
        resources: Some(ResourceRequirements {
            requests: Some({
                let mut m = BTreeMap::new();
                m.insert("cpu".to_string(), Quantity("100m".to_string()));
                m.insert("memory".to_string(), Quantity("256Mi".to_string()));
                m
            }),
            limits: Some({
                let mut m = BTreeMap::new();
                m.insert("cpu".to_string(), Quantity("1".to_string()));
                m.insert("memory".to_string(), Quantity("1Gi".to_string()));
                m
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    // init 容器：启动前把共享配置模板复制到该 agent 的专属目录
    // - cp -n 只新增不覆盖，Pod 重启不会清掉 agent 运行后产生的数据（如 .hermes/state.db）
    // - 以 root 运行，复制后 chown 给 uid 1000（desktop 容器）读写
    // - 复用 desktop 镜像：节点上必然已缓存，且带完整 GNU 工具
    let init_script = format!(
        r#"set -e
SRC="{NAS_CONFIGS_SRC}"
DEST="{agent_config_dir}"
mkdir -p "$DEST"
if [ -d "$SRC" ]; then
    cp -a -n "$SRC/." "$DEST/"
    echo "[init-configs] copied $SRC -> $DEST (existing files kept)"
else
    echo "[init-configs] WARNING: $SRC not found, falling back to in-image configs"
fi
chown -R 1000:1000 "{NAS_MOUNT_PATH}/vibeyeah/agents/{name}"
"#
    );

    let init_configs = Container {
        name: "init-agent-configs".to_string(),
        image: Some(desktop_image.to_string()),
        command: Some(vec!["sh".to_string(), "-c".to_string(), init_script]),
        volume_mounts: Some(vec![VolumeMount {
            name: "nas".to_string(),
            mount_path: NAS_MOUNT_PATH.to_string(),
            ..Default::default()
        }]),
        ..Default::default()
    };

    let deployment = Deployment {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace.to_string()),
            labels: Some(labels.clone()),
            ..Default::default()
        },
        spec: Some(DeploymentSpec {
            replicas: Some(1),
            selector: LabelSelector {
                match_labels: Some(labels.clone()),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some(labels.clone()),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    init_containers: Some(vec![init_configs]),
                    // containers: vec![desktop, sidecar],
                    containers: vec![desktop],
                    // // emptyDir 共享 X11 socket
                    // volumes: Some(vec![Volume {
                    //     name: "x11-socket".to_string(),
                    //     empty_dir: Some(EmptyDirVolumeSource::default()),
                    //     ..Default::default()
                    // }]),
                    volumes: Some(vec![Volume {
                        name: "nas".to_string(),
                        persistent_volume_claim: Some(PersistentVolumeClaimVolumeSource {
                            claim_name: nas_pvc_name.to_string(),
                            read_only: Some(false),
                        }),
                        ..Default::default()
                    }]),
                    image_pull_secrets: image_pull_secret.as_deref().map(|s| {
                        vec![LocalObjectReference {
                            name: s.to_string(),
                        }]
                    }),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    api.create(&PostParams::default(), &deployment)
        .await
        .map_err(|e| {
            println!("err: {:?}", e);
            return e;
        })
        .with_context(|| format!("创建 Deployment {} 失败", name))?;

    tracing::info!("K8s Deployment {} 已创建 (namespace={})", name, namespace);
    Ok((name, namespace.to_string()))
}

/// 按 app label 查找 agent 当前的 Pod（Deployment 的 Pod 名不固定，不能用 <name>-0）。
/// 若同时存在多个 Pod（如滚动更新），优先返回未在删除中的那个。
pub async fn find_agent_pod(
    client: &Client,
    namespace: &str,
    app_name: &str,
) -> Result<Option<Pod>> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);
    let lp = ListParams::default().labels(&format!("app={}", app_name));
    let list = pods
        .list(&lp)
        .await
        .with_context(|| format!("列出 Pod 失败 (app={})", app_name))?;
    // bool: false(未删除) < true(删除中)，min_by_key 优先返回未在删除中的
    Ok(list
        .items
        .into_iter()
        .min_by_key(|p| p.metadata.deletion_timestamp.is_some()))
}

/// 查询 agent 当前 Pod 的状态
pub async fn get_pod_status(client: &Client, pod_name: &str, namespace: &str) -> Result<PodStatus> {
    let pod = match find_agent_pod(client, namespace, pod_name).await? {
        Some(p) => p,
        None => return Ok(PodStatus::Creating),
    };

    // 先检查容器等待原因（镜像拉取失败等不会改变 phase 的错误）
    let container_fail = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
        .into_iter()
        .flatten()
        .any(|cs| {
            cs.state
                .as_ref()
                .and_then(|st| st.waiting.as_ref())
                .map(|w| {
                    matches!(
                        w.reason.as_deref(),
                        Some("ImagePullBackOff")
                            | Some("ErrImagePull")
                            | Some("ErrImageNeverPull")
                            | Some("CrashLoopBackOff")
                            | Some("CreateContainerError")
                            | Some("CreateContainerConfigError")
                            | Some("FailedScheduling")
                    )
                })
                .unwrap_or(false)
        });

    if container_fail {
        return Ok(PodStatus::Failed);
    }

    let phase = pod
        .status
        .as_ref()
        .and_then(|s| s.phase.as_deref())
        .unwrap_or("Unknown");
    Ok(phase_to_status(phase))
}

/// Pod 就绪等待结果
pub enum WaitResult {
    /// Pod 进入 Running 状态
    Running,
    /// Pod 出现明确失败状态，附带异常事件摘要
    Failed(String),
    /// 超时
    Timeout,
}

/// 轮询等待 Pod 进入 Running（或失败），每隔 `interval_secs` 秒查一次
/// `timeout_secs` 秒后放弃，同时查询 K8s Events 附上原因
pub async fn wait_for_pod_ready(
    client: &Client,
    pod_name: &str,
    namespace: &str,
    timeout_secs: u64,
    interval_secs: u64,
) -> WaitResult {
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);

    loop {
        match get_pod_status(client, pod_name, namespace).await {
            Ok(status) => {
                tracing::debug!("[k8s] Pod {}-0 status={:?}", pod_name, status);
                match status {
                    PodStatus::Running => return WaitResult::Running,
                    PodStatus::Failed | PodStatus::Succeeded => {
                        let reason = fetch_pod_events(client, pod_name, namespace)
                            .await
                            .unwrap_or_else(|e| format!("无法获取事件: {e}"));
                        tracing::warn!("[k8s] Pod {}-0 失败，事件: {}", pod_name, reason);
                        return WaitResult::Failed(reason);
                    }
                    _ => {}
                }
            }
            Err(e) => {
                tracing::warn!("[k8s] 查询 Pod 状态失败: {e}");
            }
        }

        if Instant::now() >= deadline {
            return WaitResult::Timeout;
        }

        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}

/// 查询与 Pod 相关的 K8s Warning 事件，返回最近几条的摘要
pub async fn fetch_pod_events(client: &Client, pod_name: &str, namespace: &str) -> Result<String> {
    use k8s_openapi::api::core::v1::Event;

    let pod_full_name = match find_agent_pod(client, namespace, pod_name).await? {
        Some(p) => match p.metadata.name {
            Some(n) => n,
            None => return Ok("无法获取 Pod 名称".to_string()),
        },
        None => return Ok("未找到 Pod，暂无相关事件".to_string()),
    };
    let events: Api<Event> = Api::namespaced(client.clone(), namespace);

    let lp = ListParams::default().fields(&format!("involvedObject.name={}", pod_full_name));

    let list = events.list(&lp).await?;

    // 收集所有 Warning 事件 + reason 含失败关键词的 Normal 事件
    let fail_reasons = ["Failed", "BackOff", "Error", "OOMKilled", "Evicted"];
    let mut lines: Vec<String> = list
        .items
        .iter()
        .filter(|e| {
            let is_warning = e.type_.as_deref() == Some("Warning");
            let reason = e.reason.as_deref().unwrap_or("");
            let is_fail_normal = fail_reasons.iter().any(|r| reason.contains(r));
            is_warning || is_fail_normal
        })
        .map(|e| {
            let reason = e.reason.as_deref().unwrap_or("Unknown");
            let msg = e.message.as_deref().unwrap_or("");
            let count = e.count.unwrap_or(1);
            if count > 1 {
                format!("[{}×{}] {}", count, reason, msg)
            } else {
                format!("[{}] {}", reason, msg)
            }
        })
        .collect();

    // 去重（同一条事件 K8s 会重复记录）
    lines.dedup();

    if lines.is_empty() {
        Ok("无相关事件记录".to_string())
    } else {
        Ok(lines.into_iter().take(5).collect::<Vec<_>>().join("\n"))
    }
}

/// 向 sidecar stream-ctrl 发送 HTTP 控制指令（start / stop）
/// Deployment 的 Pod 名不固定，先按 label 找到 Pod 再通过其 Pod IP 访问 9998 端口
pub async fn call_stream_ctrl(
    client: &Client,
    pod_name: &str,
    namespace: &str,
    action: &str, // "start" | "stop" | "status"
) -> Result<serde_json::Value> {
    let pod = find_agent_pod(client, namespace, pod_name)
        .await?
        .ok_or_else(|| anyhow::anyhow!("未找到 Pod (app={})", pod_name))?;
    let pod_ip = pod
        .status
        .as_ref()
        .and_then(|s| s.pod_ip.clone())
        .ok_or_else(|| anyhow::anyhow!("Pod 尚无 IP (app={})", pod_name))?;
    let url = format!("http://{}:9998/{}", pod_ip, action);

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let method = if action == "status" {
        reqwest::Method::GET
    } else {
        reqwest::Method::POST
    };

    let resp = http
        .request(method, &url)
        .send()
        .await
        .with_context(|| format!("stream-ctrl 请求失败: {}", url))?
        .json::<serde_json::Value>()
        .await?;

    Ok(resp)
}

/// 通过 K8s exec API 在 pod 的 desktop 容器内执行 shell 脚本，收集 stdout/stderr。
/// 脚本退出码非 0（或状态非 Success）时返回 Err。
pub async fn exec_in_pod(
    client: &kube::Client,
    pod_name: &str,
    namespace: &str,
    script: &str,
) -> anyhow::Result<()> {
    use k8s_openapi::api::core::v1::Pod;
    use kube::api::AttachedProcess;
    use kube::api::{Api, AttachParams};
    use tokio::io::AsyncReadExt;

    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);

    let ap = AttachParams::default()
        .container("desktop")
        .stdout(true)
        .stderr(true)
        .stdin(false)
        .tty(false);

    let mut process: AttachedProcess = pods
        .exec(pod_name, vec!["sh", "-c", script], &ap)
        .await
        .map_err(|e| anyhow::anyhow!("exec 失败 pod={}: {}", pod_name, e))?;

    // 读取 stdout
    let stdout_out = if let Some(mut reader) = process.stdout() {
        let mut buf = String::new();
        reader.read_to_string(&mut buf).await.unwrap_or(0);
        buf
    } else {
        String::new()
    };

    // 读取 stderr
    let stderr_out = if let Some(mut reader) = process.stderr() {
        let mut buf = String::new();
        reader.read_to_string(&mut buf).await.unwrap_or(0);
        buf
    } else {
        String::new()
    };

    // 等待状态
    if let Some(status_fut) = process.take_status() {
        if let Some(status) = status_fut.await {
            let ok = status
                .status
                .as_deref()
                .map(|v| v == "Success")
                .unwrap_or(false);
            if !ok {
                let reason = status.reason.as_deref().unwrap_or("unknown");
                let msg = status.message.as_deref().unwrap_or("");
                tracing::error!(
                    "pod {} exec 退出失败: reason={} message={}\nstdout={}\nstderr={}",
                    pod_name,
                    reason,
                    msg,
                    stdout_out,
                    stderr_out
                );
                return Err(anyhow::anyhow!(
                    "pod {} exec 失败: {} {}\nstdout:{}\nstderr:{}",
                    pod_name,
                    reason,
                    msg,
                    stdout_out,
                    stderr_out
                ));
            }
        }
    }

    tracing::debug!(
        "pod {} exec 完成\nstdout:{}\nstderr:{}",
        pod_name,
        stdout_out,
        stderr_out
    );
    Ok(())
}

/// 批量同步所有 agent 的 pod 状态到数据库（按组织 kubeconfig 分别操作）
pub async fn sync_all_pod_statuses(
    db: &sea_orm::DatabaseConnection,
    default_client: &Client,
) -> Result<()> {
    use crate::entity::{agent, organization};
    use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};

    let agents = agent::Entity::find()
        .filter(agent::Column::PodName.is_not_null())
        .all(db)
        .await?;

    tracing::debug!("同步 {} 个 agent Pod 状态", agents.len());

    for a in agents {
        let pod_name = match &a.pod_name {
            Some(n) => n.clone(),
            None => continue,
        };
        let ns = a
            .pod_namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());

        // 按组织 kubeconfig 构建 client
        let kubeconfig_b64 = if let Some(org_id) = a.org_id {
            organization::Entity::find_by_id(org_id)
                .one(db)
                .await
                .ok()
                .flatten()
                .and_then(|o| o.k8s_kubeconfig)
        } else {
            None
        };

        let client = match client_from_kubeconfig(kubeconfig_b64.as_deref()).await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("构建组织 K8s client 失败，使用默认: {e}");
                default_client.clone()
            }
        };

        let status = get_pod_status(&client, &pod_name, &ns)
            .await
            .unwrap_or(PodStatus::Unknown);

        let now = chrono::Utc::now();
        let mut active: agent::ActiveModel = a.into();
        active.pod_status = Set(status);
        active.pod_synced_at = Set(Some(now.into()));
        active.updated_at = Set(now.into());
        active.update(db).await?;
    }
    Ok(())
}

pub fn agent_workload_name(agent_id: &Uuid) -> String {
    format!(
        "agent-{}",
        agent_id.to_string().replace('-', "")[..16].to_lowercase()
    )
}

fn phase_to_status(phase: &str) -> PodStatus {
    match phase {
        "Pending" => PodStatus::Creating,
        "Running" => PodStatus::Running,
        "Succeeded" => PodStatus::Succeeded,
        "Failed" => PodStatus::Failed,
        _ => PodStatus::Unknown,
    }
}
