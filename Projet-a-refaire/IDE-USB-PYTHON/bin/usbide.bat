@echo off
setlocal EnableExtensions

REM --- racine (dossier usbide) ---
set "ROOT=%~dp0.."
for %%I in ("%ROOT%") do set "ROOT=%%~fI"

REM --- python portable attendu : adapte si besoin ---
REM ex : set "PY=%ROOT%\..\python\python.exe" si usbide est dans DEVKIT\usbide
set "PY=%ROOT%\..\python\python.exe"

if not exist "%PY%" (
  echo [ERREUR] Python portable introuvable: "%PY%"
  echo Edite ce fichier pour pointer vers ton python.exe sur la cle.
  pause
  exit /b 1
)

set "PYTHONUTF8=1"
set "PYTHONIOENCODING=utf-8"
set "PIP_CACHE_DIR=%ROOT%\..\pip-cache"
set "VENDOR=%ROOT%\.usbide\vendor"
set "WORKSPACE=%ROOT%\..\workspace"

if not exist "%VENDOR%" (
  REM Crée le répertoire vendor pour les dépendances portables.
  mkdir "%VENDOR%"
)

REM Installation des dépendances si absentes (sur la cle uniquement).
if not exist "%VENDOR%\textual" (
  echo [INFO] Installation des dependances dans "%VENDOR%"...
  "%PY%" -m pip install --upgrade --target "%VENDOR%" -r "%ROOT%\requirements.txt" --disable-pip-version-check --no-warn-script-location
  if errorlevel 1 (
    echo [ERREUR] Echec d'installation des dependances.
    pause
    exit /b 1
  )
)

REM PYTHONPATH pointe vers les dependances vendored sur la cle.
set "PYTHONPATH=%VENDOR%;%PYTHONPATH%"

if not exist "%WORKSPACE%" (
  REM Cree le dossier de travail s'il n'existe pas.
  mkdir "%WORKSPACE%"
)

cd /d "%ROOT%"
"%PY%" -m usbide --root "%WORKSPACE%"
