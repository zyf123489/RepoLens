use crate::{git::Git, model::*};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use url::Url;

pub fn redact_address(raw: &str) -> String {
    if raw.chars().any(char::is_control) {
        return "[地址无法安全展示]".into();
    }
    if raw.len() > 2
        && raw.as_bytes()[0].is_ascii_alphabetic()
        && raw.as_bytes()[1] == b':'
        && matches!(raw.as_bytes()[2], b'/' | b'\\')
    {
        return raw.split(['?', '#']).next().unwrap_or("").into();
    }
    if let Ok(mut url) = Url::parse(raw) {
        if ![
            "http", "https", "ssh", "git", "file", "ftp", "ftps", "rsync",
        ]
        .contains(&url.scheme())
        {
            // Custom remote helpers may embed commands and inline credentials.
            return "[自定义协议地址内容已隐藏]".into();
        }
        if url.has_host() {
            let _ = url.set_username("");
            let _ = url.set_password(None);
        }
        // URL parameters and fragments can contain opaque secrets under arbitrary names.
        url.set_query(None);
        url.set_fragment(None);
        return url.to_string();
    }
    if raw.contains("://") || raw.chars().any(char::is_control) {
        return "[地址无法安全展示]".into();
    }
    let clean = raw.split(['?', '#']).next().unwrap_or("");
    if let Some((left, right)) = clean.split_once(':') {
        if left.contains('@') {
            return format!("{}:{right}", left.rsplit_once('@').unwrap().1);
        }
    }
    clean.into()
}
fn address_parts(raw: &str) -> (String, String, Option<String>) {
    if let Ok(url) = Url::parse(raw) {
        let scheme = url.scheme();
        if raw.len() > 2 && raw.as_bytes()[1] == b':' && raw.as_bytes()[0].is_ascii_alphabetic() {
            return ("local".into(), String::new(), None);
        }
        return (
            scheme.into(),
            url.host_str().unwrap_or("").into(),
            if scheme == "ssh" && !url.username().is_empty() {
                Some(url.username().into())
            } else {
                None
            },
        );
    }
    if let Some((left, _)) = raw.split_once(':') {
        if !left.contains(['/', '\\']) && left.len() > 1 {
            let (user, host) = left
                .rsplit_once('@')
                .map_or((None, left), |(u, h)| (Some(u.into()), h));
            return ("ssh".into(), host.into(), user);
        }
    }
    ("local".into(), String::new(), None)
}
fn platform(host: &str) -> Option<String> {
    match host.to_ascii_lowercase().as_str() {
        "github.com" => Some("GitHub".into()),
        "gitlab.com" => Some("GitLab".into()),
        "bitbucket.org" => Some("Bitbucket".into()),
        "gitee.com" => Some("Gitee".into()),
        _ => None,
    }
}

pub fn read(git: &Git, ctx: &Context) -> Result<Vec<Endpoint>> {
    let cwd = Path::new(&ctx.path);
    let names = git.text(cwd, &["remote"])?;
    let mut endpoints = Vec::new();
    for remote in names.lines() {
        if remote.is_empty() || remote.starts_with('-') || remote.chars().any(char::is_control) {
            return Err(AppError::new("remote", "远程名称无法安全解析。"));
        }
        let configured_fetch = git.config(cwd, &format!("remote.{remote}.url"))?;
        let configured_push = git.config(cwd, &format!("remote.{remote}.pushurl"))?;
        for (usage, push) in [("fetch", false), ("push", true)] {
            let args = if push {
                vec!["remote", "get-url", "--all", "--push", remote]
            } else {
                vec!["remote", "get-url", "--all", remote]
            };
            let resolved = git.text(cwd, &args)?;
            for (index, address) in resolved.lines().enumerate() {
                let (protocol, host, username) = address_parts(address);
                let configured = if push && !configured_push.is_empty() {
                    &configured_push
                } else {
                    &configured_fetch
                };
                let mut endpoint = Endpoint {
                    remote: remote.into(),
                    usage: usage.into(),
                    address: redact_address(address),
                    configured_address: configured
                        .get(index)
                        .map(|v| redact_address(&v.value))
                        .unwrap_or_else(|| "未知".into()),
                    protocol: protocol.clone(),
                    host: host.clone(),
                    platform: platform(&host),
                    username,
                    metadata: vec![],
                    limitations: vec![],
                    config_state: "read".into(),
                    verification: "untested".into(),
                };
                if protocol == "https" || protocol == "http" {
                    https(git, cwd, address, &mut endpoint)?;
                } else if protocol == "ssh" {
                    let custom = !git.config(cwd, "core.sshCommand")?.is_empty()
                        || !git.config(cwd, "ssh.variant")?.is_empty()
                        || std::env::var_os("GIT_SSH").is_some()
                        || std::env::var_os("GIT_SSH_COMMAND").is_some();
                    if custom {
                        endpoint.limitations.push(
                            "存在自定义 SSH 命令、变体或进程覆盖，未推断其配置；未执行命令。"
                                .into(),
                        );
                    } else {
                        ssh_metadata(&host, &mut endpoint);
                    }
                } else {
                    endpoint
                        .limitations
                        .push("此协议不提供 HTTPS/SSH 认证配置推断。".into());
                }
                if !endpoint.limitations.is_empty() {
                    endpoint.config_state = "partial".into();
                }
                endpoints.push(endpoint);
            }
        }
    }
    Ok(endpoints)
}

fn https(git: &Git, cwd: &Path, address: &str, endpoint: &mut Endpoint) -> Result<()> {
    // Credential helpers accumulate ALL applicable entries, unlike --get-urlmatch's
    // single best value. Preserve include/scope order and empty reset entries.
    let out = git.query(
        cwd,
        &[
            "config",
            "--includes",
            "--null",
            "--show-origin",
            "--show-scope",
            "--get-regexp",
            r"^credential\.(.*\.)?(helper|username|usehttppath)$",
        ],
    )?;
    if out.code != 0 && out.code != 1 {
        return Err(AppError::new(
            "credential_config",
            "无法读取凭据配置元数据。",
        ));
    }
    let parts: Vec<_> = out.bytes.split(|b| *b == 0).collect();
    for chunk in parts[..parts.len().saturating_sub(1)].chunks(3) {
        if chunk.len() != 3 {
            endpoint
                .limitations
                .push("凭据配置来源格式不受支持。".into());
            break;
        }
        let entry = String::from_utf8_lossy(chunk[2]);
        let Some((full_key, value)) = entry.split_once('\n') else {
            continue;
        };
        let Some((section, key)) = full_key.rsplit_once('.') else {
            continue;
        };
        if section != "credential" {
            let pattern = section.strip_prefix("credential.").unwrap_or("");
            // Ask Git's URL matcher about ONE pattern using a fresh non-credential
            // section and a public sentinel. No helper or credential API is called.
            let probe = format!("repolens{}", uuid::Uuid::new_v4().simple());
            let assignment = format!("{probe}.{pattern}.match=true");
            let lookup = format!("{probe}.match");
            let matched = git.query(
                cwd,
                &[
                    "-c",
                    &assignment,
                    "config",
                    "--get-urlmatch",
                    &lookup,
                    address,
                ],
            )?;
            if matched.code != 0 {
                // Partial/non-URL credential patterns have additional Git semantics.
                // Do not present them as definitely inapplicable.
                if !pattern.contains("://") || matched.code != 1 {
                    endpoint
                        .limitations
                        .push("存在无法可靠匹配的凭据作用域；对应项未推断为适用。".into());
                }
                continue;
            }
        }
        let safe_value = if key == "helper" {
            if value.is_empty() {
                "重置此前助手列表".into()
            } else if [
                "manager",
                "manager-core",
                "osxkeychain",
                "wincred",
                "cache",
                "store",
                "libsecret",
            ]
            .contains(&value)
            {
                value.to_string()
            } else {
                "[自定义助手内容已隐藏，未执行]".into()
            }
        } else if key == "username" {
            "[已配置候选用户名；为避免夹带秘密不展示原值]".into()
        } else if ["true", "false"].contains(&value) {
            value.to_string()
        } else {
            "[非标准值已隐藏]".into()
        };
        let scope = String::from_utf8_lossy(chunk[0]);
        let origin = String::from_utf8_lossy(chunk[1]);
        endpoint.metadata.push(format!(
            "credential.{key}: {safe_value} · {scope} · {origin}"
        ));
    }
    endpoint
        .limitations
        .push("不查询凭据，实际认证账号与凭据是否存在均未知。".into());
    Ok(())
}

fn ssh_metadata(host: &str, endpoint: &mut Endpoint) {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let Some(home) = home else {
        endpoint
            .limitations
            .push("无法定位用户 SSH 配置目录。".into());
        return;
    };
    let path = home.join(".ssh/config");
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let (values, limitations) = parse_ssh(&text, host);
                for key in ["hostname", "port", "user", "identityfile", "identitiesonly"] {
                    if let Some(value) = values.get(key) {
                        endpoint
                            .metadata
                            .push(format!("{key}: {value} · {}", path.display()));
                        if key == "hostname" {
                            endpoint.platform = platform(value);
                        }
                        if key == "user" && endpoint.username.is_none() {
                            endpoint.username = Some(value.clone());
                        }
                    }
                }
                endpoint.limitations.extend(limitations);
            }
            Err(_) => endpoint.limitations.push("用户 SSH 配置不可访问。".into()),
        }
    }
    endpoint.limitations.push("仅静态解析用户 OpenSSH 配置安全子集；系统配置、代理密钥及最终密钥选择未验证。仅展示路径，不读取私钥。".into());
}

pub fn parse_ssh(text: &str, host: &str) -> (HashMap<String, String>, Vec<String>) {
    let mut values = HashMap::new();
    let mut limitations = Vec::new();
    let mut active = true;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(split) = line.find(|c: char| c.is_whitespace() || c == '=') else {
            limitations.push("SSH 配置存在未支持语法。".into());
            continue;
        };
        let key = line[..split].to_ascii_lowercase();
        let value = line[split..].trim_start_matches([' ', '\t', '=']).trim();
        if key == "host" {
            let mut positive = false;
            let mut negative = false;
            for p in value.split_whitespace() {
                let neg = p.starts_with('!');
                let pattern = p.trim_start_matches('!');
                let matches = glob::Pattern::new(pattern).is_ok_and(|g| {
                    g.matches_with(
                        host,
                        glob::MatchOptions {
                            case_sensitive: false,
                            ..Default::default()
                        },
                    )
                });
                if matches {
                    if neg {
                        negative = true;
                    } else {
                        positive = true;
                    }
                }
            }
            active = positive && !negative;
            continue;
        }
        if key == "match" {
            limitations.push("存在 Match 条件，后续配置未展开；未运行任何命令。".into());
            break;
        }
        if !active {
            continue;
        }
        if key == "include" {
            limitations.push("Include 未展开，解析可能不完整。".into());
            continue;
        }
        if [
            "proxycommand",
            "proxyjump",
            "localcommand",
            "remotecommand",
            "canonicalizehostname",
        ]
        .contains(&key.as_str())
        {
            limitations.push(format!("存在 {key}，未执行或推断其行为。"));
            continue;
        }
        if ["hostname", "port", "user", "identityfile", "identitiesonly"].contains(&key.as_str()) {
            if value.contains(['%', '$', '"', '\'', '#']) || !valid_text(value, 1024) {
                limitations.push(format!("{key} 含未支持的展开或引号语法。"));
                continue;
            }
            if key == "identityfile" && values.contains_key(&key) {
                limitations
                    .push("配置了多个 IdentityFile；当前只展示首项，不推断最终密钥选择。".into());
            }
            values.entry(key).or_insert(value.into());
        }
    }
    (values, limitations)
}
