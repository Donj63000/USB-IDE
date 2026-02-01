@echo off
setlocal

set "ROOT=%~dp0"
if "%ROOT:~-1%"=="\" set "ROOT=%ROOT:~0,-1%"

set "USBROOT=%ROOT%\..\.."
for %%I in ("%USBROOT%") do set "USBROOT=%%~fI"

set "PY=%USBROOT%\tools\python-x64\python.exe"

set "PIP_CACHE_DIR=%USBROOT%\cache\pip"
set "PYTHONPYCACHEPREFIX=%USBROOT%\cache\pycache"
set "TEMP=%USBROOT%\tmp"
set "TMP=%USBROOT%\tmp"
set "PYTHONNOUSERSITE=1"

REM Optionnel: s'assure que pyinstaller est present (offline)
"%PY%" -m pip install --no-index --find-links "%USBROOT%\tools\wheels" pyinstaller

REM Nettoyage des outputs
if exist "%ROOT%\dist" rmdir /s /q "%ROOT%\dist"
if exist "%ROOT%\build" rmdir /s /q "%ROOT%\build"

REM Build onedir (moins de faux positifs AV)
"%PY%" -m PyInstaller ^
  --noconfirm ^
  --clean ^
  --onedir ^
  --distpath "%ROOT%\dist" ^
  --workpath "%ROOT%\build" ^
  --specpath "%ROOT%" ^
  "%ROOT%\app.spec"

echo Done. Output: %ROOT%\dist
endlocal
