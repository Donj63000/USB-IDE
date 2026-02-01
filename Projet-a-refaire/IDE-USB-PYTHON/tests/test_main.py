import sys
import tempfile
import unittest
from pathlib import Path

from usbide.__main__ import ensure_vendor_path


class TestEnsureVendorPath(unittest.TestCase):
    def test_pas_de_vendor_pas_de_changement(self) -> None:
        # Sans dossier vendor, sys.path ne doit pas être modifié.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            original = list(sys.path)

            ensure_vendor_path(root_dir)

            self.assertEqual(sys.path, original)

    def test_vendor_ajoute_une_seule_fois(self) -> None:
        # Le vendor doit être inséré en tête, sans duplication.
        with tempfile.TemporaryDirectory() as tmp_dir:
            root_dir = Path(tmp_dir)
            vendor = root_dir / ".usbide" / "vendor"
            vendor.mkdir(parents=True, exist_ok=True)
            resolved = str(vendor.resolve())
            original = list(sys.path)
            try:
                ensure_vendor_path(root_dir)
                ensure_vendor_path(root_dir)
                self.assertEqual(sys.path[0], resolved)
                self.assertEqual(sys.path.count(resolved), 1)
            finally:
                sys.path = original
