from __future__ import annotations

from pathlib import Path

from usbide.preflight import main


if __name__ == "__main__":
    # Lance le preflight depuis la racine de la cle.
    raise SystemExit(main(root=Path(__file__).resolve().parent))
