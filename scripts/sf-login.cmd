@echo off
rem Thin launcher for sf-login.ps1: batch has no regex/JSON support, so it only forwards.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0sf-login.ps1" %*
exit /b %ERRORLEVEL%
