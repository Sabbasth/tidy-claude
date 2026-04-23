# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 4 — Terminée ✅ | Prochaine : Phase 5 (parité e2e)
**Dernier commit :** `migration(phase-4): port CLI`

## Checkpoints

- [x] Phase 0 terminée, commit aeca2b5
- [x] Phase 1 scaffolding: Cargo.toml, src/ stubs, rustfmt.toml
- [x] Stats phase 1 enregistrées (tokens_in=396, tokens_out=24.5k, durée=5min)
- [x] Phase 2 config/state/helpers portés — 23 tests
- [x] Stats phase 2 : 64.7k in / 44.6k out / 9 min / $1
- [x] Phase 3 ops.rs porté — 41 tests
- [x] Stats phase 3 : 87.7k in / 82.6k out / 15 min / $1
- [x] Phase 4 cli.rs porté — 50 tests (41 unit + 9 integration)
- [x] Stats phase 4 : 87.7k in / 101.7k out / 6 min / $1

## Prochain tour

1. Démarrer **Phase 5 — Parité end-to-end** avec **Sonnet 4.6**.
2. Script `tests/parity.sh` : exécute Python et Rust sur même fixture, diff des backups.
3. Snapshots `insta` sur outputs structurés.
4. Corriger les divergences éventuelles.
5. Demander stats → commit `migration(phase-5): parity validation`.

## Blockers

_(none)_

## Notes

- `serde_json` avec feature `preserve_order` : diffs git stables entre machines Python et Rust.
- Binaire Rust non installé globalement jusqu'à phase 7 (fin de cohabitation Python/Rust).
