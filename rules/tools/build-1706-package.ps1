param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\1706v2018',
    [string]$SaveDir = 'C:\Mac\Home\Downloads\forms\1706'
)

$ErrorActionPreference = 'Stop'
$formId = '1706-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1706.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1706.hta'
$pdfPath = Join-Path $PdfDir '1706 Jan 2018 ENCS Final version.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1706-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'

$expected = @{
    hta = 'f29a02e0a80fb4a72cc90046e2773d05ac631df2ec06845f4c885eabcabafca1'
    help = 'ddeba297664de08d8862616d1161ad38eea563a62f8f088bb125e9054d472bbe'
    pdf = '5237ba69d5fae6a26dceffc8f39dfcab32fe7d57081bfba74dcf5c5550c1afa3'
    encrypted = '4764678faecfca0c8830d7f5262604683372629707226b9221a54123712626ce'
    decrypted = 'eee8ff6ac46b4008186daaf8501186dc34f027f6470a40b2044035480a6c3f6d'
    inventory = '163cff842e04aa0df389997c1649ac1537467f21eafc8782166c40642a192ff3'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText($Path, (($Value | ConvertTo-Json -Depth 60) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
}
function Write-Utf8([string]$Path, [string]$Value) {
    [IO.File]::WriteAllText($Path, $Value, [Text.UTF8Encoding]::new($false))
}
function Get-HashText([string[]]$Lines) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        ([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines -join "`n"))))).Replace('-', '').ToLowerInvariant()
    } finally { $sha.Dispose() }
}
function Get-Attr([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { $match.Groups[2].Value } else { $null }
}
function Find-FileByHash([string]$Directory, [string]$Hash) {
    $matches = @(
        Get-ChildItem -LiteralPath $Directory -File |
            Where-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -eq $Hash }
    )
    if ($matches.Count -ne 1) { throw "Expected exactly one source with SHA-256 $Hash; found $($matches.Count)." }
    $matches[0].FullName
}
function Asset([string]$Id, [string]$Kind, [string]$Path, [string]$Binding, [string]$DisplayPath = '') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $Id
        kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length
        revision_binding = $Binding
    }
}
function Save-Entries([string]$Text) {
    @([regex]::Matches($Text, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>'))
}
function Decrypt-Save([string]$Path) {
    $ciphertext = [IO.File]::ReadAllBytes($Path)
    $sha = [Security.Cryptography.SHA256]::Create()
    $aes = [Security.Cryptography.Aes]::Create()
    try {
        $key = $sha.ComputeHash([Text.Encoding]::UTF8.GetBytes('T0081gP45sy0rd-To+R3m3m63r!@4/<>'))
        $aes.Mode = [Security.Cryptography.CipherMode]::ECB
        $aes.Padding = [Security.Cryptography.PaddingMode]::None
        $aes.Key = $key
        $zero = New-Object byte[] 16
        $encryptor = $aes.CreateEncryptor()
        $iv = New-Object byte[] 16
        [void]$encryptor.TransformBlock($zero, 0, 16, $iv, 0)
        $compressed = New-Object byte[] $ciphertext.Length
        $cv = [byte[]]$iv.Clone()
        $decryptor = $aes.CreateDecryptor()
        $offset = 0
        while ($offset + 16 -le $ciphertext.Length) {
            $block = New-Object byte[] 16
            [void]$decryptor.TransformBlock($ciphertext, $offset, 16, $block, 0)
            for ($index = 0; $index -lt 16; $index++) {
                $compressed[$offset + $index] = $block[$index] -bxor $cv[$index]
                $cv[$index] = $ciphertext[$offset + $index]
            }
            $offset += 16
        }
        if ($offset -lt $ciphertext.Length) {
            $streamBlock = New-Object byte[] 16
            $tailEncryptor = $aes.CreateEncryptor()
            [void]$tailEncryptor.TransformBlock($cv, 0, 16, $streamBlock, 0)
            for ($index = 0; $index -lt ($ciphertext.Length - $offset); $index++) {
                $compressed[$offset + $index] = $ciphertext[$offset + $index] -bxor $streamBlock[$index]
            }
        }
        $zlibHeader = ('{0:x2}{1:x2}' -f $compressed[0], $compressed[1])
        if ($zlibHeader -ne '78da') { throw "Unexpected zlib header: $zlibHeader" }
        $deflateBytes = $compressed[2..($compressed.Length - 5)]
        $input = New-Object IO.MemoryStream(,$deflateBytes)
        $deflate = New-Object IO.Compression.DeflateStream($input, [IO.Compression.CompressionMode]::Decompress)
        $output = New-Object IO.MemoryStream
        try { $deflate.CopyTo($output) } finally { $deflate.Dispose(); $input.Dispose() }
        $plainBytes = $output.ToArray()
        $output.Dispose()
        $xml = [Text.Encoding]::UTF8.GetString($plainBytes)
        $entries = Save-Entries $xml
        [pscustomobject]@{
            bytes = $plainBytes
            entries = $entries
            keys = @($entries | ForEach-Object { $_.Groups['key'].Value })
            sha256 = ([BitConverter]::ToString($sha.ComputeHash($plainBytes))).Replace('-', '').ToLowerInvariant()
            zlib_header = $zlibHeader
        }
    } finally {
        $aes.Dispose()
        $sha.Dispose()
    }
}

foreach ($path in @($htaPath, $helpPath, $pdfPath, $packagePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
$encryptedPath = Find-FileByHash $SaveDir $expected.encrypted
foreach ($pair in @(
    @($htaPath, 'hta'), @($helpPath, 'help'), @($pdfPath, 'pdf'),
    @($encryptedPath, 'encrypted'), @($packagePath, 'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') { throw 'PDF magic mismatch.' }
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1706["'']') { throw 'APPLICATIONNAME mismatch.' }
if ($help -notmatch '(?i)BIR\s+FORM\s+1706') { throw '1706 help content binding is absent.' }
if ($help -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']0605["'']') { throw 'Expected mislabelled help APPLICATIONNAME changed.' }
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$decrypted = Decrypt-Save $encryptedPath
$keys = @($decrypted.keys)
if ($decrypted.sha256 -ne $expected.decrypted) { throw 'Decrypted payload hash changed.' }
if ($keys.Count -ne 122 -or ($keys | Sort-Object -Unique).Count -ne 122) { throw "Expected 122 unique keys; found $($keys.Count)." }
if ((Get-HashText @($keys | Sort-Object)) -ne $expected.inventory) { throw 'Field inventory hash changed.' }
if (@($keys | Where-Object { $_ -match '@' }).Count -gt 0) { throw 'Email-like content appeared in a field key.' }

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$scriptRanges = @([regex]::Matches($body, '(?is)<script\b.*?</script>'))
$controls = @()
$ordinal = 0
foreach ($match in [regex]::Matches($body, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $insideScript = $false
    foreach ($range in $scriptRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $insideScript = $true; break }
    }
    if ($insideScript) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $controls += [pscustomobject][ordered]@{
        ordinal = $ordinal
        id = Get-Attr $tag 'id'
        name = Get-Attr $tag 'name'
        element = $element
        control_kind = $kind.ToLowerInvariant()
        source_line = 1 + [regex]::Matches($hta.Substring(0, $bodyOffset + $match.Index), "`n").Count
        value = Get-Attr $tag 'value'
        maxlength = Get-Attr $tag 'maxlength'
        disabled = $tag -match '(?i)\bdisabled(?:\s*=|\s|>)'
        readonly = $tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    }
}
$serial = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','textarea','radio','checkbox','hidden') })
$staticIds = @($serial.id | Where-Object { $_ } | Sort-Object -Unique)
if ($controls.Count -ne 145 -or $serial.Count -ne 124 -or $staticIds.Count -ne 122) {
    throw "Expected 145 controls/124 serializer candidates/122 unique static IDs; found $($controls.Count)/$($serial.Count)/$($staticIds.Count)."
}
if ($hta -match '(?i)setAttribute\s*\(\s*["''](?:id|name)["'']' -or $hta -match '(?i)Add\s+More') {
    throw 'Unexpected dynamic/add-more field construction appeared.'
}
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}

$requiredKeys = @(
    'frm1706:txtDateMonth','frm1706:txtDateDay','frm1706:txtDateYear',
    'frm1706:j_id217:_1','frm1706:j_id217:_2','frm1706:opt4','frm1706:opt4C',
    'frm1706:txtTIN1','frm1706:txtTIN2','frm1706:txtTIN3','frm1706:txtBranchCode',
    'frm1706:txtRDOCode','frm1706:txtTINB1','frm1706:txtTINB2','frm1706:txtTINB3',
    'frm1706:txtBranchCodeB','frm1706:txtRDOCodeB','frm1706:txtSellerName',
    'frm1706:txtBuyerName','frm1706:txtSellerAddress','frm1706:txtBuyerAddress',
    'frm1706:txtLocation','frm1706:txtRDOCode14A','frm1706:txtTCT',
    'frm1706:j_id394:_1','frm1706:j_id394:_2'
)
function Get-FieldMeta([string]$Key, $Control) {
    $logical = 'string'
    $enum = [object[]]@()
    $normalization = [string[]]@()
    if (($Control -and $Control.control_kind -in @('radio','checkbox')) -or $Key -match '(?i):(j_id|opt|rdTreaty)') {
        $logical = 'boolean'
        $enum = [object[]]@('true','false')
    } elseif ($Key -match '(?i)(TIN|RDO|BranchCode)') {
        $logical = 'code'
    } elseif ($Key -match '(?i)(DateMonth|DateDay|DateYear)') {
        $logical = 'date-component-string'
        $normalization = [string[]]@('MM/DD/YYYY components; no automatic padding')
    } elseif ($Key -match '(?i)(txtSelling|txtCost|txtMortgage|txtTotalP|txtAmount|txtFMV|txtGross|txtBid|txtInstallment|txtOthers30[EF]|txtTax$|txtRate|txtLess|txtTaxDue|txtSurcharge|txtInterest|txtCompromise|txtTotalPenalties|txtTotal$)') {
        $logical = 'decimal-amount'
        $normalization = [string[]]@('NumWithComma', 'formatCurrency', 'round(...,2)')
    } elseif ($Key -match '(?i)email') {
        $logical = 'email-string'
    }
    $computed = $Key -match '(?i)(txtFMVLI|txtTax$|txtRate|txtTaxDue|txtTotalPenalties|txtTotal$)'
    $status = if ($requiredKeys -contains $Key) { 'required' } else { 'optional' }
    if ($computed) { $status = 'computed' }
    if ($Key -match '^(txtFinalFlag|txtVersion|txtEnabled|txtDisabled|txtMaxPage|txtCurrentPage|txtEnroll|ebirOnline|driveSelectTPExport)') { $status = 'hidden' }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength -and $Control.maxlength -match '^\d+$') { $constraints.max_length = [int]$Control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision = 2; $constraints.sign = 'signed' }
    [pscustomobject]@{
        logical = $logical; enum = $enum; normalization = $normalization
        computed = $computed; status = $status; constraints = [pscustomobject]$constraints
    }
}
$itemMap = @{
    'txtDateMonth'='1'; 'txtDateDay'='1'; 'txtDateYear'='1'; 'j_id217:_1'='2'; 'j_id217:_2'='2'
    'opt4'='4'; 'opt4C'='4'; 'txtTIN1'='5'; 'txtTIN2'='5'; 'txtTIN3'='5'; 'txtBranchCode'='5'
    'txtRDOCode'='6'; 'txtTINB1'='7'; 'txtTINB2'='7'; 'txtTINB3'='7'; 'txtBranchCodeB'='7'
    'txtRDOCodeB'='8'; 'txtSellerName'='9'; 'txtBuyerName'='10'; 'txtSellerAddress'='11'; 'txtBuyerAddress'='12'
    'txtSellerRAddress'='13'; 'txtLocation'='14'; 'txtRDOCode14A'='14A'; 'txtTCT'='16'; 'txtArea'='16'
    'txtTaxDC'='16'; 'txtOthers'='16'; 'txtOthers21'='21'; 'txtSelling'='22'; 'txtCost'='23'; 'txtMortgage'='24'
    'txtTotalP'='25'; 'txtAmount'='26'; 'txtTotalN'='27'; 'txtDateMonthI'='28'; 'txtDateDayI'='28'; 'txtDateYearI'='28'
    'txtFMVLand'='29A'; 'txtFMVImprovements'='29B'; 'txtFMVZonal'='29C'; 'txtFMVBIR'='29D'; 'txtFMVLI'='30C'
    'txtGross'='30A'; 'txtBid'='30B'; 'txtInstallment'='30D'; 'txtOthers30E'='30E'; 'txtOtherss30F'='30F'; 'txtOthers30F'='30F'
    'txtTax'='31'; 'txtRate'='32'; 'txtLess'='33'; 'txtTaxDue'='34'; 'txtSurcharge'='35A'; 'txtInterest'='35B'
    'txtCompromise'='35C'; 'txtTotalPenalties'='35D'; 'txtTotal'='36'
}
$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Get-FieldMeta $key $control
    $short = if ($key -like 'frm1706:*') { $key.Substring(8) } else { $key }
    $item = if ($itemMap.ContainsKey($short)) { $itemMap[$short] } else { $null }
    $requiredWhen = $null
    if ($key -match 'frm1706:j_id39[23]:') { $requiredWhen = 'Item 4 indicates the applicable seller category.' }
    elseif ($key -match 'frm1706:txtOthers21') { $requiredWhen = 'Item 21 is Exempt or Others.' }
    elseif ($key -match 'frm1706:selTreaty') { $requiredWhen = 'Item 20 indicates tax-treaty relief.' }
    elseif ($key -match 'frm1706:opt37:') { $requiredWhen = 'Item 36 is negative.' }
    $refs = @("xml-final-v1#field:$key")
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" } else { $refs += 'official-hta-runtime#runtime-injected-or-final-copy-field' }
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key
        serialized_key = $key
        serialized_occurrence = 1
        label = $short
        page = 1
        item_number = $item
        control_kind = if ($control) { $control.control_kind } else { 'runtime-injected-or-final-copy-control' }
        storage_type = 'string'
        logical_type = $meta.logical
        required = $meta.status
        required_when = $requiredWhen
        enabled_when = $requiredWhen
        visible_when = $null
        default_value = if ($control) { $control.value } else { $null }
        empty_representation = ''
        constraints = $meta.constraints
        enum_values = $meta.enum
        normalization = $meta.normalization
        computed = $meta.computed
        calculation_id = if ($meta.computed) { 'See calculations.json' } else { $null }
        source_refs = $refs
        confidence = if ($control) { 'high' } else { 'medium' }
        notes = @('Observed in the reviewed 122-key decrypted final-copy inventory; source value is excluded.')
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = 122
    inventory_sha256 = Get-HashText @($fields.field_key | Sort-Object)
    fields = $fields
})

$staticOnly = @(Compare-Object @($keys | Sort-Object -Unique) $staticIds | Where-Object SideIndicator -eq '=>' | ForEach-Object { $_.InputObject })
$finalOnly = @(Compare-Object @($keys | Sort-Object -Unique) $staticIds | Where-Object SideIndicator -eq '<=' | ForEach-Object { $_.InputObject })
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    form_control_count = $controls.Count
    static_serializer_candidate_count = $serial.Count
    static_serializer_unique_id_count = $staticIds.Count
    reviewed_final_copy_key_count = $keys.Count
    active_runtime_family_count = 0
    serializer_set_differences = [ordered]@{
        final_copy_not_in_static_dom = $finalOnly
        static_dom_not_in_final_copy_snapshot = $staticOnly
    }
    controls = $controls
    dynamic_families = @()
})
Write-Json (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_path = (Join-Path $SaveDir '1706-final-copy-#email-redacted#.xml')
    ciphertext_sha256 = $expected.encrypted
    zlib_header = $decrypted.zlib_header
    decrypted_byte_count = $decrypted.bytes.Length
    decrypted_sha256 = $decrypted.sha256
    field_count = $keys.Count
    unique_field_count = ($keys | Sort-Object -Unique).Count
    field_inventory_sha256 = $expected.inventory
    values_emitted = $false
})
$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1706:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final|date|tin') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1706:' -NamePattern '(?i)compute|compare|round|fmv|tax|total|capital') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id, [string]$Phase, $Order, [string]$Condition, [string[]]$FieldKeys, $Message,
    [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.',
    [string]$Confidence = 'high'
) {
    $rules.Add([pscustomobject][ordered]@{
        rule_id = $Id; form_id = $formId; revision = $revision; phase = $Phase; order = $Order
        condition = $Condition; fields = $FieldKeys; accepted_behavior = 'Condition is false; processing continues.'
        rejected_behavior = 'The active operation stops unless official_behavior states otherwise.'
        exact_message = $Message; source_refs = $Refs; evidence_type = @('source')
        assessment = $Assessment; official_behavior = $Official; recommended_app_behavior = $Recommended
        confidence = $Confidence; unresolved_questions = @()
    })
}
Rule '1706-save-001-seller-tin' 'save' 1 'Any seller TIN segment or branch code is blank.' @('frm1706:txtTIN1','frm1706:txtTIN2','frm1706:txtTIN3','frm1706:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L3986-L3991') 'incorrect-official-behavior' 'Only nonblankness is checked.' 'Allow drafts, but require exact segment lengths, digits, and checksum before finalization.'
Rule '1706-save-002-seller-rdo' 'save' 2 'Seller RDO equals literal 000.' @('frm1706:txtRDOCode') "Please enter a valid Seller's RDO Code on Item 6." @('official-hta-runtime#initialValidateBeforeSave:L3992-L3995')
Rule '1706-save-003-seller-name' 'save' 3 'Seller name is blank.' @('frm1706:txtSellerName') "Please enter a valid Seller's Name on Item 9." @('official-hta-runtime#initialValidateBeforeSave:L3996-L4000')
Rule '1706-save-004-title-number' 'save' 4 'TCT/OCT/CCT number is blank.' @('frm1706:txtTCT') 'Please enter the TCT/OCT/CCT No.' @('official-hta-runtime#initialValidateBeforeSave:L4001-L4005')
Rule '1706-save-005-omissions' 'save' 5 'Any other identity, transaction, tax-base, or calculation field is missing or malformed.' @('return-body') $null @('official-hta-runtime#initialValidateBeforeSave:L3986-L4007') 'official-bug-compatible' 'Save performs only the four checks above.' 'Keep lossless draft persistence distinct from Validate and Final Copy.'

$order = 0
function V(
    [string]$Suffix, [string]$Condition, [string[]]$Fields, $Message, [string[]]$Refs,
    [string]$Assessment = 'verified-correct', [string]$Official = 'The branch alerts and returns.',
    [string]$Recommended = 'Retain with revision-aware wording.'
) {
    $script:order++
    Rule "1706-validate-$Suffix" 'validate' $script:order $Condition $Fields $Message $Refs $Assessment $Official $Recommended
}
V '001-date-format' 'Any transaction-date component is present and validateMonthDayYearDate returns true.' @('frm1706:txtDateMonth','frm1706:txtDateDay','frm1706:txtDateYear') 'Invalid date entry on item 1.' @('official-hta-runtime#validate:L3148-L3152','official-hta-runtime#validateMonthDayYearDate:L3933-L3985') 'official-bug-compatible' 'Returns literal true from Validate after the alert, unlike later failure branches.' 'Return a typed failure result.'
V '002-date-required' 'All three transaction-date components are blank.' @('frm1706:txtDateMonth','frm1706:txtDateDay','frm1706:txtDateYear') 'Please indicate date of transaction on item 1.' @('official-hta-runtime#validate:L3153-L3156') 'official-bug-compatible' 'Returns literal true from Validate after the alert.' 'Return a typed failure result.'
V '003-year-minimum' 'Transaction year coerces below 1904.' @('frm1706:txtDateYear') 'Invalid date entry on Item no.1. Entry should not be lower than 1904.' @('official-hta-runtime#validate:L3206-L3210') 'incorrect-official-behavior' 'The January 2018 form retains a 1904 lower bound with no documented revision justification.' 'Use the legally supported range and report its source.'
V '004-amended' 'Neither amended-return Yes nor No is selected.' @('frm1706:j_id217:_1','frm1706:j_id217:_2') 'Please choose amended return on item 2.' @('official-hta-runtime#validate:L3211-L3215')
V '005-item4' 'Neither Item 4 option is selected.' @('frm1706:opt4','frm1706:opt4C') 'Please select an option for Item 4.' @('official-hta-runtime#validate:L3216-L3220')
V '006-seller-tin' 'Any seller TIN segment or branch code is blank.' @('frm1706:txtTIN1','frm1706:txtTIN2','frm1706:txtTIN3','frm1706:txtBranchCode') "Please enter the Seller's TIN." @('official-hta-runtime#validate:L3221-L3240') 'incorrect-official-behavior' 'Four sequential branches repeat the same message and check only blankness.' 'Validate the full TIN once, including shape and checksum.'
V '007-seller-rdo' 'Seller RDO selectedIndex is zero.' @('frm1706:txtRDOCode') "Please enter the Seller's RDO Code." @('official-hta-runtime#validate:L3241-L3246')
V '008-buyer-tin' 'Any buyer TIN segment or branch code is blank.' @('frm1706:txtTINB1','frm1706:txtTINB2','frm1706:txtTINB3','frm1706:txtBranchCodeB') "Please enter the Buyer's TIN." @('official-hta-runtime#validate:L3247-L3266') 'incorrect-official-behavior' 'Four sequential branches repeat the same message and check only blankness.' 'Validate the full TIN once, including shape and checksum.'
V '009-different-tins' 'Concatenated seller and buyer TIN+branch strings compare equal.' @('seller-tin','buyer-tin') 'TIN for Buyer and Seller should be different.' @('official-hta-runtime#validate:L3267-L3271')
V '010-buyer-rdo' 'Buyer RDO selectedIndex is zero.' @('frm1706:txtRDOCodeB') "Please enter the Buyer's RDO Code." @('official-hta-runtime#validate:L3272-L3276')
V '011-seller-name' 'Seller name is blank.' @('frm1706:txtSellerName') "Please enter the Seller's Name." @('official-hta-runtime#validate:L3277-L3281')
V '012-buyer-name' 'Buyer name is blank.' @('frm1706:txtBuyerName') "Please enter the Buyer's Name." @('official-hta-runtime#validate:L3282-L3286')
V '013-seller-address' 'Seller registered address is blank.' @('frm1706:txtSellerAddress') "Please enter the Seller's Address." @('official-hta-runtime#validate:L3287-L3291')
V '014-buyer-address' 'Buyer registered address is blank.' @('frm1706:txtBuyerAddress') "Please enter the Buyer's address." @('official-hta-runtime#validate:L3292-L3296')
V '015-location' 'Property location is blank.' @('frm1706:txtLocation') 'Please enter the Location of the Property.' @('official-hta-runtime#validate:L3297-L3301')
V '016-property-rdo' 'Item 14A RDO selectedIndex is <= 0.' @('frm1706:txtRDOCode14A') 'Please enter the RDO Code on Item 14A.' @('official-hta-runtime#validate:L3302-L3305')
V '017-classification' 'No Item 15 classification radio is selected.' @('frm1706:j_id391:_1','frm1706:j_id391:_2','frm1706:j_id391:_3','frm1706:j_id391:_4','frm1706:j_id391:_5','frm1706:j_id391:_6','frm1706:j_id391:_8') 'Please select an option for Item 15.' @('official-hta-runtime#validate:L3306-L3310')
V '018-classification-other-description' 'Item 15 Others is selected and its description is blank.' @('frm1706:j_id391:_8','frm1706:j_id391:_7') $null @('official-hta-runtime#validate:L3306-L3310','official-hta-runtime#control:L930-L946') 'incorrect-official-behavior' 'Validate requires an Item 15 radio but never requires the enabled Others description.' 'Require the description when Others is selected.'
V '019-title-number' 'TCT/OCT/CCT number is blank.' @('frm1706:txtTCT') 'Please enter the TCT/OCT/CCT No.' @('official-hta-runtime#validate:L3311-L3317')
V '020-principal-residence' 'Item 4 opt4 is checked and neither Item 17 option is selected.' @('frm1706:opt4','frm1706:j_id392:_1','frm1706:j_id392:_2') 'Please select an option for Item 17.' @('official-hta-runtime#validate:L3319-L3324')
V '021-proceeds-utilized' 'Item 4 opt4 is checked and neither Item 18 option is selected.' @('frm1706:opt4','frm1706:j_id393:_1','frm1706:j_id393:_2') 'Please select an option for Item 18.' @('official-hta-runtime#validate:L3325-L3331')
V '022-item19' 'Neither Item 19 option is selected.' @('frm1706:j_id394:_1','frm1706:j_id394:_2') 'Please select an option for Item 19.' @('official-hta-runtime#validate:L3333-L3337')
V '023-transaction' 'No Item 21 transaction radio is selected.' @('frm1706:j_id395:_1','frm1706:j_id395:_2','frm1706:j_id395:_3','frm1706:j_id395:_4','frm1706:j_id395:_5') 'Please select an option for Item 21.' @('official-hta-runtime#validate:L3338-L3342')
V '024-treaty' 'Treaty relief is selected and the treaty dropdown selectedIndex is zero.' @('frm1706:rdTreaty:_1','frm1706:selTreaty') 'Please specify tax relief you are availing for Item 20.' @('official-hta-runtime#validate:L3343-L3347')
V '025-fmv-commented' 'Cash Sale is selected and an Item 29 FMV input is zero or blank.' @('frm1706:j_id395:_1','frm1706:txtFMVLand','frm1706:txtFMVImprovements','frm1706:txtFMVZonal','frm1706:txtFMVBIR') 'Please provide a valid value for 29A.' @('official-hta-runtime#validate:L3349-L3371') 'obsolete' 'All four FMV-required branches are in one block comment.' 'Apply evidence-backed Item 29 completeness rules rather than treating these stale messages as active.'
V '026-transaction-description' 'Item 21 Exempt or Others is selected and its description is blank.' @('frm1706:j_id395:_2','frm1706:j_id395:_5','frm1706:txtOthers21') 'Please specify a valid description of transaction in item 21.' @('official-hta-runtime#validate:L3373-L3377')
V '027-taxable-base-choice' 'No Item 30 taxable-base radio is selected.' @('frm1706:opt30A','frm1706:opt30B','frm1706:opt30C','frm1706:opt30D','frm1706:opt30E','frm1706:opt30F') 'Please choose Determination of taxable base in Item 30' @('official-hta-runtime#validate:L3379-L3383')
V '028-taxable-base-value' 'An Item 30 radio is selected while its corresponding amount is blank, zero, stale, or disabled.' @('item-30-fields') $null @('official-hta-runtime#validate:L3379-L3383','official-hta-runtime#computeTaxableBase:L3859-L3903') 'incorrect-official-behavior' 'Validate checks only the radio selection, not the applicable amount or whether Item 31 was recomputed.' 'Require the applicable source amount and recompute Item 31 before validation.'
V '029-overpayment-choice' 'Item 36 is negative and neither Item 37 refund nor TCC is selected.' @('frm1706:txtTotal','frm1706:opt37:_1','frm1706:opt37:_2') 'Please choose if to be refunded or to be issued a Tax Credit Certificate.' @('official-hta-runtime#validate:L3385-L3389')
V '030-success' 'All active checks pass and global flag is truthy.' @('return-body') "Validation successful. Click on 'Edit' if you wish to modify your entries." @('official-hta-runtime#validate:L3391-L3402') 'official-bug-compatible' 'Controls are disabled before the global flag is tested; if flag is falsey, validation state changes without a success alert.' 'Use an explicit local validation result and deterministic success transition.'
V '031-future-date-commented' 'Transaction date is after the current date.' @('frm1706:txtDateMonth','frm1706:txtDateDay','frm1706:txtDateYear') 'Invalid date entry on Item no.1. Entry should not be later than Current Date.' @('official-hta-runtime#validate:L3173-L3205') 'obsolete' 'Every future-date branch is commented out.' 'Reject legally impossible future transaction dates when supported by filing rules.'
V '032-month-zero' 'Transaction month is 00 with otherwise valid two-digit day and four-digit year.' @('frm1706:txtDateMonth') $null @('official-hta-runtime#validateMonthDayYearDate:L3951-L3954') 'incorrect-official-behavior' 'The lower-bound test is month < 0, so 00 is accepted.' 'Require month 01 through 12.'
V '033-component-numeric' 'Month is numeric but day or four-character year is nonnumeric.' @('frm1706:txtDateMonth','frm1706:txtDateDay','frm1706:txtDateYear') $null @('official-hta-runtime#validateMonthDayYearDate:L3945-L3961','official-hta-runtime#validate:L3206-L3210') 'incorrect-official-behavior' 'isNaN(result[0] || result[1] || result[2]) tests only the first truthy component; a four-letter year can pass all active checks.' 'Parse and validate each component independently.'
V '034-tin-blur' 'checkTIN receives a nondigit value.' @('TIN control passed to helper') 'Please enter a valid TIN' @('official-hta-runtime#checkTIN:L3474-L3481') 'official-bug-compatible' 'The helper clears the value; static segmented TIN controls are not shown invoking this full-value helper.' 'Validate without destructive clearing.'
V '035-tin-length' 'checkTIN receives fewer than 12 characters.' @('TIN control passed to helper') 'TIN should not be less 12 digits.' @('official-hta-runtime#checkTIN:L3482-L3488') 'incorrect-official-behavior' 'The grammar is defective and no checksum is checked.' 'Require the exact revision-appropriate TIN/branch shape and checksum.'
V '036-unreachable-after-return' 'enableSelTreaty finds neither Item 19 option selected.' @('frm1706:j_id394:_1','frm1706:j_id394:_2','frm1706:rdTreaty:_2') 'Please select an option for Item 19.' @('official-hta-runtime#enableSelTreaty:L3404-L3414') 'official-bug-compatible' 'The assignment that would default No appears after return and is unreachable.' 'Perform no hidden mutation; return a typed dependency error.'
Rule '1706-final-001' 'final-copy' 1 'Final Copy is requested after local validation.' @('txtFinalFlag','return-body') $null @('official-hta-runtime#saveXML-and-final-copy','encrypted-field-audit-v796') 'verified-correct' 'The reviewed encrypted artifact decrypts to exactly 122 unique keys.' 'Preserve all 122 fields losslessly and keep finalization distinct from transport.'
Rule '1706-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body') $null @('official-hta-runtime#saveXMLsubmit:L2419-L2632','official-hta-runtime#sendEmail') 'unverified' 'Transport exists but was not exercised.' 'Keep local validation/finalization independently testable.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Save returns on the first of four checks. Validate is source-ordered and returns on the first active failure; its two date failures return literal true, while later failures return undefined. Success disables controls before conditionally showing the success alert based on global flag.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Calc(
    [string]$Id, [string[]]$Outputs, [string[]]$Inputs, [string]$Formula, [string]$Trigger,
    [string[]]$Depends, [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Recommended = 'Use decimal arithmetic and recompute from authoritative inputs.',
    [string]$Condition = $null
) {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id = $Id; outputs = $Outputs; inputs = $Inputs; condition = $Condition
        official_formula = $Formula; rounding = 'Entry handlers call round(...,2); computed values use formatCurrency after NumWithComma parsing.'
        trigger = $Trigger; depends_on = $Depends; source_refs = $Refs; assessment = $Assessment
        recommended_app_behavior = $Recommended; confidence = 'high'
    })
}
Calc '1706-fmv-pair-max' @('frm1706:txtFMVLI') @('frm1706:txtFMVLand','frm1706:txtFMVImprovements','frm1706:txtFMVZonal','frm1706:txtFMVBIR') 'Item 30C = strict maximum of (29A+29B), (29C+29D), (29B+29C), and (29A+29D).' 'computeFMVLI -> compareFMVLI' @() @('official-hta-runtime#computeFMVLI:L3817-L3825','official-hta-runtime#compareFMVLI:L3826-L3857')
Calc '1706-fmv-tie-stale' @('frm1706:txtFMVLI') @('four pair sums') 'When the maximum is tied, none of the strict-greater branches assigns Item 30C, so its previous value remains.' 'compareFMVLI' @('1706-fmv-pair-max') @('official-hta-runtime#compareFMVLI:L3839-L3854') 'incorrect-official-behavior' 'Use a deterministic max operation that handles ties and never preserves stale state.'
Calc '1706-taxable-base-cash' @('frm1706:txtTax') @('frm1706:txtGross','frm1706:txtFMVLI') 'For the base cash path, Item 31 becomes max(Gross Selling Price, FMV of Land and Improvement); equal values end as FMVLI because the second independent >= branch overwrites the first.' 'computeTaxableBase' @('1706-fmv-pair-max') @('official-hta-runtime#computeTaxableBase:L3859-L3877') 'official-bug-compatible' 'Use an explicit max and record provenance without relying on overwrite order.'
Calc '1706-taxable-base-foreclosure' @('frm1706:txtTax') @('frm1706:txtGross','frm1706:txtFMVLI','frm1706:txtBid') 'When Item 21 Foreclosure is selected, Item 31 is the greatest of Gross, FMVLI, and Bid. Independent >= branches mean ties are resolved by source order, with Gross last.' 'computeTaxableBase' @('1706-fmv-pair-max') @('official-hta-runtime#computeTaxableBase:L3878-L3889') 'official-bug-compatible' 'Use a deterministic maximum with explicit provenance.' 'Item 21 Foreclosure Sale.'
Calc '1706-taxable-base-installment' @('frm1706:txtTax') @('frm1706:txtInstallment') 'When Item 21 Installment Sale is selected, Item 31 copies Item 30D.' 'computeTaxableBase' @() @('official-hta-runtime#computeTaxableBase:L3890-L3893') 'verified-correct' 'Copy the applicable typed amount.' 'Item 21 Installment Sale.'
Calc '1706-taxable-base-exempt-other' @('frm1706:txtTax') @('frm1706:txtOthers30F') 'When Item 21 Exempt or Others is selected, Item 31 copies Item 30F regardless of which Item 30 radio is selected.' 'computeTaxableBase' @() @('official-hta-runtime#computeTaxableBase:L3894-L3897') 'incorrect-official-behavior' 'Drive the base from the selected Item 30 method and validate transaction/method compatibility.' 'Item 21 Exempt or Others.'
Calc '1706-taxable-base-unutilized' @('frm1706:txtTax') @('frm1706:txtOthers30E') 'When Item 30E is selected, Item 31 copies Item 30E after all transaction-type overrides.' 'computeTaxableBase' @() @('official-hta-runtime#computeTaxableBase:L3898-L3901') 'verified-correct' 'Copy the applicable typed amount.' 'Item 30E selected.'
Calc '1706-tax-due-six-percent' @('frm1706:txtRate') @('frm1706:txtTax') 'Item 32 = Item 31 × 6%.' 'computeOfTaxDue' @('1706-taxable-base-cash') @('official-hta-runtime#computeOfTaxDue:L3423-L3429','official-help#tax-rate:L131-L145')
Calc '1706-tax-payable' @('frm1706:txtTaxDue') @('frm1706:txtRate','frm1706:txtLess') 'Item 34 = Item 32 - Item 33.' 'computeTaxPayable' @('1706-tax-due-six-percent') @('official-hta-runtime#computeTaxPayable:L3431-L3438')
Calc '1706-penalties' @('frm1706:txtTotalPenalties') @('frm1706:txtSurcharge','frm1706:txtInterest','frm1706:txtCompromise') 'Item 35D = 35A + 35B + 35C.' 'computePenalties' @() @('official-hta-runtime#computePenalties:L3440-L3450')
Calc '1706-total-amount' @('frm1706:txtTotal') @('frm1706:txtTaxDue','frm1706:txtTotalPenalties') 'Item 36 = Item 34 + Item 35D.' 'computeOfTotalAmtDue' @('1706-tax-payable','1706-penalties') @('official-hta-runtime#computeOfTotalAmtDue:L3451-L3469')
Calc '1706-overpayment-controls' @('frm1706:opt37:_1','frm1706:opt37:_2') @('frm1706:txtTotal') 'Enable Item 37 choices only when Item 36 < 0; otherwise disable and uncheck both.' 'computeOfTotalAmtDue' @('1706-total-amount') @('official-hta-runtime#computeOfTotalAmtDue:L3456-L3468')
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'
    schema_version = '1.0.0'; form_id = $formId; revision = $revision
    evaluation_order = @($calculations.calculation_id); calculations = $calculations
})

$negativeRules = @($rules | Where-Object { $_.exact_message } | Select-Object -First 40)
$cases = @()
$caseNumber = 0
foreach ($rule in $negativeRules) {
    $caseNumber++
    $cases += [pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id)
        phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }
        expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior
        rule_id = $rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; synthetic_only = $true; cases = $cases
})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    cases = @(
        @{ case_id = 'fmv-unique-max'; calculation_id = '1706-fmv-pair-max'; inputs = @{ A=100; B=20; C=90; D=10 }; pair_sums = @(120,100,110,110); official_output = 120 },
        @{ case_id = 'fmv-tied-max'; calculation_id = '1706-fmv-tie-stale'; inputs = @{ A=100; B=20; C=100; D=20; prior=999 }; pair_sums = @(120,120,120,120); official_output = 999 },
        @{ case_id = 'cash-base'; calculation_id = '1706-taxable-base-cash'; inputs = @{ gross=1000000; fmv=1200000 }; official_output = 1200000 },
        @{ case_id = 'foreclosure-base'; calculation_id = '1706-taxable-base-foreclosure'; inputs = @{ gross=1000000; fmv=1200000; bid=1300000 }; official_output = 1300000 },
        @{ case_id = 'six-percent'; calculation_id = '1706-tax-due-six-percent'; inputs = @{ taxable_base=1000000 }; official_output = 60000 },
        @{ case_id = 'negative-total'; calculation_id = '1706-overpayment-controls'; inputs = @{ tax_due=-100; penalties=0 }; official_item36=-100; item37_enabled=$true }
    )
})
$resources = @()
foreach ($src in @([regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object { $_.Groups['v'].Value } | Sort-Object -Unique)) {
    $full = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{
            src=$src; path=$full; present=$true; size=(Get-Item -LiteralPath $full).Length
            sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    } else {
        $resources += [pscustomobject][ordered]@{ src=$src; path=$full; present=$false; size=$null; sha256=$null }
    }
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{
    schema_version='1.0.0'; form_id=$formId; resources=$resources
})

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'; form_id = $formId; revision = $revision
    phases = @(
        @{ phase='edit'; official_behavior='January 2018 event-based capital-gains return for each onerous transfer of real property classified as a capital asset.'; source_refs=@('official-form-pdf','official-help#scope:L84-L98'); confidence='high' },
        @{ phase='saved-draft'; official_behavior='Save checks only seller TIN nonblankness, seller RDO not 000, seller name, and TCT/OCT/CCT number.'; source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3986-L4007'); confidence='high' },
        @{ phase='validated'; official_behavior='Validate runs source-ordered transaction, party, property, relief, taxable-base, and overpayment checks, then disables controls; the success alert also depends on global flag.'; source_refs=@('official-hta-runtime#validate:L3131-L3402'); confidence='high' },
        @{ phase='final-copy'; official_behavior='The reviewed encrypted artifact decrypts in memory to 122 unique flat keys; no plaintext editable save was available.'; source_refs=@('encrypted-field-audit-v796'); confidence='high' },
        @{ phase='submitted'; official_behavior='Online transport exists but was not exercised.'; source_refs=@('official-hta-runtime#saveXMLsubmit:L2419-L2632'); confidence='medium' }
    )
    transitions = @(
        @{ from='edit'; action='Save'; to='saved-draft'; guard='Four narrow Save checks pass.'; side_effects=@('Writes plaintext pseudo-XML in the official save directory.'); source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3986-L4007','official-hta-runtime#saveXML') },
        @{ from='edit'; action='Validate'; to='validated'; guard='All active source-ordered checks pass.'; side_effects=@('Disables controls.','Enables Edit and finalization actions through surrounding workflow.'); source_refs=@('official-hta-runtime#validate:L3131-L3402') },
        @{ from='validated'; action='Edit'; to='edit'; guard=$null; side_effects=@('Re-enables applicable controls, preserving locked identity/period fields when reopening.'); source_refs=@('official-hta-runtime#editForm:L2940-L3108') },
        @{ from='validated'; action='Final Copy'; to='final-copy'; guard='Official finalization flow permits progress.'; side_effects=@('Creates encrypted/compressed final copy; reviewed example has 122 keys.'); source_refs=@('encrypted-field-audit-v796') },
        @{ from='final-copy'; action='Transport'; to='submitted'; guard='Connectivity and remote acceptance succeed.'; side_effects=@('Attempts online submission; untested.'); source_refs=@('official-hta-runtime#sendEmail') }
    )
    prerequisites = @(
        'One return per real-property transaction or applicable installment payment',
        'Transaction date and amended-return selection',
        'Seller/buyer TIN, RDO, name, and registered-address information',
        'Property location, classification, title number, transaction description, and taxable-base method',
        'Overpayment disposition when Item 36 is negative'
    )
    required_attachments = @(
        @{ attachment_id='notarized-deed'; label='Copy of the notarized deed of sale or exchange.'; required_when='Every applicable filing.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='title-copy'; label='Owner copy for presentation with photocopy, or certified true copy, of TCT/CCT/OCT.'; required_when='Every applicable filing.'; official_ui_enforcement='Only the title number is locally required; file presence is not checked.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='tax-declaration'; label='Certified true copy of the latest tax declaration on lot and/or improvement.'; required_when='Every applicable filing.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='no-improvement-certification'; label='Assessor certification that no improvement exists or it belongs to another owner.'; required_when='Lot only is sold.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='bir-ruling'; label='Copy of BIR ruling confirming tax exemption.'; required_when='Tax exemption is claimed and a ruling applies.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='tax-debit-memo'; label='Duly approved Tax Debit Memo.'; required_when='Applicable.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='amended-proof'; label='Proof of tax payment and previously filed return.'; required_when='Amended return.'; official_ui_enforcement='Item 33 is enabled, but attachment presence is not checked.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' },
        @{ attachment_id='sworn-declaration'; label='Sworn Declaration of Intent prescribed by RR 13-99.'; required_when='Transaction is exempt because Items 17 and 18 apply.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L218-L243'); confidence='high' }
    )
    filing_deadlines = @(
        @{ quarter='Q1'; due_date_rule='Event-based: within 30 days after each sale, exchange, or disposition; for installment sale, within 30 days after first down payment or each later installment, as applicable.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' },
        @{ quarter='Q2'; due_date_rule='Not quarterly; the same transaction-relative 30-day rule applies.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' },
        @{ quarter='Q3'; due_date_rule='Not quarterly; the same transaction-relative 30-day rule applies.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' },
        @{ quarter='Q4'; due_date_rule='Not quarterly; the same transaction-relative 30-day rule applies.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1706; installed runtime used with January 2018 official PDF.'
    Asset 'official-help' 'official-runtime-help' $helpPath 'Content is Form 1706 guidance; HTA metadata is incorrectly labelled APPLICATIONNAME 0605.'
    Asset 'xml-final-v1' 'dummy-profile-encrypted-final-copy' $encryptedPath 'Reviewed 122-key final copy; decrypted in memory; values excluded.' (Join-Path $SaveDir '1706-final-copy-#email-redacted#.xml')
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1706.'
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version='1.0.0'; form_id=$formId; form_code='1706'; revision=$revision
    revision_label='January 2018'; package_version=$packageVersion; status='complete'
    official_assets=$assets
    counts=[ordered]@{
        concrete_fields=122; runtime_field_families=0; fields_total=$fields.Count; typed_fields=$fields.Count
        validation_rules=$rules.Count; confirmed_official_bugs=$bugCount; calculations=$calculations.Count
        negative_fixtures=$cases.Count; unverified_gaps=3
    }
    artifacts=[ordered]@{
        fields='fields.json'; validations='validations.json'; calculations='calculations.json'; workflow='workflow.json'
        evidence='evidence.md'; audit='audit.md'; gaps='gaps.md'
        runtime_control_fixture='fixtures/runtime-control-inventory-v796.json'
        encrypted_field_audit='fixtures/encrypted-field-audit-v796.json'
        validation_function_fixture='fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture='fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture='fixtures/official-resource-hashes-v796.json'
        negative_fixtures='fixtures/negative-cases.json'; calculation_fixtures='fixtures/calculation-boundaries.json'
    }
    scope_notes=@(
        'Research only; no renderer, typed model, migration, capability, or release metadata changed.',
        'No source values or email-bearing filenames are copied.',
        'The 122-key encrypted final copy is the only reviewed save artifact; no matching plaintext editable save was available.',
        'The exact January 2018 revision is pinned by the official PDF and source directory; the installed HTA identifies form code 1706 but carries no visible revision string.',
        'The help content is 1706-specific, but its HTA APPLICATIONNAME is incorrectly copied as 0605.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1706 - January 2018`n`nRevision-specific Offline eBIRForms rule package with 122 concrete final-copy keys and no active indexed field families. Source values and email-bearing filenames are excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- Installed HTA SHA-256: $($expected.hta); APPLICATIONNAME 1706.
- Official January 2018 PDF SHA-256: $($expected.pdf), valid PDF magic.
- Runtime help SHA-256: $($expected.help); content is 1706-specific, but HTA metadata incorrectly says APPLICATIONNAME 0605.
- Encrypted dummy final copy SHA-256: $($expected.encrypted); in-memory decrypted SHA-256 $($expected.decrypted); 122 unique keys; inventory SHA-256 $($expected.inventory); no values emitted.
- Runtime inventory: $($controls.Count) controls, $($serial.Count) serializer candidates, $($staticIds.Count) unique static IDs, and no active indexed/add-more field families.
- No existing typed 1706 model was found under crates/bir-core/src/forms; repository behavior was therefore not used as substitute official evidence.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. No matching plaintext editable save was available; the 122-key encrypted final-copy inventory is complete for the reviewed artifact, but editable-save subset differences are unobserved.
2. The installed HTA identifies form code 1706 but does not visibly state January 2018; revision binding relies on the pinned official PDF/source directory plus installed package provenance.
3. Online submission and external attachment presence were not exercised.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Revision/assets: **pass** - installed 1706 HTA/help, January 2018 PDF, package executable, and encrypted final copy are pinned.
- Fields: **pass** - all 122 decrypted final-copy keys are preserved; unknown/runtime-only fields remain lossless.
- Controls/functions: **pass** - $($controls.Count) controls, $($serial.Count) serializer candidates, $($staticIds.Count) static IDs, function inventories, and loaded resource hashes captured.
- Rules/workflow: **pass** - Save/Validate/Final Copy/Submit differences, exact first-error order, exact active messages, attachments, and 30-day event deadline captured.
- Calculations: **pass** - FMV pair maximum, taxable-base branches, 6% tax, amended payment, penalties, total, and overpayment enablement recorded.
- Official defects: **pass** - $bugCount bug-compatible/incorrect/obsolete rules include month 00, nonnumeric later date components, future-date checks commented out, unvalidated Other classification, unvalidated taxable-base value, strict-tie stale FMV, and help metadata copied from 0605.
- Privacy: **pass** - no values or email-bearing filenames copied.
- Plaintext save, explicit HTA revision string, online transport, and external attachment presence: **unverified** and explicit gaps.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 18: 1706-v2018. Next: 1606.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -LiteralPath $indexPath -Raw | ConvertFrom-Json
$entry = [pscustomobject][ordered]@{
    form_id=$formId; form_code='1706'; revision=$revision; package_version=$packageVersion
    priority=18; status='complete'; path='forms/1706-v2018/manifest.json'
}
$index.forms = @(@($index.forms | Where-Object { $_.form_id -ne $formId }) + $entry | Sort-Object priority)
$index.updated = (Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bug_classifications=$bugCount, controls=$($controls.Count), serial=$($serial.Count), static_ids=$($staticIds.Count), final_only=$($finalOnly.Count), static_only=$($staticOnly.Count)"
