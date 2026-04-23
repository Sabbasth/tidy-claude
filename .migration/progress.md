# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 3 — Code terminé, stats en attente ⏳ | Prochaine : Phase 4 (cli port)
**Dernier commit :** `migration(phase-3): port ops module`

## Checkpoints

- [x] Phase 0 terminée, commit aeca2b5
- [x] Phase 1 scaffolding: Cargo.toml, src/ stubs, rustfmt.toml
- [x] Cargo check passes
- [x] Stats phase 1 enregistrées (tokens_in=396, tokens_out=24.5k, durée=5min)
- [x] Phase 3 code: ops.rs porté (backup, restore, git, skills, cleanup)
- [x] cargo test passe (41 tests : 23 helpers + 18 ops/cleanup)
- [ ] Stats phase 3 à recueillir
- [ ] Commit phase 3

## Prochain tour

1. Recueillir les stats phase 3 de l'utilisateur.
2. Commit `migration(phase-3): port ops module`.
3. Démarrer **Phase 4 — Port `cli.rs`** avec **Sonnet 4.6**.

## Blockers

_(none)_

## Notes

- `serde_json` doit être activé avec feature `preserve_order` pour garantir des diffs git stables dans le repo de backup partagé entre machines Python et Rust pendant la transition.
- Décision à confirmer en phase 1 : nom du binaire Rust pendant cohabitation (proposition : binaire Rust non installé jusqu'à phase 7).
