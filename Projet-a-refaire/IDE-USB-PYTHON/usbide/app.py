from __future__ import annotations

import json
import os
import re
import shutil
import textwrap
import traceback
from dataclasses import dataclass
from datetime import datetime
from pathlib import Path
from typing import Callable, Optional, Sequence

from rich.markup import escape as rich_escape
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.widgets import DirectoryTree, Footer, Header, Input, RichLog, TextArea

from usbide.encoding import detect_text_encoding, is_probably_binary
from usbide.runner import (
    codex_bin_dir,
    codex_cli_available,
    codex_env,
    codex_entrypoint_js,
    codex_exec_argv,
    codex_install_argv,
    codex_install_prefix,
    codex_login_argv,
    codex_status_argv,
    node_executable,
    parse_tool_list,
    pip_install_argv,
    pyinstaller_available,
    pyinstaller_build_argv,
    pyinstaller_install_argv,
    python_run_argv,
    python_scripts_dir,
    stream_subprocess,
    tools_env,
    tools_install_prefix,
    windows_cmd_argv,
)


@dataclass
class OpenFile:
    path: Path
    encoding: str
    dirty: bool = False


class USBIDEApp(App):
    CSS_PATH = "usbide.tcss"

    # Ordre volontaire pour regrouper les actions d'execution avant les outils dev.
    # Priorite sur les raccourcis Codex pour eviter la capture par les widgets d'entree.
    BINDINGS = [
        Binding("ctrl+s", "save", "Sauvegarder"),
        Binding("f5", "run", "Executer"),
        Binding("ctrl+l", "clear_log", "Effacer les journaux"),
        Binding("ctrl+r", "reload_tree", "Recharger l'arborescence"),
        Binding("ctrl+k", "codex_login", "Connexion Codex", priority=True),
        Binding("ctrl+t", "codex_check", "Verifier Codex", priority=True),
        Binding("ctrl+i", "codex_install", "Installer Codex", priority=True),
        Binding("ctrl+m", "toggle_codex_view", "Vue Codex", priority=True),
        Binding("ctrl+e", "build_exe", "Construire l'EXE"),
        Binding("ctrl+d", "dev_tools", "Outils de dev"),
        Binding("ctrl+q", "quit", "Quitter"),
    ]

    def __init__(self, root_dir: Path) -> None:
        super().__init__()
        self.root_dir = root_dir.resolve()
        self.current: Optional[OpenFile] = None
        self._loading_editor: bool = False
        self._codex_install_attempted: bool = False
        self._pyinstaller_install_attempted: bool = False
        # Mode compact par defaut pour rendre la sortie Codex lisible.
        self._codex_compact_view: bool = True
        # Cache simple pour eviter les doublons (type + contenu).
        self._last_codex_message: Optional[str] = None
        # Journal des erreurs/problemes a la racine du workspace.
        self._bug_log_path: Path = self.root_dir / "bug.md"

    def get_css_variables(self) -> dict[str, str]:
        """Definit la palette moderne du theme Textual."""
        # Palette inspiree des UI modernes (teal + bleus profonds).
        variables = super().get_css_variables()
        variables.update(
            {
                "ui-bg": "#0b0f12",
                "ui-bg-2": "#0e1720",
                "ui-surface": "#0f161d",
                "ui-panel": "#121c24",
                "ui-panel-strong": "#15212b",
                "ui-shadow": "#0b1015",
                "ui-text": "#e6edf3",
                "ui-text-muted": "#9aa4b2",
                "ui-accent": "#2dd4bf",
                "ui-accent-2": "#38bdf8",
                "ui-warning": "#f59e0b",
                "ui-warning-strong": "#fbbf24",
                "ui-success": "#22c55e",
                "ui-header-1": "#0f1b24",
                "ui-header-2": "#12303a",
                "ui-footer": "#0d161d",
            }
        )
        return variables

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="main"):
            tree = DirectoryTree(str(self.root_dir), id="tree")
            tree.border_title = "Fichiers"
            yield tree

            with Vertical(id="right"):
                editor = self._make_editor()
                editor.border_title = "Editeur"
                yield editor

                # Double journal: shell (gauche) / Codex (droite).
                with Horizontal(id="bottom"):
                    with Vertical(id="shell"):
                        cmd = Input(placeholder="> commande shell (Entree)", id="cmd")
                        cmd.border_title = "Commande"
                        yield cmd

                        log = RichLog(id="log", markup=True)
                        log.border_title = "Journal"
                        yield log

                    with Vertical(id="codex"):
                        codex_cmd = Input(
                            placeholder="> Codex (Entree) : lance `codex exec --json <prompt>`",
                            id="codex_cmd",
                        )
                        codex_cmd.border_title = "Codex"
                        yield codex_cmd

                        codex_log = RichLog(id="codex_log", markup=True)
                        codex_log.border_title = "Sortie Codex"
                        yield codex_log

        yield Footer()

    def _make_editor(self) -> TextArea:
        if hasattr(TextArea, "code_editor"):
            return TextArea.code_editor("", language=None, id="editor")  # type: ignore[attr-defined]
        return TextArea("", id="editor")

    def on_mount(self) -> None:
        self._ensure_portable_dirs()
        self._log_ui(
            f"[b]ValDev Pro v1[/b]\nRoot: {self.root_dir}\n"
            "Shell: champ 'Commande' - Codex: champ 'Codex' - Ctrl+K login - Ctrl+I install\n"
        )
        self._update_codex_title()
        self._refresh_title()
        self._apply_intro_animation()

    def _apply_intro_animation(self) -> None:
        """Anime l'apparition des panneaux pour un rendu plus moderne."""
        try:
            widgets = [
                self.query_one("#tree"),
                self.query_one("#editor"),
                self.query_one("#bottom"),
            ]
        except Exception:
            # Ne bloque pas le demarrage si un widget manque (tests/unitaires).
            return

        # On rend les panneaux invisibles avant le fondu d'entree.
        for widget in widgets:
            widget.styles.opacity = 0.0

        for index, widget in enumerate(widgets):
            widget.styles.animate(
                "opacity",
                1.0,
                duration=0.35,
                delay=0.08 * index,
                easing="out_cubic",
            )

    def _handle_exception(self, error: Exception) -> None:
        # Journalise les exceptions fatales avant de laisser Textual afficher l'erreur.
        self._record_issue(
            "erreur",
            f"Exception non geree: {error}",
            contexte="exception_fatale",
            exc=error,
        )
        super()._handle_exception(error)

    # ---------- logs ----------
    def _log_ui(self, msg: str) -> None:
        try:
            self.query_one("#log", RichLog).write(msg)
        except Exception:
            # Evite un crash si l'UI n'est pas encore montee (tests/unitaires).
            return

    def _log_output(self, msg: str) -> None:
        try:
            self.query_one("#log", RichLog).write(rich_escape(msg))
        except Exception:
            # Evite un crash si l'UI n'est pas encore montee (tests/unitaires).
            return

    def _codex_log_ui(self, msg: str) -> None:
        try:
            self.query_one("#codex_log", RichLog).write(msg)
        except Exception:
            # Evite un crash si l'UI n'est pas encore montee (tests/unitaires).
            return

    def _codex_log_output(self, msg: str) -> None:
        try:
            self.query_one("#codex_log", RichLog).write(rich_escape(msg))
        except Exception:
            # Evite un crash si l'UI n'est pas encore montee (tests/unitaires).
            return

    def _codex_mode_label(self) -> str:
        """Libelle du mode d'affichage Codex (compact vs brut)."""
        return "Compact" if self._codex_compact_view else "Brut"

    def _update_codex_title(self) -> None:
        """Mise a jour du titre du panneau Codex."""
        codex_log = self.query_one("#codex_log", RichLog)
        codex_log.border_title = "Sortie Codex"

    def _record_issue(
        self,
        niveau: str,
        message: str,
        *,
        contexte: str,
        details: Optional[str] = None,
        exc: Optional[BaseException] = None,
    ) -> None:
        """Enregistre un incident dans bug.md (mode append)."""
        # Le format Markdown facilite la lecture des rapports sur la cle USB.
        horodatage = datetime.now().isoformat(timespec="seconds")
        lignes = [
            f"## {horodatage}",
            f"- niveau: {niveau}",
            f"- contexte: {contexte}",
            f"- message: {message}",
        ]
        if details:
            lignes.append(f"- details: {details}")
        if exc is not None:
            lignes.append(f"- exception: {type(exc).__name__}: {exc}")
            trace = "".join(traceback.format_exception(exc)).rstrip()
            if trace:
                lignes.append("```")
                lignes.extend(trace.splitlines())
                lignes.append("```")
        lignes.append("")
        try:
            with self._bug_log_path.open("a", encoding="utf-8") as handle:
                handle.write("\n".join(lignes))
        except OSError:
            # Ne pas bloquer l'UI si le fichier bug.md est indisponible.
            return

    def _log_issue(
        self,
        msg: str,
        *,
        niveau: str,
        contexte: str,
        exc: Optional[BaseException] = None,
        codex: bool = False,
    ) -> None:
        """Affiche un probleme dans l'UI et le consigne dans bug.md."""
        # On choisit le journal cible (principal ou Codex) avant d'enregistrer l'incident.
        if codex:
            self._codex_log_ui(msg)
        else:
            self._log_ui(msg)
        self._record_issue(niveau, msg, contexte=contexte, exc=exc)

    async def _stream_and_log(
        self,
        argv: Sequence[str],
        *,
        cwd: Path,
        env: dict[str, str],
        output_log: Callable[[str], None],
        ui_log: Callable[[str], None],
        contexte: str,
        codex: bool = False,
    ) -> None:
        """Stream un subprocess et journalise les erreurs."""
        # Centralise la gestion d'erreurs pour garantir un log bug.md complet.
        try:
            async for ev in stream_subprocess(argv, cwd=cwd, env=env):
                if ev["kind"] == "line":
                    output_log(ev["text"])
                    continue
                if ev["returncode"] not in (None, 0):
                    self._log_issue(
                        f"[red]{contexte} terminee en erreur (rc={ev['returncode']}).[/red]",
                        niveau="erreur",
                        contexte=contexte,
                        codex=codex,
                    )
                ui_log(f"[dim]{ev['text']}[/dim]")
        except Exception as exc:
            self._log_issue(
                f"[red]Erreur execution {contexte}:[/red] {exc}",
                niveau="erreur",
                contexte=contexte,
                exc=exc,
                codex=codex,
            )

    # ---------- env portable ----------
    def _ensure_portable_dirs(self) -> None:
        for path in (
            self.root_dir / "cache" / "pip",
            self.root_dir / "cache" / "pycache",
            self.root_dir / "cache" / "npm",
            self.root_dir / "tmp",
            self.root_dir / "codex_home",
        ):
            path.mkdir(parents=True, exist_ok=True)

    def _portable_env(self, env: dict[str, str]) -> dict[str, str]:
        env["PIP_CACHE_DIR"] = str(self.root_dir / "cache" / "pip")
        env["PYTHONPYCACHEPREFIX"] = str(self.root_dir / "cache" / "pycache")
        env["TEMP"] = str(self.root_dir / "tmp")
        env["TMP"] = str(self.root_dir / "tmp")
        env["PYTHONNOUSERSITE"] = "1"

        env["CODEX_HOME"] = str(self.root_dir / "codex_home")

        env["NPM_CONFIG_CACHE"] = str(self.root_dir / "cache" / "npm")
        env["NPM_CONFIG_UPDATE_NOTIFIER"] = "false"
        return env

    def _truthy(self, value: str | None) -> bool:
        """Retourne True si la valeur correspond a un booleen "vrai"."""
        return (value or "").strip().lower() in {"1", "true", "yes", "on"}

    def _sanitize_codex_env(self, env: dict[str, str]) -> dict[str, str]:
        """Nettoie les variables Codex pour eviter une auth involontaire."""
        # Par defaut on force le login ChatGPT via CODEX_HOME, sauf override explicite.
        allow_api_key = self._truthy(os.environ.get("USBIDE_CODEX_ALLOW_API_KEY"))
        allow_custom_base = self._truthy(os.environ.get("USBIDE_CODEX_ALLOW_CUSTOM_BASE"))

        if not allow_api_key:
            env.pop("OPENAI_API_KEY", None)
            env.pop("CODEX_API_KEY", None)

        if not allow_custom_base:
            env.pop("OPENAI_BASE_URL", None)
            env.pop("OPENAI_API_BASE", None)
            env.pop("OPENAI_API_HOST", None)

        return env

    def _codex_env(self) -> dict[str, str]:
        env = os.environ.copy()
        env.setdefault("PYTHONUTF8", "1")
        env.setdefault("PYTHONIOENCODING", "utf-8")
        env = self._portable_env(env)
        # Evite que des variables globales cassent l'authentification Codex.
        env = self._sanitize_codex_env(env)
        return codex_env(self.root_dir, env)

    def _tools_env(self) -> dict[str, str]:
        env = os.environ.copy()
        env.setdefault("PYTHONUTF8", "1")
        env.setdefault("PYTHONIOENCODING", "utf-8")
        env = self._portable_env(env)
        return tools_env(self.root_dir, env)

    def _wheelhouse_path(self) -> Optional[Path]:
        wheelhouse = self.root_dir / "tools" / "wheels"
        return wheelhouse if wheelhouse.is_dir() else None

    # ---------- UI title ----------
    def _refresh_title(self) -> None:
        if not self.current:
            self.title = "ValDev Pro v1"
            self.sub_title = str(self.root_dir)
            return
        dirty = " *" if self.current.dirty else ""
        self.title = f"ValDev Pro v1{dirty}"
        self.sub_title = f"{self.current.path}  ({self.current.encoding})"

    # ---------- tree ----------
    def on_directory_tree_file_selected(self, event: DirectoryTree.FileSelected) -> None:
        path: Path = event.path
        if path.is_dir():
            return

        try:
            if is_probably_binary(path):
                self._log_issue(
                    f"[yellow]Binaire/non texte ignore:[/yellow] {path}",
                    niveau="avertissement",
                    contexte="ouverture_fichier",
                )
                return
        except OSError as exc:
            self._log_issue(
                f"[red]Acces fichier impossible:[/red] {path} ({exc})",
                niveau="erreur",
                contexte="ouverture_fichier",
                exc=exc,
            )
            return

        encoding = detect_text_encoding(path)
        try:
            text = path.read_text(encoding=encoding)
        except UnicodeDecodeError:
            text = path.read_text(encoding=encoding, errors="replace")
        except OSError as exc:
            self._log_issue(
                f"[red]Erreur ouverture:[/red] {path} ({exc})",
                niveau="erreur",
                contexte="ouverture_fichier",
                exc=exc,
            )
            return

        editor = self.query_one(TextArea)
        self._loading_editor = True
        editor.text = text
        self._loading_editor = False

        self.current = OpenFile(path=path, encoding=encoding, dirty=False)
        self._refresh_title()

    def on_text_area_changed(self, event: TextArea.Changed) -> None:
        if self._loading_editor or not self.current:
            return
        ta = getattr(event, "text_area", None) or getattr(event, "control", None)
        if getattr(ta, "id", None) != "editor":
            return
        self.current.dirty = True
        self._refresh_title()

    # ---------- inputs ----------
    async def on_input_submitted(self, event: Input.Submitted) -> None:
        if event.input.id == "cmd":
            await self._run_shell(event)
        elif event.input.id == "codex_cmd":
            await self._run_codex(event)

    async def _run_shell(self, event: Input.Submitted) -> None:
        cmd = event.value.strip()
        event.input.value = ""
        if not cmd:
            return
        self._log_ui(f"\n[b]$[/b] {rich_escape(cmd)}")
        argv = windows_cmd_argv(cmd) if os.name == "nt" else ["sh", "-lc", cmd]
        env = self._portable_env(os.environ.copy())

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._log_output,
            ui_log=self._log_ui,
            contexte="commande shell",
        )

    async def _codex_logged_in(self, env: dict[str, str]) -> bool:
        """Retourne True si `codex login status` indique une session valide."""
        argv = codex_status_argv(self.root_dir, env)
        rc: int | None = None
        out_lines: list[str] = []

        # On collecte la sortie pour aider l'utilisateur a corriger l'auth.
        try:
            async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
                if ev["kind"] == "line":
                    out_lines.append(ev["text"])
                else:
                    rc = ev["returncode"]
        except FileNotFoundError as exc:
            # Retour clair si le binaire Codex est introuvable (evite un silence en UI).
            self._log_issue(
                "[red]Codex introuvable pour verifier le login.[/red]",
                niveau="erreur",
                contexte="codex_status",
                exc=exc,
                codex=True,
            )
            self._codex_log_action("Astuce: Ctrl+I pour installer Codex ou Ctrl+T pour diagnostiquer.")
            return False
        except Exception as exc:
            # Capture generique pour ne pas bloquer l'UI en cas d'erreur d'execution.
            self._log_issue(
                f"[red]Erreur verification login Codex:[/red] {exc}",
                niveau="erreur",
                contexte="codex_status",
                exc=exc,
                codex=True,
            )
            return False

        if rc is None:
            # Protection: si le process ne renvoie pas de code, on considere la session invalide.
            self._log_issue(
                "[red]Statut Codex indetermine.[/red]",
                niveau="erreur",
                contexte="codex_status",
                codex=True,
            )
            return False

        if rc == 0:
            return True

        if self._codex_compact_view:
            self._codex_log_action("Codex n'est pas authentifie dans ce CODEX_HOME.")
            for line in out_lines:
                if line.strip():
                    self._codex_log_action(line)
            self._codex_log_action("Fais Ctrl+K pour `codex login` (ou device auth).")
        else:
            self._codex_log_ui("[yellow]Codex n'est pas authentifie dans ce CODEX_HOME.[/yellow]")
            for line in out_lines:
                if line.strip():
                    self._codex_log_output(line)
            self._codex_log_ui("[yellow]Fais Ctrl+K pour `codex login` (ou device auth).[/yellow]")
        if not self._codex_device_auth_enabled():
            # Astuce pour eviter le blocage si le navigateur ne s'ouvre pas.
            hint = (
                "Astuce: si le navigateur ne s'ouvre pas, "
                "definis USBIDE_CODEX_DEVICE_AUTH=1 puis Ctrl+K."
            )
            if self._codex_compact_view:
                self._codex_log_action(hint)
            else:
                self._codex_log_ui(f"[yellow]{rich_escape(hint)}[/yellow]")
        return False

    def _extract_status_code(self, msg: str) -> int | None:
        """Extrait un code HTTP depuis un message d'erreur Codex."""
        # Exemples attendus: "unexpected status 401", "last status: 403".
        match = re.search(r"(?:unexpected status|last status[: ]+)\s*(\d{3})", msg, flags=re.IGNORECASE)
        if not match:
            match = re.search(r"\b(\d{3})\b", msg)
        if not match:
            return None
        try:
            return int(match.group(1))
        except Exception:
            return None

    def _codex_hint_for_status(self, status: int) -> str | None:
        """Retourne un message d'aide selon le code HTTP."""
        if status == 401:
            return "401 = authentification invalide -> Ctrl+K (login) ou `codex logout` + login ChatGPT."
        if status == 403:
            return "403 = acces interdit -> verifie login ChatGPT (pas API key) / droits / reseau."
        if status == 407:
            return "407 = proxy auth required -> configure HTTP_PROXY/HTTPS_PROXY."
        if status == 429:
            return "429 = rate limit -> reessaie plus tard / ralentis."
        if 500 <= status <= 599:
            return "5xx = erreur serveur -> reessaie, possible incident cote OpenAI."
        return None

    def _codex_extract_text(self, content: object) -> list[str]:
        """Extrait les textes utiles depuis un bloc 'content' Codex."""
        textes: list[str] = []
        if isinstance(content, list):
            for item in content:
                if isinstance(item, dict):
                    item_type = item.get("type")
                    if item_type in {"output_text", "output_markdown", "text", "input_text"}:
                        raw = item.get("text") or item.get("content")
                        if isinstance(raw, str) and raw:
                            textes.append(raw)
                elif isinstance(item, str):
                    textes.append(item)
        elif isinstance(content, str):
            textes.append(content)
        return textes

    def _codex_items_from_message_payload(self, payload: dict[str, object]) -> list[tuple[str, str]]:
        """Extrait les messages (user/assistant) d'un payload de type message."""
        items: list[tuple[str, str]] = []
        if payload.get("type") != "message":
            return items
        role = payload.get("role")
        if role not in {"assistant", "user"}:
            return items
        kind = "assistant" if role == "assistant" else "user"
        textes = self._codex_extract_text(payload.get("content"))
        if textes:
            for texte in textes:
                items.append((kind, texte))
            return items
        # Fallback si le contenu n'est pas structure.
        msg = payload.get("message")
        if isinstance(msg, str) and msg:
            items.append((kind, msg))
        return items

    def _codex_items_from_item_payload(self, item: dict[str, object]) -> list[tuple[str, str]]:
        """Extrait les messages (user/assistant) d'un payload de type item.*."""
        items: list[tuple[str, str]] = []
        item_type = item.get("type")

        def add(kind: str, msg: object) -> None:
            if isinstance(msg, str) and msg:
                items.append((kind, msg))

        if item_type == "message":
            # On reutilise le parseur "message" quand l'item est deja dans ce format.
            return self._codex_items_from_message_payload(item)

        if item_type in {"agent_message", "assistant_message"}:
            # Les messages assistant peuvent etre dans "text" ou "content".
            for texte in self._codex_extract_text(item.get("content")):
                add("assistant", texte)
            add("assistant", item.get("text") or item.get("message"))
            return items

        if item_type in {"user_message", "user"}:
            # Les messages user peuvent etre dans "text" ou "content".
            for texte in self._codex_extract_text(item.get("content")):
                add("user", texte)
            add("user", item.get("text") or item.get("message"))
            return items

        # Si le format est inconnu, on ne retourne rien ici.
        return items

    def _codex_iter_tool_calls(self, *containers: object) -> list[dict[str, object]]:
        """Collecte les tool calls depuis plusieurs conteneurs JSON."""
        appels: list[dict[str, object]] = []
        for container in containers:
            if not isinstance(container, dict):
                continue
            tool_call = container.get("tool_call")
            if isinstance(tool_call, dict):
                appels.append(tool_call)
            tool_calls = container.get("tool_calls") or container.get("tools")
            if isinstance(tool_calls, list):
                for call in tool_calls:
                    if isinstance(call, dict):
                        appels.append(call)
        return appels

    def _codex_format_action(self, payload: dict[str, object]) -> str | None:
        """Formate une action/tool call pour un affichage compact."""
        raw_type = str(payload.get("type") or "").lower()
        is_action = raw_type in {"tool_call", "function_call", "action", "tool"}
        if not is_action:
            # Heuristique: presence d'un nom d'outil + arguments.
            has_name = any(payload.get(k) for k in ("name", "tool", "tool_name"))
            has_args = any(k in payload for k in ("arguments", "args", "input", "parameters"))
            if not (has_name and has_args):
                return None

        name = payload.get("name") or payload.get("tool") or payload.get("tool_name") or payload.get("id")
        args = payload.get("arguments") or payload.get("args") or payload.get("input") or payload.get("parameters")

        if (not name and args is None) and isinstance(payload.get("tool_call"), dict):
            tool_call = payload["tool_call"]
            name = tool_call.get("name") or tool_call.get("tool") or tool_call.get("tool_name") or tool_call.get("id")
            args = tool_call.get("arguments") or tool_call.get("args") or tool_call.get("input") or tool_call.get(
                "parameters"
            )

        description = payload.get("message") or payload.get("description")
        if isinstance(description, str) and description.strip() and not (name or args is not None):
            return description.strip()

        arg_text: str | None = None
        if args is not None:
            if isinstance(args, (dict, list)):
                arg_text = json.dumps(args, ensure_ascii=False)
            else:
                arg_text = str(args)

        if name and arg_text:
            return f"{name}: {arg_text}"
        if name:
            return str(name)
        if arg_text:
            return arg_text
        return None

    def _codex_extract_display_items(self, obj: dict[str, object]) -> list[tuple[str, str]]:
        """Retourne les elements a afficher (user/assistant/action) en vue compacte."""
        items: list[tuple[str, str]] = []
        event_type = obj.get("type")
        payload = obj.get("payload")

        def add(kind: str, msg: object) -> None:
            if isinstance(msg, str) and msg:
                items.append((kind, msg))

        if event_type == "event_msg" and isinstance(payload, dict):
            payload_type = payload.get("type")
            msg = payload.get("message") or payload.get("text")
            if payload_type in {"agent_message", "assistant_message"}:
                add("assistant", msg)
            elif payload_type in {"user_message", "user"}:
                add("user", msg)
            else:
                action = self._codex_format_action(payload)
                if action:
                    add("action", action)

        if event_type == "response_item" and isinstance(payload, dict):
            items.extend(self._codex_items_from_message_payload(payload))
            action = self._codex_format_action(payload)
            if action:
                add("action", action)

        if event_type in {"response.output_text.done", "response.output_text"}:
            add("assistant", obj.get("text"))

        item = obj.get("item")
        if isinstance(item, dict):
            # Support des nouveaux events item.* (ex: item.completed).
            items.extend(self._codex_items_from_item_payload(item))
            action = self._codex_format_action(item)
            if action:
                add("action", action)

        for call in self._codex_iter_tool_calls(obj, payload, item):
            action = self._codex_format_action(call)
            if action:
                add("action", action)

        # De-dup simple pour eviter les doublons dans un meme event.
        uniques: list[tuple[str, str]] = []
        seen: set[tuple[str, str]] = set()
        for entry in items:
            if entry in seen:
                continue
            seen.add(entry)
            uniques.append(entry)
        return uniques

    def _codex_extract_messages(self, obj: dict[str, object]) -> list[str]:
        """Retourne les messages d'assistant a afficher (mode compact)."""
        return [msg for kind, msg in self._codex_extract_display_items(obj) if kind == "assistant"]

    def _codex_hard_wrap(self, line: str, width: int) -> list[str]:
        """Decoupe une ligne sans modifier les espaces (utile pour les blocs de code)."""
        if width <= 0:
            return [line]
        return [line[i : i + width] for i in range(0, len(line), width)] or [""]

    def _codex_wrap_text(self, text: str) -> list[str]:
        """Wrap le texte en respectant la largeur du panneau Codex."""
        try:
            codex_log = self.query_one("#codex_log", RichLog)
            width_attr = getattr(codex_log.size, "width", None)
            width_value = width_attr if isinstance(width_attr, int) else 80
        except Exception:
            # Fallback quand l'UI n'est pas disponible (tests unitaires).
            width_value = 80
        width = width_value - 4 if width_value else 80
        width = max(10, width)
        lignes: list[str] = []
        in_code = False
        for raw in text.splitlines():
            # On conserve les blocs de code Markdown, tout en evitant le depassement.
            if raw.strip().startswith("```"):
                in_code = not in_code
                lignes.append(raw)
                continue
            if in_code:
                if len(raw) <= width:
                    lignes.append(raw)
                else:
                    lignes.extend(self._codex_hard_wrap(raw, width))
                continue
            if not raw.strip():
                lignes.append("")
                continue
            if len(raw) <= width:
                lignes.append(raw)
                continue
            wrapped = textwrap.fill(
                raw,
                width=width,
                break_long_words=True,
                break_on_hyphens=True,
            )
            lignes.extend(wrapped.splitlines())
        return lignes

    def _codex_log_entry(self, msg: str, *, label: str, kind: str) -> None:
        """Affiche un bloc (Utilisateur/Assistant/Action) en evitant les doublons."""
        cleaned = msg.strip()
        if not cleaned:
            return
        fingerprint = f"{kind}:{cleaned}"
        if self._last_codex_message == fingerprint:
            return
        self._last_codex_message = fingerprint
        self._codex_log_ui(f"[b]{label}[/b]")
        for line in self._codex_wrap_text(msg):
            if line == "":
                self._codex_log_ui("")
            elif kind == "assistant":
                # Les reponses Codex doivent etre visibles en vert.
                self._codex_log_ui(f"[green]{rich_escape(line)}[/green]")
            else:
                self._codex_log_output(line)
        self._codex_log_ui("")

    def _codex_log_user_message(self, msg: str) -> None:
        """Affiche un message utilisateur en vue compacte."""
        self._codex_log_entry(msg, label="Utilisateur", kind="user")

    def _codex_log_action(self, msg: str) -> None:
        """Affiche une action effectuee par Codex."""
        self._codex_log_entry(msg, label="Action", kind="action")

    def _codex_log_message(self, msg: str) -> None:
        """Affiche un message assistant en mode compact."""
        self._codex_log_entry(msg, label="Assistant", kind="assistant")

    async def _run_codex(self, event: Input.Submitted) -> None:
        prompt = event.value.strip()
        event.input.value = ""
        if not prompt:
            return
        if self._codex_compact_view:
            # On affiche le message utilisateur pour garder un fil lisible.
            self._codex_log_user_message(prompt)

        env = self._codex_env()
        if not codex_cli_available(self.root_dir, env):
            ok = await self._install_codex(force=False, codex=True)
            if not ok:
                self._log_issue(
                    "[red]Codex indisponible.[/red] (Ctrl+I pour installer)",
                    niveau="erreur",
                    contexte="codex_exec",
                    codex=True,
                )
                return

        # Pre-check auth pour eviter des erreurs "unexpected status".
        if not await self._codex_logged_in(env):
            return

        argv = codex_exec_argv(prompt, root_dir=self.root_dir, env=env, json_output=True)
        if not self._codex_compact_view:
            self._codex_log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        # Robustesse: on capture les erreurs de lancement pour eviter un crash UI.
        try:
            assistant_buffer: list[str] = []
            async for ev in stream_subprocess(argv, cwd=self.root_dir, env=env):
                if ev["kind"] != "line":
                    if ev["returncode"] not in (None, 0):
                        self._log_issue(
                            f"[red]Codex termine en erreur (rc={ev['returncode']}).[/red]",
                            niveau="erreur",
                            contexte="codex_exec",
                            codex=True,
                        )
                    self._codex_log_ui(f"[dim]{ev['text']}[/dim]")
                    continue

                line = ev["text"].strip()
                if not line:
                    continue

                # Sortie JSONL => on essaye de parser pour enrichir un peu l'affichage,
                # sinon on affiche la ligne brute.
                try:
                    obj = json.loads(line)
                except Exception:
                    if self._codex_compact_view:
                        self._codex_log_action(line)
                    else:
                        self._codex_log_output(line)
                    continue

                event_type = obj.get("type") if isinstance(obj, dict) else None
                if self._codex_compact_view and isinstance(obj, dict):
                    # Gestion du streaming texte (delta) en mode compact.
                    if event_type in {"response.output_text.delta", "response.output_text"}:
                        delta = obj.get("delta") or obj.get("text")
                        if isinstance(delta, str) and delta:
                            assistant_buffer.append(delta)
                        continue
                    if event_type in {"response.output_text.done", "response.output_item.done", "response.completed"}:
                        if assistant_buffer:
                            self._codex_log_message("".join(assistant_buffer))
                            assistant_buffer.clear()

                # Affiche les erreurs de maniere lisible avec diagnostic.
                if event_type == "error" and isinstance(obj, dict):
                    msg = str(obj.get("message", ""))
                    status = self._extract_status_code(msg) if msg else None
                    if self._codex_compact_view:
                        if status:
                            self._codex_log_action(f"Erreur Codex HTTP {status}: {msg}")
                            hint = self._codex_hint_for_status(status)
                            if hint:
                                self._codex_log_action(hint)
                        else:
                            self._codex_log_action(f"Erreur Codex: {msg}")
                    else:
                        if status:
                            self._codex_log_ui(f"[red]Erreur Codex HTTP {status}[/red] {rich_escape(msg)}")
                            hint = self._codex_hint_for_status(status)
                            if hint:
                                self._codex_log_ui(f"[yellow]{rich_escape(hint)}[/yellow]")
                        else:
                            self._codex_log_ui(f"[red]Erreur Codex[/red] {rich_escape(msg)}")
                    continue

                if event_type == "turn.failed" and isinstance(obj, dict):
                    err = obj.get("error")
                    msg = ""
                    if isinstance(err, dict):
                        msg = str(err.get("message", "")) or str(err)
                    else:
                        msg = str(err)
                    status = self._extract_status_code(msg) if msg else None
                    if self._codex_compact_view:
                        if status:
                            self._codex_log_action(f"Task echouee HTTP {status}: {msg}")
                            hint = self._codex_hint_for_status(status)
                            if hint:
                                self._codex_log_action(hint)
                        else:
                            self._codex_log_action(f"Task echouee: {msg}")
                    else:
                        if status:
                            self._codex_log_ui(f"[red]Task echouee HTTP {status}[/red] {rich_escape(msg)}")
                            hint = self._codex_hint_for_status(status)
                            if hint:
                                self._codex_log_ui(f"[yellow]{rich_escape(hint)}[/yellow]")
                        else:
                            self._codex_log_ui(f"[red]Task echouee[/red] {rich_escape(msg)}")
                    continue

                # Mode compact: on affiche uniquement les messages assistant.
                if self._codex_compact_view and isinstance(obj, dict):
                    items = self._codex_extract_display_items(obj)
                    if items:
                        for kind, message in items:
                            if kind == "assistant":
                                self._codex_log_message(message)
                            elif kind == "user":
                                self._codex_log_user_message(message)
                            elif kind == "action":
                                self._codex_log_action(message)
                    # En mode compact on ignore le reste pour reduire le bruit.
                    continue

                # Mode brut: log enrichi pour debug.
                if isinstance(obj, dict) and isinstance(obj.get("type"), str):
                    self._codex_log_output(f"[{obj.get('type')}] {json.dumps(obj, ensure_ascii=False)}")
                else:
                    self._codex_log_output(json.dumps(obj, ensure_ascii=False))
            if self._codex_compact_view and assistant_buffer:
                # Flush final si la stream delta n'a pas emis d'event de fin.
                self._codex_log_message("".join(assistant_buffer))
        except FileNotFoundError as exc:
            # Cas typique: codex ou node introuvable dans le PATH.
            self._log_issue(
                f"[red]Codex introuvable.[/red] {exc}",
                niveau="erreur",
                contexte="codex_exec",
                exc=exc,
                codex=True,
            )
        except Exception as exc:
            # Capture generique pour ne pas fermer l'application.
            self._log_issue(
                f"[red]Erreur execution Codex:[/red] {exc}",
                niveau="erreur",
                contexte="codex_exec",
                exc=exc,
                codex=True,
            )

    # ---------- actions ----------
    def action_clear_log(self) -> None:
        self.query_one("#log", RichLog).clear()
        self.query_one("#codex_log", RichLog).clear()
        # Reinitialise le cache pour afficher la prochaine reponse.
        self._last_codex_message = None
        self._log_ui("[dim]journaux effaces[/dim]")

    def action_toggle_codex_view(self) -> None:
        """Bascule entre vue compacte et vue brute."""
        self._codex_compact_view = not self._codex_compact_view
        # Reset du cache pour eviter de masquer un nouveau message.
        self._last_codex_message = None
        self._update_codex_title()
        self._codex_log_ui(f"[dim]Mode Codex: {self._codex_mode_label()}[/dim]")

    def action_reload_tree(self) -> None:
        self.query_one(DirectoryTree).reload()
        self._log_ui("[dim]arborescence rechargee[/dim]")

    def action_save(self) -> bool:
        if not self.current:
            self._log_issue(
                "[yellow]Aucun fichier ouvert.[/yellow]",
                niveau="avertissement",
                contexte="sauvegarde",
            )
            return False

        editor = self.query_one(TextArea)
        content = editor.text
        path = self.current.path
        encoding = self.current.encoding

        try:
            path.write_text(content, encoding=encoding)
            self.current.dirty = False
            self._log_ui(f"[green]Sauvegarde[/green] {path}")
            return True
        except UnicodeEncodeError:
            try:
                path.write_text(content, encoding="utf-8")
            except OSError as exc:
                self._log_issue(
                    f"[red]Erreur sauvegarde (UTF-8):[/red] {path} ({exc})",
                    niveau="erreur",
                    contexte="sauvegarde",
                    exc=exc,
                )
                return False
            self.current.encoding = "utf-8"
            self.current.dirty = False
            self._log_issue(
                f"[yellow]Sauvegarde en UTF-8 (fallback)[/yellow] {path}",
                niveau="avertissement",
                contexte="sauvegarde",
            )
            return True
        except OSError as exc:
            self._log_issue(
                f"[red]Erreur sauvegarde:[/red] {path} ({exc})",
                niveau="erreur",
                contexte="sauvegarde",
                exc=exc,
            )
            return False
        finally:
            self._refresh_title()

    async def action_run(self) -> None:
        if not self.current or self.current.path.suffix.lower() != ".py":
            self._log_issue(
                "[yellow]Ouvre un fichier .py.[/yellow]",
                niveau="avertissement",
                contexte="execution_python",
            )
            return
        if self.current.dirty:
            self.action_save()

        argv = python_run_argv(self.current.path)
        env = self._portable_env(os.environ.copy())
        self._log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._log_output,
            ui_log=self._log_ui,
            contexte="execution python",
        )

    def _codex_device_auth_enabled(self) -> bool:
        return os.environ.get("USBIDE_CODEX_DEVICE_AUTH", "0").strip().lower() in {"1", "true", "yes", "on"}

    def _codex_auto_install_enabled(self) -> bool:
        return os.environ.get("USBIDE_CODEX_AUTO_INSTALL", "1").strip().lower() not in {"0", "false", "no", "off"}

    async def _install_codex(self, *, force: bool = False, codex: bool = False) -> bool:
        env = self._codex_env()
        if not force and codex_cli_available(self.root_dir, env):
            return True
        if not force and self._codex_install_attempted:
            return False
        if not force and not self._codex_auto_install_enabled():
            self._log_issue(
                "[yellow]Auto-install Codex desactive.[/yellow]",
                niveau="avertissement",
                contexte="installation_codex",
                codex=codex,
            )
            return False

        self._codex_install_attempted = True
        package = os.environ.get("USBIDE_CODEX_NPM_PACKAGE", "@openai/codex")
        prefix = codex_install_prefix(self.root_dir)
        bin_dir = codex_bin_dir(prefix)
        prefix.mkdir(parents=True, exist_ok=True)

        log_ui = self._codex_log_ui if codex else self._log_ui
        log_output = self._codex_log_output if codex else self._log_output

        # Affichage dans le panneau Codex si on est en contexte Codex.
        log_ui(f"[b]Installation Codex[/b] package={rich_escape(package)} prefix={rich_escape(str(prefix))}")

        try:
            argv = codex_install_argv(self.root_dir, prefix, package)
        except Exception as e:
            self._log_issue(
                f"[red]Impossible d'installer Codex:[/red] {e}",
                niveau="erreur",
                contexte="installation_codex",
                exc=e,
                codex=codex,
            )
            return False

        log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")
        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=log_output,
            ui_log=log_ui,
            contexte="installation Codex",
            codex=codex,
        )

        ok = codex_cli_available(self.root_dir, env)
        if ok:
            log_ui(f"[green]Codex installe.[/green] (.bin: {rich_escape(str(bin_dir))})")
        return ok

    async def action_codex_install(self) -> None:
        await self._install_codex(force=True, codex=True)

    async def action_codex_login(self) -> None:
        env = self._codex_env()
        if not codex_cli_available(self.root_dir, env):
            ok = await self._install_codex(force=False, codex=True)
            if not ok:
                self._log_issue(
                    "[red]Codex introuvable.[/red]",
                    niveau="erreur",
                    contexte="codex_login",
                    codex=True,
                )
                return

        # Log dans la sortie Codex pour plus de lisibilite.
        self._codex_log_ui("[b]Login Codex[/b] : navigateur/Device auth selon config.")
        if not self._codex_device_auth_enabled():
            # Info utile si le navigateur ne s'ouvre pas automatiquement.
            self._codex_log_ui(
                "[dim]Astuce: si le navigateur ne s'ouvre pas, "
                "definis USBIDE_CODEX_DEVICE_AUTH=1 puis relance Ctrl+K.[/dim]"
            )
        argv = codex_login_argv(self.root_dir, env, device_auth=self._codex_device_auth_enabled())
        self._codex_log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._codex_log_output,
            ui_log=self._codex_log_ui,
            contexte="login Codex",
            codex=True,
        )

    async def action_codex_check(self) -> None:
        env = self._codex_env()
        if not codex_cli_available(self.root_dir, env):
            self._log_issue(
                "[yellow]Codex non installe.[/yellow]",
                niveau="avertissement",
                contexte="codex_status",
                codex=True,
            )
            return
        # Diagnostic lisible pour comprendre rapidement la resolution Codex.
        node_path = node_executable(self.root_dir, env=env)
        entry_path = codex_entrypoint_js(codex_install_prefix(self.root_dir))
        resolved = shutil.which("codex", path=env.get("PATH"))
        self._codex_log_ui(f"[dim]node: {node_path or 'absent'}[/dim]")
        self._codex_log_ui(f"[dim]entrypoint: {entry_path or 'absent'}[/dim]")
        self._codex_log_ui(f"[dim]codex (PATH): {resolved or 'absent'}[/dim]")
        if os.name == "nt" and resolved:
            suffix = Path(resolved).suffix.lower()
            if suffix in {".cmd", ".bat"}:
                self._codex_log_ui("[dim]shim .cmd detecte: lancement via cmd.exe[/dim]")
            elif suffix == ".ps1":
                self._codex_log_ui("[dim]shim .ps1 detecte: lancement via PowerShell[/dim]")
        argv = codex_status_argv(self.root_dir, env)
        self._codex_log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._codex_log_output,
            ui_log=self._codex_log_ui,
            contexte="verification Codex",
            codex=True,
        )

    async def action_dev_tools(self) -> None:
        raw = os.environ.get("USBIDE_DEV_TOOLS", "ruff black mypy pytest")
        tools = parse_tool_list(raw)
        if not tools:
            self._log_issue(
                "[yellow]Liste outils vide.[/yellow]",
                niveau="avertissement",
                contexte="outils_dev",
            )
            return

        env = self._tools_env()
        prefix = tools_install_prefix(self.root_dir)
        prefix.mkdir(parents=True, exist_ok=True)

        wheelhouse = self._wheelhouse_path()
        argv = pip_install_argv(prefix, tools, find_links=wheelhouse, no_index=wheelhouse is not None)
        self._log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._log_output,
            ui_log=self._log_ui,
            contexte="installation outils dev",
        )

    async def _install_pyinstaller(self, *, force: bool = False) -> bool:
        env = self._tools_env()
        if not force and pyinstaller_available(self.root_dir, env):
            return True
        if not force and self._pyinstaller_install_attempted:
            return False

        self._pyinstaller_install_attempted = True
        prefix = tools_install_prefix(self.root_dir)
        bin_dir = python_scripts_dir(prefix)
        prefix.mkdir(parents=True, exist_ok=True)

        wheelhouse = self._wheelhouse_path()
        argv = pyinstaller_install_argv(prefix, find_links=wheelhouse, no_index=wheelhouse is not None)
        self._log_ui(f"[b]Installation PyInstaller[/b] bin={rich_escape(str(bin_dir))}")
        self._log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._log_output,
            ui_log=self._log_ui,
            contexte="installation PyInstaller",
        )

        return pyinstaller_available(self.root_dir, env)

    async def action_build_exe(self) -> None:
        if not self.current or self.current.path.suffix.lower() != ".py":
            self._log_issue(
                "[yellow]Ouvre un fichier .py.[/yellow]",
                niveau="avertissement",
                contexte="build_exe",
            )
            return
        if self.current.dirty:
            self.action_save()

        env = self._tools_env()
        if not pyinstaller_available(self.root_dir, env):
            ok = await self._install_pyinstaller(force=False)
            if not ok:
                self._log_issue(
                    "[red]PyInstaller indisponible.[/red]",
                    niveau="erreur",
                    contexte="build_exe",
                )
                return

        dist_dir = self.root_dir / "dist"
        dist_dir.mkdir(parents=True, exist_ok=True)
        argv = pyinstaller_build_argv(self.current.path, dist_dir, onefile=False, work_dir=self.root_dir / "tmp")
        self._log_ui(f"\n[b]$[/b] {rich_escape(' '.join(argv))}")

        await self._stream_and_log(
            argv,
            cwd=self.root_dir,
            env=env,
            output_log=self._log_output,
            ui_log=self._log_ui,
            contexte="construction exe",
        )

