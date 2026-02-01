import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from usbide.runner import (
    codex_bin_dir,
    codex_cli_available,
    codex_entrypoint_js,
    codex_env,
    codex_exec_argv,
    codex_install_argv,
    codex_install_prefix,
    codex_login_argv,
    codex_status_argv,
    node_executable,
    npm_cli_js,
    parse_tool_list,
    pip_install_argv,
    pyinstaller_available,
    pyinstaller_build_argv,
    pyinstaller_install_argv,
    python_scripts_dir,
    stream_subprocess,
    tool_available,
    tools_env,
    tools_install_prefix,
)


def _create_portable_node(root_dir: Path) -> Path:
    # Cree un node portable conforme au layout attendu.
    node_dir = root_dir / "tools" / "node"
    if os.name == "nt":
        node_path = node_dir / "node.exe"
    else:
        node_path = node_dir / "node"
    node_path.parent.mkdir(parents=True, exist_ok=True)
    node_path.write_text("", encoding="utf-8")
    return node_path


def _create_npm_cli(root_dir: Path, node_path: Path) -> Path:
    # Cree un npm-cli.js portable pour les tests.
    npm_path = node_path.parent / "node_modules" / "npm" / "bin" / "npm-cli.js"
    npm_path.parent.mkdir(parents=True, exist_ok=True)
    npm_path.write_text("", encoding="utf-8")
    return npm_path


def _create_codex_package(prefix: Path) -> Path:
    # Cree un package @openai/codex minimal avec un bin JS.
    pkg_dir = prefix / "node_modules" / "@openai" / "codex"
    pkg_dir.mkdir(parents=True, exist_ok=True)
    entry_rel = Path("bin") / "codex.js"
    entry_path = pkg_dir / entry_rel
    entry_path.parent.mkdir(parents=True, exist_ok=True)
    entry_path.write_text("", encoding="utf-8")
    pkg_json = pkg_dir / "package.json"
    pkg_json.write_text('{"bin": {"codex": "bin/codex.js"}}', encoding="utf-8")
    return entry_path


class TestStreamSubprocess(unittest.IsolatedAsyncioTestCase):
    async def test_argv_vide_declenche_erreur(self) -> None:
        # Une commande vide doit etre rejetee pour eviter un subprocess invalide.
        with self.assertRaises(ValueError):
            async for _ in stream_subprocess([]):
                pass


class TestCodexHelpers(unittest.TestCase):
    def test_codex_login_argv_default(self) -> None:
        # Verifie la commande d'authentification par defaut.
        # Force un environnement non-Windows pour un resultat deterministe.
        with patch("usbide.runner._is_windows", return_value=False):
            argv = codex_login_argv()
            self.assertEqual(argv[0], "codex")
            self.assertEqual(argv[1:], ["login"])

    def test_codex_login_argv_device_auth(self) -> None:
        # Le mode device auth doit ajouter le flag --device-auth.
        with patch("usbide.runner._is_windows", return_value=False):
            argv = codex_login_argv(device_auth=True)
            self.assertIn("--device-auth", argv)

    def test_codex_status_argv_default(self) -> None:
        # Verifie la commande de statut par defaut.
        # Force un environnement non-Windows pour un resultat deterministe.
        with patch("usbide.runner._is_windows", return_value=False):
            argv = codex_status_argv()
            self.assertEqual(argv[0], "codex")
            self.assertEqual(argv[1:], ["login", "status"])

    def test_codex_exec_argv_json(self) -> None:
        # Verifie le mode JSONL et les arguments additionnels.
        # Force un environnement non-Windows pour un resultat deterministe.
        with patch("usbide.runner._is_windows", return_value=False):
            argv = codex_exec_argv("hello", json_output=True, extra_args=["--model", "gpt-5"])
            self.assertEqual(argv[0], "codex")
            self.assertIn("--json", argv)
            self.assertIn("--model", argv)
            self.assertIn("gpt-5", argv)
            self.assertEqual(argv[-1], "hello")

    def test_codex_exec_argv_portable_prioritaire(self) -> None:
        # Le mode portable (node + entrypoint) doit etre prioritaire si present.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            node_path = _create_portable_node(root_dir)
            entry_path = _create_codex_package(codex_install_prefix(root_dir))
            argv = codex_exec_argv("hello", root_dir=root_dir, env={}, json_output=True)
            self.assertEqual(argv[0], str(node_path.resolve()))
            self.assertEqual(argv[1], str(entry_path.resolve()))
            self.assertIn("exec", argv)

    def test_codex_exec_argv_windows_cmd_shim(self) -> None:
        """Sur Windows, `codex` est souvent un `codex.cmd` (npm shim).

        Dans ce cas, on doit passer par `cmd.exe /c` sinon CreateProcess peut lever WinError 2.
        Ce test simule ce scenario en mockant la detection Windows + shutil.which().
        """

        def fake_which(cmd: str, path: str | None = None) -> str | None:
            if cmd == "codex":
                return r"C:\Users\me\AppData\Roaming\npm\codex.cmd"
            return None

        with patch("usbide.runner._is_windows", return_value=True):
            with patch("usbide.runner.shutil.which", side_effect=fake_which):
                env = {"PATH": r"C:\Users\me\AppData\Roaming\npm", "COMSPEC": r"C:\Windows\System32\cmd.exe"}
                argv = codex_exec_argv("hello", root_dir=Path("C:/tmp/usbide"), env=env, json_output=True)

                # cmd.exe wrapper
                self.assertEqual(argv[0], env["COMSPEC"])
                self.assertIn("/c", argv)
                self.assertIn(r"C:\Users\me\AppData\Roaming\npm\codex.cmd", argv)

                # suite normale des args codex
                self.assertIn("exec", argv)
                self.assertIn("--json", argv)
                self.assertEqual(argv[-1], "hello")

    def test_codex_exec_argv_rejecte_vide(self) -> None:
        # Un prompt vide doit declencher une erreur.
        with self.assertRaises(ValueError):
            codex_exec_argv(" ")

    def test_codex_env_prepend_path(self) -> None:
        # L'environnement Codex doit prefixer le PATH.
        root_dir = Path("/tmp/usbide")
        base_env = {"PATH": "/bin"}
        env = codex_env(root_dir, base_env)
        expected_bin = str(codex_bin_dir(codex_install_prefix(root_dir)))
        expected_node = str(root_dir / "tools" / "node")
        self.assertTrue(env["PATH"].startswith(expected_bin + os.pathsep))
        self.assertIn(expected_node, env["PATH"])
        self.assertEqual(base_env["PATH"], "/bin")

    def test_codex_cli_available_fallback(self) -> None:
        # Le helper doit refleter la disponibilite du binaire via PATH.
        with patch("usbide.runner.shutil.which", return_value="/usr/bin/codex"):
            self.assertTrue(codex_cli_available())
        with patch("usbide.runner.shutil.which", return_value=None):
            self.assertFalse(codex_cli_available())

    def test_codex_cli_available_portable(self) -> None:
        # Le helper doit detecter un Codex portable (node + entrypoint).
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            _create_portable_node(root_dir)
            _create_codex_package(codex_install_prefix(root_dir))
            with patch("usbide.runner.shutil.which", return_value=None):
                self.assertTrue(codex_cli_available(root_dir))

    def test_node_executable_prefers_portable(self) -> None:
        # Le node portable doit etre resolu si present.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            node_path = _create_portable_node(root_dir)
            with patch("usbide.runner.shutil.which", return_value=None):
                self.assertEqual(node_executable(root_dir), node_path.resolve())

    def test_npm_cli_js_finds(self) -> None:
        # npm-cli.js doit etre resolu via le node portable.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            node_path = _create_portable_node(root_dir)
            npm_path = _create_npm_cli(root_dir, node_path)
            self.assertEqual(npm_cli_js(root_dir, node=node_path), npm_path.resolve())

    def test_codex_entrypoint_js(self) -> None:
        # L'entrypoint doit etre resolu depuis package.json.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            prefix = codex_install_prefix(root_dir)
            entry_path = _create_codex_package(prefix)
            self.assertEqual(codex_entrypoint_js(prefix), entry_path.resolve())

    def test_codex_install_argv(self) -> None:
        # La commande npm doit cibler le prefix portable.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            node_path = _create_portable_node(root_dir)
            npm_path = _create_npm_cli(root_dir, node_path)
            prefix = codex_install_prefix(root_dir)
            argv = codex_install_argv(root_dir, prefix, "@openai/codex")
            self.assertIn(str(node_path), argv)
            self.assertIn(str(npm_path), argv)
            self.assertIn("--prefix", argv)
            self.assertIn(str(prefix), argv)
            self.assertTrue(prefix.exists())

    def test_codex_install_argv_rejecte_vide(self) -> None:
        # Un nom de package vide doit declencher une erreur.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            prefix = codex_install_prefix(root_dir)
            with self.assertRaises(ValueError):
                codex_install_argv(root_dir, prefix, " ")


class TestToolsHelpers(unittest.TestCase):
    def test_parse_tool_list(self) -> None:
        # La liste doit etre nettoyee (separateurs et doublons).
        tools = parse_tool_list("ruff, black  mypy, pytest ruff")
        self.assertEqual(tools, ["ruff", "black", "mypy", "pytest"])

    def test_tool_available_rejecte_vide(self) -> None:
        # Un nom d'outil vide doit declencher une erreur.
        with self.assertRaises(ValueError):
            tool_available(" ")

    def test_tools_env_prepend_path(self) -> None:
        # L'environnement outils doit prefixer le PATH avec le binaire portable.
        root_dir = Path("/tmp/usbide")
        base_env = {"PATH": "/bin"}
        env = tools_env(root_dir, base_env)
        expected_bin = str(python_scripts_dir(tools_install_prefix(root_dir)))
        self.assertTrue(env["PATH"].startswith(expected_bin + os.pathsep))
        self.assertEqual(base_env["PATH"], "/bin")

    def test_pyinstaller_install_argv(self) -> None:
        # La commande pip doit cibler le prefix outils.
        prefix = Path("/tmp/usbide/.usbide/tools")
        argv = pyinstaller_install_argv(prefix)
        self.assertIn("--prefix", argv)
        self.assertIn(str(prefix), argv)

    def test_pip_install_argv(self) -> None:
        # La commande pip doit inclure les packages demandes.
        prefix = Path("/tmp/usbide/.usbide/tools")
        argv = pip_install_argv(prefix, ["ruff", "black"])
        self.assertIn("--prefix", argv)
        self.assertIn(str(prefix), argv)
        self.assertIn("ruff", argv)
        self.assertIn("black", argv)

    def test_pip_install_argv_offline(self) -> None:
        # Le mode offline doit ajouter --no-index et --find-links.
        prefix = Path("/tmp/usbide/.usbide/tools")
        wheelhouse = Path("/tmp/usbide/tools/wheels")
        argv = pip_install_argv(prefix, ["ruff"], find_links=wheelhouse, no_index=True)
        self.assertIn("--no-index", argv)
        self.assertIn("--find-links", argv)
        self.assertIn(str(wheelhouse), argv)

    def test_pip_install_argv_rejecte_vide(self) -> None:
        # Une liste vide doit declencher une erreur.
        with self.assertRaises(ValueError):
            pip_install_argv(Path("/tmp/usbide/.usbide/tools"), [" "])

    def test_pyinstaller_build_argv(self) -> None:
        # La commande PyInstaller doit inclure le script et le dist.
        script = Path("/tmp/usbide/app.py")
        dist_dir = Path("/tmp/usbide/dist")
        argv = pyinstaller_build_argv(script, dist_dir, onefile=True)
        self.assertIn(str(script), argv)
        self.assertIn(str(dist_dir), argv)
        self.assertIn("--onefile", argv)
        self.assertNotIn("--onedir", argv)

    def test_pyinstaller_build_argv_onedir_par_defaut(self) -> None:
        # Par defaut, on privilegie --onedir pour limiter les faux positifs AV.
        script = Path("/tmp/usbide/app.py")
        dist_dir = Path("/tmp/usbide/dist")
        argv = pyinstaller_build_argv(script, dist_dir)
        self.assertIn("--onedir", argv)

    def test_pyinstaller_build_argv_work_spec(self) -> None:
        # Les chemins work/spec doivent etre inclus si fournis.
        script = Path("/tmp/usbide/app.py")
        dist_dir = Path("/tmp/usbide/dist")
        work_dir = Path("/tmp/usbide/build")
        spec_dir = Path("/tmp/usbide")
        argv = pyinstaller_build_argv(
            script,
            dist_dir,
            work_dir=work_dir,
            spec_dir=spec_dir,
        )
        self.assertIn("--workpath", argv)
        self.assertIn(str(work_dir), argv)
        self.assertIn("--specpath", argv)
        self.assertIn(str(spec_dir), argv)

    def test_pyinstaller_build_argv_rejecte_vide(self) -> None:
        # Un script vide doit declencher une erreur.
        with self.assertRaises(ValueError):
            pyinstaller_build_argv(Path(""), Path("/tmp/usbide/dist"))

    def test_pyinstaller_available_avec_env(self) -> None:
        # La disponibilite doit tenir compte du PATH fourni.
        root_dir = Path("/tmp/usbide")
        env = {"PATH": "/bin"}
        expected_bin = str(python_scripts_dir(tools_install_prefix(root_dir)))
        expected_path = os.pathsep.join([expected_bin, env["PATH"]])
        with patch("usbide.runner.shutil.which", return_value="/usr/bin/pyinstaller") as which:
            self.assertTrue(pyinstaller_available(root_dir, env))
            which.assert_called_once_with("pyinstaller", path=expected_path)

    def test_tool_available_avec_env(self) -> None:
        # La disponibilite doit tenir compte du PATH fourni.
        root_dir = Path("/tmp/usbide")
        env = {"PATH": "/bin"}
        expected_bin = str(python_scripts_dir(tools_install_prefix(root_dir)))
        expected_path = os.pathsep.join([expected_bin, env["PATH"]])
        with patch("usbide.runner.shutil.which", return_value="/usr/bin/ruff") as which:
            self.assertTrue(tool_available("ruff", root_dir, env))
            which.assert_called_once_with("ruff", path=expected_path)

