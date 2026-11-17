param(
    [Parameter(Mandatory = $true)]
    [string]$HtaPath
)

$ErrorActionPreference = 'Stop'
$text = [IO.File]::ReadAllText($HtaPath)
$pattern = '(?i)(?:id=[''"]|getElementById\([''"])(?<prefix>frm1701MS:[^''"]+?)[''"]?\s*\+\s*i'
$matches = @(foreach ($match in [regex]::Matches($text, $pattern)) {
    [pscustomobject][ordered]@{
        line = 1 + [regex]::Matches($text.Substring(0, $match.Index), "`n").Count
        prefix = $match.Groups['prefix'].Value
    }
})
$families = @($matches | Group-Object prefix | Sort-Object Name | ForEach-Object {
    [pscustomobject][ordered]@{
        prefix = $_.Name
        occurrence_count = $_.Count
        source_lines = @($_.Group.line | Sort-Object -Unique)
    }
})

[ordered]@{
    source_sha256 = (Get-FileHash -LiteralPath $HtaPath -Algorithm SHA256).Hash.ToLowerInvariant()
    occurrence_count = @($matches).Count
    unique_prefix_count = $families.Count
    families = $families
} | ConvertTo-Json -Depth 5
