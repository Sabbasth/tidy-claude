# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 2.x     | ✅ Active |
| 1.x     | ❌ Python implementation, no longer maintained |

## Reporting a Vulnerability

Please **do not** open a public GitHub issue for security vulnerabilities.

Instead, open a private [security advisory](https://github.com/sabbasth/tidy-claude/security/advisories/new)
on GitHub. We aim to acknowledge reports within **7 days** and to ship a fix
or mitigation within **30 days** for high-severity issues.

## Threat Model

`tidy-claude` is a personal, per-user tool that reads and writes files under
`~/.claude/` and executes `git` and `sh -c <cmd>` against a **user-controlled**
git remote.

Assumptions:
- The local `~/.claude/` directory is trusted.
- The git remote configured via `tidy-claude config --remote-backup <url>` is
  trusted and controlled by the same user (or their team).
- The `skills.json` manifest inside the backup repo is treated as code: the
  `install` field is executed via `sh -c` at every `sync`. **Never point
  `--remote-backup` at a third-party or untrusted repo.**

Out of scope:
- Supply-chain compromise of the backup git remote (mitigate with signed
  commits / branch protection on your side).
- Compromised shell environment or `$PATH` hijacking of `git` / `sh`.

## Hardening Practices

- All path manipulation uses `PathBuf`; no string concatenation for paths.
- Subprocess invocations use `Command::args(...)`, never `shell=true` except
  for the documented `skills.json::install` case.
- `serde_json` dependency is pinned with lockfile; `cargo audit` runs in CI.
- No `unsafe` code in the crate.

## Disclosure

Security advisories are published on the GitHub Security tab once a fix is
available and users have had reasonable time to upgrade.
