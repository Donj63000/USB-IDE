@echo off
setlocal enabledelayedexpansion

REM Bootstrap Codex pour une cle USB (installation locale npm).
REM Executez ce script sur la machine de dev avec Internet.

set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"

set "NODE=%ROOT%\tools\node\node.exe"
set "NPMCLI=%ROOT%\tools\node\node_modules\npm\bin\npm-cli.js"
set "PREFIX=%ROOT%\.usbide\codex"

if not exist "%NODE%" (
  echo [ERREUR] node.exe introuvable: "%NODE%"
  exit /b 1
)

if not exist "%NPMCLI%" (
  echo [ERREUR] npm-cli.js introuvable: "%NPMCLI%"
  exit /b 1
)

if not exist "%ROOT%\cache\npm" mkdir "%ROOT%\cache\npm"
if not exist "%PREFIX%" mkdir "%PREFIX%"

set "NPM_CONFIG_CACHE=%ROOT%\cache\npm"
set "NPM_CONFIG_UPDATE_NOTIFIER=false"

echo [INFO] Installation Codex dans "%PREFIX%"...
"%NODE%" "%NPMCLI%" install --prefix "%PREFIX%" --no-audit --no-fund @openai/codex@latest

echo [OK] Termine.
endlocal
