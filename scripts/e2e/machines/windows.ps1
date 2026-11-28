# The smoke suite, then rdo-row-press (the SearchPick combobox under a person's presses, #425/#458), in a real Windows window. gpui-agent has no Windows window screenshots (screenshot_unavailable):
# record the run with the machine's own recorder.
$ErrorActionPreference = 'Stop'
Set-Location (Resolve-Path "$PSScriptRoot\..\..\..")
. (Join-Path $PSScriptRoot '..\..\lib\Prebuilt.ps1')
foreach ($tool in 'just', 'gpui-agent', 'python') {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) { throw "e2e: $tool is not installed: run scripts/setup-windows.ps1" }
}
$gpuiAgentVersion = Get-PrebuiltInstalledVersion 'gpui-agent-cli' 'gpui-agent'
if (-not $gpuiAgentVersion) { $gpuiAgentVersion = 'unknown' }
"app $(git rev-parse --short=8 HEAD), gpui-agent-cli $gpuiAgentVersion"
just e2e check smoke
if ($LASTEXITCODE) { exit $LASTEXITCODE }
just e2e check rdo-row-press
if ($LASTEXITCODE) { exit $LASTEXITCODE }
# A dry-run test build only: the runner refuses the Queue recipe without these and --allow-send.
$env:EBIR_TEST_ENV = '1'; $env:BIR_SFTP_DRY_RUN = '1'; Remove-Item Env:BIR_SFTP_LIVE -ErrorAction SilentlyContinue
just e2e windows smoke --allow-send @args
$status = $LASTEXITCODE
# Both suites run; the run fails if either does.
just e2e windows rdo-row-press @args
if ($LASTEXITCODE) { $status = $LASTEXITCODE }
exit $status
