from pathlib import Path
import tempfile
import unittest

from usbide.app import USBIDEApp


class TestThemeFrancais(unittest.TestCase):
    def test_bindings_sont_en_francais(self) -> None:
        # Verifie que les libelles des raccourcis sont bien en francais.
        expected_labels = [
            "Sauvegarder",
            "Executer",
            "Effacer les journaux",
            "Recharger l'arborescence",
            "Connexion Codex",
            "Verifier Codex",
            "Installer Codex",
            "Vue Codex",
            "Construire l'EXE",
            "Outils de dev",
            "Quitter",
        ]
        actual_labels = []
        # Supporte les tuples historiques et les objets Binding.
        for binding in USBIDEApp.BINDINGS:
            label = binding.description if hasattr(binding, "description") else binding[2]
            actual_labels.append(label)

        self.assertEqual(actual_labels, expected_labels)

    def test_theme_tcss_contient_les_sections_principales(self) -> None:
        # Garantit que le theme inclut les sections cles de l'UI.
        repo_root = Path(__file__).resolve().parents[1]
        css_path = repo_root / "usbide" / "usbide.tcss"
        css_text = css_path.read_text(encoding="utf-8")

        for selector in ("Screen", "#main", "#tree:focus", "#editor:focus", "#cmd:focus"):
            with self.subTest(selector=selector):
                self.assertIn(selector, css_text)


class TestTitresBordures(unittest.IsolatedAsyncioTestCase):
    async def test_titres_bordures_definis(self) -> None:
        # Verifie que les titres de bordure sont definis via l'API Textual.
        with tempfile.TemporaryDirectory() as tmp_dir:
            app = USBIDEApp(root_dir=Path(tmp_dir))
            async with app.run_test() as pilot:
                await pilot.pause()
                self.assertEqual(app.query_one("#tree").border_title, "Fichiers")
                self.assertEqual(app.query_one("#editor").border_title, "Editeur")
                self.assertEqual(app.query_one("#cmd").border_title, "Commande")
                self.assertEqual(app.query_one("#log").border_title, "Journal")

