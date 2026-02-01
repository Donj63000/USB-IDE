from __future__ import annotations

import argparse
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(prog="usbide", description="Mini IDE terminal portable (Textual).")
    p.add_argument(
        "--root",
        type=Path,
        default=Path.cwd(),
        help="Dossier racine du workspace (par défaut: répertoire courant).",
    )
    return p.parse_args()


def ensure_vendor_path(root_dir: Path) -> None:
    """Ajoute le répertoire vendor à sys.path si présent (portable)."""
    vendor_path = root_dir / ".usbide" / "vendor"
    if not vendor_path.exists():
        # Rien à faire si le vendor n'existe pas.
        return
    resolved = str(vendor_path.resolve())
    if resolved not in sys.path:
        # On injecte en tête pour privilégier les dépendances portables.
        sys.path.insert(0, resolved)


def main() -> None:
    args = parse_args()
    # Supporte un environnement portable où les dépendances sont "vendored".
    ensure_vendor_path(args.root)
    from usbide.app import USBIDEApp

    app = USBIDEApp(root_dir=args.root)
    app.run()


if __name__ == "__main__":
    main()
