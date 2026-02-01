from __future__ import annotations

import tokenize
from pathlib import Path


def detect_text_encoding(path: Path) -> str:
    """Détecte un encodage 'raisonnable' pour un fichier.

    - Pour .py : respecte PEP 263 via tokenize.detect_encoding().
    - Sinon : tente utf-8 / utf-8-sig / cp1252 / latin-1.

    Retourne un nom d'encodage Python (ex: 'utf-8').
    """
    if path.suffix.lower() == ".py":
        try:
            with path.open("rb") as bf:
                enc, _ = tokenize.detect_encoding(bf.readline)
            return enc
        except OSError:
            # Fallback sûr si le fichier n'est pas accessible.
            return "utf-8"

    for enc in ("utf-8", "utf-8-sig", "cp1252", "latin-1"):
        try:
            path.read_text(encoding=enc)
            return enc
        except UnicodeDecodeError:
            continue
        except OSError:
            # Fallback sûr si le fichier n'est pas accessible.
            return "utf-8"

    return "utf-8"


def is_probably_binary(path: Path, sniff_bytes: int = 2048) -> bool:
    """Heuristique simple pour éviter d'ouvrir des binaires dans l'éditeur.

    - présence d'un NUL dans les premiers bytes
    - trop de caractères de contrôle non-textuels
    """
    # Protection: une taille invalide ne permet pas d'échantillonnage fiable.
    if sniff_bytes <= 0:
        return False

    try:
        # Lecture limitée pour éviter de charger de gros fichiers en mémoire.
        with path.open("rb") as fh:
            data = fh.read(sniff_bytes)
    except OSError:
        # On laisse remonter l'erreur pour distinguer "binaire" d'"inaccessible".
        raise

    if b"\x00" in data:
        return True

    if not data:
        return False

    # Compte des caractères de contrôle (hors \n \r \t)
    ctrl = 0
    for b in data:
        if b in (9, 10, 13):  # \t \n \r
            continue
        if b < 32 or b == 127:
            ctrl += 1

    # Seuil empirique : si >10% de contrôles, probable binaire
    return (ctrl / len(data)) > 0.10
