use crate::{git::Git, model::*};
use std::path::Path;
pub const KEYS: [&str; 6] = [
    "user.name",
    "user.email",
    "author.name",
    "author.email",
    "committer.name",
    "committer.email",
];

pub fn validate_email(email: &str) -> bool {
    if !valid_text(email, 320) || email.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((local, domain)) = email.rsplit_once('@') else {
        return false;
    };
    !local.is_empty() && !local.contains('@') && valid_domain(domain)
}
pub fn valid_domain(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.split('.').all(|s| {
            !s.is_empty()
                && s.len() <= 63
                && !s.starts_with('-')
                && !s.ends_with('-')
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}
pub fn validate_policy(policy: &Policy) -> Result<()> {
    if policy.name.as_ref().is_some_and(|s| !valid_text(s, 256)) {
        return Err(AppError::new("policy", "期望姓名不能为空或包含控制字符。"));
    }
    if policy.email.as_ref().is_some_and(|s| !validate_email(s)) {
        return Err(AppError::new("policy", "期望邮箱格式无效。"));
    }
    if policy.domains.len() > 100 || policy.domains.iter().any(|s| !valid_domain(s)) {
        return Err(AppError::new(
            "policy",
            "请输入完整域名，不能包含通配符、空白或无效字符。",
        ));
    }
    if let Some(email) = &policy.email {
        if !domain_match(email, &policy.domains) {
            return Err(AppError::new(
                "policy",
                "期望邮箱与允许域名冲突，规则未保存。",
            ));
        }
    }
    Ok(())
}
fn domain_match(email: &str, domains: &[String]) -> bool {
    domains.is_empty()
        || email.rsplit_once('@').is_some_and(|(_, d)| {
            domains
                .iter()
                .any(|allowed| d.eq_ignore_ascii_case(allowed))
        })
}

pub fn read(git: &Git, cwd: &Path, policy: &Policy) -> Result<Identity> {
    let mut keys = Vec::new();
    for key in KEYS {
        keys.push(ConfigKey {
            key: key.into(),
            values: git.config(cwd, key)?,
        });
    }
    Ok(evaluate(keys, policy))
}

pub fn read_module(git: &Git, cwd: &Path, policy: &Policy) -> Module<Identity> {
    let mut keys = Vec::new();
    let mut failed = Vec::new();
    for key in KEYS {
        match git.config(cwd, key) {
            Ok(values) => keys.push(ConfigKey {
                key: key.into(),
                values,
            }),
            Err(_) => failed.push(key),
        }
    }
    if failed.is_empty() {
        return Module::success(evaluate(keys, policy));
    }
    if keys.is_empty() {
        return Module::failed(AppError::new(
            "config",
            "身份配置读取失败，无法完成配置检查。",
        ));
    }
    Module {
        state: "partial".into(),
        data: Some(evaluate(keys, policy)),
        error: Some(AppError::new(
            "config_partial",
            &format!("部分身份配置未读取：{}", failed.join("、")),
        )),
        read_at: now(),
    }
}

pub fn evaluate(keys: Vec<ConfigKey>, policy: &Policy) -> Identity {
    // A present key with no values means Git confirmed it is unconfigured.
    // An absent key was not read and must never trigger an inferred fallback.
    let get = |key: &str| {
        keys.iter()
            .find(|v| v.key == key)
            .and_then(ConfigKey::effective)
            .map(String::from)
    };
    let mut findings = Vec::new();
    let mut missing = false;
    let mut mismatch = false;
    let mut review = false;
    let mut incomplete = false;
    for role in ["user", "author", "committer"] {
        for field in ["name", "email"] {
            let own_key = format!("{role}.{field}");
            let base_key = format!("user.{field}");
            let base = get(&format!("user.{field}"));
            let own = get(&format!("{role}.{field}"));
            if !keys.iter().any(|k| k.key == own_key)
                || (own.is_none() && !keys.iter().any(|k| k.key == base_key))
            {
                incomplete = true;
                findings.push(Finding {
                    field: own_key,
                    value: None,
                    state: "incomplete".into(),
                    reason: "此项或其回退所需的基础项无法读取，未将其当作缺失或匹配".into(),
                });
                continue;
            }
            let value = if role == "user" {
                base.clone()
            } else {
                own.clone().or_else(|| base.clone())
            };
            let constrained = if field == "name" {
                policy.name.is_some()
            } else {
                policy.email.is_some() || !policy.domains.is_empty()
            };
            let valid = value.as_ref().is_some_and(|v| {
                if field == "name" {
                    valid_text(v, 256)
                } else {
                    validate_email(v)
                }
            });
            let matches = value.as_ref().is_some_and(|v| {
                if field == "name" {
                    policy.name.as_ref().is_none_or(|n| n == v)
                } else {
                    policy.email.as_ref().is_none_or(|e| e == v) && domain_match(v, &policy.domains)
                }
            });
            let (state, reason) = if !valid {
                if role == "user" {
                    missing = true;
                }
                if constrained && role != "user" {
                    mismatch = true;
                }
                if role != "user" && own.is_some() && own != base && !constrained {
                    review = true;
                    ("review", "专属覆盖为空或格式无效，未设置对应规则，需要核实")
                } else {
                    (
                        "missing",
                        "缺失、显式空值或基本格式无效；不代表 Git 一定无法提交",
                    )
                }
            } else if constrained && !matches {
                mismatch = true;
                ("mismatch", "不符合已启用的期望约束")
            } else if role != "user" && own.is_some() && own != base && !constrained {
                review = true;
                ("review", "与基础值不同，未设置对应规则，覆盖待核实")
            } else if constrained {
                ("match", "符合已启用的约束")
            } else {
                ("unconfigured", "已配置；未设置对应检查规则")
            };
            findings.push(Finding {
                field: format!("{role}.{field}"),
                value,
                state: state.into(),
                reason: reason.into(),
            });
        }
    }
    let any = policy.name.is_some() || policy.email.is_some() || !policy.domains.is_empty();
    let status = if incomplete {
        "incomplete"
    } else if mismatch {
        "mismatch"
    } else if missing {
        "missing"
    } else if review {
        "review"
    } else if !any {
        "unconfigured"
    } else {
        "passed"
    };
    let checked = format!(
        "姓名{}；邮箱{}；仅检查当前上下文配置，实际提交可能受环境变量、命令参数或其他工具影响",
        if policy.name.is_some() {
            "已设置规则"
        } else {
            "未设置规则"
        },
        if policy.email.is_some() || !policy.domains.is_empty() {
            "已设置规则"
        } else {
            "未设置规则"
        }
    );
    Identity {
        keys,
        findings,
        status: status.into(),
        checked,
    }
}
