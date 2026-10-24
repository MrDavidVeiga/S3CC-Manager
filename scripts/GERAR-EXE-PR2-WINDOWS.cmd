@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-beta3-pr2-windows.ps1" %*
set "status=%ERRORLEVEL%"
if not "%status%"=="0" (
  echo.
  echo [PR2] The build failed. Send the build.log file from TESTE-PR2-BETA3-WINDOWS-X64.
  pause
)
exit /b %status%
