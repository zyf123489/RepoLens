use crate::{
    git::{same_path, Git},
    model::*,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    pub roots: Vec<String>,
    pub max_depth: usize,
    pub excluded_names: Vec<String>,
    pub excluded_paths: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanState {
    pub id: String,
    pub state: String,
    pub visited: usize,
    pub found: Vec<Context>,
    pub issues: Vec<String>,
}
pub struct ScanJob {
    pub cancel: Arc<AtomicBool>,
    pub state: Arc<Mutex<ScanState>>,
}
pub fn start(git: Git, options: ScanOptions) -> Result<ScanJob> {
    git.version()?;
    if options.roots.is_empty() || options.roots.len() > 32 || options.max_depth > 64 {
        return Err(AppError::new(
            "scan_input",
            "请选择扫描目录，深度范围为 0–64，根目录不超过 32 个。",
        ));
    }
    if options
        .excluded_names
        .iter()
        .any(|s| s.is_empty() || s.contains(['/', '\\']))
        || options.excluded_paths.iter().any(|s| {
            Path::new(s).is_absolute() || s.replace('\\', "/").split('/').any(|p| p == "..")
        })
    {
        return Err(AppError::new(
            "scan_input",
            "排除项需要是目录名或根目录内的相对路径。",
        ));
    }
    let cancel = Arc::new(AtomicBool::new(false));
    let state = Arc::new(Mutex::new(ScanState {
        id: uuid::Uuid::new_v4().to_string(),
        state: "running".into(),
        ..Default::default()
    }));
    let job = ScanJob {
        cancel: cancel.clone(),
        state: state.clone(),
    };
    std::thread::spawn(move || {
        let mut visited = HashSet::new();
        let mut stack: Vec<(PathBuf, PathBuf, usize)> = options
            .roots
            .iter()
            .map(|s| (PathBuf::from(s), PathBuf::from(s), 0))
            .collect();
        while let Some((path, root, depth)) = stack.pop() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            if path
                .components()
                .any(|part| eq_segment(&part.as_os_str().to_string_lossy(), ".git"))
            {
                continue;
            }
            let meta = match std::fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    issue(&state, &path, "目录不可访问，已跳过。");
                    continue;
                }
            };
            if !meta.is_dir() || is_link(&meta) {
                continue;
            }
            let canonical = match std::fs::canonicalize(&path) {
                Ok(p) => p,
                Err(_) => {
                    issue(&state, &path, "无法规范化目录，已跳过。");
                    continue;
                }
            };
            if !visited.insert(canonical.clone()) {
                continue;
            }
            state.lock().unwrap().visited += 1;
            // Only candidates are probed, preventing every child directory from rediscovering its parent.
            let candidate = path.join(".git").exists()
                || (path.join("HEAD").is_file() && path.join("objects").is_dir())
                || depth == 0;
            let mut bare = false;
            if candidate {
                match git.discover(&path) {
                    Ok(ctx) => {
                        bare = ctx.kind == "bare";
                        if same_path(Path::new(&ctx.path), &canonical) {
                            let mut s = state.lock().unwrap();
                            if !s
                                .found
                                .iter()
                                .any(|c| same_path(Path::new(&c.path), Path::new(&ctx.path)))
                            {
                                s.found.push(ctx);
                            }
                        }
                    }
                    Err(_) if path.join(".git").exists() => issue(
                        &state,
                        &path,
                        "检测到仓库标记但 Git 无法读取，请检查配置、权限或信任设置。",
                    ),
                    _ => (),
                }
            }
            if bare || depth >= options.max_depth {
                continue;
            }
            let children = match std::fs::read_dir(&path) {
                Ok(c) => c,
                Err(_) => {
                    issue(&state, &path, "部分子目录无权限，已继续其他目录。");
                    continue;
                }
            };
            for child in children {
                let Ok(child) = child else {
                    issue(&state, &path, "部分目录项读取失败。");
                    continue;
                };
                let child_path = child.path();
                let name = child.file_name().to_string_lossy().into_owned();
                if name == ".git" || options.excluded_names.iter().any(|s| eq_segment(&name, s)) {
                    continue;
                }
                if excluded(&child_path, &root, &options.excluded_paths) {
                    continue;
                }
                stack.push((child_path, root.clone(), depth + 1));
            }
        }
        state.lock().unwrap().state = if cancel.load(Ordering::Relaxed) {
            "cancelled"
        } else {
            "completed"
        }
        .into();
    });
    Ok(job)
}
fn issue(state: &Arc<Mutex<ScanState>>, path: &Path, message: &str) {
    let mut s = state.lock().unwrap();
    if s.issues.len() < 200 {
        s.issues.push(format!("{}：{message}", path.display()));
    } else if s.issues.len() == 200 {
        s.issues
            .push("扫描问题超过 200 项；其余问题未逐项记录，请缩小扫描范围后重试。".into());
    }
}
fn eq_segment(a: &str, b: &str) -> bool {
    #[cfg(windows)]
    {
        a.eq_ignore_ascii_case(b)
    }
    #[cfg(not(windows))]
    {
        a == b
    }
}
fn excluded(path: &Path, root: &Path, exclusions: &[String]) -> bool {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    exclusions.iter().any(|e| {
        let normalized = e.replace('\\', "/");
        let a: Vec<_> = relative.split('/').collect();
        let b: Vec<_> = normalized.trim_matches('/').split('/').collect();
        b.len() <= a.len() && b.iter().zip(a.iter()).all(|(b, a)| eq_segment(a, b))
    })
}
fn is_link(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}
