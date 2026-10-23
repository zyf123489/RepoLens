//! Local, isolated measurements. Never imports or edits a user's repositories.
use repolens_core::{
    auth,
    git::Git,
    identity,
    model::Policy,
    scan::{self, ScanOptions},
};
use serde_json::json;
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

fn git(cwd: &Path, global: &Path, args: &[&str]) {
    assert!(Command::new("git")
        .current_dir(cwd)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", global)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env_remove("GIT_CONFIG_COUNT")
        .env_remove("GIT_CONFIG_PARAMETERS")
        .output()
        .unwrap()
        .status
        .success());
}
fn main() {
    let dir = tempfile::tempdir().unwrap();
    let global = dir.path().join("global");
    std::fs::write(&global, "").unwrap();
    let g = Git::isolated("git".into(), global.clone());
    let mut paths = Vec::<PathBuf>::new();
    for index in 0..500 {
        if index % 10 == 9 {
            let source = &paths[index - 1];
            let path = dir.path().join(format!("tree-{index}"));
            git(
                source,
                &global,
                &[
                    "-c",
                    "core.hooksPath=",
                    "commit",
                    "--allow-empty",
                    "-m",
                    "fixture",
                ],
            );
            git(
                source,
                &global,
                &[
                    "worktree",
                    "add",
                    "-b",
                    &format!("tree-{index}"),
                    path.to_str().unwrap(),
                ],
            );
            paths.push(path);
        } else {
            let path = dir.path().join(format!("repo-{index}"));
            std::fs::create_dir(&path).unwrap();
            git(&path, &global, &["init", "--initial-branch=main"]);
            git(&path, &global, &["config", "user.name", "Benchmark"]);
            git(
                &path,
                &global,
                &["config", "user.email", "benchmark@example.com"],
            );
            if index == 0 {
                // One larger untracked working directory, including a deeper path.
                let nested = path.join("a/b/c/d/e");
                std::fs::create_dir_all(&nested).unwrap();
                for file in 0..10_000 {
                    std::fs::write(nested.join(format!("file-{file}.txt")), "fixture").unwrap();
                }
            }
            paths.push(path);
        }
    }
    let mut rows = Vec::new();
    for count in [10, 100, 500] {
        let start = Instant::now();
        let next = Arc::new(AtomicUsize::new(0));
        let errors = Arc::new(AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..3 {
                let paths = &paths;
                let next = next.clone();
                let errors = errors.clone();
                let g = &g;
                scope.spawn(move || loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= count {
                        break;
                    }
                    let path = &paths[index];
                    let read = || -> repolens_core::model::Result<()> {
                        let ctx = g.discover(path)?;
                        let head = g.head(path)?;
                        g.status(path)?;
                        if !head.unborn {
                            g.history(path)?;
                        }
                        g.upstream(path, &head)?;
                        identity::read(g, path, &Policy::default())?;
                        auth::read(g, &ctx)?;
                        Ok(())
                    };
                    if read().is_err() {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                });
            }
        });
        rows.push(json!({"entries":count, "independentRepositories":count-count/10, "linkedWorktrees":count/10,
            "untrackedFiles":10000, "maximumDirectoryDepth":6, "workers":3,
            "refreshMs":start.elapsed().as_millis(), "errors":errors.load(Ordering::Relaxed)}));
    }
    let start = Instant::now();
    let job = scan::start(
        g,
        ScanOptions {
            roots: vec![dir.path().to_string_lossy().into()],
            max_depth: 1,
            excluded_names: vec![],
            excluded_paths: vec![],
        },
    )
    .unwrap();
    loop {
        if job.state.lock().unwrap().state != "running" {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let state = job.state.lock().unwrap();
    println!("{}", serde_json::to_string_pretty(&json!({"fixture":"isolated temporary repositories; one 10000-file untracked directory", "measurements":rows, "scan":{"durationMs":start.elapsed().as_millis(), "found":state.found.len(), "state":state.state}})).unwrap());
}
