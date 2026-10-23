use crate::{
    git::Git,
    identity::{self, validate_email},
    model::*,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    path::{Path, PathBuf},
};

// Only a lock created by this process is removed. Never adopt an existing Git lock.
struct ConfigLock {
    path: PathBuf,
    _file: File,
}
impl Drop for ConfigLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn commit_changes(git: &Git, ctx: &Context, preview: &Preview) -> Result<()> {
    let config = target(ctx);
    let lock_path = config.with_extension("lock");
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock_path)
        .map_err(|_| {
            AppError::new(
                "config_locked",
                "配置锁冲突或不可写；未删除已有锁，未修改目标。",
            )
        })?;
    let _lock = ConfigLock {
        path: lock_path,
        _file: file,
    };
    if fingerprint(git, ctx)? != preview.fingerprint {
        return Err(AppError::new(
            "preview_stale",
            "加锁后检测到外部变化，请重新预览。",
        ));
    }
    let meta = std::fs::symlink_metadata(&config)?;
    if meta.file_type().is_symlink() || !meta.is_file() {
        return Err(AppError::new(
            "config_link",
            "目标不再是普通配置文件，未执行写入。",
        ));
    }
    let stage_dir = tempfile::Builder::new()
        .prefix("repolens-")
        .tempdir_in(config.parent().unwrap())
        .map_err(|error| AppError::filesystem("创建暂存目录失败", error))?;
    let stage = stage_dir.path().join("config");
    let original =
        std::fs::read(&config).map_err(|error| AppError::filesystem("读取目标配置失败", error))?;
    std::fs::write(&stage, original)
        .map_err(|error| AppError::filesystem("写入暂存配置失败", error))?;
    std::fs::set_permissions(&stage, meta.permissions())?;
    for item in &preview.items {
        let mut args = vec![
            "config".into(),
            "--file".into(),
            stage.as_os_str().into(),
            "--no-includes".into(),
            if item.new.is_some() {
                "--replace-all".into()
            } else {
                "--unset-all".into()
            },
            "--".into(),
            item.key.clone().into(),
        ];
        if let Some(value) = &item.new {
            args.push(value.into());
        }
        let out = git.run(Path::new(&ctx.path), &args)?;
        if out.code != 0 && !(item.new.is_none() && out.code == 5) {
            return Err(AppError::new(
                "config_write",
                "暂存配置修改失败；目标配置未替换。",
            ));
        }
    }
    // Recheck contributing includes and repository context while the target lock is held.
    if fingerprint(git, ctx)? != preview.fingerprint {
        return Err(AppError::new(
            "preview_stale",
            "配置来源或上下文变化，请重新预览。",
        ));
    }
    OpenOptions::new().write(true).open(&stage)?.sync_all()?;
    std::fs::rename(&stage, &config)
        .map_err(|error| AppError::filesystem("替换目标配置失败", error))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub key: String,
    pub value: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewItem {
    pub key: String,
    pub old: Vec<String>,
    pub new: Option<String>,
    pub effective: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub token: String,
    pub entry_id: String,
    pub target: String,
    pub context: Context,
    pub items: Vec<PreviewItem>,
    pub warnings: Vec<String>,
    pub affected: Vec<String>,
    pub created_at: u64,
    #[serde(skip)]
    pub fingerprint: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemResult {
    pub key: String,
    pub state: String,
    pub message: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteResult {
    pub entry_id: String,
    pub state: String,
    pub items: Vec<ItemResult>,
}

pub fn validate_changes(changes: &[Change]) -> Result<()> {
    if changes.is_empty() || changes.len() > 2 {
        return Err(AppError::new("change", "请选择要修改的姓名或邮箱。"));
    }
    let mut keys = BTreeSet::new();
    for c in changes {
        if !["user.name", "user.email"].contains(&c.key.as_str()) || !keys.insert(&c.key) {
            return Err(AppError::new(
                "change",
                "只允许修改 local 基础姓名与邮箱，每项一次。",
            ));
        }
        if let Some(v) = &c.value {
            if !valid_text(v, if c.key == "user.name" { 256 } else { 320 })
                || (c.key == "user.email" && !validate_email(v))
            {
                return Err(AppError::new(
                    "input",
                    "姓名或邮箱无效；空值不能用来移除配置，请使用独立移除操作。",
                ));
            }
        }
    }
    Ok(())
}
pub fn local_values(git: &Git, ctx: &Context, key: &str) -> Result<Vec<String>> {
    let out = git.query(
        Path::new(&ctx.path),
        &[
            "config",
            "--local",
            "--no-includes",
            "--null",
            "--get-all",
            key,
        ],
    )?;
    if out.code == 1 {
        return Ok(vec![]);
    }
    if out.code != 0 {
        return Err(AppError::new("local_config", "无法读取目标 local 文件。"));
    }
    let parts: Vec<_> = out.bytes.split(|b| *b == 0).collect();
    parts[..parts.len().saturating_sub(1)]
        .iter()
        .map(|s| {
            String::from_utf8(s.to_vec())
                .map_err(|_| AppError::new("encoding", "local 配置编码不受支持。"))
        })
        .collect()
}
fn target(ctx: &Context) -> PathBuf {
    Path::new(&ctx.common_dir).join("config")
}
pub fn fingerprint(git: &Git, ctx: &Context) -> Result<String> {
    let mut hash = Sha256::new();
    hash.update(git.executable.as_bytes());
    hash.update(serde_json::to_vec(ctx).unwrap());
    let head = git.head(Path::new(&ctx.path))?;
    hash.update(serde_json::to_vec(&head).unwrap());
    let mut files = BTreeSet::new();
    files.insert(target(ctx));
    for key in identity::KEYS {
        let values = git.config(Path::new(&ctx.path), key)?;
        hash.update(serde_json::to_vec(&values).unwrap());
        for v in values {
            if let Some(path) = v.origin.strip_prefix("file:") {
                let path = Path::new(path);
                files.insert(if path.is_absolute() {
                    path.into()
                } else {
                    Path::new(&ctx.path).join(path)
                });
            }
        }
    }
    // Track conditional and unconditional directives even when their targets
    // contribute no identity values. Also fingerprint their source files.
    let directives = git.query(
        Path::new(&ctx.path),
        &[
            "config",
            "--includes",
            "--null",
            "--show-origin",
            "--show-scope",
            "--get-regexp",
            r"^(include\.path|includeif\..*\.path|extensions\.worktreeconfig)$",
        ],
    )?;
    if directives.code != 0 && directives.code != 1 {
        return Err(AppError::new(
            "config",
            "无法核对配置包含关系，请重新读取。",
        ));
    }
    hash.update(&directives.bytes);
    if !directives.bytes.is_empty() {
        let Some(records) = directives.bytes.strip_suffix(&[0]) else {
            return Err(AppError::new("config_format", "配置来源输出格式不受支持。"));
        };
        let parts: Vec<_> = records.split(|b| *b == 0).collect();
        for record in parts.chunks(3) {
            if record.len() != 3 {
                return Err(AppError::new("config_format", "配置来源输出格式不受支持。"));
            }
            let origin = std::str::from_utf8(record[1])
                .map_err(|_| AppError::new("encoding", "配置来源编码不受支持。"))?;
            if let Some(path) = origin.strip_prefix("file:") {
                let path = Path::new(path);
                files.insert(if path.is_absolute() {
                    path.into()
                } else {
                    Path::new(&ctx.path).join(path)
                });
            }
        }
    }
    for path in files {
        hash.update(path.to_string_lossy().as_bytes());
        let meta = std::fs::metadata(&path)?;
        if meta.len() > 4 * 1024 * 1024 {
            return Err(AppError::new(
                "config_limit",
                "配置文件过大，未创建可确认预览。",
            ));
        }
        hash.update(std::fs::read(&path)?);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn preview(
    git: &Git,
    entry: &Entry,
    changes: Vec<Change>,
    affected: Vec<String>,
) -> Result<Preview> {
    validate_changes(&changes)?;
    let ctx = git.discover(Path::new(&entry.path))?;
    let path = target(&ctx);
    let meta = std::fs::symlink_metadata(&path)?;
    if meta.file_type().is_symlink() {
        return Err(AppError::new(
            "config_link",
            "目标 config 是链接；当前版本仅支持读取，不通过此入口写入。",
        ));
    }
    let baseline = fingerprint(git, &ctx)?;
    let id = identity::read(git, Path::new(&ctx.path), &entry.policy)?;
    let mut warnings = vec![
        "本次只修改 local 的基础姓名、邮箱，不修改作者/提交者、worktree 专属配置或历史。".into(),
        "修改后的继承与包含关系可能复杂，预计有效值为未知；保存后重新读取验证。".into(),
        "共享配置可能影响其他工作树，包括尚未收录的工作树；此处列表可能不完整。".into(),
    ];
    if id.keys.iter().any(|k| {
        k.key.starts_with("author.") && !k.values.is_empty()
            || k.key.starts_with("committer.") && !k.values.is_empty()
            || k.values
                .iter()
                .any(|v| v.scope == "worktree" || v.scope == "command")
    }) {
        warnings.push(
            "存在已知覆盖项，本次不会修改这些项；选中工作树的相关有效配置可能保持原值。".into(),
        );
    }
    let mut items = Vec::new();
    for change in changes {
        let old = local_values(git, &ctx, &change.key)?;
        let effective = id
            .keys
            .iter()
            .find(|k| k.key == change.key)
            .and_then(ConfigKey::effective)
            .map(String::from);
        items.push(PreviewItem {
            key: change.key,
            old,
            new: change.value,
            effective,
        });
    }
    if fingerprint(git, &ctx)? != baseline {
        return Err(AppError::new(
            "preview_stale",
            "生成预览时配置或上下文发生变化，请重新读取后预览。",
        ));
    }
    Ok(Preview {
        token: uuid::Uuid::new_v4().to_string(),
        entry_id: entry.id.clone(),
        target: path.to_string_lossy().into(),
        fingerprint: baseline,
        context: ctx,
        items,
        warnings,
        affected,
        created_at: now(),
    })
}
pub fn apply(git: &Git, preview: Preview) -> Result<WriteResult> {
    if now().saturating_sub(preview.created_at) > 300 {
        return Err(AppError::new(
            "preview_expired",
            "预览已超过 5 分钟，请重新预览并确认。",
        ));
    }
    let ctx = git.discover(Path::new(&preview.context.path))?;
    if fingerprint(git, &ctx)? != preview.fingerprint {
        return Err(AppError::new(
            "preview_stale",
            "配置、来源或仓库上下文已变化。未执行写入，请重新预览并确认新差异。",
        ));
    }
    let mut items = Vec::new();
    if let Err(error) = commit_changes(git, &ctx, &preview) {
        if error.code == "preview_stale" {
            return Err(error);
        }
        return Ok(WriteResult {
            entry_id: preview.entry_id,
            state: "failed".into(),
            items: preview
                .items
                .iter()
                .map(|item| ItemResult {
                    key: item.key.clone(),
                    state: "failed".into(),
                    message: error.message.clone(),
                })
                .collect(),
        });
    }
    for item in &preview.items {
        let verified = local_values(git, &ctx, &item.key);
        match verified {
            Ok(values)
                if match &item.new {
                    Some(v) => values == vec![v.clone()],
                    None => values.is_empty(),
                } =>
            {
                let effective = git.config(Path::new(&ctx.path), &item.key);
                let state = match effective {
                    Ok(values) => {
                        if item
                            .new
                            .as_ref()
                            .is_none_or(|new| values.last().is_some_and(|v| &v.value == new))
                        {
                            "applied"
                        } else {
                            "overridden"
                        }
                    }
                    Err(_) => "unverified",
                };
                items.push(ItemResult {
                    key: item.key.clone(),
                    state: state.into(),
                    message: match state {
                        "applied" => "local 结果已验证；当前生效值已重新读取。",
                        "overridden" => "local 写入已验证，但当前基础配置仍被覆盖。",
                        _ => "写入后无法完整验证生效结果。",
                    }
                    .into(),
                });
            }
            _ => items.push(ItemResult {
                key: item.key.clone(),
                state: "unverified".into(),
                message: "写入后无法验证，或配置再次发生变化，请刷新后检查。".into(),
            }),
        }
    }
    let failed = items.iter().any(|i| i.state == "failed");
    let succeeded = items.iter().any(|i| i.state != "failed");
    let state = if failed && succeeded {
        "partial"
    } else if failed {
        "failed"
    } else if items.len() != preview.items.len() || items.iter().any(|i| i.state == "unverified") {
        "unverified"
    } else if items.iter().any(|i| i.state == "overridden") {
        "overridden"
    } else {
        "applied"
    };
    Ok(WriteResult {
        entry_id: preview.entry_id,
        state: state.into(),
        items,
    })
}
