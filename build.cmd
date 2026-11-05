@echo off
rem Double-click to build the FreeSheet installers (runs scripts\build.js with Node).
cd /d "%~dp0"
where node >nul 2>nul || (echo Node.js is not installed or not on PATH. & pause & exit /b 1)
node scripts\build.js %*
echo.
pause
