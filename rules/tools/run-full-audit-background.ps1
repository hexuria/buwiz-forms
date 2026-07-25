param(
    [string]$RepoRoot = 'R:\',
    [string]$Label = 'current'
)

$ErrorActionPreference = 'Stop'
if ($Label -notmatch '^[0-9A-Za-z-]+$') {
    throw 'Label must contain only ASCII letters, digits, and hyphens.'
}

$tempRoot = 'C:\Users\uriah\AppData\Local\Temp'
$stdoutPath = Join-Path $tempRoot "bir-rules-$Label-full-audit.out.log"
$stderrPath = Join-Path $tempRoot "bir-rules-$Label-full-audit.err.log"
foreach ($path in @($stdoutPath, $stderrPath)) {
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        Remove-Item -LiteralPath $path -Force
    }
}

$arguments = @(
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-File',
    (Join-Path $RepoRoot 'rules\validate.ps1'),
    '-RequireJsonSchema'
)
$process = Start-Process `
    -FilePath 'powershell.exe' `
    -ArgumentList $arguments `
    -WorkingDirectory $RepoRoot `
    -WindowStyle Hidden `
    -RedirectStandardOutput $stdoutPath `
    -RedirectStandardError $stderrPath `
    -PassThru

[pscustomobject][ordered]@{
    pid = $process.Id
    stdout = $stdoutPath
    stderr = $stderrPath
} | ConvertTo-Json
