param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9A-Za-z-]+$')]
    [string]$FormCode,

    [ValidateSet('form', 'help')]
    [string]$Kind = 'form',

    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 1000000)]
    [int]$StartLine,

    [Parameter(Mandatory = $true)]
    [ValidateRange(1, 1000000)]
    [int]$EndLine
)

$ErrorActionPreference = 'Stop'
if ($EndLine -lt $StartLine) {
    throw 'EndLine must not precede StartLine.'
}

$name = if ($Kind -eq 'form') { "BIR-Form$FormCode.hta" } else { "Help$FormCode.hta" }
$candidates = @(
    Get-ChildItem -LiteralPath 'C:\Users\uriah\AppData\Local\Temp' `
        -Recurse -File -Filter $name -ErrorAction SilentlyContinue
)
if ($candidates.Count -ne 1) {
    throw "Expected one extracted $Kind HTA for $FormCode; found $($candidates.Count)."
}

$lines = Get-Content -LiteralPath $candidates[0].FullName
$last = [Math]::Min($EndLine, $lines.Count)
for ($lineNumber = $StartLine; $lineNumber -le $last; $lineNumber++) {
    '{0}:{1}' -f $lineNumber, $lines[$lineNumber - 1]
}
