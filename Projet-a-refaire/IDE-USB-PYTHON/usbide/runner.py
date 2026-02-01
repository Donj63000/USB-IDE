from __future__ import annotations

import asyncio
import json
import os
import shutil
import sys
from pathlib import Path
from typing import AsyncIterator, Dict, Iterable, Literal, Optional, Sequence, TypedDict


class ProcEvent(TypedDict):
    kind: Literal["line", "exit"]
    text: str
    returncode: Optional[int]


def _is_windows() -> bool:
    """Retourne True si l'OS courant est Windows.

    Note: on factorise ce test pour pouvoir le mocker facilement en tests unitaires.
    """
    return os.name == "nt"


async def stream_subprocess(
    argv: Sequence[str],
    *,
    cwd: Optional[Path] = None,
    env: Optional[Dict[str, str]] = None,
) -> AsyncIterator[ProcEvent]:
    """Lance un subprocess et stream la sortie.

    - stdout est capture
    - stderr est redirige vers stdout
    - encodage sortie: UTF-8 (errors='replace')

    Yield:
      - {'kind': 'line', 'text': '...', 'returncode': None}
      - {'kind': 'exit', 'text': 'exit <rc>', 'returncode': <rc>}
    """
    if not argv:
        # Protection: une commande vide ne doit pas lancer de subprocess.
        raise ValueError("argv ne doit pas etre vide")

    proc = await asyncio.create_subprocess_exec(
        *argv,
        cwd=str(cwd) if cwd else None,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.STDOUT,
        env=env,
    )

    assert proc.stdout is not None
    while True:
        raw = await proc.stdout.readline()
        if not raw:
            break
        yield {
            "kind": "line",
            "text": raw.decode("utf-8", errors="replace").rstrip("\n"),
            "returncode": None,
        }

    rc = await proc.wait()
    yield {"kind": "exit", "text": f"exit {rc}", "returncode": rc}


def windows_cmd_argv(command: str) -> list[str]:
    """Construit argv pour executer une commande via cmd.exe sur Windows."""
    comspec = os.environ.get("COMSPEC") or "cmd.exe"
    return [comspec, "/d", "/s", "/c", command]


def python_run_argv(script: Path) -> list[str]:
    """Commande pour executer un script python avec l'interpreteur courant."""
    return [sys.executable, str(script)]


# =============================================================================
# Outils Python (pip --prefix) : PyInstaller + outils dev
# =============================================================================


def tools_install_prefix(root_dir: Path) -> Path:
    """Prefix d'installation portable pour les outils Python."""
    return root_dir / ".usbide" / "tools"


def python_scripts_dir(prefix: Path) -> Path:
    """Dossier Scripts/bin d'un --prefix pip."""
    return prefix / ("Scripts" if os.name == "nt" else "bin")


def tools_env(root_dir: Path, base_env: Optional[Dict[str, str]] = None) -> Dict[str, str]:
    """Construit un environnement incluant les outils portables."""
    env = dict(base_env) if base_env is not None else os.environ.copy()
    bin_dir = python_scripts_dir(tools_install_prefix(root_dir))
    path_value = env.get("PATH", "")
    path_parts = path_value.split(os.pathsep) if path_value else []
    if str(bin_dir) not in path_parts:
        env["PATH"] = os.pathsep.join([str(bin_dir), *path_parts]) if path_parts else str(bin_dir)
    return env


def parse_tool_list(raw: str) -> list[str]:
    """Nettoie une liste d'outils depuis une chaine (virgules / espaces)."""
    items = [item.strip() for item in raw.replace(",", " ").split()]
    seen: set[str] = set()
    cleaned: list[str] = []
    for item in items:
        if not item:
            continue
        if item in seen:
            continue
        seen.add(item)
        cleaned.append(item)
    return cleaned


def tool_available(
    tool: str, root_dir: Optional[Path] = None, env: Optional[Dict[str, str]] = None
) -> bool:
    """Verifie la presence d'un outil dans le PATH (local ou systeme)."""
    if not tool.strip():
        # Protection: un nom vide n'est pas valide.
        raise ValueError("tool ne doit pas etre vide")
    search_env = env
    if root_dir is not None:
        search_env = tools_env(root_dir, env)
    return shutil.which(tool, path=search_env.get("PATH") if search_env else None) is not None


def pyinstaller_available(root_dir: Optional[Path] = None, env: Optional[Dict[str, str]] = None) -> bool:
    """Verifie la presence du binaire `pyinstaller` dans le PATH."""
    return tool_available("pyinstaller", root_dir=root_dir, env=env)


def pip_install_argv(
    prefix: Path,
    packages: Iterable[str],
    *,
    find_links: Optional[Path] = None,
    no_index: bool = False,
) -> list[str]:
    """Commande pour installer des packages via pip dans un prefix portable."""
    cleaned = [pkg.strip() for pkg in packages if pkg.strip()]
    if not cleaned:
        # Protection: il faut au moins un package valide.
        raise ValueError("packages ne doit pas etre vide")
    argv = [
        sys.executable,
        "-m",
        "pip",
        "install",
        "--upgrade",
        "--prefix",
        str(prefix),
    ]
    if no_index:
        argv.append("--no-index")
    if find_links is not None:
        argv.extend(["--find-links", str(find_links)])
    argv.extend(cleaned)
    return argv


def pyinstaller_install_argv(
    prefix: Path,
    *,
    find_links: Optional[Path] = None,
    no_index: bool = False,
) -> list[str]:
    """Commande pour installer PyInstaller via pip dans un prefix portable."""
    return pip_install_argv(prefix, ["pyinstaller"], find_links=find_links, no_index=no_index)


def pyinstaller_build_argv(
    script: Path,
    dist_dir: Path,
    *,
    onefile: bool = False,
    work_dir: Optional[Path] = None,
    spec_dir: Optional[Path] = None,
) -> list[str]:
    """Commande pour generer un executable depuis un script Python."""
    if not script.name.strip():
        # Protection: un script vide n'est pas valide.
        raise ValueError("script ne doit pas etre vide")
    argv = [
        "pyinstaller",
        "--noconfirm",
        "--onedir",
        "--distpath",
        str(dist_dir),
    ]
    if onefile:
        # En mode onefile, on remplace explicitement le mode onedir.
        argv.remove("--onedir")
        argv.insert(1, "--onefile")
    if work_dir is not None:
        argv.extend(["--workpath", str(work_dir)])
    if spec_dir is not None:
        argv.extend(["--specpath", str(spec_dir)])
    argv.append(str(script))
    return argv


# =============================================================================
# Codex CLI officiel (npm: @openai/codex)
# =============================================================================


def codex_install_prefix(root_dir: Path) -> Path:
    """Prefix npm portable pour Codex."""
    return root_dir / ".usbide" / "codex"


def codex_bin_dir(prefix: Path) -> Path:
    """Repertoire .bin npm."""
    return prefix / "node_modules" / ".bin"


def node_tools_dir(root_dir: Path) -> Path:
    """Dossier Node portable attendu."""
    return root_dir / "tools" / "node"


def node_executable(root_dir: Path, env: Optional[Dict[str, str]] = None) -> Optional[Path]:
    """Resout node (portable puis fallback PATH)."""
    candidates: list[Path] = []
    node_dir = node_tools_dir(root_dir)

    if _is_windows():
        candidates.append(node_dir / "node.exe")
    else:
        candidates.extend([node_dir / "bin" / "node", node_dir / "node"])

    search_path = (env or os.environ).get("PATH")
    which = shutil.which("node", path=search_path)
    if which:
        candidates.append(Path(which))

    for candidate in candidates:
        if candidate.exists():
            return candidate.resolve()
    return None


def npm_cli_js(root_dir: Path, node: Optional[Path] = None) -> Optional[Path]:
    """Chemin npm-cli.js (executer npm via node)."""
    node = node or node_executable(root_dir)
    if node is None:
        return None

    node_dir = node.parent
    candidate = node_dir / "node_modules" / "npm" / "bin" / "npm-cli.js"
    if candidate.exists():
        return candidate.resolve()

    # Fallback permissif (node systeme)
    for alt in (
        node_dir.parent / "lib" / "node_modules" / "npm" / "bin" / "npm-cli.js",
        node_dir / ".." / "lib" / "node_modules" / "npm" / "bin" / "npm-cli.js",
    ):
        try:
            alt = alt.resolve()
        except Exception:
            continue
        if alt.exists():
            return alt
    return None


def codex_env(root_dir: Path, base_env: Optional[Dict[str, str]] = None) -> Dict[str, str]:
    """Prefixe PATH avec .bin Codex + Node portable (meme si pas encore installes)."""
    env = dict(base_env) if base_env is not None else os.environ.copy()
    path_value = env.get("PATH", "")
    path_parts = path_value.split(os.pathsep) if path_value else []

    node_dir = node_tools_dir(root_dir)
    if str(node_dir) not in path_parts:
        path_parts.insert(0, str(node_dir))

    bin_dir = codex_bin_dir(codex_install_prefix(root_dir))
    if str(bin_dir) not in path_parts:
        path_parts.insert(0, str(bin_dir))

    env["PATH"] = os.pathsep.join(path_parts)
    return env


def codex_package_json(prefix: Path) -> Path:
    """Chemin vers package.json du package @openai/codex."""
    return prefix / "node_modules" / "@openai" / "codex" / "package.json"


def codex_entrypoint_js(prefix: Path) -> Optional[Path]:
    """Resout l'entrypoint CLI via la cle 'bin' du package.json."""
    pkg_json = codex_package_json(prefix)
    if not pkg_json.exists():
        return None
    try:
        data = json.loads(pkg_json.read_text(encoding="utf-8"))
    except Exception:
        return None

    bin_field = data.get("bin")
    rel: Optional[str] = None
    if isinstance(bin_field, str):
        rel = bin_field
    elif isinstance(bin_field, dict):
        if isinstance(bin_field.get("codex"), str):
            rel = bin_field["codex"]
        else:
            for value in bin_field.values():
                if isinstance(value, str):
                    rel = value
                    break

    if not rel:
        return None

    entry = pkg_json.parent / rel
    return entry.resolve() if entry.exists() else None


def codex_cli_available(root_dir: Optional[Path] = None, env: Optional[Dict[str, str]] = None) -> bool:
    """Codex OK si:
    - portable (node + entrypoint) disponible, OU
    - codex dispo dans PATH (fallback)
    """
    if root_dir is not None:
        node = node_executable(root_dir, env=env)
        entry = codex_entrypoint_js(codex_install_prefix(root_dir))
        if node is not None and entry is not None:
            return True

    search_env = env
    if root_dir is not None:
        search_env = codex_env(root_dir, env)
    path = search_env.get("PATH") if search_env else None
    return shutil.which("codex", path=path) is not None


def _codex_base_argv(root_dir: Optional[Path] = None, env: Optional[Dict[str, str]] = None) -> list[str]:
    """Retourne la commande de base pour lancer Codex.

    Priorite :
    1) Mode portable : node.exe + entrypoint JS de @openai/codex (fiable, pas de .cmd/.bat).
    2) Fallback systeme : binaire `codex` dans le PATH.

    Sur Windows, `npm install -g @openai/codex` cree souvent un shim `codex.cmd`.
    Or, `asyncio.create_subprocess_exec(..., shell=False)` ne sait pas lancer un `.cmd` directement,
    ce qui se traduit typiquement par : [WinError 2] Le fichier specifie est introuvable.

    Donc en fallback Windows, si `codex` resolu est un `.cmd`/`.bat`, on l'execute via cmd.exe.
    """
    # --- (1) Mode portable : node + entrypoint ---
    if root_dir is not None:
        node = node_executable(root_dir, env=env)
        entry = codex_entrypoint_js(codex_install_prefix(root_dir))
        if node is not None and entry is not None:
            return [str(node), str(entry)]

    # --- (2) Fallback systeme ---
    if _is_windows():
        # `which` doit utiliser le PATH de l'env fourni (celui de l'app).
        search_path = (env or os.environ).get("PATH")
        resolved = shutil.which("codex", path=search_path)
        if resolved:
            suffix = Path(resolved).suffix.lower()

            # Cas npm Windows : codex.cmd / codex.bat (doit passer par cmd.exe)
            if suffix in {".cmd", ".bat"}:
                comspec = (env or os.environ).get("COMSPEC") or os.environ.get("COMSPEC") or "cmd.exe"
                return [comspec, "/d", "/s", "/c", resolved]

            # Certains environnements ajoutent aussi un shim PowerShell.
            if suffix == ".ps1":
                powershell = shutil.which("powershell", path=search_path) or "powershell"
                return [powershell, "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", resolved]

            # Si c'est un vrai .exe (ou autre), on peut le lancer directement.
            return [resolved]

    # Par defaut (Linux/macOS, ou PATH qui resolvra un binaire executable)
    return ["codex"]


def codex_login_argv(
    root_dir: Optional[Path] = None,
    env: Optional[Dict[str, str]] = None,
    *,
    device_auth: bool = False,
) -> list[str]:
    """Commande pour initier l'authentification Codex."""
    argv = [*_codex_base_argv(root_dir, env), "login"]
    if device_auth:
        argv.append("--device-auth")
    return argv


def codex_status_argv(root_dir: Optional[Path] = None, env: Optional[Dict[str, str]] = None) -> list[str]:
    """Commande pour verifier le statut d'authentification Codex."""
    return [*_codex_base_argv(root_dir, env), "login", "status"]


def codex_exec_argv(
    prompt: str,
    *,
    root_dir: Optional[Path] = None,
    env: Optional[Dict[str, str]] = None,
    json_output: bool = False,
    extra_args: Optional[Sequence[str]] = None,
) -> list[str]:
    """Commande codex exec non-interactive. JSONL via --json."""
    if not prompt.strip():
        # Protection: un prompt vide est invalide.
        raise ValueError("prompt ne doit pas etre vide")

    argv = [*_codex_base_argv(root_dir, env), "exec"]
    if json_output:
        argv.append("--json")
    if extra_args:
        argv.extend([arg for arg in extra_args if arg.strip()])
    argv.append(prompt)
    return argv


def codex_install_argv(root_dir: Path, prefix: Path, package: str = "@openai/codex") -> list[str]:
    """Installe Codex via npm dans prefix (sur la cle)."""
    if not package.strip():
        # Protection: un nom de package vide n'est pas valide.
        raise ValueError("package ne doit pas etre vide")

    node = node_executable(root_dir)
    npm = npm_cli_js(root_dir, node=node)
    if node is None or npm is None:
        raise FileNotFoundError(
            "Node portable introuvable. Place une distribution Node dans tools/node/ "
            "(node.exe + node_modules/npm/...)."
        )

    prefix.mkdir(parents=True, exist_ok=True)
    return [
        str(node),
        str(npm),
        "install",
        "--prefix",
        str(prefix),
        "--no-audit",
        "--no-fund",
        package,
    ]

