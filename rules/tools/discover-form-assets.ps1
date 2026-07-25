param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[0-9A-Za-z-]+$')]
    [string]$FormCode,

    [string[]]$Roots = @(
        'C:\Users\uriah\AppData\Local\Temp',
        'C:\eBIRForms\savefile',
        'C:\eBIRForms\IAF_RDO_Copy',
        'C:\Mac\Home\Downloads\forms'
    )
)

$ErrorActionPreference = 'Stop'

function ConvertTo-SafePath([string]$Path) {
    $parts = $Path -split '[\\/]'
    (($parts | ForEach-Object {
        if ($_ -match '@') { '#email-redacted#' } else { $_ }
    }) -join '\')
}

$assetMatches = [Collections.Generic.List[object]]::new()
foreach ($root in $Roots) {
    if (-not (Test-Path -LiteralPath $root -PathType Container)) {
        continue
    }

    Get-ChildItem -LiteralPath $root -Recurse -File -ErrorAction SilentlyContinue |
        Where-Object {
            $_.Name -match [regex]::Escape($FormCode) -or
            $_.DirectoryName -match [regex]::Escape($FormCode)
        } |
        ForEach-Object {
            $match = [pscustomobject][ordered]@{
                root = $root
                path = ConvertTo-SafePath $_.FullName
                length = $_.Length
                sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            }
            [void]$assetMatches.Add($match)
        }
}

[pscustomobject][ordered]@{
    form_code = $FormCode
    match_count = $assetMatches.Count
    matches = $assetMatches
} | ConvertTo-Json -Depth 4
