# Migration Plan — `tidy-claude` Python → Rust

## Vue d'ensemble

`tidy-claude` est un outil CLI qui :
- **Backup / restore** la config Claude Code (`~/.claude/`, `~/.claude.json`) vers/depuis un dépôt git distant
- **Sync** complète : `pull → restore → install skills → backup → commit → push`
- **Cleanup** les sessions et fichiers de conversation Claude (avec menu interactif)
- **Config** la configuration via un fichier JSON (`~/.config/tidy-claude/config.json`)

### Stack Python actuelle

| Python | Rust équivalent |
|---|---|
| `click` | `clap` v4 (derive) |
| `platformdirs` | `directories` crate |
| `simple-term-menu` | `dialoguer` crate |
| `json` (stdlib) | `serde` + `serde_json` |
| `subprocess` | `std::process::Command` |
| `shutil` | `fs_extra` crate |
| `pathlib.Path` | `std::path::PathBuf` |
| `pytest` | `cargo test` (unit) + `assert_cmd` (integration CLI) |

---

## Structure cible Rust

```text
tidy-claude/
├── Cargo.toml
├── src/
│   ├── main.rs        ← point d'entrée + CLI (clap)
│   ├── config.rs      ← paths & lecture/écriture config.json
│   ├── state.rs       ← RunState (debug flag + compteurs)
│   ├── helpers.rs     ← fonctions pures (diff, merge, format, etc.)
│   └── ops.rs         ← opérations FS/git (backup, restore, cleanup…)
└── tests/
    ├── test_helpers.rs
    ├── test_cleanup.rs
    └── test_config_cli.rs
```

---

## Phases de migration

### Phase 1 — Initialisation du projet Rust

**Statut : ✅ Terminé**

- [x] Installation de `rustup` via Homebrew + toolchain stable (rustc 1.95.0)
- [x] `cargo init` dans le sous-répertoire `rust/`
- [x] Rédaction du `Cargo.toml` avec toutes les dépendances (clap, serde, serde_json avec `preserve_order`, directories, dialoguer, fs_extra, anyhow, regex + dev-deps assert_cmd, predicates, tempfile)
- [x] `cargo build` passe sans warning

---

### Phase 2 — `state.rs` : RunState

**Statut : ✅ Terminé**
**Source Python :** `src/tidy_claude/state.py`

- [x] Ajout d'un target lib (`src/lib.rs`) en plus du bin — pattern plus propre pour les tests d'intégration à venir
- [x] `#![allow(dead_code)]` temporaire au niveau du crate, à retirer en Phase 7
- [x] Implémentation de `RunState` avec `new`, `log`, `count`, `count_one`, `stats`, `get`
- [x] 3 tests unitaires inline : `new_defaults`, `count_increments`, `log_does_not_panic`
- [x] `cargo test` ✅ · `cargo clippy --all-targets -- -D warnings` ✅

---

### Phase 3 — `config.rs` : chemins & configuration

**Statut : ✅ Terminé**
**Source Python :** `src/tidy_claude/config.py`

- [x] Struct `AppPaths` (config_dir, config_file, default_data_dir, home, claude_dir, claude_json, settings_json) — **injection explicite** de dépendances (plus propre que les globals Python, testable sans monkey-patch)
- [x] `AppPaths::from_system()` utilise `directories::ProjectDirs` + `BaseDirs`
- [x] `AppPaths::for_test(home, config_root, data_root)` pour les tests
- [x] Struct `Config { data_dir, remote_backup: Option<String> }` avec serde
- [x] `load_config`, `save_config`, `ensure_config`, `Config::backup_dir`, `Config::default_for`
- [x] Constantes JSON : `CLAUDE_JSON_KEYS`, `SETTINGS_JSON_KEYS`, `settings_json_defaults()`, `category_for()`
- [x] 8 tests unitaires (load defaults, save/load roundtrip, ensure idempotent, backup_dir, paths derivation, category mapping, from_system smoke test)
- [x] `cargo test` ✅ 11/11 · `cargo clippy --all-targets -- -D warnings` ✅

**Écart volontaire avec Python :** pas de cache global `_config_cache`. La CLI chargera la config une fois au démarrage et la passera aux fonctions.

---

### Phase 4 — `helpers.rs` : fonctions pures

**Statut : ✅ Terminé**
**Source Python :** `src/tidy_claude/helpers.py` + `tests/test_helpers.py`

**Baseline pytest (capt. avant migration) :** 55 tests passent, dont 27 dans `test_helpers.py`.

#### Mapping fonctions

| Python | Rust |
|---|---|
| `diff_files(src, dst) -> int` | `fn diff_files(src: &Path, dst: &Path) -> usize` |
| `deep_merge(base, overlay) -> dict` | `fn deep_merge(base: &mut Value, overlay: &Value)` |
| `format_size(n) -> str` | `fn format_size(n: u64) -> String` |
| `resolve_claude_md(dir) -> list[Path]` | `fn resolve_claude_md(dir: &Path) -> Vec<PathBuf>` |
| `pretty_project_name(dirname, home) -> str` | `fn pretty_project_name(dirname: &str, home: &Path) -> String` |
| `extract_keys(data, keys, defaults) -> dict` | `fn extract_keys(data: &Value, keys: &[&str], defaults: Option<&Value>) -> Value` |
| `merge_keys_data(backed_up, current) -> dict` | `fn merge_keys_data(backed_up: &Value, current: &mut Value)` |

#### Mapping tests (1:1 avec `tests/test_helpers.py`)

| Python `TestClass::test_*` | Rust `mod::fn` | Statut |
|---|---|---|
| `TestDeepMerge::test_scalar_overwrite` | `deep_merge::scalar_overwrite` | ⬜ |
| `TestDeepMerge::test_new_key` | `deep_merge::new_key` | ⬜ |
| `TestDeepMerge::test_nested_dict` | `deep_merge::nested_dict` | ⬜ |
| `TestDeepMerge::test_list_union` | `deep_merge::list_union` | ⬜ |
| `TestDeepMerge::test_empty_overlay` | `deep_merge::empty_overlay` | ⬜ |
| `TestDeepMerge::test_empty_base` | `deep_merge::empty_base` | ⬜ |
| `TestDiffFiles::test_identical_files` | `diff_files::identical_files` | ⬜ |
| `TestDiffFiles::test_modified_file` | `diff_files::modified_file` | ⬜ |
| `TestDiffFiles::test_missing_dst` | `diff_files::missing_dst` | ⬜ |
| `TestDiffFiles::test_directory_comparison` | `diff_files::directory_comparison` | ⬜ |
| `TestFormatSize::test_bytes` | `format_size::bytes` | ⬜ |
| `TestFormatSize::test_kilobytes` | `format_size::kilobytes` | ⬜ |
| `TestFormatSize::test_megabytes` | `format_size::megabytes` | ⬜ |
| `TestFormatSize::test_gigabytes` | `format_size::gigabytes` | ⬜ |
| `TestResolveClaudeMd::test_no_claude_md` | `resolve_claude_md::no_claude_md` | ⬜ |
| `TestResolveClaudeMd::test_with_valid_refs` | `resolve_claude_md::with_valid_refs` | ⬜ |
| `TestResolveClaudeMd::test_no_refs` | `resolve_claude_md::no_refs` | ⬜ |
| `TestPrettyProjectName::test_home_dir` | `pretty_project_name::home_dir` | ⬜ |
| `TestPrettyProjectName::test_github_org_repo` | `pretty_project_name::github_org_repo` | ⬜ |
| `TestPrettyProjectName::test_github_repo_with_dashes` | `pretty_project_name::github_repo_with_dashes` | ⬜ |
| `TestPrettyProjectName::test_dotfile_dir` | `pretty_project_name::dotfile_dir` | ⬜ |
| `TestPrettyProjectName::test_plain_subdir` | `pretty_project_name::plain_subdir` | ⬜ |
| `TestPrettyProjectName::test_unknown_prefix` | `pretty_project_name::unknown_prefix` | ⬜ |
| `TestExtractKeys::test_subset` | `extract_keys::subset` | ⬜ |
| `TestExtractKeys::test_missing_key` | `extract_keys::missing_key` | ⬜ |
| `TestExtractKeys::test_with_defaults` | `extract_keys::with_defaults` | ⬜ |
| `TestMergeKeysData::test_into_empty` | `merge_keys_data::into_empty` | ⬜ |
| `TestMergeKeysData::test_with_overlap` | `merge_keys_data::with_overlap` | ⬜ |

**Fixtures utilisées :** `claude_tree` (conftest.py) → reproduite dans `tests/common/mod.rs` comme `fn claude_tree_fixture(tmp: &Path) -> PathBuf`.

#### Check de parité

À chaque fin de sous-phase, lancer :

```bash
# Python
rg -o '^\s*def (test_\w+)' tests/test_helpers.py -r '$1' --no-filename | sort
# Rust
rg -oU '#\[test\]\s*\n\s*fn (\w+)' rust/tests/test_helpers.rs -r '$1' --no-filename | sort
```text
Les deux sorties doivent contenir la même cardinalité (27), et chaque nom Python doit avoir son équivalent Rust (modulo suppression du préfixe `test_`).

### Phase 4 — `helpers.rs` : fonctions pures

**Statut : ✅ Terminé**
**Source Python :** `src/tidy_claude/helpers.py` + `tests/test_helpers.py`

**Résultats** :
- `src/helpers.rs` : 7 fonctions portées (`diff_files`, `deep_merge`, `format_size`, `resolve_claude_md`, `pretty_project_name`, `extract_keys`, `merge_keys_data`)
- `tests/test_helpers.rs` + `tests/common/mod.rs` : 28 tests, tous portés 1:1 depuis Python
- Script de parité `scripts/check-parity.sh` : ✅ `helpers: 28/28 tests ported`
- Python intact : `pytest` 55/55 ✅
- `cargo test` : 39/39 (11 lib + 28 helpers)
- `cargo clippy --all-targets -- -D warnings` ✅

**Étapes réalisées :**
- [x] Step 1 : Baseline `pytest -v` capturée (55 pass, 28 dans helpers)
- [x] Step 2 : `conftest.py` lu, fixture `claude_tree` reproduite dans `rust/tests/common/mod.rs`
- [x] Step 3 : Tableau de correspondance établi (ci-dessous)
- [x] Step 4 : `rust/src/helpers.rs` implémenté
- [x] Step 5 : `rust/tests/test_helpers.rs` + fixture écrits
- [x] Step 6 : `cargo test` ✅ · script de parité ✅ · clippy ✅

**Point d'attention résolu** : bug mineur dans ma 1re impl de `format_size` (warning `unused_assignments` sur une boucle ré-assignant une variable déjà init). Refactor en initialisant `value` juste avant la boucle.

---

### Phase 5 — `ops.rs` : opérations FS & git

**Statut : ✅ Terminé**
**Source Python :** `src/tidy_claude/ops.py`

**Résultats** :
- `src/ops.rs` implémenté avec les helpers FS, backup/restore, skills, git, cleanup
- Toutes les opérations prennent `&Config` + `&AppPaths` explicitement (pas de globals à la Python)
- `tests/test_cleanup.rs` : 18 tests portés 1:1 depuis `tests/test_cleanup.py`
- `tests/test_ops.rs` : 2 smoke tests additionnels pour `do_backup` / `do_restore`
- Script de parité : ✅ `helpers: 28/28`, ✅ `cleanup: 18/18`
- Python intact : `pytest` 55/55 ✅
- Rust : `cargo test` 59/59 ✅ · `cargo clippy --all-targets -- -D warnings` ✅

#### 5a — Helpers internes (copies)

- [x] `copy_to_backup(state, src, dst_rel, category)`
- [x] `restore_copy(state, backup_rel, target, category)`
- [x] `extract_keys_to_file(state, src, keys, dst_rel, category, defaults)`
- [x] `merge_keys_from_file(state, backup_rel, target, category)`

#### 5b — Opérations principales

- [x] `do_backup(state, cfg, paths)` — copie agents, memory, CLAUDE.md, extrait clés JSON
- [x] `do_restore(state, cfg, paths)` — restaure agents, memory, .md, merge clés JSON
- [x] `do_skills(state, cfg, paths)` — lit `skills.json`, installe via `sh -c`
- [x] `do_pull(state, cfg) -> Result<bool>` — `git rev-parse HEAD` puis `git pull --ff-only`
- [x] `do_commit(state, cfg, message)` — `git add -A` + `git diff --cached --quiet` + `git commit`
- [x] `do_push(state, cfg)` — `git push -u origin HEAD`

#### 5c — Cleanup

- [x] `ProjectInfo` struct (dirname, path, display_name, session_count, total_size)
- [x] `collect_projects(projects_dir) -> Vec<ProjectInfo>`
- [x] `named_sessions(claude_dir, project_paths) -> HashMap<String, String>`
  (lit `sessions/*.json` + fallback scan `.jsonl` pour `custom-title`)
- [x] `CleanupResult` struct (deleted_files, deleted_dirs, freed_bytes)
- [x] `do_cleanup(state, project_paths, older_than, dry_run, claude_dir, with_named_sessions) -> Result<CleanupResult>`

- [x] Porter **tous** les tests de `tests/test_cleanup.py` en `tests/test_cleanup.rs`

**Écart volontaire avec Python :** les opérations git/FS retournent `anyhow::Result<_>` au lieu d'exceptions implicites. `do_pull` retourne `Result<bool>` pour conserver la sémantique `false` = pull refusé (historique divergent) sans panique.

---

### Phase 6 — `main.rs` : CLI avec clap

**Statut : ⬜ À faire**
**Source Python :** `src/tidy_claude/cli.py`

```rust
#[derive(Parser)]
#[command(name = "tidy-claude", version, about)]
struct Cli {
    #[arg(long, global = true)]
    debug: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Sync,
    Status,
    Config { #[arg(long)] data_dir: Option<String>, #[arg(long)] remote_backup: Option<String> },
    Cleanup { #[arg(long, default_value_t = 7)] older_than: u32, #[arg(short = 'a', long)] all: bool, #[arg(long)] dry_run: bool, #[arg(long)] with_named_sessions: bool },
}
```

- Pas de subcommand → invoquer `sync` (comme Python)
- `_print_summary` : formatter les stats de `RunState`
- Menu interactif (`dialoguer::MultiSelect`) pour le cleanup

- [ ] Implémenter tous les sous-commandes
- [ ] Porter les tests de `tests/test_config_cli.py` en `tests/test_config_cli.rs` avec `assert_cmd`

---

### Phase 7 — Packaging & distribution

**Statut : ⬜ À faire**

- [ ] `cargo build --release` produit un binaire statique (ou quasi) `tidy-claude`
- [ ] Mettre à jour le `README.md` avec les instructions d'installation Rust
- [ ] GitHub Actions : build matrix (macOS ARM, macOS x86, Linux x86_64)
- [ ] Optionnel : `cargo install --path .` ou Homebrew tap

> **Note** : la suppression du code Python est traitée séparément en Phase 8 pour garder la Phase 7 focalisée sur le packaging du binaire.

---

### Phase 8 — Suppression de Python (decommissioning)

**Statut : ⬜ À faire**
**Prérequis :** Phase 7 terminée, binaire Rust validé en prod sur les machines cibles (au moins un cycle `sync` complet réussi).

**Objectif :** éliminer toute trace de l'implémentation Python du repo une fois la version Rust éprouvée. Avant cette phase, l'ancien code Python reste accessible pour rollback.

#### 8a — Point de non-retour (tag + branche de sauvegarde)

- [ ] Créer un tag git `python-final` sur le dernier commit contenant Python (`git tag python-final && git push --tags`)
- [ ] Optionnel : créer une branche `legacy/python` poussée en remote (archive long terme)
- [ ] Vérifier que le binaire Rust tourne correctement sur **toutes** les machines où l'outil est utilisé (macOS ARM minimum, Linux si concerné)
- [ ] Annoncer le retrait (CHANGELOG / release notes)

#### 8b — Suppression du code Python

- [ ] `rm -rf src/tidy_claude/` (package Python principal)
- [ ] `rm -rf tests/` (tests `pytest`)
- [ ] `rm -rf .venv/ .pytest_cache/ __pycache__/ dist/ *.egg-info/`
- [ ] `rm pyproject.toml uv.lock`

#### 8c — Promotion de la structure Rust à la racine

Actuellement le crate est dans `rust/`. Une fois Python retiré, la racine devient disponible.

- [ ] `git mv rust/Cargo.toml Cargo.toml`
- [ ] `git mv rust/src src`
- [ ] `git mv rust/tests tests`
- [ ] `rmdir rust`
- [ ] Vérifier `cargo build`, `cargo test`, `cargo clippy --all-targets -- -D warnings` depuis la racine
- [ ] Mettre à jour `Cargo.toml` : retirer `[[bin]] path = "src/main.rs"` (redevient implicite à la racine)

#### 8d — Nettoyage des références

- [ ] **`README.md`** :
  - Retirer la section "Dual Implementation" et la mention "Python (legacy)"
  - Retirer toute commande `pipx` / `uv` résiduelle
  - Ajuster les chemins (plus de `cd rust/` nécessaire)
- [ ] **`CONTRIBUTING.md`** : réécrire intégralement
  - Remplacer `uv sync --group dev` par `cargo build`
  - Remplacer `uv run pytest -v` par `cargo test`
  - Remplacer l'arbre `src/tidy_claude/*.py` par l'arbre `src/*.rs`
  - Remplacer la guideline "Add tests in `tests/test_helpers.py`" par l'équivalent Rust
- [ ] **`.gitignore`** : retirer le bloc `# ── Python ─────` (`__pycache__/`, `.venv/`, `dist/`, `*.egg-info/`, `.pytest_cache/`)
- [ ] **`.github/workflows/ci.yml`** :
  - Retirer tout job / step Python s'il en reste
  - Retirer `working-directory: ./rust` (plus nécessaire après 8c)
  - Vérifier que la matrice OS couvre bien les cibles finales
- [ ] **`scripts/check-parity.sh`** : supprimer (n'a plus de sens sans source Python de référence)
- [ ] **`.claude/`** : auditer les éventuelles références Python
- [ ] **`.markdownlint.yml`** : vérifier qu'aucune règle ne cible du Markdown Python-specific

#### 8e — Nettoyage du code Rust (dette technique accumulée pendant la migration)

- [ ] Retirer `#![allow(dead_code)]` au niveau du crate dans `src/lib.rs` (était temporaire, cf. Phase 2)
- [ ] Traiter les `dead_code` warnings résultants (soit utiliser le code, soit le supprimer)
- [ ] Retirer les éventuels commentaires `// Python: ...` ou `// ported from Python` qui traînent
- [ ] Retirer les TODOs liés à la migration
- [ ] `cargo clippy --all-targets -- -D warnings` doit passer clean

#### 8f — Métadonnées & outillage

- [ ] **`Cargo.toml`** : renseigner `repository`, `homepage`, `readme`, `keywords`, `categories` (propre pour un éventuel publish crates.io)
- [ ] **Badges README** : remplacer tout badge Python (coverage pytest, PyPI version) par leurs équivalents Rust (crates.io, docs.rs, CI status)
- [ ] **LICENSE** : vérifier que la mention reste cohérente
- [ ] **GitHub repo settings** : mettre à jour le "primary language" si bloqué sur Python (généralement auto-détecté après suppression)
- [ ] **GitHub topics** : retirer `python`, ajouter `rust`, `cli`

#### 8g — Validation finale

- [ ] `rg -i 'python|pytest|pyproject|pipx|\buv\b|click|platformdirs' -g '!MIGRATION_PLAN.md' -g '!.git'` doit ne remonter que des faux positifs justifiables
- [ ] `cargo test` : tous les tests passent depuis la racine
- [ ] `cargo build --release` : binaire produit
- [ ] CI verte sur un PR dédié
- [ ] Un cycle `tidy-claude sync` complet réussit sur la machine principale
- [ ] Commit final : `chore: remove Python implementation (migration complete)`

**Écart volontaire :** on **ne conserve pas** de dossier `python-legacy/`. Le tag `python-final` + la branche `legacy/python` en remote suffisent comme archive — pas de pollution du repo courant.

---

## Suivi global & modèle optimal par phase

> **Tarifs** : source OpenRouter API + Google AI + Anthropic, vérifiés au 23 avril 2026.
> Les coûts sont calculés sur la base du tarif API public (pay-as-you-go), sans tenir compte d'aucun abonnement.

### Tarifs API de référence (USD / 1M tokens)

| Modèle | Input | Output | Positionnement |
|---|---|---|---|
| **Qwen 3.6 35B** (local MLX) | $0.00 | $0.00 | Local, gratuit |
| **GPT-4.1 Nano** | $0.10 | $0.40 | Ultra-cheap cloud |
| **Gemini 2.5 Flash-Lite** | $0.10 | $0.40 | Ultra-cheap cloud |
| **GPT-4.1 Mini** | $0.40 | $1.60 | Cheap cloud |
| **Gemini 2.5 Flash** | $0.30 | $2.50 | Cheap cloud, reasoning |
| **Gemini 2.5 Pro** | $1.25 | $10.00 | Mid-range, code-capable |
| **GPT-4.1** | $2.00 | $8.00 | Mid-range, coding fort |
| **Claude Haiku 4.5** | $1.00 | $5.00 | Cheap Anthropic |
| **Claude Sonnet 4.6** | $3.00 | $15.00 | Balanced Anthropic |
| **Claude Opus 4.7** | $5.00 | $25.00 | Flagship Anthropic |

### Coût estimé par phase et par modèle (USD)

> Estimations basées sur les volumes réels observés en session (ratio input/output ~90/10 avec prompt caching).

| Phase | Tokens (in/out) | Qwen local | GPT-4.1 Nano | Gemini 2.5 Flash | GPT-4.1 | Claude Haiku 4.5 | Claude Sonnet 4.6 | Claude Opus 4.7 |
|---|---|---|---|---|---|---|---|---|
| **1** Planning | 140K / 10K | Free | $0.018 | $0.067 | $0.360 | $0.190 | $0.570 | $0.950 |
| **2** state.rs | 45K / 5K | Free | <$0.01 | $0.026 | $0.130 | $0.070 | $0.210 | $0.350 |
| **3** config.rs | 135K / 15K | Free | $0.020 | $0.078 | $0.390 | $0.210 | $0.630 | $1.050 |
| **4** helpers.rs | 360K / 40K | Free | $0.052 | $0.208 | $1.040 | $0.560 | $1.680 | $2.800 |
| **5** ops.rs | 820K / 80K | Free | $0.114 | $0.446 | $2.280 | $1.220 | $3.660 | $6.100 |
| **6** main.rs | 310K / 40K | Free | $0.047 | $0.193 | $0.940 | $0.510 | $1.530 | $2.550 |
| **7** Packaging | 130K / 20K | Free | $0.021 | $0.089 | $0.420 | $0.230 | $0.690 | $1.150 |
| **TOTAL** | **1.94M / 210K** | **Free** | **$0.278** | **$1.107** | **$5.560** | **$2.990** | **$8.970** | **$14.950** |

### Modèle optimal par phase

| Phase | Statut | Modèle optimal | Justification |
|---|---|---|---|
| **1** Planning | ✅ Terminé | **Claude Sonnet 4.6** | Complexité architecturale élevée, décisions structurantes |
| **2** state.rs | ✅ Terminé | **Qwen 3.6 35B** | Implémentation triviale, zéro raisonnement complexe |
| **3** config.rs | ✅ Terminé | **GPT-4.1** ou **Gemini 2.5 Pro** | Cross-platform logic, serde edge cases — bon rapport qualité/prix |
| **4** helpers.rs | ✅ Terminé | **GPT-4.1** (code) + **Qwen** (tests) | Sémantique deep_merge critique ; 28 tests répétitifs → local |
| **5** ops.rs | ✅ Terminé | **Claude Sonnet 4.6** (code) + **Qwen** (tests) | Risque élevé (suppression de fichiers), raisonnement fin nécessaire |
| **6** main.rs | ⬜ À faire | **GPT-4.1** ou **Gemini 2.5 Flash** | CLI wiring + 9 tests intégration — routinier, rapport qualité/prix optimal |
| **7** Packaging | ⬜ À faire | **Qwen 3.6 35B** | Boilerplate pur, aucun raisonnement requis |
| **8** Decommissioning Python | ⬜ À faire | **Qwen 3.6 35B** (suppressions) + **GPT-4.1 Mini** (nettoyage Rust) | Majoritairement `rm` + édition de doc → local. Nettoyage du `#![allow(dead_code)]` peut générer des warnings à trier → cloud cheap |

**Légende** : ⬜ À faire · 🔄 En cours · ✅ Terminé · ❌ Bloqué

### Stratégie de coûts optimale (phases 1–7)

| Stratégie | Coût total | Détail |
|---|---|---|
| **All Qwen local** | **Free** | Qualité moindre sur phases à risque élevé |
| **All GPT-4.1 Nano** | **$0.28** | Très cheap mais qualité insuffisante pour phases 1/5 |
| **All Gemini 2.5 Flash** | **$1.11** | Bon rapport, capable sur la plupart des phases |
| **All GPT-4.1** | **$5.56** | Overkill sur boilerplate |
| **All Claude Sonnet 4.6** | **$8.97** | Overkill, ~10x inutile pour phases triviales |
| **All Claude Opus 4.7** | **$14.95** | Massif overkill |
| **Hybrid optimal** *(Sonnet sur 1+5, GPT-4.1 sur 3+4+6, Qwen sur 2+7)* | **~$1.60** | Meilleur ratio qualité/coût global |

### Justifications détaillées

#### Phase 1 (Planning) — Sonnet 4.6

Nombreuses décisions structurantes (lib vs bin crate, injection de dépendances, serde features). Raisonnement haut. Volume faible (140K). Sonnet = $0.57, Opus = $0.95. Le delta Sonnet→Opus ($0.38) ne justifie pas l'écart pour une tâche de planning.

#### Phase 2 (state.rs) — Qwen local

Struct basique + HashMap + 3 méthodes + 3 tests. Aucun raisonnement nécessaire. GPT-4.1 Nano ($ <$0.01) serait le maximum raisonnable en cloud.

#### Phase 3 (config.rs) — GPT-4.1 ou Gemini 2.5 Pro

`directories` crate + serde defaults + round-trip test = raisonnement moyen. GPT-4.1 ($0.39) ou Gemini 2.5 Pro ($0.32) suffisent largement. Sonnet ($0.63) serait acceptable mais 60% plus cher sans gain visible.

#### Phase 4 (helpers.rs) — GPT-4.1 (code) + Qwen (tests)

- `deep_merge` union semantics (Python equality = JSON Value equality) : raisonnement requis → GPT-4.1.
- 28 tests répétitifs (1:1 avec Python) : pur copier-adapter → Qwen local, économie $1.04.

#### Phase 5 (ops.rs) — Sonnet 4.6 (code) + Qwen (tests)

**Phase la plus critique.** `do_cleanup` avec 4 niveaux de logique imbriquee (mtime/cutoff/named sessions/subagent dirs). Une erreur = suppression involontaire de fichiers utilisateur. Sonnet ($3.66 pour 820K tokens) justifie son coût. Les 18 tests cleanup sont répétitifs → Qwen local.

#### Phase 6 (main.rs + CLI) — GPT-4.1 ou Gemini 2.5 Flash

CLI wiring avec clap (patterns standards), 9 tests `assert_cmd` (patterns très réguliers). Raisonnement moyen. GPT-4.1 ($0.94) = bon rapport. Gemini 2.5 Flash ($0.19) serait suffisant si on veut économiser.

#### Phase 7 (Packaging) — Qwen local

README + GitHub Actions YAML. Aucun raisonnement technique. Coût cloud (même Nano) = $0.02 inutilement.

---

## Points d'attention

### Gestion d'erreurs

Python lève des exceptions et laisse `click` les afficher. En Rust, utiliser `anyhow::Result<T>` partout et convertir en message d'erreur propre dans `main.rs` avec `eprintln!` + `std::process::exit(1)`.

### Cache config

Python utilise une variable globale `_config_cache`. En Rust, passer `&Config` explicitement aux fonctions — plus propre, plus testable.

### Menu interactif

`simple-term-menu` (Python) ↔ `dialoguer::MultiSelect` (Rust). L'API est légèrement différente : `dialoguer` retourne les indices sélectionnés directement.

### `deep_merge` sur JSON

Utiliser `serde_json::Value::Object` pour la récursion. Pour l'union de tableaux JSON, itérer et dédoublonner en comparant les valeurs sérialisées.

### Tests d'intégration CLI

Les tests Python utilisent `click.testing.CliRunner`. En Rust, utiliser `assert_cmd::Command::cargo_bin("tidy-claude")` qui lance le vrai binaire dans un répertoire temporaire.

### Regex

`re.findall(r"@(\S+\.md)", content)` → `regex::Regex::new(r"@(\S+\.md)")` avec `.captures_iter()`.

### `time.time()` / mtime

`std::fs::metadata(path)?.modified()?` retourne un `SystemTime`. Comparer avec `SystemTime::now() - Duration::from_secs(days * 86400)`.
