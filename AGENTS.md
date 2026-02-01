# AGENTS — Règles de travail (Codex) pour la réécriture Rust

Ce dépôt est une réécriture en Rust d’une application “IDE terminal portable” destinée à fonctionner depuis une clé USB. L’application fournit un explorateur de fichiers, un éditeur, un panneau “commande shell”, et une intégration “Codex” (exécution de `codex exec --json ...`) sans installation sur la machine hôte.

Objectif principal : obtenir une base Rust robuste, maintenable, testée, et portable (Windows en priorité), puis itérer dessus proprement.

---

## 1) Contexte produit et contraintes non négociables

- Usage “USB / portable” : l’app doit fonctionner depuis un dossier racine (workspace) sur clé USB.
- Aucune installation requise sur la machine hôte :
    - ne pas écrire dans `~`, `%APPDATA%`, `C:\Program Files`, etc.
    - ne pas dépendre d’un service installé localement (sauf si c’est explicitement un mode fallback).
- Toute donnée/cache temporaire doit aller dans le workspace (root) :
    - `root/cache/**` (caches)
    - `root/tmp/**` (temporaires)
    - `root/codex_home/**` (auth/config Codex via `CODEX_HOME`)
    - `root/.usbide/**` (outils/installs portables gérés par l’app)
- Sécurité : les fichiers d’auth (ex: `codex_home/auth.json`) contiennent des tokens.
    - Ne jamais committer ni exposer ces fichiers.
    - Ne jamais imprimer un token dans les logs.
    - Ajouter/maintenir `.gitignore` en conséquence.

---

## 2) Méthode de travail attendue (comme un ingénieur logiciel Rust)

À chaque tâche (bugfix, feature, refacto), appliquer cette séquence :

1) Comprendre
- Lire le contexte : code concerné, docs du repo, issues/bug.md/notes.
- Identifier les invariants et comportements attendus (parité fonctionnelle si c’est une réécriture).
- Lister les cas limites (Windows, chemins, encodages, absence réseau, absence node/codex, proxy, etc.).

2) Planifier (toujours explicite)
- Écrire un plan court avant de coder :
    - ce qui change (fichiers/modules)
    - stratégie de tests
    - stratégie de rollback si risque
- Si une dépendance est ajoutée : justifier (pourquoi, alternatives, coût).

3) Implémenter en petits incréments
- Petites PR logiques, faciles à relire.
- Pas de “gros commit monolithique” qui mélange refacto + feature + formatting.

4) Tester et valider
- Ajouter des tests unitaires / d’intégration pour toute logique non triviale.
- Ne pas dépendre du réseau ni d’un vrai `codex` dans les tests (mock/stub).
- Exécuter et reporter les commandes réellement lancées (ne jamais prétendre avoir exécuté).

5) Finaliser
- `cargo fmt`, `cargo clippy`, `cargo test` (voir checklists plus bas).
- Mettre à jour la doc (README/notes) si le comportement utilisateur change.
- S’assurer que Windows reste supporté (paths, `.cmd`, encodage console).

---

## 3) Qualité de code Rust (conventions obligatoires)

Robustesse
- Zéro panique en production : pas de `unwrap()` / `expect()` hors tests.
- Utiliser des erreurs typées et contextualisées :
    - Recommandé : `thiserror` pour les erreurs de domaine + `anyhow` (ou `eyre`) au bord (main/handlers).
    - Toujours ajouter du contexte (`.context("...")`) sur les erreurs I/O/process.
- Les fonctions “utilitaires” doivent valider leurs entrées (argv vide, chemins invalides, prompt vide, etc.).

Style
- `rustfmt` obligatoire (format automatique).
- `clippy` sans warnings (ou warnings justifiés, exceptionnellement).
- Nommage clair, responsabilités séparées (UI ≠ exécution process ≠ FS/encoding).
- Commentaires et messages UI : en français.
    - Commenter “pourquoi” (raison/contrainte), pas “quoi” (évident).

Dépendances
- Préférer une base simple et stable.
- Chaque crate ajouté doit être nécessaire et utilisé de façon non superficielle.
- Éviter les crates non maintenues / expérimentales si une alternative mature existe.

---

## 4) Architecture cible (principe, à adapter au repo)

Même si la structure exacte dépend du projet, on vise une séparation nette :

- `src/main.rs` :
    - parse CLI (`--root`), init, lancement UI.
- `src/config/` :
    - lecture env vars, options runtime.
- `src/fs/` :
    - arborescence, lecture/écriture fichiers, détection binaire/encoding.
- `src/process/` :
    - runner subprocess cross-platform (stream stdout/stderr), wrappers Windows (`cmd.exe /c`).
- `src/codex/` :
    - construction env (PATH, CODEX_HOME, sanitize), argv `login/status/exec/install`,
    - parsing JSONL, extraction messages, diagnostics HTTP.
- `src/ui/` :
    - TUI (layout, keybindings, widgets), état, dispatch d’actions,
    - aucune logique “métier” lourde ici, juste orchestration.
- `tests/` et/ou `src/**/tests.rs` :
    - tests unitaires (pure logic),
    - tests d’intégration (parsing CLI, invariants de layout/env/argv).

---

## 5) Portabilité et environnement “USB”

Règles de chemins (invariants)
- Tout part d’un `root_dir` (workspace) fourni par `--root` (ou `.` par défaut).
- Dossiers à créer au boot si absents :
    - `root/cache/pip` (si utile), `root/cache/pycache` (si utile), `root/cache/npm`
    - `root/tmp`
    - `root/codex_home`
    - `root/.usbide/*` (outils installés par l’app)
- Ne pas écrire ailleurs que `root_dir` (sauf contraintes OS temporaires, à minimiser).

Variables d’environnement (compatibilité et contrôle)
- Toujours définir pour les subprocess lancés par l’app (dans leur `env`) :
    - `CODEX_HOME = root/codex_home`
    - `TEMP` / `TMP = root/tmp` (Windows)
    - `NPM_CONFIG_CACHE = root/cache/npm`
    - optionnel : `PYTHONUTF8=1`, `PYTHONIOENCODING=utf-8` si on lance Python
- Nettoyage “anti-surprise” pour Codex (par défaut) :
    - retirer `OPENAI_API_KEY`, `CODEX_API_KEY` (sauf override explicite)
    - retirer `OPENAI_BASE_URL`, `OPENAI_API_BASE`, `OPENAI_API_HOST` (sauf override explicite)
- Conserver les switches via env vars (compatibles si possible avec l’app originale) :
    - `USBIDE_CODEX_ALLOW_API_KEY=1`
    - `USBIDE_CODEX_ALLOW_CUSTOM_BASE=1`
    - `USBIDE_CODEX_DEVICE_AUTH=1`
    - `USBIDE_CODEX_AUTO_INSTALL=0/1`
    - `USBIDE_CODEX_NPM_PACKAGE=@openai/codex` (ou autre)

---

## 6) Intégration Codex (exigences fonctionnelles)

Objectif : permettre à l’utilisateur de lancer `codex` même si la machine hôte n’a rien installé, via un outillage portable embarqué dans le workspace.

6.1 Détection & installation
- Support “portable” recommandé :
    - Node portable attendu dans `root/tools/node/` (Windows: `node.exe`, sinon `bin/node` ou `node`)
    - Installation `npm install --prefix root/.usbide/codex ...`
- `codex` est considéré disponible si :
    - (portable) `node` + entrypoint JS du package installé sont présents, ou
    - (fallback) `codex` est trouvable dans le PATH

6.2 Windows : shims `.cmd` / `.bat` / `.ps1`
- Sur Windows, un `codex.cmd` ne se lance pas comme un `.exe` via un spawn “direct”.
- Si fallback PATH retourne un `.cmd`/`.bat` :
    - lancer via `cmd.exe /d /s /c <path_to_cmd>`
- Si `.ps1` :
    - lancer via PowerShell avec `-ExecutionPolicy Bypass` (si et seulement si nécessaire)

6.3 Auth : pré-check obligatoire
- Avant toute exécution `codex exec`, faire `codex login status`.
- Si status != 0 :
    - expliquer clairement “pas authentifié dans ce CODEX_HOME”
    - guider vers la commande login (et device auth si le navigateur est bloqué)

6.4 Sortie JSONL : affichage lisible
- L’app doit supporter la sortie `--json` (JSONL streaming).
- En “vue compacte” :
    - afficher messages “Utilisateur”, “Assistant”, “Action”
    - dédupliquer les répétitions
    - concaténer les deltas (`response.output_text.delta`) et flush à la fin
- Diagnostiquer les erreurs HTTP (exemples) :
    - 401 : auth invalide (login)
    - 403 : accès interdit (droits, méthode login, réseau)
    - 407 : proxy auth
    - 429 : rate limit
    - 5xx : serveur

---

## 7) UI TUI : attentes minimales

L’UI Rust doit viser une ergonomie proche de l’existant :
- Panneau gauche : arborescence fichiers.
- Panneau droit : éditeur (texte) avec état “dirty”.
- Bas : deux zones :
    - “Commande” (shell) + “Journal”
    - “Codex” (input prompt) + “Sortie Codex”
- Keybindings cibles (peuvent évoluer mais garder l’esprit) :
    - Ctrl+S : sauvegarder
    - F5 : exécuter (au moins commande associée au fichier courant si applicable)
    - Ctrl+L : clear logs
    - Ctrl+R : reload tree
    - Ctrl+K : codex login
    - Ctrl+T : codex check/status
    - Ctrl+I : codex install
    - Ctrl+M : toggle compact/brut codex
    - Ctrl+Q : quitter
- Les textes UI sont en français.

Note : si un choix technique impose une adaptation (limitations d’un crate TUI), l’expliquer et proposer un compromis.

---

## 8) Tests : stratégie obligatoire

Principe : on teste la logique, pas le réseau.

À couvrir systématiquement :
- Construction des argv (login/status/exec/install) :
    - prompt vide refusé
    - flags ajoutés correctement (`--json`, `--device-auth`, etc.)
- Construction env :
    - PATH prefixé comme attendu
    - `CODEX_HOME` = `root/codex_home`
    - sanitize des variables “dangereuses”
- Résolution node/npm/codex :
    - priorité au portable si présent
    - fallback PATH
    - cas Windows `.cmd` wrapper
- Parser JSONL :
    - extraction messages assistant/user/action
    - déduplication
    - gestion delta + flush
- Runner subprocess :
    - refuse argv vide
    - stream de lignes (tests via mock / fake process)

Recommandation structure testable :
- Isoler la logique pure (build argv/env, parse JSON) dans des fonctions sans I/O.
- Abstraire le runner process via un trait (ex: `ProcessRunner`) et fournir un fake pour tests.

---

## 9) Journalisation d’incidents (bug.md)

- Toute erreur significative doit pouvoir être consignée dans `root/bug.md` (append).
- Format lisible (Markdown) avec :
    - horodatage ISO
    - niveau (info/avertissement/erreur)
    - contexte (ex: `codex_exec`, `codex_status`, `fs_open`, etc.)
    - message
    - détails et backtrace si disponible (sans secrets)
- L’écriture du log ne doit jamais faire crasher l’app (best-effort).

---

## 10) Definition of Done (checklist)

Avant de considérer une tâche “terminée”, vérifier :

Qualité
- Le code compile sans warnings bloquants.
- Pas de panics en production, pas de `unwrap/expect` hors tests.
- Les erreurs sont contextualisées.

Tests & tooling
- `cargo fmt --all` OK
- `cargo clippy --all-targets --all-features -- -D warnings` OK
- `cargo test --all` OK
- Les tests ne requièrent pas Internet ni un vrai `codex`.

Produit
- Le comportement respecte les contraintes “USB portable”.
- Les logs ne contiennent pas de secrets.
- Les messages utilisateur sont compréhensibles et actionnables.

---

## 11) Commandes standard (à exécuter réellement)

```sh
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

Ne jamais déclarer “OK” si ces commandes n’ont pas été exécutées dans l’environnement courant.

---

## 12) Règles de communication (dans les réponses de l’agent)

- Toujours expliquer :
    - la cause du problème (ou l’hypothèse la plus probable),
    - le correctif,
    - les impacts (risques, compatibilité, Windows),
    - les tests ajoutés / exécutés.
- Si une information est incertaine, le dire explicitement et proposer un moyen de vérification.
- Ne pas “inventer” des sorties de commandes, des logs, ou des résultats de tests.
