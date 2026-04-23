# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 2 — Code terminé, stats en attente ⏳ | Prochaine : capture des stats phase 2
**Dernier commit :** `migration(phase-1): cargo scaffolding`

## Checkpoints

- [x] Phase 0 terminée, commit aeca2b5
- [x] Phase 1 scaffolding: Cargo.toml, src/ stubs, rustfmt.toml
- [x] Cargo check passes
- [x] Stats phase 1 enregistrées (tokens_in=396, tokens_out=24.5k, durée=5min)
- [x] Phase 2 code: config.rs, state.rs, helpers.rs portés
- [x] cargo check passe
- [x] cargo test passe (23 tests)
- [ ] Stats phase 2 à recueillir auprès de l'utilisateur
- [ ] Commit phase 2 final

## Prochain tour

1. Démarrer **Phase 2 — Port config/state/helpers** avec **GPT 5.4 mini**.
2. Implémenter fully : `config.rs` (const, paths), `state.rs` (RunState), `helpers.rs` (pures functions).
3. Tests unitaires Rust pour `helpers` (deep_merge, format_size, etc.) avec parité Python.
4. `cargo test` doit passer.
5. Demander stats utilisateur → commit `migration(phase-2): port config/state/helpers with tests`.

## Blockers

_(none)_

## Notes

- `serde_json` doit être activé avec feature `preserve_order` pour garantir des diffs git stables dans le repo de backup partagé entre machines Python et Rust pendant la transition.
- Décision à confirmer en phase 1 : nom du binaire Rust pendant cohabitation (proposition : binaire Rust non installé jusqu'à phase 7).
