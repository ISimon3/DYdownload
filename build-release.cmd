@echo off
setlocal
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0build-release.ps1" %*
if errorlevel 1 goto failed
echo.
echo Release build completed.
pause
exit /b 0

:failed
set "build_exit_code=%ERRORLEVEL%"
echo.
echo Release build failed with exit code %build_exit_code%.
pause
exit /b %build_exit_code%
