$ErrorActionPreference='Continue'
$env:PATH="C:\Users\Administrator\.rustup\toolchains\1.99.0-x86_64-pc-windows-msvc\bin;C:\Users\Administrator\BuwizNsh\tools\bin;C:\Strawberry\c\bin;C:\Strawberry\perl\site\bin;C:\Strawberry\perl\bin;" + $env:PATH
$env:RUSTC_WRAPPER=""
$env:CARGO_INCREMENTAL="0"
$env:CARGO_TARGET_DIR="C:\Users\Administrator\370main\buwiz-forms\target"
$env:LIBCLANG_PATH="C:\Program Files\LLVM\bin"
$env:RUSTUP_TOOLCHAIN="1.99.0"
$env:BUWIZ_NO_KEYCHAIN="1"; $env:EBIR_TEST_ENV="1"; $env:BIR_AUTH_BYPASS="1"; $env:BIR_SFTP_DRY_RUN="1"
$env:GPUI_AGENT_TOKEN="win3proof370token"
Set-Location C:\Users\Administrator\370main\buwiz-forms-fix
just nsh-run-agent windows "C:\Users\Administrator\370fix-scratch.db" *> C:\Users\Administrator\370fix-run.log
