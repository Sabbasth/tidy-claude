# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 1 — Terminée ✅ | Prochaine : Phase 2 (config/state/helpers port)
**Dernier commit :** `migration(phase-1): cargo scaffolding`

## Checkpoints

- [x] Phase 0 terminée, commit aeca2b5
- [x] Phase 1 scaffolding: Cargo.toml, src/ stubs, rustfmt.toml
- [x] Cargo check passes
- [x] Stats phase 1 enregistrées (tokens_in=396, tokens_out=24.5k, durée=5min)
- [ ] Phase 2 : port config/state/helpers (GPT 5.4 mini)
- [ ] Tests unitaires Rust pour helpers
- [ ] Commit phase 2

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
