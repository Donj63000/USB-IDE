import tempfile
import unittest
from pathlib import Path

from usbide.encoding import detect_text_encoding, is_probably_binary


class TestIsProbablyBinary(unittest.TestCase):
    def test_detecte_fichier_texte(self) -> None:
        # Fichier texte simple ne doit pas être considéré binaire.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "texte.txt"
            path.write_text("Bonjour\nCeci est un test.\n", encoding="utf-8")
            self.assertFalse(is_probably_binary(path))

    def test_detecte_fichier_binaire(self) -> None:
        # Présence d'un byte NUL => binaire probable.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "data.bin"
            path.write_bytes(b"\x00\x01\x02texte")
            self.assertTrue(is_probably_binary(path))

    def test_taille_invalide_ne_bloque_pas(self) -> None:
        # sniff_bytes <= 0 doit rester sûr et retourner False.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "texte.txt"
            path.write_text("abc", encoding="utf-8")
            self.assertFalse(is_probably_binary(path, sniff_bytes=0))

    def test_acces_impossible_declenche_erreur(self) -> None:
        # Un fichier inaccessible doit remonter une erreur pour un message précis.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "absent.bin"
            with self.assertRaises(OSError):
                is_probably_binary(path)


class TestDetectTextEncoding(unittest.TestCase):
    def test_fallback_sur_fichier_py_inaccessible(self) -> None:
        # Un fichier .py manquant ne doit pas faire planter la détection.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "absent.py"
            self.assertEqual(detect_text_encoding(path), "utf-8")

    def test_fallback_sur_fichier_texte_inaccessible(self) -> None:
        # Un fichier non-.py manquant doit retourner un encodage par défaut.
        with tempfile.TemporaryDirectory() as tmp_dir:
            path = Path(tmp_dir) / "absent.txt"
            self.assertEqual(detect_text_encoding(path), "utf-8")
