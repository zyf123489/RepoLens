# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

Independent developers and small-team developers who maintain repositories across multiple platforms (GitHub, GitLab, Gitee, private hosts) and multiple identities (personal, work, open-source) on a single machine. They need to see at a glance which commit name and email is configured where, whether it matches their expectations, and how to fix it safely.

## Product Purpose

RepoLens is a local desktop workbench for checking and managing Git commit identity configuration across many repositories. It answers: what identity is configured in each repo, where does it come from, does it match what I expect, and how do I fix it without breaking anything else. It does not perform daily Git operations (commit, push, pull, merge). Success means the user trusts the tool to show the truth about their configuration and never silently changes anything without explicit confirmation.

## Positioning

Multi-repository identity checking. Other Git clients focus on commit/push/branch workflows and treat identity as a settings footnote. RepoLens makes identity configuration its primary concern: scanning, rule-based checking, source tracing, safe editing with preview and verification, and authentication configuration overview — all across hundreds of repos at once.

## Operating Context

Runs as a native desktop application (Tauri 2 with system WebView) on Windows, macOS, and Linux. Shells out to the locally installed Git CLI for all repository operations. Stores application data (entries, groups, rules) in a local SQLite database. Default behavior is read-only and offline: no network access, no automatic sync, no telemetry. Configuration writes are limited to local-scope user.name and user.email, require explicit preview and confirmation, and are verified after execution.

## Capabilities and Constraints

- Scans directories to discover repos (normal, worktree, bare, submodule), with depth/exclusion controls
- Reads HEAD, work status, history, upstream, identity config (user/author/committer with sources and override order), and remote authentication config
- Per-entry identity rules: expected name, expected email, allowed email domains — checked independently, AND-combined
- Edits only local-scope user.name and user.email; preview → confirm → write → re-read → verify
- Authentication overview is read-only metadata; never queries credentials or executes helpers
- Secrets in URLs, config, and error output are redacted before reaching the UI or storage
- No Git operations: no commit, stage, fetch, pull, push, merge, rebase, reset
- Long-term direction: evolve toward a full Git visual client with TortoiseGit as the capability reference

## Brand Commitments

Name: RepoLens (confirmed). All other visual identity (logo, colors, typography, iconography) is open for design.

## Evidence on Hand

- Design document v1.0.0 at docs/design-v1.0.0.md with full requirements, interaction rules, and acceptance criteria
- Working alpha (0.1.0-alpha.1) with complete backend, 47 integration tests, 10 frontend tests
- No real user feedback, testimonials, or usage data yet
- No marketing site, press, or external documentation

## Product Principles

1. **Show truth, not assumptions.** Display what Git actually reports; distinguish configured vs. verified vs. unknown; never fabricate a certainty the tool cannot prove.
2. **Read by default, write by consent.** Every write requires preview, explicit confirmation, and post-write verification. The tool never silently changes configuration, history, or working files.
3. **Isolate failures, preserve results.** A module that fails to read does not erase or override the results of modules that succeeded. Partial information is shown as partial, not hidden.
4. **One machine, many contexts.** The fundamental unit is a repo entry with its own path, identity rules, and group — never a single global identity or account.
5. **Explain the limits.** Configuration checking does not guarantee actual commit identity. Authentication overview does not prove login status. The UI always says what it checked and what it did not.
