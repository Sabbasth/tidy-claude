# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 5 — Terminée ✅ | Prochaine : Phase 6 (CI & release)
**Dernier commit :** `migration(phase-5): parity validation`

## Checkpoints

- [x] Phase 0 — Plan & tracking (Sonnet 4.6)
- [x] Phase 1 — Scaffolding Cargo (Haiku 4.5)
- [x] Phase 2 — config/state/helpers (GPT 5.4 mini) — 23 tests
- [x] Phase 3 — ops (Sonnet 4.6) — 41 tests
- [x] Phase 4 — cli (Sonnet 4.6) — 50 tests
- [x] Phase 5 — parité e2e (Sonnet 4.6) — 56 tests + parity.sh
- [x] Stats phase 5 : 87.8k in / 123.7k out / 8 min / $1

## Prochaine phase

**Phase 6 — CI & release (Haiku 4.5)**

1. `.github/workflows/rust.yml` : `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`
2. Matrix : macOS (apple-silicon) + Linux x86_64
3. Badge README
4. `cargo install` instructions dans README

## Blockers

_(none)_

## Notes

- Binaire Rust non installé globalement jusqu'à phase 7.
- `serde_json` feature `preserve_order` active — diffs git stables.
- Snapshot insta commitée : `tests/snapshots/`.
