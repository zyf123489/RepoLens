use repolens_core::{
    auth,
    edit::{self, Change},
    git::{parse_status, Git},
    identity,
    model::*,
    scan::{self, ScanOptions},
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::Ordering,
    time::Duration,
};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    repo: PathBuf,
    global: PathBuf,
    git: Git,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("中文 repo");
        let global = dir.path().join("global.config");
        std::fs::create_dir(&repo).unwrap();
        std::fs::write(&global, "").unwrap();
        let f = Self {
            git: Git::isolated("git".into(), global.clone()),
            dir,
            repo,
            global,
        };
        f.cmd(&f.repo, &["init", "--initial-branch=main"]);
        f.set("user.name", "Example User");
        f.set("user.email", "user@company.com");
        f
    }
    fn cmd(&self, cwd: &Path, args: &[&str]) -> String {
        let o = Command::new("git")
            .current_dir(cwd)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", &self.global)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env_remove("GIT_CONFIG_COUNT")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .output()
            .unwrap();
        assert!(o.status.success(), "fixture Git failed: {:?}", args);
        String::from_utf8_lossy(&o.stdout).trim().into()
    }
    fn set(&self, key: &str, value: &str) {
        self.cmd(&self.repo, &["config", "--local", key, value]);
    }
    fn entry(&self) -> Entry {
        let ctx = self.git.discover(&self.repo).unwrap();
        Entry {
            id: "fixture".into(),
            path: ctx.path,
            alias: String::new(),
            group: None,
            kind: ctx.kind,
            policy: Policy::default(),
        }
    }
    fn commit(&self) {
        self.cmd(
            &self.repo,
            &[
                "-c",
                "core.hooksPath=",
                "commit",
                "--allow-empty",
                "-m",
                "fixture",
            ],
        );
    }
}
fn key(k: &str, value: Option<&str>) -> ConfigKey {
    ConfigKey {
        key: k.into(),
        values: value
            .map(|s| {
                vec![ConfigValue {
                    value: s.into(),
                    scope: "local".into(),
                    origin: "file:test".into(),
                }]
            })
            .unwrap_or_default(),
    }
}
fn identity_keys(author: Option<&str>) -> Vec<ConfigKey> {
    vec![
        key("user.name", Some("User")),
        key("user.email", Some("a@company.com")),
        key("author.email", author),
    ]
}

#[test]
fn author_mismatch_cannot_be_hidden_by_base_match() {
    let p = Policy {
        email: Some("a@company.com".into()),
        ..Default::default()
    };
    let i = identity::evaluate(identity_keys(Some("a@personal.com")), &p);
    assert_eq!(i.status, "mismatch");
    assert_eq!(
        i.findings
            .iter()
            .find(|f| f.field == "user.email")
            .unwrap()
            .state,
        "match"
    );
}
#[test]
fn missing_without_rules_needs_attention() {
    let i = identity::evaluate(vec![key("user.name", Some("User"))], &Policy::default());
    assert_eq!(i.status, "missing");
}
#[test]
fn different_unconstrained_override_needs_review() {
    let i = identity::evaluate(identity_keys(Some("a@personal.com")), &Policy::default());
    assert_eq!(i.status, "review");
}
#[test]
fn empty_override_is_not_missing_fallback() {
    let p = Policy {
        email: Some("a@company.com".into()),
        ..Default::default()
    };
    let i = identity::evaluate(identity_keys(Some("")), &p);
    assert_eq!(i.status, "mismatch");
    assert_eq!(
        i.findings
            .iter()
            .find(|f| f.field == "author.email")
            .unwrap()
            .value
            .as_deref(),
        Some("")
    );
}
#[test]
fn domain_rules_and_conflicts() {
    let p = Policy {
        email: Some("A@COMPANY.COM".into()),
        domains: vec!["company.com".into()],
        ..Default::default()
    };
    assert!(identity::validate_policy(&p).is_ok());
    let bad = Policy {
        email: Some("a@sub.company.com".into()),
        ..p
    };
    assert!(identity::validate_policy(&bad).is_err());
}
#[test]
fn constraints_are_and_and_exact_email_is_case_sensitive() {
    let p = Policy {
        email: Some("A@company.com".into()),
        domains: vec!["company.com".into(), "other.com".into()],
        ..Default::default()
    };
    assert_eq!(
        identity::evaluate(identity_keys(None), &p).status,
        "mismatch"
    );
}
#[test]
fn no_rules_is_not_passed() {
    assert_eq!(
        identity::evaluate(identity_keys(None), &Policy::default()).status,
        "unconfigured"
    );
}
#[test]
fn status_counts_overlap_and_rename_source_is_skipped() {
    let bytes=b"1 MM N... 100644 100644 100644 a b file\0? newline\nfile\0\x32 R. N... 100644 100644 100644 a b R100 target\0source\0";
    let s = parse_status(bytes).unwrap();
    assert_eq!((s.staged, s.unstaged, s.untracked), (2, 1, 1));
}
#[test]
fn address_secrets_are_removed() {
    let out = auth::redact_address("https://user:secret@example.com/repo?opaque=secret#secret");
    assert!(!out.contains("secret"));
    assert!(!out.contains("user"));
    assert!(!auth::redact_address("ext::command secret-inline-credential").contains("secret"));
    assert_eq!(auth::redact_address(r"C:\source repo"), r"C:\source repo");
    assert!(out.contains("example.com"));
    assert_eq!(
        auth::redact_address("git@alias:group/repo.git"),
        "alias:group/repo.git"
    );
}
#[test]
fn ssh_is_static_and_first_value_wins() {
    let text="Host work\n HostName github.com\n User git\n IdentityFile ~/.ssh/work\nHost *\n User fallback\nMatch exec touch-secret\n HostName evil\n";
    let (values, issues) = auth::parse_ssh(text, "work");
    assert_eq!(values["hostname"], "github.com");
    assert_eq!(values["user"], "git");
    assert!(!issues.is_empty());
}
#[test]
fn ssh_host_negation() {
    let (v, _) = auth::parse_ssh("Host * !excluded\nUser test\n", "excluded");
    assert!(v.is_empty());
}
#[test]
fn reads_include_and_sources_without_full_chain() {
    let f = Fixture::new();
    let included = f.dir.path().join("included.config");
    std::fs::write(&included, "[user]\n email = included@company.com\n").unwrap();
    f.set("include.path", included.to_str().unwrap());
    let v = f.git.config(&f.repo, "user.email").unwrap();
    assert_eq!(v.last().unwrap().value, "included@company.com");
    assert!(v.last().unwrap().origin.contains("included.config"));
}
#[test]
fn conditional_branch_include_is_applied_by_git() {
    let f = Fixture::new();
    let included = f.dir.path().join("branch.config");
    std::fs::write(&included, "[author]\n email = branch@company.com\n").unwrap();
    f.set("includeIf.onbranch:main.path", included.to_str().unwrap());
    assert_eq!(
        f.git
            .config(&f.repo, "author.email")
            .unwrap()
            .last()
            .unwrap()
            .value,
        "branch@company.com"
    );
}
#[test]
fn inspect_internal_directory_and_empty_repository() {
    let f = Fixture::new();
    let child = f.repo.join("nested");
    std::fs::create_dir(&child).unwrap();
    let c = f.git.discover(&child).unwrap();
    assert_eq!(c.kind, "normal");
    assert!(f.git.head(Path::new(&c.path)).unwrap().unborn);
}
#[test]
fn bare_and_worktree_are_distinct_and_share_config() {
    let f = Fixture::new();
    f.commit();
    let linked = f.dir.path().join("linked");
    f.cmd(
        &f.repo,
        &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
    );
    let a = f.git.discover(&f.repo).unwrap();
    let b = f.git.discover(&linked).unwrap();
    assert_ne!(a.path, b.path);
    assert_eq!(a.common_dir, b.common_dir);
    assert_eq!(b.kind, "worktree");
    let bare = f.dir.path().join("bare.git");
    f.cmd(f.dir.path(), &["init", "--bare", bare.to_str().unwrap()]);
    assert_eq!(f.git.discover(&bare).unwrap().kind, "bare");
}
#[test]
fn worktree_override_survives_local_edit() {
    let f = Fixture::new();
    f.set("extensions.worktreeConfig", "true");
    f.cmd(
        &f.repo,
        &["config", "--worktree", "user.email", "own@company.com"],
    );
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.email".into(),
            value: Some("new@company.com".into()),
        }],
        vec![],
    )
    .unwrap();
    assert!(p.warnings.iter().any(|w| w.contains("已知覆盖")));
    let r = edit::apply(&f.git, p).unwrap();
    assert_eq!(r.state, "overridden");
    assert_eq!(
        f.git
            .config(&f.repo, "user.email")
            .unwrap()
            .last()
            .unwrap()
            .value,
        "own@company.com"
    );
}
#[test]
fn preview_does_not_write_and_apply_changes_only_local() {
    let f = Fixture::new();
    let before = std::fs::read(f.repo.join(".git/config")).unwrap();
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.email".into(),
            value: Some("next@company.com".into()),
        }],
        vec![],
    )
    .unwrap();
    assert_eq!(before, std::fs::read(f.repo.join(".git/config")).unwrap());
    let r = edit::apply(&f.git, p).unwrap();
    assert_eq!(r.state, "applied");
    assert_eq!(
        f.git
            .config(&f.repo, "user.email")
            .unwrap()
            .last()
            .unwrap()
            .value,
        "next@company.com"
    );
}
#[test]
fn external_modification_invalidates_preview() {
    let f = Fixture::new();
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.name".into(),
            value: Some("Changed".into()),
        }],
        vec![],
    )
    .unwrap();
    f.set("user.email", "external@company.com");
    assert_eq!(edit::apply(&f.git, p).unwrap_err().code, "preview_stale");
    assert_eq!(
        f.git
            .config(&f.repo, "user.name")
            .unwrap()
            .last()
            .unwrap()
            .value,
        "Example User"
    );
}
#[test]
fn removal_keeps_included_configuration() {
    let f = Fixture::new();
    let include = f.dir.path().join("included.config");
    std::fs::write(&include, "[user]\nemail = inherited@company.com\n").unwrap();
    f.set("include.path", include.to_str().unwrap());
    let original = std::fs::read(&include).unwrap();
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.email".into(),
            value: None,
        }],
        vec![],
    )
    .unwrap();
    edit::apply(&f.git, p).unwrap();
    assert!(
        edit::local_values(&f.git, &f.git.discover(&f.repo).unwrap(), "user.email")
            .unwrap()
            .is_empty()
    );
    assert_eq!(original, std::fs::read(&include).unwrap());
    assert_eq!(
        f.git
            .config(&f.repo, "user.email")
            .unwrap()
            .last()
            .unwrap()
            .value,
        "inherited@company.com"
    );
}
#[test]
fn duplicate_direct_keys_are_replaced_together() {
    let f = Fixture::new();
    f.cmd(
        &f.repo,
        &["config", "--add", "user.email", "duplicate@company.com"],
    );
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.email".into(),
            value: Some("single@company.com".into()),
        }],
        vec![],
    )
    .unwrap();
    assert_eq!(p.items[0].old.len(), 2);
    edit::apply(&f.git, p).unwrap();
    assert_eq!(
        edit::local_values(&f.git, &f.git.discover(&f.repo).unwrap(), "user.email").unwrap(),
        vec!["single@company.com"]
    );
}
#[test]
fn existing_git_lock_is_never_removed() {
    let f = Fixture::new();
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.email".into(),
            value: Some("next@company.com".into()),
        }],
        vec![],
    )
    .unwrap();
    let lock = f.repo.join(".git/config.lock");
    std::fs::write(&lock, "external").unwrap();
    let r = edit::apply(&f.git, p).unwrap();
    assert_eq!(r.state, "failed");
    assert_eq!(std::fs::read_to_string(lock).unwrap(), "external");
}
#[test]
fn malformed_changes_cannot_expand_write_scope() {
    assert!(edit::validate_changes(&[Change {
        key: "core.sshCommand".into(),
        value: Some("anything".into())
    }])
    .is_err());
    assert!(edit::validate_changes(&[Change {
        key: "user.name".into(),
        value: Some("".into())
    }])
    .is_err());
}
#[test]
fn fsmonitor_is_not_executed_when_reading_status() {
    let f = Fixture::new();
    let marker = f.dir.path().join("must-not-exist");
    f.set(
        "core.fsmonitor",
        &format!("echo unsafe > {}", marker.display()),
    );
    f.git.status(&f.repo).unwrap();
    assert!(!marker.exists());
}
#[test]
fn store_remove_keeps_repository_and_policies_are_independent() {
    let f = Fixture::new();
    let store = Store::open(&f.dir.path().join("data")).unwrap();
    let c = f.git.discover(&f.repo).unwrap();
    let e = store.add(&c).unwrap();
    assert_eq!(store.add(&c).unwrap().id, e.id);
    store
        .update(
            &e.id,
            "Alias",
            &Some("Work".into()),
            &Policy {
                email: Some("user@company.com".into()),
                ..Default::default()
            },
        )
        .unwrap();
    store.delete_group("Work").unwrap();
    assert!(store.entry(&e.id).unwrap().group.is_none());
    assert!(store.entry(&e.id).unwrap().policy.email.is_some());
    store.remove(&e.id).unwrap();
    assert!(f.repo.join(".git/config").exists());
}
#[test]
fn scan_depth_exclusions_nested_and_no_auto_collect() {
    let f = Fixture::new();
    let nested = f.repo.join("nested");
    std::fs::create_dir(&nested).unwrap();
    f.cmd(&nested, &["init"]);
    let job = scan::start(
        f.git.clone(),
        ScanOptions {
            roots: vec![f.repo.to_string_lossy().into()],
            max_depth: 1,
            excluded_names: vec![],
            excluded_paths: vec![],
        },
    )
    .unwrap();
    for _ in 0..200 {
        if job.state.lock().unwrap().state != "running" {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let state = job.state.lock().unwrap().clone();
    assert_eq!(state.state, "completed");
    assert_eq!(state.found.len(), 2);
    let store = Store::open(&f.dir.path().join("data")).unwrap();
    assert!(store.entries().unwrap().is_empty());
    let job = scan::start(
        f.git,
        ScanOptions {
            roots: vec![f.repo.to_string_lossy().into()],
            max_depth: 1,
            excluded_names: vec!["nested".into()],
            excluded_paths: vec![],
        },
    )
    .unwrap();
    for _ in 0..200 {
        if job.state.lock().unwrap().state != "running" {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(job.state.lock().unwrap().found.len(), 1);
}
#[test]
fn scan_cancellation_keeps_results() {
    let f = Fixture::new();
    let job = scan::start(
        f.git,
        ScanOptions {
            roots: vec![f.repo.to_string_lossy().into()],
            max_depth: 3,
            excluded_names: vec![],
            excluded_paths: vec![],
        },
    )
    .unwrap();
    job.cancel.store(true, Ordering::Relaxed);
    for _ in 0..100 {
        if job.state.lock().unwrap().state != "running" {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(job.state.lock().unwrap().state, "cancelled");
}

#[test]
fn https_overview_preserves_helpers_and_never_executes_them() {
    let f = Fixture::new();
    f.set(
        "remote.origin.url",
        "https://someone:secret@example.com/repo?token=secret",
    );
    f.cmd(
        &f.repo,
        &["config", "--add", "credential.helper", "manager"],
    );
    f.cmd(&f.repo, &["config", "--add", "credential.helper", ""]);
    let marker = f.dir.path().join("never-run");
    f.cmd(
        &f.repo,
        &[
            "config",
            "--add",
            "credential.helper",
            &format!("!echo secret > {}", marker.display()),
        ],
    );
    let endpoints = auth::read(&f.git, &f.git.discover(&f.repo).unwrap()).unwrap();
    let rendered = serde_json::to_string(&endpoints).unwrap();
    assert!(!rendered.contains("secret"));
    assert!(!marker.exists());
    assert_eq!(endpoints.len(), 2);
    assert!(endpoints[0].metadata.iter().any(|m| m.contains("manager")));
    assert!(endpoints[0].metadata.iter().any(|m| m.contains("重置")));
}

#[test]
fn confirmed_two_key_edit_is_atomic_and_leaves_no_owned_lock() {
    let f = Fixture::new();
    let p = edit::preview(
        &f.git,
        &f.entry(),
        vec![
            Change {
                key: "user.name".into(),
                value: Some("New Name".into()),
            },
            Change {
                key: "user.email".into(),
                value: Some("new@company.com".into()),
            },
        ],
        vec![],
    )
    .unwrap();
    let r = edit::apply(&f.git, p).unwrap();
    assert_eq!(r.state, "applied");
    assert_eq!(r.items.len(), 2);
    assert!(!f.repo.join(".git/config.lock").exists());
    assert_eq!(
        edit::local_values(&f.git, &f.git.discover(&f.repo).unwrap(), "user.name").unwrap(),
        vec!["New Name"]
    );
}

#[test]
fn partial_identity_read_retains_other_known_fields() {
    use std::io::Write;
    let f = Fixture::new();
    std::fs::OpenOptions::new()
        .append(true)
        .open(f.repo.join(".git/config"))
        .unwrap()
        .write_all(b"\n[author]\n email = \xff\n")
        .unwrap();
    let module = identity::read_module(&f.git, &f.repo, &Policy::default());
    assert_eq!(module.state, "partial");
    let known = module.data.unwrap();
    assert_eq!(known.status, "incomplete");
    assert!(known
        .keys
        .iter()
        .any(|k| k.key == "user.name" && k.effective() == Some("Example User")));
}

#[test]
fn initialized_submodule_is_an_independent_entry() {
    let parent = Fixture::new();
    let source = Fixture::new();
    source.commit();
    parent.cmd(
        &parent.repo,
        &[
            "-c",
            "protocol.file.allow=always",
            "submodule",
            "add",
            source.repo.to_str().unwrap(),
            "child",
        ],
    );
    let ctx = parent.git.discover(&parent.repo.join("child")).unwrap();
    assert_eq!(ctx.kind, "submodule");
    assert!(ctx.parent.is_some());
    assert_ne!(
        ctx.common_dir,
        parent.git.discover(&parent.repo).unwrap().common_dir
    );
}

#[test]
fn reading_repository_does_not_modify_config_index_or_refs() {
    let f = Fixture::new();
    f.commit();
    std::fs::write(f.repo.join("tracked.txt"), "original").unwrap();
    f.cmd(&f.repo, &["add", "tracked.txt"]);
    let files = [
        ".git/config",
        ".git/index",
        ".git/HEAD",
        ".git/refs/heads/main",
        "tracked.txt",
    ];
    let before: Vec<_> = files
        .iter()
        .map(|p| std::fs::read(f.repo.join(p)).unwrap())
        .collect();
    let ctx = f.git.discover(&f.repo).unwrap();
    let head = f.git.head(&f.repo).unwrap();
    f.git.status(&f.repo).unwrap();
    f.git.history(&f.repo).unwrap();
    f.git.upstream(&f.repo, &head).unwrap();
    identity::read(&f.git, &f.repo, &Policy::default()).unwrap();
    auth::read(&f.git, &ctx).unwrap();
    for (index, path) in files.iter().enumerate() {
        assert_eq!(
            before[index],
            std::fs::read(f.repo.join(path)).unwrap(),
            "changed {path}"
        );
    }
}

#[test]
fn scoped_helpers_keep_order_and_exclude_nonmatching_hosts() {
    let f = Fixture::new();
    f.set("remote.origin.url", "https://example.com/group/repo");
    f.set("credential.helper", "manager");
    f.set("credential.https://example.com.helper", "cache");
    f.set("credential.https://other.example.helper", "store");
    let endpoints = auth::read(&f.git, &f.git.discover(&f.repo).unwrap()).unwrap();
    let entries = &endpoints[0].metadata;
    assert!(entries[0].contains("manager"));
    assert!(entries[1].contains("cache"));
    assert_eq!(entries.len(), 2);
}

#[test]
fn expired_preview_never_writes() {
    let f = Fixture::new();
    let before = std::fs::read(f.repo.join(".git/config")).unwrap();
    let mut p = edit::preview(
        &f.git,
        &f.entry(),
        vec![Change {
            key: "user.name".into(),
            value: Some("After".into()),
        }],
        vec![],
    )
    .unwrap();
    p.created_at = 0;
    assert_eq!(edit::apply(&f.git, p).unwrap_err().code, "preview_expired");
    assert_eq!(before, std::fs::read(f.repo.join(".git/config")).unwrap());
}

#[test]
fn missing_promisor_object_is_not_automatically_fetched() {
    let source = Fixture::new();
    source.commit();
    let oid = source.cmd(&source.repo, &["rev-parse", "HEAD"]);
    let f = Fixture::new();
    let relative = format!(".git/objects/{}/{}", &oid[..2], &oid[2..]);
    let object = f.repo.join(&relative);
    std::fs::create_dir_all(object.parent().unwrap()).unwrap();
    std::fs::copy(source.repo.join(&relative), &object).unwrap();
    f.cmd(&f.repo, &["update-ref", "HEAD", &oid]);
    f.set("remote.origin.url", source.repo.to_str().unwrap());
    f.set("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*");
    f.set("remote.origin.promisor", "true");
    f.set("remote.origin.partialCloneFilter", "blob:none");
    std::fs::remove_file(&object).unwrap();
    assert!(f.git.history(&f.repo).is_err());
    assert!(!object.exists(), "missing commit was fetched during a read");
    assert!(!f.repo.join(".git/FETCH_HEAD").exists());
}

#[test]
fn scan_explicit_internal_roots_never_walks_git_metadata_or_bare_objects() {
    let f = Fixture::new();
    let bare = f.dir.path().join("bare.git");
    f.cmd(f.dir.path(), &["init", "--bare", bare.to_str().unwrap()]);
    std::fs::create_dir_all(bare.join("objects/ab/nested")).unwrap();
    for root in [f.repo.join(".git"), bare.join("objects")] {
        let job = scan::start(
            f.git.clone(),
            ScanOptions {
                roots: vec![root.to_string_lossy().into()],
                max_depth: 8,
                excluded_names: vec![],
                excluded_paths: vec![],
            },
        )
        .unwrap();
        for _ in 0..200 {
            if job.state.lock().unwrap().state != "running" {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let s = job.state.lock().unwrap();
        assert_eq!(s.state, "completed");
        assert!(s.found.is_empty());
        assert!(
            s.visited <= 1,
            "internal directory was traversed: {}",
            root.display()
        );
    }
}
