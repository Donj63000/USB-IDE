import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from usbide import preflight


class TestPreflight(unittest.TestCase):
    def test_resolve_root_default(self) -> None:
        # Sans parametre, la racine doit etre le cwd.
        with tempfile.TemporaryDirectory() as tmp_dir:
            with patch("usbide.preflight.Path.cwd", return_value=Path(tmp_dir)):
                resolved = preflight.resolve_root()
                self.assertEqual(resolved, Path(tmp_dir).resolve())

    def test_resolve_root_parametre(self) -> None:
        # Le parametre doit etre resolve correctement.
        with tempfile.TemporaryDirectory() as tmp_dir:
            resolved = preflight.resolve_root(Path(tmp_dir))
            self.assertEqual(resolved, Path(tmp_dir).resolve())

    def test_can_write_ok(self) -> None:
        # Un dossier temporaire doit etre inscriptible.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "sub"
            self.assertTrue(preflight.can_write(path))

    def test_can_write_ko(self) -> None:
        # Une exception d'ecriture doit retourner False.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "sub"
            with patch("pathlib.Path.write_text", side_effect=OSError("boom")):
                self.assertFalse(preflight.can_write(path))

    def test_dns_ok_true(self) -> None:
        # La resolution DNS doit retourner True si l'appel reussit.
        with patch("usbide.preflight.socket.gethostbyname", return_value="1.2.3.4"):
            self.assertTrue(preflight.dns_ok("example.com"))

    def test_dns_ok_false(self) -> None:
        # La resolution DNS doit retourner False si l'appel echoue.
        with patch("usbide.preflight.socket.gethostbyname", side_effect=OSError("boom")):
            self.assertFalse(preflight.dns_ok("example.com"))
