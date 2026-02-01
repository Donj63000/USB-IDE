from __future__ import annotations

import os
import platform
import socket
import tempfile
from pathlib import Path
from typing import Optional


def resolve_root(root: Optional[Path] = None) -> Path:
    """Resout la racine utilisee pour le preflight."""
    if root is None:
        return Path.cwd().resolve()
    return Path(root).resolve()


def can_write(path: Path) -> bool:
    """Verifie si un chemin est inscriptible."""
    try:
        path.mkdir(parents=True, exist_ok=True)
        test = path / ".__write_test__"
        test.write_text("ok", encoding="utf-8")
        test.unlink()
        return True
    except Exception:
        return False


def dns_ok(host: str = "api.openai.com") -> bool:
    """Verifie la resolution DNS d'un hote."""
    try:
        socket.gethostbyname(host)
        return True
    except Exception:
        return False


def print_report(root: Path) -> None:
    """Affiche un rapport de preflight."""
    print("== Preflight ==")
    print("OS:", platform.platform())
    print("Arch:", platform.machine(), platform.architecture())
    print("USB root:", root)

    print("\n[Write tests]")
    print("Write USB cache:", can_write(root / "cache"))
    print("Write USB tmp:", can_write(root / "tmp"))
    print("Write tempdir:", can_write(Path(tempfile.gettempdir()) / "usbdev_test"))

    print("\n[Network]")
    print("DNS api.openai.com:", dns_ok())

    print("\n[Env]")
    print("OPENAI_API_KEY set:", bool(os.environ.get("OPENAI_API_KEY")))
    print(
        "HTTP_PROXY set:",
        bool(os.environ.get("HTTP_PROXY") or os.environ.get("HTTPS_PROXY")),
    )


def main(root: Optional[Path] = None) -> int:
    """Point d'entree CLI pour le preflight."""
    resolved = resolve_root(root)
    print_report(resolved)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
