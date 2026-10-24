@echo off
setlocal DisableDelayedExpansion
rem Keep all archive verification and atomic installation in the adjacent script.
rem Use the OS host, never a powershell.exe found in the current directory/PATH.
if not exist "%~dp0install-windows.ps1" (
  >&2 echo SwapCtx: install-windows.ps1 must be beside install-windows.cmd. Download and verify both installer files.
  exit /b 2
)
if not exist "%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" (
  >&2 echo SwapCtx: Windows PowerShell 5.1 is required. You can also run install-windows.ps1 with PowerShell 7.
  exit /b 9009
)
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0install-windows.ps1" %*
exit /b %errorlevel%
