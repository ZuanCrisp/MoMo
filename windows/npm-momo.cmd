@echo off
setlocal
rem Prefer the portable Node distribution kept inside the MoMo workspace.
set "MOMO_WORKSPACE=%~dp0.."
set "MOMO_NODE="
for /d %%D in ("%MOMO_WORKSPACE%\.tools\node-v*-win-x64") do (
  if exist "%%~fD\node.exe" if exist "%%~fD\npm.cmd" set "MOMO_NODE=%%~fD"
)
if defined MOMO_NODE set "PATH=%MOMO_NODE%;%PATH%"
if exist "%MOMO_WORKSPACE%\.tools\cargo\bin\cargo.exe" (
  set "CARGO_HOME=%MOMO_WORKSPACE%\.tools\cargo"
  set "RUSTUP_HOME=%MOMO_WORKSPACE%\.tools\rustup"
  set "PATH=%MOMO_WORKSPACE%\.tools\cargo\bin;%PATH%"
)
if exist "%MOMO_WORKSPACE%\.tools\msvc\VC\Auxiliary\Build\vcvars64.bat" (
  call "%MOMO_WORKSPACE%\.tools\msvc\VC\Auxiliary\Build\vcvars64.bat" >nul
  if errorlevel 1 exit /b 1
)
where npm.cmd >nul 2>&1
if errorlevel 1 (
  echo Node.js/npm was not found. Install Node.js or restore the portable Node folder in .tools.
  exit /b 1
)
set "npm_config_cache=%MOMO_WORKSPACE%\.tools\npm-cache"
set "TEMP=%MOMO_WORKSPACE%\.tools\tmp"
set "TMP=%TEMP%"
if not exist "%TEMP%" mkdir "%TEMP%"
if not exist "%TEMP%" exit /b 1
pushd "%~dp0"
if errorlevel 1 exit /b 1
call npm.cmd %*
set "MOMO_EXIT=%ERRORLEVEL%"
popd
exit /b %MOMO_EXIT%
