@echo off
rem Windows counterpart of cargo-with-agent.sh: stands in for `cargo` while nsh builds the app's generated hosts, so
rem they carry the app's `agent` feature. Only a `cargo build` in the generated workspace (nsh\.nsh\rust) is changed;
rem every other command passes through. Used by `just nsh-run-agent` on Windows; nothing else sets %CARGO%.
setlocal
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
if not "!a:.nsh\rust\Cargo.toml=!"=="!a!" set "hit=!a!"
shift
goto scan
:scanned
endlocal & set "manifest=%hit%"
if not defined manifest goto pass
rem The plug's crates name hexuria/nsh as Git dependencies. The generated workspace is its own workspace root, so it
rem has to redirect them to the app's checkout itself, or the host links a second NSH and the window's listener never
rem starts (no error). nsh writes that table since the pinned commit: it carries the app's own [patch] tables over.
rem An older nsh does not, so stop here instead of building that.
findstr /L /C:"patch.\"https://github.com/hexuria/nsh\"" "%manifest%" >nul 2>&1
if not errorlevel 1 goto build
echo cargo-with-agent: "%manifest%" has no [patch."https://github.com/hexuria/nsh"] table. 1>&2
echo   This nsh is older than the app's NSH pin (NSH_PINNED_REF in .github\workflows\nsh.yml). 1>&2
echo   Rebuild it from the sibling checkout at that commit: 1>&2
echo   cargo install --locked --path ..\nsh\crates\nsh-cli --root %%USERPROFILE%%\BuwizNsh\tools 1>&2
exit /b 1
:build
"%real%" %* --features app_nsh_app_ebirforms/agent
exit /b %errorlevel%
:pass
"%real%" %*
exit /b %errorlevel%
