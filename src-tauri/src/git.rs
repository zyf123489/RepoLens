use crate::model::*;
use std::{
    ffi::OsString,
    io::Read,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct Git {
    pub executable: String,
    config_root: Option<std::path::PathBuf>,
}
pub struct Output {
    pub code: i32,
    pub bytes: Vec<u8>,
}
const MAX_OUTPUT: usize = 16 * 1024 * 1024;

impl Git {
    pub fn new(path: String) -> Self {
        Self {
            executable: path,
            config_root: None,
        }
    }
    /// Isolate integration fixtures from the host's personal configuration.
    pub fn isolated(path: String, config_file: std::path::PathBuf) -> Self {
        Self {
            executable: path,
            config_root: Some(config_file),
        }
    }
    pub(crate) fn run(&self, cwd: &Path, args: &[OsString]) -> Result<Output> {
        let mut command = Command::new(&self.executable);
        command
            .current_dir(cwd)
            .args([
                "--no-pager",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
            ])
            .args(args)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", "")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        for key in [
            "GIT_DIR",
            "GIT_WORK_TREE",
            "GIT_COMMON_DIR",
            "GIT_INDEX_FILE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_NAMESPACE",
            "GIT_PREFIX",
        ] {
            command.env_remove(key);
        }
        if let Some(config) = &self.config_root {
            command
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", config)
                .env_remove("GIT_CONFIG_COUNT")
                .env_remove("GIT_CONFIG_PARAMETERS");
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().map_err(|_| {
            AppError::new(
                "git_missing",
                "无法启动所选 Git，请在设置中检查可执行文件。",
            )
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::new("git_io", "无法读取 Git 输出。"))?;
        let reader = thread::spawn(move || {
            let mut bytes = Vec::new();
            let read = stdout.take((MAX_OUTPUT + 1) as u64).read_to_end(&mut bytes);
            (read, bytes)
        });
        let start = Instant::now();
        let code = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status.code().unwrap_or(-1),
                Ok(None) if start.elapsed() < Duration::from_secs(20) => {
                    thread::sleep(Duration::from_millis(15))
                }
                other => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = reader.join();
                    return Err(AppError::new(
                        if other.is_err() {
                            "git_process"
                        } else {
                            "timeout"
                        },
                        "Git 查询超时或进程异常，请检查仓库后重试。",
                    ));
                }
            }
        };
        let (read, bytes) = reader
            .join()
            .map_err(|_| AppError::new("git_io", "Git 输出读取中断。"))?;
        read.map_err(|_| AppError::new("git_io", "无法读取 Git 输出。"))?;
        if bytes.len() > MAX_OUTPUT {
            return Err(AppError::new(
                "output_limit",
                "Git 输出超过安全上限，未将截断数据作为完整结果。",
            ));
        }
        Ok(Output { code, bytes })
    }
    pub(crate) fn query(&self, cwd: &Path, args: &[&str]) -> Result<Output> {
        self.run(cwd, &args.iter().map(OsString::from).collect::<Vec<_>>())
    }
    pub(crate) fn text(&self, cwd: &Path, args: &[&str]) -> Result<String> {
        let out = self.query(cwd, args)?;
        if out.code != 0 {
            return Err(AppError::new(
                "git_read",
                "Git 无法读取此信息，请检查配置语法、仓库权限及安全目录信任设置。",
            ));
        }
        String::from_utf8(out.bytes)
            .map(|s| s.trim_end_matches(['\r', '\n']).to_string())
            .map_err(|_| AppError::new("encoding", "Git 输出不是有效 UTF-8，无法安全展示。"))
    }
    pub fn version(&self) -> Result<String> {
        let version = self.text(Path::new("."), &["--version"])?;
        let nums: Vec<u32> = version
            .split_whitespace()
            .nth(2)
            .unwrap_or("")
            .split('.')
            .take(2)
            .map(|n| n.parse().unwrap_or(0))
            .collect();
        if nums.len() < 2 || nums[0] < 2 || (nums[0] == 2 && nums[1] < 46) {
            return Err(AppError::new(
                "git_version",
                "需要 Git 2.46 或更高版本，以确保部分克隆仓库不会自动获取缺失对象。",
            ));
        }
        Ok(version)
    }
    pub fn discover(&self, path: &Path) -> Result<Context> {
        let canonical = std::fs::canonicalize(path)
            .map_err(|_| AppError::new("path", "路径不存在或不可访问，收录记录会保留。"))?;
        if !canonical.is_dir() {
            return Err(AppError::new("path", "请选择仓库目录。"));
        }
        let bare = self.text(&canonical, &["rev-parse", "--is-bare-repository"])? == "true";
        let git_dir = self.text(&canonical, &["rev-parse", "--absolute-git-dir"])?;
        let common_dir = self.text(
            &canonical,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?;
        let root = if bare {
            git_dir.clone()
        } else {
            self.text(&canonical, &["rev-parse", "--show-toplevel"])?
        };
        let parent = if bare {
            None
        } else {
            self.text(
                &canonical,
                &["rev-parse", "--show-superproject-working-tree"],
            )
            .ok()
            .filter(|s| !s.is_empty())
        };
        let kind = if bare {
            "bare"
        } else if parent.is_some() {
            "submodule"
        } else if !same_path(Path::new(&git_dir), Path::new(&common_dir)) {
            "worktree"
        } else {
            "normal"
        };
        Ok(Context {
            path: root,
            git_dir,
            common_dir,
            kind: kind.into(),
            parent,
        })
    }
    pub fn config(&self, cwd: &Path, key: &str) -> Result<Vec<ConfigValue>> {
        let out = self.query(
            cwd,
            &[
                "config",
                "--includes",
                "--null",
                "--show-origin",
                "--show-scope",
                "--get-all",
                key,
            ],
        )?;
        if out.code == 1 {
            return Ok(vec![]);
        }
        if out.code != 0 {
            return Err(AppError::new(
                "config",
                "身份或认证配置无法解析，请检查 Git 配置语法。",
            ));
        }
        let parts: Vec<_> = out.bytes.split(|b| *b == 0).collect();
        let mut values = Vec::new();
        for chunk in parts[..parts.len().saturating_sub(1)].chunks(3) {
            if chunk.len() != 3 {
                return Err(AppError::new("config_format", "Git 来源输出格式不受支持。"));
            }
            let strings: std::result::Result<Vec<String>, _> = chunk
                .iter()
                .map(|p| String::from_utf8(p.to_vec()))
                .collect();
            let s = strings.map_err(|_| AppError::new("encoding", "配置值编码不受支持。"))?;
            values.push(ConfigValue {
                scope: s[0].clone(),
                origin: s[1].clone(),
                value: s[2].clone(),
            });
        }
        Ok(values)
    }
    pub fn head(&self, cwd: &Path) -> Result<Head> {
        let branch = self.query(cwd, &["symbolic-ref", "--quiet", "--short", "HEAD"])?;
        let oid = self.query(cwd, &["rev-parse", "--verify", "HEAD"])?;
        if ![0, 1].contains(&branch.code) || ![0, 128].contains(&oid.code) {
            return Err(AppError::new("head", "无法读取 HEAD。"));
        }
        let oid = if oid.code == 0 {
            Some(String::from_utf8_lossy(&oid.bytes).trim().into())
        } else {
            None
        };
        let branch = if branch.code == 0 {
            Some(String::from_utf8_lossy(&branch.bytes).trim().into())
        } else {
            None
        };
        Ok(Head {
            detached: branch.is_none(),
            unborn: oid.is_none(),
            branch,
            oid,
        })
    }
    pub fn status(&self, cwd: &Path) -> Result<WorkStatus> {
        let out = self.query(
            cwd,
            &[
                "status",
                "--porcelain=v2",
                "-z",
                "--untracked-files=all",
                "--ignore-submodules=all",
            ],
        )?;
        if out.code != 0 {
            return Err(AppError::new(
                "work_status",
                "工作区状态读取失败，不能据此判断零修改。",
            ));
        }
        parse_status(&out.bytes)
    }
    pub fn history(&self, cwd: &Path) -> Result<Commit> {
        let out = self.text(
            cwd,
            &[
                "log",
                "-1",
                "--no-show-signature",
                "--no-use-mailmap",
                "--format=%H%x00%aI%x00%s%x00%an%x00%ae",
            ],
        )?;
        let p: Vec<_> = out.split('\0').collect();
        if p.len() != 5 {
            return Err(AppError::new("history", "最近提交格式无法解析。"));
        }
        Ok(Commit {
            oid: p[0].into(),
            date: p[1].into(),
            subject: p[2].into(),
            author: p[3].into(),
            email: p[4].into(),
        })
    }
    pub fn upstream(&self, cwd: &Path, head: &Head) -> Result<Upstream> {
        let configured = if let Some(branch) = &head.branch {
            !self
                .config(cwd, &format!("branch.{branch}.merge"))?
                .is_empty()
        } else {
            false
        };
        if !configured {
            return Ok(Upstream {
                state: "unconfigured".into(),
                name: None,
                ahead: None,
                behind: None,
            });
        }
        let name = self.text(
            cwd,
            &[
                "rev-parse",
                "--abbrev-ref",
                "--symbolic-full-name",
                "@{upstream}",
            ],
        );
        let Ok(name) = name else {
            return Ok(Upstream {
                state: "missing".into(),
                name: None,
                ahead: None,
                behind: None,
            });
        };
        let counts = self.text(
            cwd,
            &["rev-list", "--left-right", "--count", "HEAD...@{upstream}"],
        )?;
        let p: Vec<usize> = counts
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if p.len() != 2 {
            return Err(AppError::new("upstream", "无法计算本地上游差异。"));
        }
        Ok(Upstream {
            state: "available".into(),
            name: Some(name),
            ahead: Some(p[0]),
            behind: Some(p[1]),
        })
    }
}

pub fn same_path(a: &Path, b: &Path) -> bool {
    same_file::is_same_file(a, b).unwrap_or_else(|_| a == b)
}

pub fn parse_status(bytes: &[u8]) -> Result<WorkStatus> {
    let mut stats = WorkStatus::default();
    let mut records = bytes.split(|b| *b == 0);
    while let Some(r) = records.next() {
        if r.is_empty() {
            continue;
        }
        match r[0] {
            b'?' => stats.untracked += 1,
            b'1' | b'2' | b'u' if r.len() > 4 => {
                if r[2] != b'.' {
                    stats.staged += 1;
                }
                if r[3] != b'.' {
                    stats.unstaged += 1;
                }
                if r[0] == b'u' {
                    stats.conflicts += 1;
                }
                if r[0] == b'2' {
                    records.next();
                }
            }
            b'!' | b'#' => (),
            _ => return Err(AppError::new("status_format", "工作区输出格式无法解析。")),
        }
    }
    Ok(stats)
}
