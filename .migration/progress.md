# Progress — Rust migration

**Branche :** `rust-migration`
**Phase courante :** 0 — Terminée ✅ | Prochaine : Phase 1 (Scaffolding Cargo)
**Dernier commit :** `migration(phase-0): bootstrap plan & tracking`

## Checkpoints

- [x] Analyse code source Python
- [x] Rédaction `MIGRATION_PLAN.md`
- [x] Scaffolding suivi (`.migration/`)
- [x] Création branche `rust-migration`
- [x] Stats phase 0 consignées (tokens_in=22, tokens_out=8900, cost=$0)
- [x] Commit phase 0

## Prochain tour

1. Démarrer **Phase 1 — Scaffolding Cargo** avec **Haiku 4.5**.
2. Créer `Cargo.toml`, structure `src/` miroir, `rustfmt.toml`.
3. Vérifier `cargo check` OK.
4. Demander stats à l'utilisateur → commit `migration(phase-1): cargo scaffolding`.

## Blockers

_(none)_

## Notes

- `serde_json` doit être activé avec feature `preserve_order` pour garantir des diffs git stables dans le repo de backup partagé entre machines Python et Rust pendant la transition.
- Décision à confirmer en phase 1 : nom du binaire Rust pendant cohabitation (proposition : binaire Rust non installé jusqu'à phase 7).
