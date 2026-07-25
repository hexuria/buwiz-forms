param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\0605'
)

$ErrorActionPreference = 'Stop'
$formId = '0605-v2003'
$revision = '2003-09-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form0605.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help0605.hta'
$atcPath = Join-Path $ExtractedRoot 'xml\atcCodes.xml'
$taxTypePath = Join-Path $ExtractedRoot 'xml\taxTypeCodes.xml'
$pdfPath = Join-Path $OfficialDir '0605version1999_09.02.2022_copy.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'

$expected = @{
    hta = '6200b57ef2a3d9d296dc79cbdb9d2f7a605fb92d1dfcdcc4883f08de7c7fa324'
    help = 'b820994a937a73f3299a352f6b2a69759d4cbc0b18e583221a1bff5013dc40a2'
    atc = '16e4db6ce456a6fb0a97f085cf8ef19349c2f6fc183971e97d4e253f63cdd22b'
    tax_type = '496fd8e64b8854f1012a314b3d8576518fafd081b4e4726f2ea2f05ea6e3a72b'
    pdf = 'de04419766c59bf27fdeb854c0f7c3f98601900caa20630442e671e2313e536f'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher_a = '09cd3626efd6a7490b5922c9dbb6fad98b0b066ffb5de87c3ea6a6677210620f'
    cipher_b = 'c53a196dcfe1fb585fefc7b48c2a4f2abe9ec9114d55541e44a40e4399c39928'
    decrypted_a = '597e0b7496c1bdd9ecc0e4b3a7c2d27a79f48e2cdf0b9a4fb61354ead5e27476'
    decrypted_b = '381fd9c88d9339c5b329bff3a2ac4bd0006d326cf6646b50c3ad2617c7e7401c'
    plain_a = '01992fcdaef50493e756b89728af8d107ec1a0cafa94e677edbac1e2f08dc499'
    plain_b = 'f8659d2011d2914073725ccef1fc4f2e74d4f315bf333d5ec3084a1fdff524f7'
    inventory = '50600e48510a3ea1bbc6c7bf533fbe336c04af7a87700edd503e7a96fa51177d'
}

function Get-AttributeValue([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { return $match.Groups[2].Value }
    return $null
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

foreach ($asset in @(
    @($htaPath, 'hta'),
    @($helpPath, 'help'),
    @($atcPath, 'atc'),
    @($taxTypePath, 'tax_type'),
    @($pdfPath, 'pdf'),
    @($packagePath, 'package')
)) {
    if (-not (Test-Path -LiteralPath $asset[0] -PathType Leaf)) {
        throw "Missing official asset: $($asset[0])"
    }
    if ((Get-Sha256 $asset[0]) -ne $expected[$asset[1]]) {
        throw "Official asset hash changed: $($asset[0])"
    }
}

$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch 'APPLICATIONNAME="0605"' -or $hta -notmatch 'September 2003\(ENCS\)') {
    throw 'September 2003 runtime binding changed.'
}
if ($help -notmatch '(?i)Payment Form') { throw 'Packaged help binding changed.' }
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') {
    throw 'September 1999 reference PDF magic mismatch.'
}

$sampleFiles = @(Get-ChildItem -LiteralPath $OfficialDir -File -Filter '*.xml')
$sampleByHash = @{}
foreach ($sampleFile in $sampleFiles) {
    $sampleByHash[(Get-Sha256 $sampleFile.FullName)] = $sampleFile
}
foreach ($hashName in @('cipher_a','cipher_b','plain_a','plain_b')) {
    if (-not $sampleByHash.ContainsKey($expected[$hashName])) {
        throw "Pinned 0605 sample is missing: $hashName"
    }
}

$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJsonA = & $keyTool `
    -SourcePath $sampleByHash[$expected.cipher_a].FullName `
    -RedactedSourcePath (Join-Path $OfficialDir '0605-final-copy-a-#email-redacted#.xml') `
    -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher_a `
    -ExpectedDecryptedSha256 $expected.decrypted_a `
    -ExpectedFieldCount 235 `
    -ExpectedFieldInventorySha256 $expected.inventory
$keyJsonB = & $keyTool `
    -SourcePath $sampleByHash[$expected.cipher_b].FullName `
    -RedactedSourcePath (Join-Path $OfficialDir '0605-final-copy-b-#email-redacted#.xml') `
    -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher_b `
    -ExpectedDecryptedSha256 $expected.decrypted_b `
    -ExpectedFieldCount 235 `
    -ExpectedFieldInventorySha256 $expected.inventory
$keyAuditA = $keyJsonA | ConvertFrom-Json
$keyAuditB = $keyJsonB | ConvertFrom-Json
$keys = @($keyAuditA.keys)
if ((@($keyAuditB.keys) -join "`n") -ne ($keys -join "`n")) {
    throw 'Encrypted sample key order differs.'
}

function Get-PlainKeyAudit([string]$Path, [string]$ExpectedHash) {
    if ((Get-Sha256 $Path) -ne $ExpectedHash) { throw 'Plaintext sample hash changed.' }
    $text = [IO.File]::ReadAllText($Path)
    $plainKeys = @(
        [regex]::Matches($text, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') |
            ForEach-Object { $_.Groups['key'].Value }
    )
    if ($plainKeys.Count -ne 235 -or @($plainKeys | Sort-Object -Unique).Count -ne 235) {
        throw "Plaintext sample inventory changed: $($plainKeys.Count)."
    }
    if (($plainKeys -join "`n") -ne ($keys -join "`n")) {
        throw 'Plaintext and encrypted sample key order differs.'
    }
    [pscustomobject][ordered]@{
        sha256 = $ExpectedHash
        field_count = $plainKeys.Count
        unique_field_count = @($plainKeys | Sort-Object -Unique).Count
        field_inventory_sha256 = $expected.inventory
        values_emitted = $false
    }
}
$plainAuditA = Get-PlainKeyAudit $sampleByHash[$expected.plain_a].FullName $expected.plain_a
$plainAuditB = Get-PlainKeyAudit $sampleByHash[$expected.plain_b].FullName $expected.plain_b

$excludedRanges = @(
    @([regex]::Matches($hta, '(?is)<script\b.*?</script>')) +
    @([regex]::Matches($hta, '(?is)<!--.*?-->'))
)
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($hta, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $excluded = $false
    foreach ($range in $excludedRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) {
            $excluded = $true
            break
        }
    }
    if ($excluded) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-AttributeValue $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $kind = $kind.ToLowerInvariant()
    $defaultValue = Get-AttributeValue $tag 'value'
    if ($kind -in @('radio','checkbox')) {
        $defaultValue = if ($tag -match '(?i)\bchecked(?:\s*=|\s|>)') { 'true' } else { 'false' }
    }
    $controls.Add([pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-AttributeValue $tag 'id'
        name = Get-AttributeValue $tag 'name'
        element = $element
        control_kind = $kind
        source_line = 1 + [regex]::Matches($hta.Substring(0, $match.Index), "`n").Count
        default_value = $defaultValue
        maxlength = Get-AttributeValue $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) {
        $controlById[$control.id] = $control
    }
}
$staticMatches = @($keys | Where-Object { $controlById.ContainsKey($_) })
$runtimeRdo = @($keys | Where-Object { $_ -eq 'frm0605:txtRDOCode' })
$runtimeAtc = @($keys | Where-Object { $_ -match '^AtcCode(?:[1-9]|[1-9]\d|1[0-3]\d|14[0-2])$' })
$runtimeTaxTypes = @($keys | Where-Object { $_ -match '^TaxTypeCode(?:[1-9]|[12]\d|3[0-7])$' })
$unexplained = @(
    $keys | Where-Object {
        -not $controlById.ContainsKey($_) -and
        $_ -ne 'frm0605:txtRDOCode' -and
        $_ -notmatch '^AtcCode(?:[1-9]|[1-9]\d|1[0-3]\d|14[0-2])$' -and
        $_ -notmatch '^TaxTypeCode(?:[1-9]|[12]\d|3[0-7])$'
    }
)

$discovery = [pscustomobject][ordered]@{
    form_id = $formId
    revision = $revision
    live_static_controls = $controls.Count
    sample_keys = $keys.Count
    static_sample_matches = $staticMatches.Count
    runtime_rdo_matches = $runtimeRdo.Count
    runtime_atc_lookup_matches = $runtimeAtc.Count
    runtime_tax_type_lookup_matches = $runtimeTaxTypes.Count
    unexplained_sample_keys = $unexplained.Count
    two_encrypted_inventories_identical = $true
    two_plaintext_inventories_identical = $true
    next = 'Generate complete revision-specific package.'
}

$outDir = Join-Path $RepoRoot 'rules\forms\0605-v2003'
$fixtureDir = Join-Path $outDir 'fixtures'
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

function Write-JsonFile([string]$Path, $Value) {
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )
}

function Write-TextFile([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}

function Get-LineInventoryHash([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $bytes = [Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))
        return ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally {
        $sha.Dispose()
    }
}

function New-Asset(
    [string]$AssetId,
    [string]$Kind,
    [string]$Path,
    [string]$RevisionBinding,
    [string]$DisplayPath = ''
) {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $AssetId
        kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = Get-Sha256 $Path
        size = $item.Length
        revision_binding = $RevisionBinding
    }
}

Write-TextFile (Join-Path $fixtureDir 'encrypted-field-audit-a-v796.json') (
    $keyJsonA -join [Environment]::NewLine
)
Write-TextFile (Join-Path $fixtureDir 'encrypted-field-audit-b-v796.json') (
    $keyJsonB -join [Environment]::NewLine
)
Write-JsonFile (Join-Path $fixtureDir 'plaintext-field-audits-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    values_emitted = $false
    samples = @(
        [ordered]@{
            sample_id = 'plain-a'
            source_path = (Join-Path $OfficialDir '0605-save-a-#email-redacted#.xml')
            audit = $plainAuditA
        },
        [ordered]@{
            sample_id = 'plain-b'
            source_path = (Join-Path $OfficialDir '0605-save-b-#email-redacted#.xml')
            audit = $plainAuditB
        }
    )
})

$atcCatalog = [Collections.Generic.List[object]]::new()
$atcText = [IO.File]::ReadAllText($atcPath)
foreach ($match in [regex]::Matches($atcText, '(?is)<div>atc(?<index>\d+):(?<payload>.*?)atc\k<index>:</div>')) {
    $parts = $match.Groups['payload'].Value -split '~', 10
    if ($parts.Count -ge 10 -and $parts[9] -match '0605') {
        $atcCatalog.Add([pscustomobject][ordered]@{
            source_index = [int]$match.Groups['index'].Value
            code = $parts[0]
            description = [Net.WebUtility]::HtmlDecode($parts[1]).Trim()
            rate = $parts[2]
            category = $parts[3]
            form_binding = '0605'
        })
    }
}
$taxTypeCatalog = [Collections.Generic.List[object]]::new()
$taxTypeText = [IO.File]::ReadAllText($taxTypePath)
foreach ($match in [regex]::Matches($taxTypeText, '(?is)<div>taxTypeCode(?<index>\d+):(?<payload>.*?)taxTypeCode\k<index>:</div>')) {
    $parts = $match.Groups['payload'].Value -split '~', 3
    if ($parts.Count -eq 3 -and $parts[2] -match '0605') {
        $taxTypeCatalog.Add([pscustomobject][ordered]@{
            source_index = [int]$match.Groups['index'].Value
            code = $parts[0]
            description = [Net.WebUtility]::HtmlDecode($parts[1]).Trim()
            form_binding = '0605'
            runtime_reachable = [int]$match.Groups['index'].Value -le 37
        })
    }
}
if ($atcCatalog.Count -ne 142 -or $taxTypeCatalog.Count -ne 39 -or @($taxTypeCatalog | Where-Object runtime_reachable).Count -ne 37) {
    throw "0605 catalog binding changed: ATC/tax-type $($atcCatalog.Count)/$($taxTypeCatalog.Count)."
}
Write-JsonFile (Join-Path $fixtureDir 'atc-catalog-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_sha256 = $expected.atc
    count = $atcCatalog.Count
    entries = $atcCatalog
})
Write-JsonFile (Join-Path $fixtureDir 'tax-type-catalog-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_sha256 = $expected.tax_type
    declared_runtime_count = 37
    physical_form_bound_count = $taxTypeCatalog.Count
    runtime_reachable_count = @($taxTypeCatalog | Where-Object runtime_reachable).Count
    unreachable_count = @($taxTypeCatalog | Where-Object { -not $_.runtime_reachable }).Count
    entries = $taxTypeCatalog
})

$selectEnums = @{}
foreach ($selectMatch in [regex]::Matches($hta, '(?is)<select\b(?<open>[^>]*)>(?<body>.*?)</select>')) {
    $isExcluded = $false
    foreach ($range in $excludedRanges) {
        if ($selectMatch.Index -ge $range.Index -and $selectMatch.Index -lt ($range.Index + $range.Length)) {
            $isExcluded = $true
            break
        }
    }
    if ($isExcluded) { continue }
    $selectId = Get-AttributeValue $selectMatch.Groups['open'].Value 'id'
    if (-not $selectId) { continue }
    $values = [Collections.Generic.List[object]]::new()
    foreach ($option in [regex]::Matches($selectMatch.Groups['body'].Value, '(?is)<option\b(?<open>[^>]*)>(?<text>.*?)</option>')) {
        $values.Add([pscustomobject][ordered]@{
            value = Get-AttributeValue $option.Groups['open'].Value 'value'
            label = [Net.WebUtility]::HtmlDecode(
                [regex]::Replace($option.Groups['text'].Value, '<[^>]+>', '')
            ).Trim()
        })
    }
    $selectEnums[$selectId] = @($values)
}

$requiredKeys = @(
    'frm0605:itemFiscalStartMonth:_1',
    'frm0605:itemFiscalStartMonth:_2',
    'frm0605:txtYearEnded',
    'frm0605:txtDueDateMonth',
    'frm0605:txtDueDateDay',
    'frm0605:txtDueDateYear',
    'txtATCCode',
    'frm0605:txtReturnPeriodMonth',
    'frm0605:txtReturnPeriodDay',
    'frm0605:txtReturnPeriodYear',
    'txtTaxTypeCode',
    'frm0605:txtTIN1',
    'frm0605:txtTIN2',
    'frm0605:txtTIN3',
    'frm0605:txtBranchCode',
    'frm0605:txtRDOCode',
    'frm0605:txtClassification:_1',
    'frm0605:txtClassification:_2',
    'frm0605:txtLineBus',
    'frm0605:txtTaxPayerName',
    'frm0605:txtTelNum',
    'frm0605:txtAddress',
    'frm0605:txtZipCode',
    'frm0605:txtTax19'
)
$computedKeys = @('frm0605:txtTax20D','frm0605:txtTax21')

function Get-ItemNumber([string]$Key) {
    if ($Key -match 'itemFiscalStartMonth') { return '1' }
    if ($Key -match 'itemYearEndMonth|txtYearEnded') { return '2' }
    if ($Key -match '^itemQuarter_') { return '3' }
    if ($Key -match 'txtDueDate') { return '4' }
    if ($Key -eq 'txtATCCode') { return '6' }
    if ($Key -match 'txtReturnPeriod') { return '7' }
    if ($Key -eq 'txtTaxTypeCode') { return '8' }
    if ($Key -match 'txtTIN|txtBranchCode') { return '9' }
    if ($Key -match 'txtRDOCode') { return '10' }
    if ($Key -match 'txtClassification') { return '11' }
    if ($Key -match 'txtLineBus') { return '12' }
    if ($Key -match 'txtTaxPayerName') { return '13' }
    if ($Key -match 'txtTelNum') { return '14' }
    if ($Key -match 'txtAddress') { return '15' }
    if ($Key -match 'txtZipCode') { return '16' }
    if ($Key -match 'itemMannerOfPayment|txtOthersName|itemApprovedYN') { return '17' }
    if ($Key -match 'itemModeOfPayment|txtNumOfInstallment') { return '18' }
    if ($Key -match 'txtTax19') { return '19' }
    if ($Key -match 'txtTax20') { return '20' }
    if ($Key -match 'txtTax21') { return '21' }
    if ($Key -match '^AtcCode') { return 'ATC lookup catalog' }
    if ($Key -match '^TaxTypeCode') { return 'Tax-type lookup catalog' }
    return $null
}

function New-Field([string]$Key) {
    $control = if ($controlById.ContainsKey($Key)) { $controlById[$Key] } else { $null }
    $kind = if ($control) {
        $control.control_kind
    }
    elseif ($Key -eq 'frm0605:txtRDOCode') {
        'runtime-generated-select'
    }
    else {
        'hidden'
    }
    $logicalType = if ($kind -in @('radio','checkbox')) {
        'boolean'
    }
    elseif ($Key -match '(?i)(ATC|TaxTypeCode|TIN|Branch|RDO|Zip|Month|Day|Year|Quarter)') {
        'code'
    }
    elseif ($Key -match '(?i)(txtTax|Installment)') {
        'decimal-amount'
    }
    else {
        'string'
    }
    $computed = $computedKeys -contains $Key
    $required = if ($computed) {
        'computed'
    }
    elseif ($requiredKeys -contains $Key) {
        'required'
    }
    else {
        'optional'
    }
    $requiredWhen = $null
    if ($Key -eq 'frm0605:txtNumOfInstallment') {
        $required = 'conditional'
        $requiredWhen = 'Installment mode of payment is selected.'
    }
    elseif ($Key -match 'itemApprovedYN') {
        $required = 'conditional'
        $requiredWhen = 'Preliminary/final assessment or deficiency-tax manner is selected.'
    }
    elseif ($Key -eq 'frm0605:txtOthersName') {
        $required = 'conditional'
        $requiredWhen = 'Other manner of payment is selected; the official source fails to enforce this.'
    }
    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') {
        $constraints.max_length = [int]$control.maxlength
    }
    if ($logicalType -eq 'decimal-amount') {
        $constraints.precision = 2
        $constraints.sign = if ($Key -eq 'frm0605:txtTax19') { 'strictly positive at Validate' } else { 'nonnegative/source-dependent' }
    }
    if ($Key -eq 'frm0605:txtNumOfInstallment') {
        $constraints.minimum = 1
        $constraints.maximum = 20
    }
    $enumValues = [object[]]::new(0)
    if ($kind -in @('radio','checkbox')) {
        $enumValues = [object[]]@('true','false')
    }
    elseif ($selectEnums.ContainsKey($Key)) {
        $enumValues = [object[]]@($selectEnums[$Key])
    }
    $normalization = [string[]]::new(0)
    if ($logicalType -eq 'decimal-amount') {
        $normalization = [string[]]@('parseFloat().toFixed(2)','NumWithComma','formatCurrency')
    }
    $sourceLine = if ($control) {
        $control.source_line
    }
    elseif ($Key -eq 'frm0605:txtRDOCode') {
        2555
    }
    elseif ($Key -match '^AtcCode') {
        1612
    }
    else {
        1631
    }
    $notes = [Collections.Generic.List[string]]::new()
    $notes.Add('Present in all four revision-matched dummy save/final-copy inventories; values excluded.')
    if ($Key -match '^AtcCode') {
        $notes.Add('Runtime lookup payload loaded from the hash-pinned ATC catalog; serialized incidentally with the form.')
    }
    elseif ($Key -match '^TaxTypeCode') {
        $notes.Add('Runtime lookup payload loaded from the hash-pinned tax-type catalog; serialized incidentally with the form.')
    }
    [pscustomobject][ordered]@{
        field_key = $Key
        serialized_key = $Key
        serialized_occurrence = 1
        label = if ($Key -match '^AtcCode(?<n>\d+)$') {
            "ATC lookup entry $($Matches.n)"
        }
        elseif ($Key -match '^TaxTypeCode(?<n>\d+)$') {
            "Tax-type lookup entry $($Matches.n)"
        }
        else {
            $Key
        }
        page = 1
        item_number = Get-ItemNumber $Key
        control_kind = $kind
        storage_type = 'string'
        logical_type = $logicalType
        required = $required
        required_when = $requiredWhen
        enabled_when = if ($Key -eq 'frm0605:txtNumOfInstallment') {
            'Installment mode is selected.'
        }
        elseif ($Key -match 'itemApprovedYN') {
            'Assessment/deficiency-tax manner is selected.'
        }
        elseif ($Key -eq 'frm0605:txtOthersName') {
            'Other manner is selected.'
        }
        else {
            $null
        }
        visible_when = $null
        default_value = if ($control) { $control.default_value } else { '' }
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enumValues
        normalization = $normalization
        computed = $computed
        calculation_id = if ($computed) { 'See calculations.json' } else { $null }
        source_refs = @(
            "xml-encrypted-a#decrypted-field:$Key",
            "xml-encrypted-b#decrypted-field:$Key",
            "official-hta-runtime#control:L$sourceLine"
        )
        confidence = 'high'
        notes = @($notes)
    }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $fields.Add((New-Field $key))
}
if ($fields.Count -ne 235) {
    throw "0605 typed inventory changed: expected 235, found $($fields.Count)."
}
Write-JsonFile (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = $fields.Count
    inventory_sha256 = Get-LineInventoryHash @($fields.field_key | Sort-Object)
    fields = $fields
})
Write-JsonFile (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    live_static_control_count = $controls.Count
    sample_key_count = $keys.Count
    static_sample_match_count = $staticMatches.Count
    runtime_rdo_match_count = $runtimeRdo.Count
    runtime_atc_lookup_match_count = $runtimeAtc.Count
    runtime_tax_type_lookup_match_count = $runtimeTaxTypes.Count
    unexplained_sample_key_count = $unexplained.Count
    controls = $controls
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-TextFile (Join-Path $fixtureDir 'validation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm0605:' `
        -NamePattern '(?i)valid|check|save|date|submit|final|atc|tax') -join [Environment]::NewLine
)
Write-TextFile (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm0605:' `
        -NamePattern '(?i)comput|total|tax|penalt|amount|format') -join [Environment]::NewLine
)

$rules = [Collections.Generic.List[object]]::new()
function Add-Rule(
    [string]$Id,
    [string]$Phase,
    [int]$Order,
    [string]$Condition,
    [string[]]$FieldKeys,
    $ExactMessage,
    [string[]]$SourceRefs,
    [string]$Assessment = 'verified-correct',
    [string]$OfficialBehavior = 'The branch alerts and stops the active operation.',
    [string]$RecommendedBehavior = 'Retain as a structured revision-aware error.'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id = $Id
        form_id = $formId
        revision = $revision
        phase = $Phase
        order = $Order
        condition = $Condition
        fields = $FieldKeys
        accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $ExactMessage
        source_refs = $SourceRefs
        evidence_type = @('source')
        assessment = $Assessment
        official_behavior = $OfficialBehavior
        recommended_app_behavior = $RecommendedBehavior
        confidence = 'high'
        unresolved_questions = @()
    })
}

Add-Rule '0605-input-001-amount-format' input 1 `
    'An amount wired to blockletter is nonnumeric on blur.' @('amount-fields') $null `
    @('official-hta-runtime#blockletter:L2566-L2578') 'verified-correct' `
    'The source silently replaces the value with 0.00; otherwise it applies parseFloat(...).toFixed(2).' `
    'Use typed decimal parsing and expose invalid input rather than silently converting it to zero.'
Add-Rule '0605-input-002-calendar-year-end' 'blur/change' 2 `
    'Calendar Year is selected.' @('frm0605:itemFiscalStartMonth:_1','frm0605:itemYearEndMonth') $null `
    @('official-hta-runtime#dateyear:L3152-L3159') 'verified-correct' `
    'Year-end month is forced to December and disabled.' `
    'Preserve the dependency explicitly.'
Add-Rule '0605-input-003-fiscal-december' 'blur/change' 3 `
    'Fiscal Year is selected while year-end month is December.' `
    @('frm0605:itemFiscalStartMonth:_2','frm0605:itemYearEndMonth') `
    'You have entered invalid month for Fiscal Year' `
    @('official-hta-runtime#dateyear:L3159-L3165')
Add-Rule '0605-input-004-calendar-nondecember' 'blur/change' 4 `
    'Calendar Year is selected and year-end month changes away from December.' `
    @('frm0605:itemFiscalStartMonth:_1','frm0605:itemFiscalStartMonth:_2','frm0605:itemYearEndMonth') `
    'You have entered a filing year not ending in December. This filing will be considered as a Fiscal Year Filing.' `
    @('official-hta-runtime#datemonth:L3166-L3173') 'official-bug-compatible' `
    'The source automatically checks Fiscal Year after the alert.' `
    'Ask for confirmation before changing the filing-period classification.'
Add-Rule '0605-input-005-fiscal-december-switch' 'blur/change' 5 `
    'Fiscal Year is selected and year-end month changes to December.' `
    @('frm0605:itemFiscalStartMonth:_1','frm0605:itemFiscalStartMonth:_2','frm0605:itemYearEndMonth') `
    'You have entered a filing year ending in December. This filing will be considered as a Calendar Year Filing.' `
    @('official-hta-runtime#datemonth:L3173-L3179') 'official-bug-compatible' `
    'The source automatically checks Calendar Year after the alert.' `
    'Ask for confirmation before changing the filing-period classification.'
Add-Rule '0605-input-006-atc-needs-tax-type' input 6 `
    'The user opens the ATC selector while tax type is empty.' @('txtTaxTypeCode','txtATCCode') `
    'Please select a valid Tax Type on Item 8.' `
    @('official-hta-runtime#showATCDiv:L2975-L2982')

Add-Rule '0605-save-007-return-period' save 7 `
    'Any return-period component is blank.' `
    @('frm0605:txtReturnPeriodMonth','frm0605:txtReturnPeriodDay','frm0605:txtReturnPeriodYear') `
    'Please enter a valid Return Period on Item 7.' `
    @('official-hta-runtime#initialValidateBeforeSave:L3180-L3187')
Add-Rule '0605-save-008-tin' save 8 `
    'Any TIN segment or branch code is blank.' `
    @('frm0605:txtTIN1','frm0605:txtTIN2','frm0605:txtTIN3','frm0605:txtBranchCode') `
    'Please enter a valid TIN number on Item 9.' `
    @('official-hta-runtime#initialValidateBeforeSave:L3188-L3194')
Add-Rule '0605-save-009-rdo' save 9 `
    'RDO value is 000.' @('frm0605:txtRDOCode') `
    'Please enter a valid RDO Code on Item 10.' `
    @('official-hta-runtime#initialValidateBeforeSave:L3195-L3199')
Add-Rule '0605-save-010-name' save 10 `
    'Taxpayer name is blank.' @('frm0605:txtTaxPayerName') `
    'Please enter a valid Taxpayer Name on Item 13.' `
    @('official-hta-runtime#initialValidateBeforeSave:L3200-L3204')
Add-Rule '0605-save-011-sparse' save 11 `
    'Fields beyond return period, TIN/RDO, and taxpayer name are missing or invalid.' @('all-other-form-fields') $null `
    @('official-hta-runtime#initialValidateBeforeSave:L3180-L3205') 'incorrect-official-behavior' `
    'Save does not inspect fiscal/year-end choice, due date, ATC, tax type, classification, address, payment choices, or tax amounts.' `
    'Permit incomplete drafts explicitly, but never represent sparse Save checks as full validity.'

Add-Rule '0605-validate-012-filing-basis' validate 12 `
    'Neither Calendar Year nor Fiscal Year is selected.' `
    @('frm0605:itemFiscalStartMonth:_1','frm0605:itemFiscalStartMonth:_2') `
    'Please select an option on Item 1.' @('official-hta-runtime#validate:L2621-L2628')
Add-Rule '0605-validate-013-year-ended' validate 13 `
    'Year ended is blank, below 1904, or above 3000.' @('frm0605:txtYearEnded') `
    'Please enter a valid year on Item 2.' @('official-hta-runtime#validate:L2629-L2635') `
    'incorrect-official-behavior' `
    'The September 2003 runtime permits years from 1904 through 3000 without a revision/current-period constraint.' `
    'Validate against the filing period actually supported by the selected return context.'
Add-Rule '0605-validate-014-quarter-commented' validate 14 `
    'No quarter is selected.' @('itemQuarter_1','itemQuarter_2','itemQuarter_3','itemQuarter_4') $null `
    @('official-hta-runtime#validate:L2636-L2641') 'obsolete' `
    'The entire quarter-required branch, including its alert, is commented out and unreachable.' `
    'Do not require quarter unless the payment context legally requires it; otherwise omit the dormant controls.'
Add-Rule '0605-validate-015-due-format' validate 15 `
    'Due-date month or day has exactly one character.' `
    @('frm0605:txtDueDateMonth','frm0605:txtDueDateDay') `
    'Please enter a valid day/month on item 4. Format should be MM/DD/YYYY.' `
    @('official-hta-runtime#validate:L2647-L2652')
Add-Rule '0605-validate-016-due-day-positive' validate 16 `
    'Due-date day coerces below 1, including an empty string.' @('frm0605:txtDueDateDay') `
    'Invalid date entry on item 4.' @('official-hta-runtime#validate:L2653-L2658')
Add-Rule '0605-validate-017-due-leap29' validate 17 `
    'Due date is February 29 in a non-leap year.' @('due-date-fields') `
    'Filing year is not a leap year.' @('official-hta-runtime#validate:L2661-L2665')
Add-Rule '0605-validate-018-due-february-overflow' validate 18 `
    'Due-date day exceeds the February maximum.' @('due-date-fields') `
    'Invalid date entry on item 4.' @('official-hta-runtime#validate:L2666-L2674')
Add-Rule '0605-validate-019-due-month-length' validate 19 `
    'Due-date day exceeds 31 for a 31-day month or 30 for a 30-day month.' @('due-date-fields') `
    'Invalid date entry on item 4.' @('official-hta-runtime#validate:L2675-L2679')
Add-Rule '0605-validate-020-atc' validate 20 `
    'Selected ATC is empty.' @('txtATCCode') 'Please enter a valid ATC on Item 6.' `
    @('official-hta-runtime#validate:L2680-L2685')
Add-Rule '0605-validate-021-return-required' validate 21 `
    'Any return-period component is blank.' @('return-period-fields') `
    'Please enter a valid Return Period on Item 7.' @('official-hta-runtime#validate:L2686-L2692')
Add-Rule '0605-validate-022-return-format' validate 22 `
    'Return-period month or day has exactly one character.' @('return-period-fields') `
    'Please enter a valid day/month on item 7. Format should be MM/DD/YYYY.' `
    @('official-hta-runtime#validate:L2693-L2698')
Add-Rule '0605-validate-023-return-day-positive' validate 23 `
    'Return-period day coerces below 1.' @('frm0605:txtReturnPeriodDay') `
    'Invalid date entry on item 7.' @('official-hta-runtime#validate:L2699-L2704')
Add-Rule '0605-validate-024-return-leap29' validate 24 `
    'Return period is February 29 in a non-leap year.' @('return-period-fields') `
    'Filing year is not a leap year.' @('official-hta-runtime#validate:L2705-L2711')
Add-Rule '0605-validate-025-return-february-overflow' validate 25 `
    'Return-period day exceeds the February maximum.' @('return-period-fields') `
    'Invalid date entry on item 7.' @('official-hta-runtime#validate:L2712-L2720')
Add-Rule '0605-validate-026-return-month-length' validate 26 `
    'Return-period day exceeds 31 for a 31-day month or 30 for a 30-day month.' @('return-period-fields') `
    'Invalid date entry on item 7.' @('official-hta-runtime#validate:L2721-L2730')
Add-Rule '0605-validate-027-tin-presence' validate 27 `
    'Any TIN segment or branch code is blank.' `
    @('frm0605:txtTIN1','frm0605:txtTIN2','frm0605:txtTIN3','frm0605:txtBranchCode') `
    'Please enter a valid TIN number on Item 9.' @('official-hta-runtime#validate:L2731-L2736')
Add-Rule '0605-validate-028-tin-checksum-omitted' validate 28 `
    'TIN segments are nonblank but fail the shared checksum or branch-code semantics.' `
    @('frm0605:txtTIN1','frm0605:txtTIN2','frm0605:txtTIN3','frm0605:txtBranchCode') $null `
    @('official-hta-runtime#validate:L2731-L2736') 'incorrect-official-behavior' `
    'Validate tests presence only.' 'Apply the shared evidence-backed TIN and branch-code validation.'
Add-Rule '0605-validate-029-rdo' validate 29 `
    'RDO selectedIndex is zero.' @('frm0605:txtRDOCode') `
    'Please enter a valid RDO Code on Item 10.' @('official-hta-runtime#validate:L2732-L2736')
Add-Rule '0605-validate-030-classification' validate 30 `
    'Neither taxpayer-classification radio is selected.' `
    @('frm0605:txtClassification:_1','frm0605:txtClassification:_2') `
    'Please select Taxpayer Classification on Item 11.' @('official-hta-runtime#validate:L2737-L2742')
Add-Rule '0605-validate-031-line-business' validate 31 `
    'Line of business/occupation is blank.' @('frm0605:txtLineBus') `
    'Please enter a valid Line of Business/Occupation on Item 12.' @('official-hta-runtime#validate:L2743-L2748')
Add-Rule '0605-validate-032-name' validate 32 `
    'Taxpayer name is blank.' @('frm0605:txtTaxPayerName') `
    'Please enter a valid Taxpayer Name on Item 13.' @('official-hta-runtime#validate:L2749-L2754')
Add-Rule '0605-validate-033-phone' validate 33 `
    'Telephone number is blank.' @('frm0605:txtTelNum') `
    'Please enter a valid Taxpayer Telephone Number on Item 14.' @('official-hta-runtime#validate:L2755-L2760')
Add-Rule '0605-validate-034-address' validate 34 `
    'Registered address is blank.' @('frm0605:txtAddress') `
    'Please enter a valid Taxpayer Registered Address on Item 15.' @('official-hta-runtime#validate:L2761-L2766')
Add-Rule '0605-validate-035-zip' validate 35 `
    'ZIP code is blank.' @('frm0605:txtZipCode') `
    'Please enter a valid Taxpayer Zip Code on Item 16.' @('official-hta-runtime#validate:L2767-L2772')
Add-Rule '0605-validate-036-due-month' validate 36 `
    'A partially supplied due date has a blank month or month outside 1..12.' @('frm0605:txtDueDateMonth') `
    'Please enter valid month on item 4.' @('official-hta-runtime#validate:L2765-L2775')
Add-Rule '0605-validate-037-due-day' validate 37 `
    'A supplied due date has a missing or out-of-range day.' @('frm0605:txtDueDateDay') `
    'Please enter a valid day on item 4.' @('official-hta-runtime#validate:L2776-L2805')
Add-Rule '0605-validate-038-due-date-calendar' validate 38 `
    'A supplied due date violates its month/leap-year maximum.' @('due-date-fields') `
    'Please enter a valid date on item 4.' @('official-hta-runtime#validate:L2776-L2816')
Add-Rule '0605-validate-039-due-year' validate 39 `
    'A supplied due date has blank year or year outside 1904..3000.' @('frm0605:txtDueDateYear') `
    'Please enter valid year on item 4.' @('official-hta-runtime#validate:L2817-L2825')
Add-Rule '0605-validate-040-future-due-accepted' validate 40 `
    'Due date is a valid calendar date in the future, up to year 3000.' @('due-date-fields') $null `
    @('official-hta-runtime#validate:L2765-L2825') 'incorrect-official-behavior' `
    'No future-date comparison exists.' 'Reject a payment due date that is inconsistent with the selected obligation and current filing context.'
Add-Rule '0605-validate-041-return-month' validate 41 `
    'Return-period month is blank or outside 1..12.' @('frm0605:txtReturnPeriodMonth') `
    'Please enter a valid month on item 7.' @('official-hta-runtime#validate:L2827-L2840')
Add-Rule '0605-validate-042-return-day' validate 42 `
    'Return-period day is blank or outside its non-February month maximum.' @('frm0605:txtReturnPeriodDay') `
    'Please enter a valid day on item 7.' @('official-hta-runtime#validate:L2841-L2853')
Add-Rule '0605-validate-043-return-date-calendar' validate 43 `
    'Return-period date violates the 30-day or February/leap-year maximum.' @('return-period-fields') `
    'Please enter a valid date on item 7.' @('official-hta-runtime#validate:L2846-L2878')
Add-Rule '0605-validate-044-return-year' validate 44 `
    'Return-period year is blank or outside 1904..3000.' @('frm0605:txtReturnPeriodYear') `
    'Please enter a valid year on item 7.' @('official-hta-runtime#validate:L2861-L2888')
Add-Rule '0605-validate-045-return-after-year-end' validate 45 `
    'Return-period year exceeds year ended, or equal year has month after year-end month.' `
    @('return-period-fields','frm0605:itemYearEndMonth','frm0605:txtYearEnded') `
    'The return period date should not be later than the year ended date.' @('official-hta-runtime#validate:L2889-L2898')
Add-Rule '0605-validate-046-tax-type' validate 46 `
    'Tax Type Code is empty.' @('txtTaxTypeCode') 'Please select Tax Type Code on Item 8.' `
    @('official-hta-runtime#validate:L2899-L2903')
Add-Rule '0605-validate-047-manner-payment' validate 47 `
    'None of the seven manner-of-payment radios is selected.' @('manner-of-payment-fields') `
    'Please select Manner of Payment on item 17.' @('official-hta-runtime#validate:L2904-L2908')
Add-Rule '0605-validate-048-assessment-approval' validate 48 `
    'Assessment/deficiency-tax manner is selected and neither approval radio is selected.' `
    @('frm0605:itemMannerOfPaymentB:_1','frm0605:itemMannerOfPaymentB:_2','frm0605:itemApprovedYN:_1','frm0605:itemApprovedYN:_2') `
    "Since you have selected Preliminary/Final Assess/Deficiency Tax on`nitem 17, you must choose either:Pre-approved or Not approved byInvestigating Office." `
    @('official-hta-runtime#validate:L2909-L2916')
Add-Rule '0605-validate-049-installment-count' validate 49 `
    'Installment mode is selected and installment count is outside 1..20.' `
    @('frm0605:itemModeOfPayment:_1','frm0605:txtNumOfInstallment') `
    'Please re-enter No. of Installment. Allowed values from 1 to 20 only.' `
    @('official-hta-runtime#validate:L2917-L2923')
Add-Rule '0605-validate-050-payment-type' validate 50 `
    'None of the three type-of-payment radios is selected.' @('type-of-payment-fields') `
    'Please select Type of Payment on item 18.' @('official-hta-runtime#validate:L2924-L2929')
Add-Rule '0605-validate-051-tax-positive' validate 51 `
    'Item 19 is zero or negative.' @('frm0605:txtTax19') `
    'Please enter valid value (greater than 0) for item 19 under Computation of Tax.' `
    @('official-hta-runtime#validate:L2930-L2934')
Add-Rule '0605-validate-052-other-description-omitted' validate 52 `
    'Other manner of payment is selected and description is blank.' @('frm0605:txtOthersName') $null `
    @('official-hta-runtime#validate:L2899-L2934','official-hta-runtime#enabletxtOthers:L2609-L2612') `
    'incorrect-official-behavior' 'Validate accepts the missing description.' `
    'Require the description when Other is selected.'
Add-Rule '0605-validate-053-email-omitted' validate 53 `
    'Email is blank or malformed.' @('txtEmail') $null `
    @('official-hta-runtime#validate:L2621-L2948') 'incorrect-official-behavior' `
    'Validate never checks the serialized email field.' 'Validate email syntax before any online submission path.'
Add-Rule '0605-validate-054-catalog-payload-serialized' validate 54 `
    'The runtime ATC and tax-type lookup payloads are present.' @('AtcCode1..142','TaxTypeCode1..37') $null `
    @('official-hta-runtime#loadXMLATC:L1612-L1630','official-hta-runtime#loadXMLTaxTypeCode:L1631-L1652','xml-encrypted-a') `
    'official-bug-compatible' `
    'All 179 lookup controls are serialized into every save/final copy even though only the selected codes are taxpayer data.' `
    'Preserve them when importing for losslessness, but do not model lookup payloads as taxpayer-entered values.'
Add-Rule '0605-validate-055-success' validate 55 `
    'All active Validate branches pass.' @('frm0605:cmdValidate','frm0605:cmdEdit') `
    'Validation successful. Click on Edit if you wish to modify your entries.' `
    @('official-hta-runtime#validate:L2936-L2947') 'verified-correct' `
    'Validate disables controls and enables Edit/Final Copy.' `
    'Tie the validated state to the exact current field snapshot.'
Add-Rule '0605-validate-056-unreachable-tax-types' validate 56 `
    'The user needs either physical catalog entry 38 or 39.' @('TaxTypeCode38','TaxTypeCode39') $null `
    @('official-tax-type-catalog#entries:38-39','official-hta-runtime#loadTaxTypeCode:L1653-L1684') `
    'incorrect-official-behavior' `
    'The catalog physically contains 39 Form 0605 tax types, but taxTypeCodeCount is 37 and the runtime loop never loads entries 38 or 39.' `
    'Derive selectable entries from reviewed legal/catalog evidence, not a stale declared-count field; retain the source inconsistency in audit evidence.'

Write-JsonFile (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Save and Validate alert the first active matching branch and stop; several dormant/commented branches never execute.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Add-Calculation(
    [string]$Id,
    [string[]]$Outputs,
    [string[]]$Inputs,
    [string]$Formula,
    [string]$Trigger,
    [string[]]$DependsOn,
    [string[]]$SourceRefs
) {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id = $Id
        outputs = $Outputs
        inputs = $Inputs
        condition = $null
        official_formula = $Formula
        rounding = 'formatCurrency after NumWithComma conversion; displayed to two decimal places.'
        trigger = $Trigger
        depends_on = $DependsOn
        source_refs = $SourceRefs
        assessment = 'verified-correct'
        recommended_app_behavior = 'Use typed decimals, explicit two-decimal rounding, and preserve the source dependency order.'
        confidence = 'high'
    })
}
Add-Calculation '0605-item20d-penalties' `
    @('frm0605:txtTax20D') `
    @('frm0605:txtTax20A','frm0605:txtTax20B','frm0605:txtTax20C') `
    '20D = 20A + 20B + 20C.' 'computePenalties' @() `
    @('official-hta-runtime#computePenalties:L2578-L2585')
Add-Calculation '0605-item21-total' `
    @('frm0605:txtTax21') `
    @('frm0605:txtTax19','frm0605:txtTax20D') `
    '21 = 19 + 20D.' 'computeOfTax' @('0605-item20d-penalties') `
    @('official-hta-runtime#computeOfTax:L2586-L2591')
Write-JsonFile (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    evaluation_order = @($calculations.calculation_id)
    calculations = $calculations
})

$negativeCases = [Collections.Generic.List[object]]::new()
$caseNumber = 0
foreach ($rule in @($rules | Where-Object { $_.exact_message })) {
    $caseNumber++
    $negativeCases.Add([pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id)
        phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }
        expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior
        rule_id = $rule.rule_id
    })
}
Write-JsonFile (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    synthetic_only = $true
    cases = $negativeCases
})
Write-JsonFile (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    cases = @(
        @{
            case_id = 'penalty-sum'
            calculation_id = '0605-item20d-penalties'
            surcharge = 10.25
            interest = 20.25
            compromise = 30.25
            official_output = 60.75
        },
        @{
            case_id = 'total-payment'
            calculation_id = '0605-item21-total'
            basic_tax = 1000
            penalties = 60.75
            official_output = 1060.75
        }
    )
})

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    phases = @(
        @{
            phase = 'edit'
            official_behavior = 'September 2003 payment form with runtime-loaded RDO, ATC, and tax-type catalogs.'
            source_refs = @('official-hta-runtime','official-atc-catalog','official-tax-type-catalog')
            confidence = 'high'
        },
        @{
            phase = 'saved-draft'
            official_behavior = 'Save requires only complete return period, TIN segments, RDO, and taxpayer name; it serializes 235 controls including 179 lookup payload controls.'
            source_refs = @('official-hta-runtime#initialValidateBeforeSave:L3180-L3205','official-hta-runtime#saveXML:L1968-L2164')
            confidence = 'high'
        },
        @{
            phase = 'validated'
            official_behavior = 'Validate runs the ordered active branches in validate(); the quarter branch is commented out.'
            source_refs = @('official-hta-runtime#validate:L2621-L2948')
            confidence = 'high'
        },
        @{
            phase = 'final-copy'
            official_behavior = 'Final Copy is enabled after Validate and writes an encrypted/compressed copy.'
            source_refs = @('official-hta-runtime#openAlertEmail:L3667-L3706','official-hta-runtime#saveEncryptedProfile:L1899-L1967')
            confidence = 'high'
        },
        @{
            phase = 'submitted'
            official_behavior = 'Online transport code exists but was not exercised.'
            source_refs = @('official-hta-runtime#sendEmail:L3707-L3803')
            confidence = 'medium'
        }
    )
    transitions = @(
        @{
            from = 'edit'
            action = 'Save'
            to = 'saved-draft'
            guard = 'Sparse Save checks pass.'
            side_effects = @('Writes flat pseudo-XML control state.')
            source_refs = @('official-hta-runtime#saveXML:L1968-L2164')
        },
        @{
            from = 'edit'
            action = 'Validate'
            to = 'validated'
            guard = 'The ordered active Validate branches pass.'
            side_effects = @('Disables editable controls.','Enables Edit and Final Copy.')
            source_refs = @('official-hta-runtime#validate:L2621-L2948')
        },
        @{
            from = 'validated'
            action = 'Edit'
            to = 'edit'
            guard = $null
            side_effects = @('Re-enables editable controls.','Disables Final Copy.')
            source_refs = @('official-hta-runtime#enableAllControl:L2484-L2552')
        },
        @{
            from = 'validated'
            action = 'Final Copy'
            to = 'final-copy'
            guard = 'Final-copy confirmation and save succeed.'
            side_effects = @('Writes encrypted/compressed copy.')
            source_refs = @('official-hta-runtime#openAlertEmail:L3667-L3706')
        },
        @{
            from = 'final-copy'
            action = 'Online transport'
            to = 'submitted'
            guard = 'Connectivity and remote acceptance succeed.'
            side_effects = @('Untested online attempt.')
            source_refs = @('official-hta-runtime#sendEmail:L3707-L3803')
        }
    )
    prerequisites = @(
        'Filing basis and year ended',
        'Due date',
        'ATC and return period',
        'Tax type',
        'TIN, RDO, and taxpayer identity',
        'Manner and type of payment',
        'Positive Item 19 tax amount'
    )
    required_attachments = @()
    filing_deadlines = @(
        @{
            quarter = 'Q1'
            due_date_rule = 'Payment timing depends on the selected tax type and ATC; use the obligation-specific rule rather than inferring a universal quarterly deadline from Form 0605.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q2'
            due_date_rule = 'Payment timing depends on the selected tax type and ATC; use the obligation-specific rule rather than inferring a universal quarterly deadline from Form 0605.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q3'
            due_date_rule = 'Payment timing depends on the selected tax type and ATC; use the obligation-specific rule rather than inferring a universal quarterly deadline from Form 0605.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q4'
            due_date_rule = 'Payment timing depends on the selected tax type and ATC; use the obligation-specific rule rather than inferring a universal quarterly deadline from Form 0605.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        }
    )
}
Write-JsonFile (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @(
    $rules | Where-Object {
        $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete')
    }
).Count
$assets = @(
    New-Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'September 2003 Offline runtime; authoritative for this package.'
    New-Asset 'packaged-help' 'official-runtime-help' $helpPath 'Packaged 0605 help.'
    New-Asset 'official-atc-catalog' 'runtime-lookup-catalog' $atcPath 'Hash-pinned runtime ATC catalog; 142 entries bind to Form 0605.'
    New-Asset 'official-tax-type-catalog' 'runtime-lookup-catalog' $taxTypePath 'Hash-pinned runtime tax-type catalog; 39 entries bind to Form 0605 physically, but the declared count exposes only 37.'
    New-Asset 'earlier-form-pdf' 'official-form-pdf-earlier-revision' $pdfPath 'September 1999 PDF; recorded as a revision boundary and not merged into September 2003 rules.'
    New-Asset 'xml-encrypted-a' 'dummy-profile-encrypted-final-copy' `
        $sampleByHash[$expected.cipher_a].FullName 'Revision-matched 235-key dummy final copy A; values excluded.' `
        (Join-Path $OfficialDir '0605-final-copy-a-#email-redacted#.xml')
    New-Asset 'xml-encrypted-b' 'dummy-profile-encrypted-final-copy' `
        $sampleByHash[$expected.cipher_b].FullName 'Revision-matched 235-key dummy final copy B; values excluded.' `
        (Join-Path $OfficialDir '0605-final-copy-b-#email-redacted#.xml')
    New-Asset 'xml-plaintext-a' 'dummy-profile-plaintext-save' `
        $sampleByHash[$expected.plain_a].FullName 'Revision-matched 235-key dummy plaintext save A; values excluded.' `
        (Join-Path $OfficialDir '0605-save-a-#email-redacted#.xml')
    New-Asset 'xml-plaintext-b' 'dummy-profile-plaintext-save' `
        $sampleByHash[$expected.plain_b].FullName 'Revision-matched 235-key dummy plaintext save B; values excluded.' `
        (Join-Path $OfficialDir '0605-save-b-#email-redacted#.xml')
)

$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    form_code = '0605'
    revision = $revision
    package_version = $packageVersion
    status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = $fields.Count
        runtime_field_families = 0
        fields_total = $fields.Count
        typed_fields = $fields.Count
        validation_rules = $rules.Count
        confirmed_official_bugs = $bugCount
        calculations = $calculations.Count
        negative_fixtures = $negativeCases.Count
        unverified_gaps = 2
    }
    artifacts = [ordered]@{
        fields = 'fields.json'
        validations = 'validations.json'
        calculations = 'calculations.json'
        workflow = 'workflow.json'
        evidence = 'evidence.md'
        audit = 'audit.md'
        gaps = 'gaps.md'
        encrypted_field_audit_a = 'fixtures/encrypted-field-audit-a-v796.json'
        encrypted_field_audit_b = 'fixtures/encrypted-field-audit-b-v796.json'
        plaintext_field_audits = 'fixtures/plaintext-field-audits-v796.json'
        runtime_controls = 'fixtures/runtime-control-inventory-v796.json'
        atc_catalog = 'fixtures/atc-catalog-v796.json'
        tax_type_catalog = 'fixtures/tax-type-catalog-v796.json'
        validation_functions = 'fixtures/validation-function-inventory-v796.json'
        calculation_functions = 'fixtures/calculation-function-inventory-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'
        calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release metadata changed.',
        'No decrypted values or email-bearing filenames are emitted.',
        'All 235 keys are accounted for: 55 static controls, one runtime RDO, 142 ATC lookup controls, and 37 reachable tax-type lookup controls.',
        'The tax-type catalog physically contains 39 Form 0605 entries but declares 37, leaving entries 38 and 39 unreachable.',
        'The September 1999 PDF is an earlier revision boundary and is not merged into the September 2003 runtime rules.'
    )
}
Write-JsonFile (Join-Path $outDir 'manifest.json') $manifest

Write-TextFile (Join-Path $outDir 'README.md') @"
# BIR Form 0605 - September 2003

Revision-specific Offline eBIRForms payment-form rules with 235 concrete serialized keys and no runtime field families.
"@
Write-TextFile (Join-Path $outDir 'evidence.md') @"
# Evidence

- September 2003 runtime SHA-256: $($expected.hta); packaged help: $($expected.help).
- Runtime lookup catalogs: ATC $($expected.atc) with 142 Form 0605 entries; tax type $($expected.tax_type) with 39 physical entries but only 37 runtime-reachable entries.
- Four revision-matched dummy artifacts independently carry the same ordered 235-key inventory $($expected.inventory): two plaintext saves and two encrypted final copies. Values are never emitted.
- Key accounting: 55 static controls + 1 runtime RDO + 142 ATC lookup controls + 37 tax-type lookup controls = 235; zero unexplained keys.
- The local PDF is September 1999, SHA-256 $($expected.pdf), and is recorded only as an earlier revision boundary.
- All email-bearing filenames are represented only as ``#email-redacted#``.
"@
Write-TextFile (Join-Path $outDir 'gaps.md') @"
# Gaps

1. No revision-matched September 2003 PDF is locally pinned; the available PDF is September 1999.
2. Online submission was not exercised.
"@
Write-TextFile (Join-Path $outDir 'audit.md') @"
# Audit

- September 2003 runtime binding: pass.
- Four-sample ordered field inventory: pass; 235 unique keys in each artifact.
- Typed inventory: 235 concrete keys; no field families; zero unexplained keys.
- Validations: $($rules.Count); calculations: $($calculations.Count); negatives: $($negativeCases.Count); confirmed official defects: $bugCount.
- Focused and full strict structural/schema audits must run after generation.
- No renderer/release/capability/commit/push changes.
"@

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '0605'
    $entry.revision = $revision
    $entry.package_version = $packageVersion
    $entry.priority = 38
    $entry.status = 'complete'
    $entry.path = 'forms/0605-v2003/manifest.json'
}
else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId
        form_code = '0605'
        revision = $revision
        package_version = $packageVersion
        priority = 38
        status = 'complete'
        path = 'forms/0605-v2003/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-JsonFile $indexPath $index

$actualCounts = [ordered]@{
    live_static_controls = $controls.Count
    sample_keys = $keys.Count
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    runtime_atc = $runtimeAtc.Count
    runtime_tax_types = $runtimeTaxTypes.Count
    unexplained = $unexplained.Count
    atc_catalog = $atcCatalog.Count
    tax_type_catalog = $taxTypeCatalog.Count
    reachable_tax_types = @($taxTypeCatalog | Where-Object runtime_reachable).Count
    typed_fields = $fields.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negative_fixtures = $negativeCases.Count
    confirmed_official_bugs = $bugCount
}
$expectedCounts = [ordered]@{
    live_static_controls = 85
    sample_keys = 235
    static_matches = 55
    runtime_rdo = 1
    runtime_atc = 142
    runtime_tax_types = 37
    unexplained = 0
    atc_catalog = 142
    tax_type_catalog = 39
    reachable_tax_types = 37
    typed_fields = 235
    validations = 56
    calculations = 2
    negative_fixtures = 46
    confirmed_official_bugs = 11
}
foreach ($name in $expectedCounts.Keys) {
    if ($actualCounts[$name] -ne $expectedCounts[$name]) {
        throw "0605 fail-closed count changed: $name expected $($expectedCounts[$name]), found $($actualCounts[$name])."
    }
}

[pscustomobject][ordered]@{
    form_id = $formId
    live_static_controls = $controls.Count
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    runtime_atc = $runtimeAtc.Count
    runtime_tax_types = $runtimeTaxTypes.Count
    unexplained = $unexplained.Count
    typed_fields = $fields.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negative_fixtures = $negativeCases.Count
    confirmed_official_bugs = $bugCount
    next_form = '0619E'
} | ConvertTo-Json
