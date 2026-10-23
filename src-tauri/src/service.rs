use crate::{
    auth,
    edit::{self, Change, Preview, WriteResult},
    git::{same_path, Git},
    identity,
    model::*,
    scan::{self, ScanJob, ScanOptions, ScanState},
    store::Store,
};
use std::{
    collections::HashMap,
    path::Path,
    sync::{atomic::Ordering, Arc, Mutex},
};

pub struct Service {
    pub store: Mutex<Store>,
    pub scans: Mutex<HashMap<String, ScanJob>>,
    previews: Mutex<HashMap<String, Preview>>,
    writes: Mutex<()>,
}
impl Service {
    pub fn open(dir: &Path) -> Result<Arc<Self>> {
        Ok(Arc::new(Self {
            store: Mutex::new(Store::open(dir)?),
            scans: Mutex::new(HashMap::new()),
            previews: Mutex::new(HashMap::new()),
            writes: Mutex::new(()),
        }))
    }
    pub fn git(&self) -> Git {
        Git::new(
            self.store
                .lock()
                .unwrap()
                .setting("git")
                .unwrap_or_else(|| "git".into()),
        )
    }
    pub fn diagnostics(&self) -> Diagnostics {
        let git = self.git();
        let v = git.version();
        Diagnostics {
            git: git.executable,
            git_version: v.as_ref().ok().cloned(),
            git_error: v.err(),
            data_dir: self.store.lock().unwrap().dir.to_string_lossy().into(),
            version: env!("CARGO_PKG_VERSION").into(),
        }
    }
    pub fn entries(&self) -> Result<Vec<Entry>> {
        self.store.lock().unwrap().entries()
    }
    pub fn inspect(&self, path: &str) -> Result<Context> {
        let git = self.git();
        git.version()?;
        git.discover(Path::new(path))
    }
    pub fn add(&self, paths: Vec<String>) -> Result<Vec<Entry>> {
        if paths.len() > 500 {
            return Err(AppError::new("limit", "每次最多收录 500 个条目。"));
        }
        let git = self.git();
        git.version()?;
        for path in paths {
            let ctx = git.discover(Path::new(&path))?;
            self.store.lock().unwrap().add(&ctx)?;
        }
        self.entries()
    }
    pub fn snapshot(&self, id: &str) -> Result<Snapshot> {
        let entry = self.store.lock().unwrap().entry(id)?;
        let git = self.git();
        let context = git
            .version()
            .and_then(|_| git.discover(Path::new(&entry.path)));
        let mut s = Snapshot {
            entry_id: id.into(),
            git: git.executable.clone(),
            read_at: now(),
            context: Module::from_result(context),
            head: Module::empty("unavailable"),
            work: Module::empty("unavailable"),
            history: Module::empty("unavailable"),
            upstream: Module::empty("unavailable"),
            identity: Module::empty("unavailable"),
            remotes: Module::empty("unavailable"),
        };
        let Some(ctx) = s.context.data.as_ref() else {
            return Ok(s);
        };
        let path = Path::new(&ctx.path);
        s.head = Module::from_result(git.head(path));
        s.work = if ctx.kind == "bare" {
            Module::empty("notApplicable")
        } else {
            Module::from_result(git.status(path))
        };
        s.history = if s.head.data.as_ref().is_some_and(|h| h.unborn) {
            Module::empty("empty")
        } else {
            Module::from_result(git.history(path))
        };
        s.upstream = if let Some(h) = &s.head.data {
            Module::from_result(git.upstream(path, h))
        } else {
            Module::empty("unavailable")
        };
        s.identity = identity::read_module(&git, path, &entry.policy);
        s.remotes = Module::from_result(auth::read(&git, ctx));
        Ok(s)
    }
    pub fn preview(&self, id: &str, changes: Vec<Change>) -> Result<Preview> {
        let entry = self.store.lock().unwrap().entry(id)?;
        let git = self.git();
        let ctx = git.discover(Path::new(&entry.path))?;
        let entries = self.entries()?;
        let mut affected = vec![];
        for e in entries {
            if let Ok(other) = git.discover(Path::new(&e.path)) {
                if same_path(Path::new(&ctx.common_dir), Path::new(&other.common_dir)) {
                    affected.push(e.path);
                }
            }
        }
        let preview = edit::preview(&git, &entry, changes, affected)?;
        let mut previews = self.previews.lock().unwrap();
        previews.retain(|_, p| now().saturating_sub(p.created_at) <= 300);
        if previews.len() > 100 {
            return Err(AppError::new(
                "preview_limit",
                "待确认预览过多，请稍后重试。",
            ));
        }
        previews.insert(preview.token.clone(), preview.clone());
        Ok(preview)
    }
    pub fn apply(&self, token: &str) -> Result<WriteResult> {
        let _guard = self.writes.lock().unwrap();
        let preview = self.previews.lock().unwrap().remove(token).ok_or_else(|| {
            AppError::new("preview", "确认预览不存在、已使用或已过期，请重新预览。")
        })?;
        let entry = self.store.lock().unwrap().entry(&preview.entry_id)?;
        if !same_path(Path::new(&entry.path), Path::new(&preview.context.path)) {
            return Err(AppError::new("preview", "条目路径已变化，请重新预览。"));
        }
        edit::apply(&self.git(), preview)
    }
    pub fn scan_start(&self, options: ScanOptions) -> Result<String> {
        let mut jobs = self.scans.lock().unwrap();
        if jobs.values().any(|j| j.snapshot().state == "running") {
            return Err(AppError::new(
                "scan_busy",
                "已有扫描正在执行，请先停止或等待完成。",
            ));
        }
        jobs.clear();
        let job = scan::start(self.git(), options)?;
        let id = job.snapshot().id;
        jobs.insert(id.clone(), job);
        Ok(id)
    }
    pub fn scan_poll(&self, id: &str) -> Result<ScanState> {
        let jobs = self.scans.lock().unwrap();
        let job = jobs
            .get(id)
            .ok_or_else(|| AppError::new("scan", "扫描任务不存在。"))?;
        Ok(job.snapshot())
    }
    pub fn scan_cancel(&self, id: &str) -> Result<()> {
        let jobs = self.scans.lock().unwrap();
        let job = jobs
            .get(id)
            .ok_or_else(|| AppError::new("scan", "扫描任务不存在。"))?;
        job.cancel.store(true, Ordering::Relaxed);
        Ok(())
    }
}
