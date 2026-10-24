@echo off
setlocal
echo [PR2] Generating Windows x64 portable S3CC Manager executable...
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-beta3-pr2-windows.ps1" %*
set "status=%ERRORLEVEL%"
if not "%status%"=="0" (
  echo.
  echo [PR2] Compilation failed. See TESTE-PR2-BETA3-WINDOWS-X64\build.log.
  pause
)
exit /b %status%
