# Migration Plan — `tidy-claude` : Python → Rust

> **Branche de travail :** `rust-migration`
> **Stratégie :** remplacement progressif in-place, commits atomiques par phase.
> **Parité :** fonctionnelle stricte (format backup, chemins, clés JSON identiques).
> **Distribution cible :** `cargo install`.

---

## 1. Contexte

Code source actuel (Python 3.12, ~765 LoC + 665 LoC de tests) :

| Fichier | LoC | Rôle |
|---|---|---|
| `src/tidy_claude/config.py` | 70 | Constantes, chemins, clés préservées |
| `src/tidy_claude/helpers.py` | 101 | Pures : `deep_merge`, `diff_files`, `extract_keys`, `resolve_claude_md`, etc. |
| `src/tidy_claude/ops.py` | 383 | Backup/restore, git, subprocess (`npx skills`), logique métier |
| `src/tidy_claude/cli.py` | 192 | Commandes `sync` / `config` / `cleanup` (Click + simple-term-menu) |
| `src/tidy_claude/state.py` | 16 | `RunState` (logger/debug) |
| `tests/*` | 665 | pytest, fixtures `tmp_path`, monkeypatch |

Dépendances Python → équivalents Rust :

| Python | Rust |
|---|---|
| `click` | `clap` (derive) |
| `simple-term-menu` | `dialoguer` (MultiSelect) |
| `platformdirs` | `directories` ou `etcetera` |
| `subprocess` (git) | `git2` |
| `subprocess` (`npx skills add`) | `std::process::Command` |
| `json` stdlib | `serde_json` (préserve l'ordre avec `preserve_order`) |
| `pytest` + `tmp_path` | `assert_fs` + `predicates` + `insta` (snapshots) |

**Point d'attention parité :** `serde_json` doit préserver l'ordre des clés pour ne pas polluer les diffs git du repo de backup partagé. Activer feature `preserve_order`.

---

## 2. Phases

Chaque phase se termine par **un commit** sur `rust-migration` avec un message `migration(phase-N): ...`.
Le choix de modèle privilégie le meilleur ratio qualité/tokens : **Haiku 4.5** pour boilerplate/tests répétitifs, **Sonnet 4.6** pour logique métier non triviale, **Opus 4.7** uniquement pour revue de sécurité finale, **GPT 5.4 mini** pour ports mécaniques pur-fonction.

### Phase 0 — Cadrage & outillage de suivi _(cette requête)_

- Analyse du code source existant
- Plan de migration (ce document)
- Workflow de suivi anti-compaction (`.migration/stats.jsonl`, `.migration/progress.md`)
- Création branche `rust-migration`
- **Modèle :** Sonnet 4.6 (compromis analyse + rédaction structurée)
- **Livrables :** `MIGRATION_PLAN.md`, `.migration/progress.md`, `.migration/stats.jsonl`
- **Commit :** `migration(phase-0): bootstrap plan & tracking`

### Phase 1 — Scaffolding Cargo

- `Cargo.toml` (edition 2021, deps : clap, dialoguer, git2, serde, serde_json[preserve_order], anyhow, thiserror, directories)
- Structure `src/` miroir : `config.rs`, `helpers.rs`, `ops.rs`, `cli.rs`, `state.rs`, `main.rs`, `lib.rs`
- `rustfmt.toml`, `clippy` strict dans CI
- Cohabitation temporaire : Python reste fonctionnel, binaire Rust s'appelle `tidy-claude` mais Python en `tidy-claude-py` le temps de la migration ? → **Décision : on garde Python intact jusqu'à phase 7, binaire Rust construit en parallèle non installé.**
- **Modèle :** Haiku 4.5 (boilerplate)
- **Commit :** `migration(phase-1): cargo scaffolding`

### Phase 2 — Port `config.rs` + `state.rs` + `helpers.rs`

- Constantes (chemins, clés préservées) → `config.rs`
- `RunState` → struct avec méthode `log(&self, msg: &str)` gardée par `debug: bool`
- Fonctions pures (`deep_merge` sur `serde_json::Value`, `diff_files`, `extract_keys`, `pretty_project_name`, `format_size`, `resolve_claude_md`)
- Tests unitaires Rust refondus (couverture identique à `test_helpers.py`)
- **Modèle :** GPT 5.4 mini (port mécanique 1:1, déterministe)
- **Commit :** `migration(phase-2): port config/state/helpers with tests`

### Phase 3 — Port `ops.rs` (cœur métier)

- Sous-modules : `ops/backup.rs`, `ops/restore.rs`, `ops/git.rs`, `ops/skills.rs`, `ops/cleanup.rs`
- Git via `git2` (pull rebase, add, commit, push ; fallback shell-out si MFA/ssh-agent pose problème)
- `npx skills add` via `std::process::Command`
- Tests d'intégration avec `assert_fs` + repo git temporaire
- **Modèle :** Sonnet 4.6 (logique non triviale, branches d'erreur)
- **Commit :** `migration(phase-3): port ops module with integration tests`

### Phase 4 — Port `cli.rs`

- `clap` derive avec sous-commandes `sync` (default), `config`, `cleanup`
- Flag global `--debug`
- `cleanup` : `dialoguer::MultiSelect` pour le picker projets, flags `--older-than`, `-a`, `--dry-run`
- Tests CLI avec `assert_cmd` + `predicates`
- **Modèle :** Sonnet 4.6 (interaction UX + args parsing subtil)
- **Commit :** `migration(phase-4): port CLI with clap + dialoguer`

### Phase 5 — Parité end-to-end & golden tests

- Script `tests/parity.sh` : exécute Python et Rust sur la même fixture, diff les outputs
- Snapshots `insta` sur sorties structurées
- Fix des divergences détectées
- **Modèle :** Sonnet 4.6 (debug de divergences)
- **Commit :** `migration(phase-5): parity validation`

### Phase 6 — CI & release

- GitHub Actions : `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`
- Matrix build macOS + Linux
- Badge README
- **Modèle :** Haiku 4.5 (YAML boilerplate)
- **Commit :** `migration(phase-6): rust CI pipeline`

### Phase 7 — Revue sécurité & suppression du Python

- Audit : gestion des paths (traversal), permissions fichiers, secrets jamais loggés, appels subprocess
- `cargo audit`, `cargo deny`
- Suppression `src/tidy_claude/`, `tests/*.py`, `pyproject.toml`, `uv.lock`
- Mise à jour README (install : `cargo install --git ...`)
- **Modèle :** Opus 4.7 (audit sécurité one-shot critique) + Haiku 4.5 (nettoyage)
- **Commit :** `migration(phase-7): security audit + remove python sources`

---

## 3. Sélection de modèle — rationale

| Profil de tâche | Modèle retenu | Justification |
|---|---|---|
| Analyse architecturale, plan | **Sonnet 4.6** | Bon rapport structure/coût ; Opus surdimensionné |
| Port mécanique pur-fonctions | **GPT 5.4 mini** | Tâches déterministes, pas besoin de raisonnement profond |
| Logique métier (ops, git, CLI) | **Sonnet 4.6** | Arbitrage erreurs, edge cases, UX |
| Boilerplate (Cargo.toml, CI YAML) | **Haiku 4.5** | Très cheap, qualité suffisante sur patterns connus |
| Tests répétitifs | **Haiku 4.5** | Volume élevé, patterns répétés |
| Audit sécurité final | **Opus 4.7** | Un seul passage, coût justifié par criticité |
| Debug divergence inter-langages | **Sonnet 4.6** | Raisonnement mais pas besoin d'Opus |

Modèles non retenus :
- **GPT 5.4** : chevauche Sonnet 4.6 sans avantage net pour ce projet.
- **Gemini 3.1 pro** : pas d'usage différencié ici ; si un besoin de contexte très long apparaît (>200k tokens), on l'introduira.

---

## 4. Workflow de suivi anti-compaction

À chaque **début de tour**, je recharge :
1. `MIGRATION_PLAN.md` (ce fichier — source de vérité structurelle)
2. `.migration/progress.md` (phase courante, checkpoints, blockers)
3. Dernière entrée de `.migration/stats.jsonl` (dernière action effectuée)

À chaque **fin de tour**, je :
1. Demande à l'utilisateur les chiffres réels (tokens in/out, durée) affichés par pi-agent
2. Append une ligne JSON à `.migration/stats.jsonl`
3. Mets à jour `.migration/progress.md` (phase en cours, prochaine étape)
4. Mets à jour le tableau §5 ci-dessous avec les totaux de phase

**Format `.migration/stats.jsonl`** (une ligne par tour) :

```json
{"ts":"2026-04-23T15:10:00Z","phase":0,"turn":1,"model":"sonnet-4.6","duration_s":null,"tokens_in":null,"tokens_out":null,"cost_usd":null,"source":"pending","summary":"build migration plan + tracking scaffolding"}
```

Champ `source` : `reported` (chiffres donnés par l'utilisateur) | `estimated` | `pending`.

---

## 5. Statistiques par phase _(cumulatif, mis à jour en continu)_

Tarifs de référence (USD / 1M tokens, input / output) :

| Modèle | Input | Output |
|---|---:|---:|
| Claude Opus 4.7 | $5 | $25 |
| Claude Sonnet 4.6 | $3 | $15 |
| Claude Haiku 4.5 | $0.80 | $4 |
| GPT 5.4 | $2 | $8 |
| GPT 5.4 mini | $0.40 | $1.60 |
| Gemini 3.1 pro | $1.25 | $5 |

| Phase | Modèle principal | Tours | Temps cumulé | Tokens in | Tokens out | Coût (USD) | Statut |
|---|---|---:|---:|---:|---:|---:|---|
| 0 — Plan & tracking | Sonnet 4.6 | 1 | n/a | 22 † | 8 900 | $0.00 ‡ | 🟢 terminé |
| 1 — Scaffolding Cargo | Haiku 4.5 | — | — | — | — | — | ⚪ à faire |
| 2 — config/state/helpers | GPT 5.4 mini | — | — | — | — | — | ⚪ à faire |
| 3 — ops | Sonnet 4.6 | — | — | — | — | — | ⚪ à faire |
| 4 — cli | Sonnet 4.6 | — | — | — | — | — | ⚪ à faire |
| 5 — parité e2e | Sonnet 4.6 | — | — | — | — | — | ⚪ à faire |
| 6 — CI & release | Haiku 4.5 | — | — | — | — | — | ⚪ à faire |
| 7 — audit & cleanup | Opus 4.7 + Haiku 4.5 | — | — | — | — | — | ⚪ à faire |
| **TOTAL** | | | | | | | |

Légende : ⚪ à faire · 🟡 en cours · 🟢 terminé · 🔴 bloqué

† `tokens_in` = delta non-caché rapporté par pi-agent (la majorité du contexte est servie depuis le cache, d'où la valeur basse).
‡ Coût facturé $0 (plan d'abonnement). Prix catalogue équivalent Sonnet 4.6 : ~$0.13 (22×$3/1M + 8 900×$15/1M).

---

## 6. Critères d'acceptation globaux

- [ ] `cargo test` tout vert, couverture ≥ celle de la suite pytest
- [ ] `cargo clippy -- -D warnings` clean
- [ ] Script de parité : Python et Rust produisent des backups byte-identiques sur fixture
- [ ] `cargo install --git` fonctionne sur macOS (Apple Silicon) + Linux x86_64
- [ ] README à jour, CONTRIBUTING adapté au workflow Rust
- [ ] Sources Python supprimées
