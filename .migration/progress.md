# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 2 — Terminée ✅ | Prochaine : Phase 3 (ops port)
**Dernier commit :** `migration(phase-2): port config/state/helpers`

## Checkpoints

- [x] Phase 0 terminée, commit aeca2b5
- [x] Phase 1 scaffolding: Cargo.toml, src/ stubs, rustfmt.toml
- [x] Cargo check passes
- [x] Stats phase 1 enregistrées (tokens_in=396, tokens_out=24.5k, durée=5min)
- [x] Phase 2 code: config.rs, state.rs, helpers.rs portés
- [x] cargo check passe
- [x] cargo test passe (23 tests)
- [x] Stats phase 2 enregistrées (tokens_in=64.7k, tokens_out=44.6k, durée=9min, coût=$1)
- [x] Commit phase 2 final

## Prochain tour

1. Démarrer **Phase 3 — Port `ops.rs`** avec **Sonnet 4.6**.
2. Porter le cœur métier : backup/restore, git, `npx skills`, cleanup.
3. Écrire les tests d'intégration Rust correspondants.
4. Valider `cargo test`.
5. Demander stats utilisateur → commit `migration(phase-3): port ops module with integration tests`.

## Blockers

_(none)_

## Notes

- `serde_json` doit être activé avec feature `preserve_order` pour garantir des diffs git stables dans le repo de backup partagé entre machines Python et Rust pendant la transition.
- Décision à confirmer en phase 1 : nom du binaire Rust pendant cohabitation (proposition : binaire Rust non installé jusqu'à phase 7).
