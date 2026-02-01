import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

from usbide.app import OpenFile, USBIDEApp


class TestUSBIDEAppTitle(unittest.TestCase):
    def test_refresh_title_sans_fichier(self) -> None:
        # Le titre doit afficher le nom officiel et le chemin racine.
        # Utilise un dossier temporaire pour rester compatible Windows/Linux.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            app = USBIDEApp(root_dir=root_dir)
            app.current = None

            app._refresh_title()

            self.assertEqual(app.title, "ValDev Pro v1")
            self.assertEqual(app.sub_title, str(root_dir))

    def test_refresh_title_avec_fichier_dirty(self) -> None:
        # Le titre doit garder le branding et signaler les modifications.
        # Utilise un dossier temporaire pour rester compatible Windows/Linux.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            app = USBIDEApp(root_dir=root_dir)
            app.current = OpenFile(path=root_dir / "main.py", encoding="utf-8", dirty=True)

            app._refresh_title()

            self.assertEqual(app.title, "ValDev Pro v1 *")
            self.assertIn("main.py", app.sub_title)
            self.assertIn("utf-8", app.sub_title)


class TestUSBIDEAppSave(unittest.TestCase):
    def test_action_save_ok(self) -> None:
        # Une sauvegarde reussie doit retourner True et ecrire le contenu.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            path = root_dir / "note.txt"
            app = USBIDEApp(root_dir=root_dir)
            app.current = OpenFile(path=path, encoding="utf-8", dirty=True)
            fake_editor = MagicMock()
            fake_editor.text = "Bonjour"

            with patch.object(app, "query_one", return_value=fake_editor):
                self.assertTrue(app.action_save())

            self.assertEqual(path.read_text(encoding="utf-8"), "Bonjour")

    def test_action_save_fallback_utf8(self) -> None:
        # Un encodage incompatible doit declencher un fallback UTF-8.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            path = root_dir / "accent.txt"
            app = USBIDEApp(root_dir=root_dir)
            app.current = OpenFile(path=path, encoding="ascii", dirty=True)
            fake_editor = MagicMock()
            accent = "\u00e9"
            fake_editor.text = accent

            with patch.object(app, "query_one", return_value=fake_editor):
                self.assertTrue(app.action_save())

            self.assertEqual(path.read_text(encoding="utf-8"), accent)

    def test_action_save_oserror(self) -> None:
        # Une erreur d'ecriture doit retourner False.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            path = root_dir / "note.txt"
            app = USBIDEApp(root_dir=root_dir)
            app.current = OpenFile(path=path, encoding="utf-8", dirty=True)
            fake_editor = MagicMock()
            fake_editor.text = "Bonjour"

            with patch.object(app, "query_one", return_value=fake_editor):
                with patch.object(Path, "write_text", side_effect=OSError("boom")):
                    self.assertFalse(app.action_save())


class TestUSBIDEAppPortableEnv(unittest.TestCase):
    def test_portable_env_defauts(self) -> None:
        # Les variables doivent pointer vers des dossiers sur la cle.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            app = USBIDEApp(root_dir=root_dir)
            env = app._portable_env({"PATH": "X"})
            self.assertEqual(env["PIP_CACHE_DIR"], str(root_dir / "cache" / "pip"))
            self.assertEqual(env["PYTHONPYCACHEPREFIX"], str(root_dir / "cache" / "pycache"))
            self.assertEqual(env["TEMP"], str(root_dir / "tmp"))
            self.assertEqual(env["TMP"], str(root_dir / "tmp"))
            self.assertEqual(env["PYTHONNOUSERSITE"], "1")
            self.assertEqual(env["CODEX_HOME"], str(root_dir / "codex_home"))
            self.assertEqual(env["NPM_CONFIG_CACHE"], str(root_dir / "cache" / "npm"))
            self.assertEqual(env["NPM_CONFIG_UPDATE_NOTIFIER"], "false")

    def test_portable_env_ne_ecrase_pas(self) -> None:
        # Les variables doivent etre forcees sur la cle meme si deja definies.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            app = USBIDEApp(root_dir=root_dir)
            env = {
                "PIP_CACHE_DIR": "X",
                "PYTHONPYCACHEPREFIX": "Y",
                "TEMP": "Z",
                "TMP": "W",
                "PYTHONNOUSERSITE": "0",
                "CODEX_HOME": "C",
                "NPM_CONFIG_CACHE": "N",
                "NPM_CONFIG_UPDATE_NOTIFIER": "true",
            }
            result = app._portable_env(env)
            self.assertEqual(result["PIP_CACHE_DIR"], str(root_dir / "cache" / "pip"))
            self.assertEqual(result["PYTHONPYCACHEPREFIX"], str(root_dir / "cache" / "pycache"))
            self.assertEqual(result["TEMP"], str(root_dir / "tmp"))
            self.assertEqual(result["TMP"], str(root_dir / "tmp"))
            self.assertEqual(result["PYTHONNOUSERSITE"], "1")
            self.assertEqual(result["CODEX_HOME"], str(root_dir / "codex_home"))
            self.assertEqual(result["NPM_CONFIG_CACHE"], str(root_dir / "cache" / "npm"))
            self.assertEqual(result["NPM_CONFIG_UPDATE_NOTIFIER"], "false")

    def test_wheelhouse_path(self) -> None:
        # Le wheelhouse doit etre detecte quand le dossier existe.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            wheelhouse = root_dir / "tools" / "wheels"
            wheelhouse.mkdir(parents=True, exist_ok=True)
            app = USBIDEApp(root_dir=root_dir)
            self.assertEqual(app._wheelhouse_path(), wheelhouse)


class TestUSBIDEAppCodexFlags(unittest.TestCase):
    def test_codex_device_auth_enabled(self) -> None:
        # La variable d'environnement doit activer le device auth.
        app = USBIDEApp(root_dir=Path.cwd())
        with patch.dict(os.environ, {"USBIDE_CODEX_DEVICE_AUTH": "1"}, clear=True):
            self.assertTrue(app._codex_device_auth_enabled())
        with patch.dict(os.environ, {"USBIDE_CODEX_DEVICE_AUTH": "0"}, clear=True):
            self.assertFalse(app._codex_device_auth_enabled())

    def test_codex_auto_install_enabled(self) -> None:
        # La variable d'environnement doit activer ou desactiver l'auto-install.
        app = USBIDEApp(root_dir=Path.cwd())
        with patch.dict(os.environ, {"USBIDE_CODEX_AUTO_INSTALL": "0"}, clear=True):
            self.assertFalse(app._codex_auto_install_enabled())
        with patch.dict(os.environ, {"USBIDE_CODEX_AUTO_INSTALL": "1"}, clear=True):
            self.assertTrue(app._codex_auto_install_enabled())


class TestUSBIDEAppCodexEnvSanitize(unittest.TestCase):
    def test_sanitize_codex_env_supprime_api_key_et_base(self) -> None:
        # Sans override, les variables sensibles doivent etre retirees.
        app = USBIDEApp(root_dir=Path.cwd())
        env = {
            "OPENAI_API_KEY": "sk-test",
            "CODEX_API_KEY": "sk-codex",
            "OPENAI_BASE_URL": "https://example.com",
            "OPENAI_API_BASE": "https://example.org",
            "OPENAI_API_HOST": "example.net",
            "OTHER": "ok",
        }
        with patch.dict(os.environ, {}, clear=True):
            result = app._sanitize_codex_env(env)

        self.assertNotIn("OPENAI_API_KEY", result)
        self.assertNotIn("CODEX_API_KEY", result)
        self.assertNotIn("OPENAI_BASE_URL", result)
        self.assertNotIn("OPENAI_API_BASE", result)
        self.assertNotIn("OPENAI_API_HOST", result)
        self.assertEqual(result["OTHER"], "ok")

    def test_sanitize_codex_env_respecte_overrides(self) -> None:
        # Les flags USBIDE_CODEX_ALLOW_* doivent conserver les variables.
        app = USBIDEApp(root_dir=Path.cwd())
        env = {
            "OPENAI_API_KEY": "sk-test",
            "CODEX_API_KEY": "sk-codex",
            "OPENAI_BASE_URL": "https://example.com",
        }
        with patch.dict(
            os.environ,
            {"USBIDE_CODEX_ALLOW_API_KEY": "1", "USBIDE_CODEX_ALLOW_CUSTOM_BASE": "true"},
            clear=True,
        ):
            result = app._sanitize_codex_env(env)

        self.assertEqual(result["OPENAI_API_KEY"], "sk-test")
        self.assertEqual(result["CODEX_API_KEY"], "sk-codex")
        self.assertEqual(result["OPENAI_BASE_URL"], "https://example.com")


class TestUSBIDEAppCodexDiagnostics(unittest.TestCase):
    def test_extract_status_code(self) -> None:
        # Les codes HTTP doivent etre detectes dans les messages courants.
        app = USBIDEApp(root_dir=Path.cwd())
        self.assertEqual(app._extract_status_code("unexpected status 401 Unauthorized"), 401)
        self.assertEqual(app._extract_status_code("last status: 403 Forbidden"), 403)
        self.assertEqual(app._extract_status_code("HTTP 429"), 429)
        self.assertIsNone(app._extract_status_code("aucun code ici"))

    def test_codex_hint_for_status(self) -> None:
        # Les hints doivent etre retournes pour les codes connus.
        app = USBIDEApp(root_dir=Path.cwd())
        self.assertIn("401", app._codex_hint_for_status(401) or "")
        self.assertIn("403", app._codex_hint_for_status(403) or "")
        self.assertIn("proxy", app._codex_hint_for_status(407) or "")


class TestUSBIDEAppCodexCompactView(unittest.TestCase):
    def test_codex_extract_messages_response_item(self) -> None:
        # Le mode compact doit extraire uniquement les messages assistant.
        app = USBIDEApp(root_dir=Path.cwd())
        obj = {
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "assistant",
                "content": [{"type": "output_text", "text": "Bonjour"}],
            },
        }
        self.assertEqual(app._codex_extract_messages(obj), ["Bonjour"])

    def test_codex_extract_messages_event_msg(self) -> None:
        # Le mode compact doit lire les event_msg agent_message.
        app = USBIDEApp(root_dir=Path.cwd())
        obj = {"type": "event_msg", "payload": {"type": "agent_message", "message": "Salut"}}
        self.assertEqual(app._codex_extract_messages(obj), ["Salut"])

    def test_codex_extract_display_items_user(self) -> None:
        # La vue compacte doit recuperer le message utilisateur.
        app = USBIDEApp(root_dir=Path.cwd())
        obj = {
            "type": "response_item",
            "payload": {
                "type": "message",
                "role": "user",
                "content": [{"type": "input_text", "text": "Bonjour"}],
            },
        }
        self.assertIn(("user", "Bonjour"), app._codex_extract_display_items(obj))

    def test_codex_extract_display_items_action(self) -> None:
        # La vue compacte doit extraire les actions (tool call).
        app = USBIDEApp(root_dir=Path.cwd())
        obj = {
            "type": "response_item",
            "payload": {"type": "tool_call", "name": "list_files", "arguments": {"path": "."}},
        }
        items = app._codex_extract_display_items(obj)
        self.assertTrue(any(kind == "action" and "list_files" in msg for kind, msg in items))

    def test_codex_extract_display_items_item_completed(self) -> None:
        # La vue compacte doit lire les items completes (agent_message).
        app = USBIDEApp(root_dir=Path.cwd())
        obj = {"type": "item.completed", "item": {"type": "agent_message", "text": "Salut"}}
        self.assertIn(("assistant", "Salut"), app._codex_extract_display_items(obj))

    def test_codex_extract_text_filtre_types(self) -> None:
        # Les types inconnus ne doivent pas polluer la sortie.
        app = USBIDEApp(root_dir=Path.cwd())
        content = [{"type": "output_text", "text": "OK"}, {"type": "image", "text": "NO"}]
        self.assertEqual(app._codex_extract_text(content), ["OK"])

    def test_toggle_codex_view(self) -> None:
        # Le toggle doit inverser le mode et rafraichir le titre.
        app = USBIDEApp(root_dir=Path.cwd())
        with (
            patch.object(app, "_update_codex_title") as update_mock,
            patch.object(app, "_codex_log_ui") as log_mock,
        ):
            app._codex_compact_view = True
            app.action_toggle_codex_view()
        self.assertFalse(app._codex_compact_view)
        update_mock.assert_called()
        log_mock.assert_called()


class TestUSBIDEAppCodexAffichage(unittest.TestCase):
    def test_codex_wrap_text_wrappe(self) -> None:
        # Le wrap doit couper les longues lignes avec des espaces.
        app = USBIDEApp(root_dir=Path.cwd())

        # Faux objet RichLog pour simuler la largeur disponible.
        class DummySize:
            def __init__(self, width: int) -> None:
                self.width = width

        # Faux RichLog minimal pour passer au helper.
        class DummyLog:
            def __init__(self, width: int) -> None:
                self.size = DummySize(width)
                self.border_title = ""

        dummy_width = 24
        dummy_log = DummyLog(width=dummy_width)
        with patch.object(app, "query_one", return_value=dummy_log):
            lignes = app._codex_wrap_text("Texte tres long avec des espaces pour verifier le wrap")

        # La largeur effective suit le minimum defini dans l'helper.
        largeur_effective = max(10, dummy_width - 4)
        self.assertTrue(all(len(line) <= largeur_effective for line in lignes if line))

    def test_codex_wrap_text_coupe_mot_long(self) -> None:
        # Les mots trop longs doivent etre coupes pour eviter le debordement.
        app = USBIDEApp(root_dir=Path.cwd())

        class DummySize:
            def __init__(self, width: int) -> None:
                self.width = width

        class DummyLog:
            def __init__(self, width: int) -> None:
                self.size = DummySize(width)
                self.border_title = ""

        dummy_log = DummyLog(width=18)
        with patch.object(app, "query_one", return_value=dummy_log):
            lignes = app._codex_wrap_text("AAAAAAAAAAAAAAAAAAAA")

        self.assertTrue(all(len(line) <= max(10, dummy_log.size.width - 4) for line in lignes if line))

    def test_codex_wrap_text_preserve_bloc_code(self) -> None:
        # Les lignes dans un bloc de code Markdown ne doivent pas etre wrappees.
        app = USBIDEApp(root_dir=Path.cwd())

        # Faux objet RichLog pour simuler la largeur disponible.
        class DummySize:
            def __init__(self, width: int) -> None:
                self.width = width

        # Faux RichLog minimal pour passer au helper.
        class DummyLog:
            def __init__(self, width: int) -> None:
                self.size = DummySize(width)
                self.border_title = ""

        dummy_log = DummyLog(width=20)
        texte = "```python\nprint('x' * 50)\n```\nFin"
        with patch.object(app, "query_one", return_value=dummy_log):
            lignes = app._codex_wrap_text(texte)

        self.assertIn("print('x' * 50)", lignes)

    def test_codex_log_message_evite_doublon(self) -> None:
        # Le cache doit empecher l'affichage d'un message identique.
        app = USBIDEApp(root_dir=Path.cwd())
        with (
            patch.object(app, "_codex_wrap_text", return_value=["ligne"]),
            patch.object(app, "_codex_log_ui") as log_ui,
            patch.object(app, "_codex_log_output") as log_output,
        ):
            app._codex_log_message("Bonjour")
            appel_1 = log_ui.call_count + log_output.call_count
            app._codex_log_message("Bonjour")

        self.assertEqual(log_ui.call_count + log_output.call_count, appel_1)

    def test_codex_log_message_utilise_vert(self) -> None:
        # Les reponses assistant doivent etre colorees en vert.
        app = USBIDEApp(root_dir=Path.cwd())
        with (
            patch.object(app, "_codex_wrap_text", return_value=["ligne"]),
            patch.object(app, "_codex_log_ui") as log_ui,
            patch.object(app, "_codex_log_output") as log_output,
        ):
            app._codex_log_message("Bonjour")

        messages_ui = [call.args[0] for call in log_ui.call_args_list if call.args]
        self.assertTrue(any("[green]" in msg for msg in messages_ui))
        self.assertFalse(log_output.called)

    def test_update_codex_title_reflete_mode(self) -> None:
        # Le titre du panneau Codex doit rester stable.
        app = USBIDEApp(root_dir=Path.cwd())

        # Faux RichLog minimal pour capter le titre.
        class DummyLog:
            def __init__(self) -> None:
                self.border_title = ""

        dummy_log = DummyLog()
        with patch.object(app, "query_one", return_value=dummy_log):
            app._codex_compact_view = True
            app._update_codex_title()
            app._codex_compact_view = False
            app._update_codex_title()
            self.assertEqual(dummy_log.border_title, "Sortie Codex")

    def test_action_clear_log_reset_cache(self) -> None:
        # Le clear doit reinitialiser le cache des messages Codex.
        app = USBIDEApp(root_dir=Path.cwd())
        app._last_codex_message = "hello"
        fake_log = MagicMock()
        with patch.object(app, "query_one", return_value=fake_log):
            app.action_clear_log()

        self.assertIsNone(app._last_codex_message)


class TestUSBIDEAppCodexRun(unittest.IsolatedAsyncioTestCase):
    async def test_run_codex_capture_erreur_lancement(self) -> None:
        # Verifie qu'une erreur de lancement Codex ne fait pas crasher l'UI.
        app = USBIDEApp(root_dir=Path.cwd())

        class DummyInput:
            def __init__(self) -> None:
                self.value = ""

        class DummyEvent:
            def __init__(self, value: str) -> None:
                self.value = value
                self.input = DummyInput()

        async def fake_stream(*_args, **_kwargs):
            # On force une erreur de lancement pour tester la robustesse.
            if False:
                yield  # pragma: no cover
            raise FileNotFoundError("codex missing")

        dummy_log = MagicMock()
        with (
            patch("usbide.app.codex_cli_available", return_value=True),
            patch("usbide.app.codex_exec_argv", return_value=["codex", "exec", "hello"]),
            patch("usbide.app.stream_subprocess", fake_stream),
            patch.object(app, "_codex_logged_in", AsyncMock(return_value=True)),
            patch.object(app, "query_one", return_value=dummy_log),
        ):
            await app._run_codex(DummyEvent("hello"))

        # Le message d'erreur doit etre ecrit dans le panneau Codex.
        messages = [call.args[0] for call in dummy_log.write.call_args_list if call.args]
        self.assertTrue(any("Codex introuvable" in msg for msg in messages))

    async def test_run_codex_stop_si_non_logge(self) -> None:
        # Si le login Codex est absent, on ne lance pas l'exec.
        app = USBIDEApp(root_dir=Path.cwd())

        class DummyInput:
            def __init__(self) -> None:
                self.value = ""

        class DummyEvent:
            def __init__(self, value: str) -> None:
                self.value = value
                self.input = DummyInput()

        with (
            patch("usbide.app.codex_cli_available", return_value=True),
            patch.object(app, "_codex_logged_in", AsyncMock(return_value=False)),
            patch("usbide.app.codex_exec_argv") as exec_mock,
        ):
            await app._run_codex(DummyEvent("hello"))

        exec_mock.assert_not_called()


class TestUSBIDEAppCodexInstall(unittest.IsolatedAsyncioTestCase):
    async def test_install_codex_logue_dans_panneau_codex(self) -> None:
        # Si l'auto-install est desactive, l'erreur doit viser le panneau Codex.
        app = USBIDEApp(root_dir=Path.cwd())
        with (
            patch.dict(os.environ, {"USBIDE_CODEX_AUTO_INSTALL": "0"}, clear=True),
            patch("usbide.app.codex_cli_available", return_value=False),
            patch.object(app, "_log_issue") as log_issue,
        ):
            ok = await app._install_codex(force=False, codex=True)

        self.assertFalse(ok)
        self.assertTrue(log_issue.call_args.kwargs.get("codex"))


class TestUSBIDEAppCodexLoginStatus(unittest.IsolatedAsyncioTestCase):
    async def test_codex_logged_in_ok(self) -> None:
        # Un status OK (rc=0) doit retourner True.
        app = USBIDEApp(root_dir=Path.cwd())

        async def fake_stream(*_args, **_kwargs):
            yield {"kind": "line", "text": "ok", "returncode": None}
            yield {"kind": "exit", "text": "exit 0", "returncode": 0}

        with (
            patch("usbide.app.codex_status_argv", return_value=["codex", "login", "status"]),
            patch("usbide.app.stream_subprocess", fake_stream),
        ):
            self.assertTrue(await app._codex_logged_in(env={}))

    async def test_codex_logged_in_ko(self) -> None:
        # Un status KO (rc!=0) doit retourner False et journaliser l'info.
        app = USBIDEApp(root_dir=Path.cwd())

        async def fake_stream(*_args, **_kwargs):
            yield {"kind": "line", "text": "not logged", "returncode": None}
            yield {"kind": "exit", "text": "exit 1", "returncode": 1}

        fake_ui = MagicMock()
        fake_output = MagicMock()

        with (
            patch("usbide.app.codex_status_argv", return_value=["codex", "login", "status"]),
            patch("usbide.app.stream_subprocess", fake_stream),
            patch.object(app, "_codex_log_ui", fake_ui),
            patch.object(app, "_codex_log_output", fake_output),
        ):
            self.assertFalse(await app._codex_logged_in(env={}))

        messages_ui = [call.args[0] for call in fake_ui.call_args_list if call.args]
        messages_out = [call.args[0] for call in fake_output.call_args_list if call.args]
        messages = messages_ui + messages_out
        self.assertTrue(any("Codex n'est pas authentifie" in msg for msg in messages))

    async def test_codex_logged_in_capture_erreur_lancement(self) -> None:
        # Une erreur de lancement doit etre loguee et ne pas faire crasher l'UI.
        app = USBIDEApp(root_dir=Path.cwd())

        async def fake_stream(*_args, **_kwargs):
            if False:
                yield  # pragma: no cover
            raise FileNotFoundError("codex missing")

        with (
            patch("usbide.app.codex_status_argv", return_value=["codex", "login", "status"]),
            patch("usbide.app.stream_subprocess", fake_stream),
            patch.object(app, "_log_issue") as log_issue,
            patch.object(app, "_codex_log_action") as log_action,
        ):
            ok = await app._codex_logged_in(env={})

        self.assertFalse(ok)
        self.assertTrue(log_issue.called)
        self.assertTrue(log_action.called)
        self.assertTrue(log_issue.call_args.kwargs.get("codex"))


class TestUSBIDEAppCodexActions(unittest.IsolatedAsyncioTestCase):
    async def test_action_codex_login_utilise_panneau_codex(self) -> None:
        # L'action login doit loguer dans le panneau Codex.
        app = USBIDEApp(root_dir=Path.cwd())
        with (
            patch("usbide.app.codex_cli_available", return_value=True),
            patch("usbide.app.codex_login_argv", return_value=["codex", "login"]),
            patch.object(app, "_codex_log_ui") as log_ui,
            patch.object(app, "_codex_log_output") as log_output,
            patch.object(app, "_stream_and_log", AsyncMock()) as stream_mock,
        ):
            await app.action_codex_login()

        self.assertTrue(log_ui.called)
        self.assertTrue(stream_mock.called)
        kwargs = stream_mock.call_args.kwargs
        self.assertIs(kwargs.get("output_log"), log_output)
        self.assertIs(kwargs.get("ui_log"), log_ui)


class TestUSBIDEAppBugLog(unittest.TestCase):
    def test_record_issue_cree_bug_md(self) -> None:
        # Un incident doit etre ajoute dans bug.md avec les champs essentiels.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            app = USBIDEApp(root_dir=root_dir)

            app._record_issue("erreur", "Erreur test", contexte="test_unitaire")

            contenu = (root_dir / "bug.md").read_text(encoding="utf-8")
            self.assertIn("niveau: erreur", contenu)
            self.assertIn("contexte: test_unitaire", contenu)
            self.assertIn("message: Erreur test", contenu)


class TestUSBIDEAppStreamLog(unittest.IsolatedAsyncioTestCase):
    async def test_stream_and_log_retour_nonzero(self) -> None:
        # Un exit code non nul doit etre consigne dans bug.md.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            app = USBIDEApp(root_dir=root_dir)

            async def fake_stream(*_args, **_kwargs):
                yield {"kind": "line", "text": "ok", "returncode": None}
                yield {"kind": "exit", "text": "exit 2", "returncode": 2}

            fake_log = MagicMock()
            output_lines: list[str] = []
            ui_lines: list[str] = []

            def output_log(text: str) -> None:
                output_lines.append(text)

            def ui_log(text: str) -> None:
                ui_lines.append(text)

            with (
                patch("usbide.app.stream_subprocess", fake_stream),
                patch.object(app, "query_one", return_value=fake_log),
            ):
                await app._stream_and_log(
                    ["cmd"],
                    cwd=root_dir,
                    env={},
                    output_log=output_log,
                    ui_log=ui_log,
                    contexte="test subprocess",
                )

            contenu = (root_dir / "bug.md").read_text(encoding="utf-8")
            self.assertIn("rc=2", contenu)

