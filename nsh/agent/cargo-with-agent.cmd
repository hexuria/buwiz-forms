@echo off
rem Windows counterpart of cargo-with-agent.sh: stands in for `cargo` while nsh builds the app's generated hosts.
rem Used by `just nsh-run-agent` on Windows; nothing else sets %CARGO%.
rem
rem The .sh is the one implementation, run here through Git Bash: it adds the app's `agent` feature and re-applies
rem the [patch."https://github.com/hexuria/nsh"] block the generated workspace needs. Without that block the plug
rem links a second NSH, its surface hook registers on the wrong copy, and the window's listener never starts.
rem
rem Git Bash is found from where git.exe is, never from PATH: on Windows the first `bash` on PATH is usually
rem C:\Windows\System32\bash.exe, which starts WSL. NSH_AGENT_BASH names another one.
setlocal
set "bash=%NSH_AGENT_BASH%"
if defined bash goto run
for /f "delims=" %%G in ('where git 2^>nul') do if not defined bash call :from_git "%%~dpG"
if defined bash goto run
for %%P in ("%ProgramFiles%\Git" "%ProgramFiles(x86)%\Git" "%LocalAppData%\Programs\Git") do if not defined bash if exist "%%~P\bin\bash.exe" set "bash=%%~P\bin\bash.exe"
if defined bash goto run
echo cargo-with-agent: Git Bash not found. Install Git for Windows, or set NSH_AGENT_BASH to a bash.exe that is not WSL's. 1>&2
exit /b 1

:run
if not exist "%bash%" (
    echo cargo-with-agent: no bash at "%bash%" 1>&2
    exit /b 1
)
"%bash%" "%~dp0cargo-with-agent.sh" %*
exit /b %errorlevel%

rem git.exe sits in <Git>\cmd, <Git>\bin or <Git>\mingw64\bin; bash.exe is <Git>\bin\bash.exe.
:from_git
for %%D in ("%~1.." "%~1..\..") do if not defined bash if exist "%%~fD\bin\bash.exe" set "bash=%%~fD\bin\bash.exe"
exit /b 0
