@echo off
rem Windows counterpart of cargo-with-agent.sh: stands in for `cargo` while nsh builds the app's generated hosts, so
rem they carry the app's `agent` feature. Only a `cargo build` in the generated workspace (nsh\.nsh\rust) is changed;
rem every other command passes through. Used by `just nsh-run-agent` on Windows; nothing else sets %CARGO%.
setlocal
rem The .sh is the one implementation: it also re-applies the [patch."https://github.com/hexuria/nsh"] block the
rem generated workspace needs, or the plug links a second NSH and install_window_plane registers on the wrong copy.
rem Git Bash ships with Git for Windows, which any Rust checkout already needs.
where bash >nul 2>&1
if not errorlevel 1 (
    bash "%~dp0cargo-with-agent.sh" %*
    exit /b %errorlevel%
)
rem Fallback when bash is unavailable: feature-append only, which leaves the split-NSH bug this wrapper exists
rem to fix, so builds here need Git Bash on PATH.
set "real=%NSH_REAL_CARGO%"
if not defined real set "real=cargo"
if /I not "%~1"=="build" goto pass
rem Each argument is read without its quotes into a quoted assignment and compared with delayed expansion, so a path
rem with & or a space is only text. nsh names the manifest with either slash, even both (...\nsh\.nsh/rust\Cargo.toml).
set "hit="
setlocal enabledelayedexpansion
:scan
if "%~1"=="" goto scanned
set "a=%~1"
set "a=!a:/=\!"
if not "!a:.nsh\rust\Cargo.toml=!"=="!a!" set "hit=1"
shift
goto scan
:scanned
endlocal & set "hit=%hit%"
if not defined hit goto pass
"%real%" %* --features app_nsh_app_ebirforms/agent
exit /b %errorlevel%
:pass
"%real%" %*
exit /b %errorlevel%
