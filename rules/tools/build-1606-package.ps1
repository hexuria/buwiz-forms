param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\1606v2018',
    [string]$SaveDir = 'C:\Mac\Home\Downloads\forms\1606'
)

$ErrorActionPreference = 'Stop'
$formId = '1606-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1606.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1606.hta'
$pdfPath = Join-Path $PdfDir '1606 Jan 2018 ENCS Final version.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1606-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'

$expected = @{
    hta = 'e7c7831e29cdd110a5cc325cb0bed7ee620684c21a483d59b25555c41d378a80'
    help = '152aa24f88b058d4b40ad89f2836eb8178ca2007446fbf41960611f047265c3b'
    pdf = '374eca083888f36ae18612741d8473c61376db44cd281318def831c73dadabfe'
    encrypted = 'cda554d5014fbc6953aa128de55acb6ffcf5fab99fe6cc65e7f1a709576881e5'
    decrypted = '78b8a1b615fb2145bbd633b02dd77c1a6ce474329aa56fecb9b8c79c30e810ea'
    inventory = '01323df7e0eb8d81a5e9002ef834e8c19fda86c25870bf2fe73ff293fc627fbe'
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
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1606["'']') { throw 'APPLICATIONNAME mismatch.' }
if ($help -notmatch '(?i)BIR\s+FORM\s+1606') { throw '1606 help content binding is absent.' }
if ($help -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']0605["'']') { throw 'Expected mislabelled help APPLICATIONNAME changed.' }
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$decrypted = Decrypt-Save $encryptedPath
$keys = @($decrypted.keys)
if ($decrypted.sha256 -ne $expected.decrypted) { throw 'Decrypted payload hash changed.' }
if ($keys.Count -ne 99 -or ($keys | Sort-Object -Unique).Count -ne 99) { throw "Expected 99 unique keys; found $($keys.Count)." }
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
if ($controls.Count -ne 120 -or $serial.Count -ne 101 -or $staticIds.Count -ne 99) {
    throw "Expected 120 controls/101 serializer candidates/99 unique static IDs; found $($controls.Count)/$($serial.Count)/$($staticIds.Count)."
}
if ($hta -match '(?i)setAttribute\s*\(\s*["''](?:id|name)["'']' -or $hta -match '(?i)Add\s+More') {
    throw 'Unexpected dynamic/add-more field construction appeared.'
}
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}

$requiredKeys = @(
    'frm1606:txtDateMonth','frm1606:txtDateDay','frm1606:txtDateYear',
    'frm1606:j_id217:_1','frm1606:j_id217:_2','frm1606:j_id252:_1','frm1606:j_id252:_2',
    'frm1606:txtTIN1','frm1606:txtTIN2','frm1606:txtTIN3','frm1606:txtBranchCode',
    'frm1606:txtRDOCode','frm1606:txtTINS1','frm1606:txtTINS2','frm1606:txtTINS3',
    'frm1606:txtBranchCodeS','frm1606:txtRDOCodeS','frm1606:txtBuyerName',
    'frm1606:txtSellerName','frm1606:txtBuyerAddress','frm1606:txtLocation',
    'frm1606:txtRDOCode16A','frm1606:txtTCT','frm1606:optATC13_2',
    'frm1606:optATC13_3','frm1606:j_id392:_1','frm1606:j_id392:_2'
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
    } elseif ($Key -match '(?i)(txtSelling|txtCost|txtMortgage|txtTotalP|txtAmount|txtFMV|txtGross|txtBid|txtInstallment|txtOthers28E|txtTax$|txtTaxRate|txtTaxR|txtLess|txtTaxDue|txtSurcharge|txtInterest|txtCompromise|txtTotalPenalties|txtTotal$)') {
        $logical = 'decimal-amount'
        $normalization = [string[]]@('NumWithComma', 'formatCurrency', 'round(...,2)')
    } elseif ($Key -match '(?i)email') {
        $logical = 'email-string'
    }
    $computed = $Key -match '(?i)(txtFMVLI|txtTax$|txtTaxR|txtTaxDue|txtTotalPenalties|txtTotal$)'
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
    'j_id252:_1'='4'; 'j_id252:_2'='4'; 'txtTIN1'='5'; 'txtTIN2'='5'; 'txtTIN3'='5'; 'txtBranchCode'='5'
    'txtRDOCode'='6'; 'txtTINS1'='7'; 'txtTINS2'='7'; 'txtTINS3'='7'; 'txtBranchCodeS'='7'
    'txtRDOCodeS'='8'; 'txtBuyerName'='9'; 'txtSellerName'='10'; 'txtBuyerAddress'='11'; 'txtSellerAddress'='12'
    'txt13'='13'; 'txt13C'='13'; 'optATC13_2'='13'; 'optATC13_3'='13'; 'txtLocation'='16'; 'txtRDOCode16A'='16A'
    'txtTCT'='17'; 'txtArea'='17'; 'txtTaxDC'='17'; 'txtOthers'='17'; 'txtOthers20'='20'; 'txtSelling'='21'
    'txtCost'='22'; 'txtMortgage'='23'; 'txtTotalP'='24'; 'txtAmount'='25'; 'txtTotalN'='26'
    'txtFMVLand'='27A'; 'txtFMVImprovements'='27B'; 'txtFMVZonal'='27C'; 'txtFMVBIR'='27D'; 'txtFMVLI'='28B'
    'txtGross'='28A'; 'txtBid'='28C'; 'txtInstallment'='28D'; 'txtOthers28E'='28E'; 'txtOtherss28E'='28E'
    'Habitual_1'='29'; 'Habitual_2'='29'; 'txtTax'='30'; 'txtTaxRate'='31'; 'txtTaxR'='32'; 'txtLess'='33'; 'txtTaxDue'='34'; 'txtSurcharge'='35A'; 'txtInterest'='35B'
    'txtCompromise'='35C'; 'txtTotalPenalties'='35D'; 'txtTotal'='36'
}
$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Get-FieldMeta $key $control
    $short = if ($key -like 'frm1606:*') { $key.Substring(8) } else { $key }
    $item = if ($itemMap.ContainsKey($short)) { $itemMap[$short] } else { $null }
    $requiredWhen = $null
    if ($key -match 'frm1606:j_id393_7') { $requiredWhen = 'Item 15 classification is Others.' }
    elseif ($key -match 'frm1606:txtOthers20') { $requiredWhen = 'Item 20 is Exempt or Others.' }
    elseif ($key -match 'frm1606:selTreaty') { $requiredWhen = 'Item 19 indicates treaty or special-law relief.' }
    elseif ($key -match 'frm1606:opt37:') { $requiredWhen = 'Item 36 is negative.' }
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
        notes = @('Observed in the reviewed 99-key decrypted final-copy inventory; source value is excluded.')
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = 99
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
    source_path = (Join-Path $SaveDir '1606-final-copy-#email-redacted#.xml')
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
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1606:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final|date|tin') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1606:' -NamePattern '(?i)compute|compare|round|fmv|tax|total|capital') -join [Environment]::NewLine)

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
Rule '1606-save-001-buyer-tin' 'save' 1 'Any buyer TIN segment or branch code is blank.' @('frm1606:txtTIN1','frm1606:txtTIN2','frm1606:txtTIN3','frm1606:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L3535-L3539') 'incorrect-official-behavior' 'Only nonblankness is checked.' 'Allow drafts, but require exact segment lengths, digits, and checksum before finalization.'
Rule '1606-save-002-buyer-rdo' 'save' 2 'Buyer RDO equals literal 000.' @('frm1606:txtRDOCode') "Please enter a valid Buyer's RDO Code on Item 6." @('official-hta-runtime#initialValidateBeforeSave:L3540-L3543')
Rule '1606-save-003-buyer-name' 'save' 3 'Buyer name is blank.' @('frm1606:txtBuyerName') "Please enter a valid Buyer's Name on Item 9." @('official-hta-runtime#initialValidateBeforeSave:L3544-L3547')
Rule '1606-save-004-title-number' 'save' 4 'TCT/OCT/CCT number is blank.' @('frm1606:txtTCT') 'Please enter the TCT/OCT/CCT No.' @('official-hta-runtime#initialValidateBeforeSave:L3548-L3551')
Rule '1606-save-005-omissions' 'save' 5 'Any other identity, transaction, tax-base, or calculation field is missing or malformed.' @('return-body') $null @('official-hta-runtime#initialValidateBeforeSave:L3535-L3553') 'official-bug-compatible' 'Save performs only the four checks above.' 'Keep lossless draft persistence distinct from Validate and Final Copy.'

$order = 0
function V(
    [string]$Suffix, [string]$Condition, [string[]]$Fields, $Message, [string[]]$Refs,
    [string]$Assessment = 'verified-correct', [string]$Official = 'The branch alerts and returns.',
    [string]$Recommended = 'Retain with revision-aware wording.'
) {
    $script:order++
    Rule "1606-validate-$Suffix" 'validate' $script:order $Condition $Fields $Message $Refs $Assessment $Official $Recommended
}
V '001-month-width' 'Transaction month has exactly one character.' @('frm1606:txtDateMonth') 'Please enter a valid month on item 1. Format should be MM/DD/YYYY.' @('official-hta-runtime#validate:L2915-L2921')
V '002-day-width' 'Transaction day has exactly one character.' @('frm1606:txtDateDay') 'Please enter a valid day on item 1. Format should be MM/DD/YYYY.' @('official-hta-runtime#validate:L2915-L2925')
V '003-nonleap-feb29' 'The year is not leap and Item 1 is February 29.' @('frm1606:txtDateMonth','frm1606:txtDateDay','frm1606:txtDateYear') 'Filing year is not a leap year.' @('official-hta-runtime#validate:L2926-L2931')
V '004-invalid-february' 'February day exceeds the active leap/non-leap bound.' @('frm1606:txtDateMonth','frm1606:txtDateDay','frm1606:txtDateYear') 'Invalid date entry on item 1.' @('official-hta-runtime#validate:L2932-L2939')
V '005-invalid-month-day' 'Day exceeds 31 for a 31-day month or 30 for a 30-day month.' @('frm1606:txtDateMonth','frm1606:txtDateDay') 'Invalid date entry on item 1.' @('official-hta-runtime#validate:L2940-L2950')
V '006-month-upper-bound' 'Transaction month coerces above 12.' @('frm1606:txtDateMonth') 'Invalid month entry on Item no.1. Please enter a valid month.' @('official-hta-runtime#validate:L2966-L2970')
V '007-month-required' 'Transaction month is blank.' @('frm1606:txtDateMonth') 'Please enter a valid month on Item 1.' @('official-hta-runtime#validate:L2971-L2975')
V '008-day-required' 'Transaction day is blank.' @('frm1606:txtDateDay') 'Please enter a valid day on Item 1.' @('official-hta-runtime#validate:L2976-L2980')
V '009-year-required' 'Transaction year is blank.' @('frm1606:txtDateYear') 'Please enter a valid year on Item 1.' @('official-hta-runtime#validate:L2981-L2985')
V '010-year-minimum' 'Transaction year coerces below 1904.' @('frm1606:txtDateYear') 'Invalid year entry on Item no.1. Year should not be lower than 1904.' @('official-hta-runtime#validate:L2986-L2990') 'incorrect-official-behavior' 'A stale 1904 lower bound is enforced without revision justification.' 'Use an evidence-backed legal range.'
V '011-amended' 'Neither amended-return Yes nor No is selected.' @('frm1606:j_id217:_1','frm1606:j_id217:_2') 'Please choose amended return on item 2.' @('official-hta-runtime#validate:L2991-L2995')
V '012-item4' 'Neither Any Taxes Withheld option is selected.' @('frm1606:j_id252:_1','frm1606:j_id252:_2') 'Please select an option for Item 4.' @('official-hta-runtime#validate:L2996-L3000')
V '013-buyer-tin' 'Any buyer TIN segment or branch code is blank.' @('frm1606:txtTIN1','frm1606:txtTIN2','frm1606:txtTIN3','frm1606:txtBranchCode') "Please enter the Buyer's TIN." @('official-hta-runtime#validate:L3001-L3020') 'incorrect-official-behavior' 'Four repeated branches check blankness only.' 'Validate complete shape and checksum once.'
V '014-buyer-rdo' 'Buyer RDO selectedIndex is zero.' @('frm1606:txtRDOCode') "Please enter the Buyer's RDO Code." @('official-hta-runtime#validate:L3021-L3025')
V '015-seller-tin' 'Any seller TIN segment or branch code is blank.' @('frm1606:txtTINS1','frm1606:txtTINS2','frm1606:txtTINS3','frm1606:txtBranchCodeS') "Please enter the Seller's TIN." @('official-hta-runtime#validate:L3026-L3040') 'incorrect-official-behavior' 'Four repeated branches check blankness only.' 'Validate complete shape and checksum once.'
V '016-different-tins' 'Concatenated buyer and seller TIN+branch strings compare equal.' @('buyer-tin','seller-tin') 'TIN for Buyer and Seller should be different.' @('official-hta-runtime#validate:L3041-L3044')
V '017-seller-rdo' 'Seller RDO selectedIndex is zero.' @('frm1606:txtRDOCodeS') "Please enter the Seller's RDO Code." @('official-hta-runtime#validate:L3045-L3048')
V '018-buyer-name' 'Buyer name is blank.' @('frm1606:txtBuyerName') "Please enter the Buyer's Name." @('official-hta-runtime#validate:L3049-L3052')
V '019-seller-name' 'Seller name is blank.' @('frm1606:txtSellerName') "Please enter the Seller's Name." @('official-hta-runtime#validate:L3053-L3056')
V '020-buyer-address' 'Buyer address is blank.' @('frm1606:txtBuyerAddress') "Please enter the Buyer's Address." @('official-hta-runtime#validate:L3057-L3060')
V '021-atc' 'Neither Individual WI155 nor Corporation WC155 is selected.' @('frm1606:optATC13_2','frm1606:optATC13_3') 'Please select an option for Item 13.' @('official-hta-runtime#validate:L3061-L3064')
V '022-agent-category' 'Neither Private nor Government withholding-agent category is selected.' @('frm1606:j_id392:_1','frm1606:j_id392:_2') 'Please select an option for Item 14.' @('official-hta-runtime#validate:L3065-L3068')
V '023-classification' 'No Item 15 classification radio is selected.' @('frm1606:j_id393:_1','frm1606:j_id393:_2','frm1606:j_id393:_3','frm1606:j_id393:_4','frm1606:j_id393:_5','frm1606:j_id393:_6','frm1606:j_id393_8') 'Please select an option for Item 15.' @('official-hta-runtime#validate:L3069-L3072')
V '024-location' 'Property location is blank.' @('frm1606:txtLocation') 'Please enter the Location of the Property on Item 16.' @('official-hta-runtime#validate:L3073-L3076')
V '025-property-rdo' 'Item 16A RDO selectedIndex is zero.' @('frm1606:txtRDOCode16A') 'Please enter the RDO Code on Item 16A.' @('official-hta-runtime#validate:L3077-L3080')
V '026-title-number' 'TCT/OCT/CCT number is blank.' @('frm1606:txtTCT') 'Please enter the TCT/OCT/CCT No.' @('official-hta-runtime#validate:L3081-L3084')
V '027-treaty' 'Treaty relief Yes is selected and the relief dropdown selectedIndex is zero.' @('frm1606:rdTreaty:_1','frm1606:selTreaty') 'Please select a tax relief for Item 19.' @('official-hta-runtime#validate:L3085-L3091')
V '028-success' 'All active checks pass.' @('return-body') "Validation successful. Click on 'Edit' if you wish to modify your entries." @('official-hta-runtime#validate:L3123-L3135')
V '029-future-date-commented' 'Transaction date is after the current date.' @('frm1606:txtDateMonth','frm1606:txtDateDay','frm1606:txtDateYear') 'Invalid date entry on Item no.1. Entry should not be later than Current Date.' @('official-hta-runtime#validate:L2951-L2965') 'obsolete' 'All future-date branches are commented out.' 'Reject future transaction dates when legally required.'
V '030-month-day-zero' 'Month or day is 00 with otherwise width-valid components.' @('frm1606:txtDateMonth','frm1606:txtDateDay') $null @('official-hta-runtime#validate:L2915-L2990') 'incorrect-official-behavior' 'No lower-bound check exists, so 00 can pass.' 'Require month and day to begin at 1.'
V '031-component-numeric' 'A later date component is nonnumeric but has the expected width.' @('frm1606:txtDateMonth','frm1606:txtDateDay','frm1606:txtDateYear') $null @('official-hta-runtime#validate:L2915-L2990') 'incorrect-official-behavior' 'JavaScript coercion lets some nonnumeric values bypass comparisons.' 'Parse every component independently.'
V '032-classification-other-description' 'Item 15 Others is selected and its enabled description is blank.' @('frm1606:j_id393_8','frm1606:j_id393_7') $null @('official-hta-runtime#validate:L3069-L3072','official-hta-runtime#control:L905-L906') 'incorrect-official-behavior' 'Validate requires the radio only.' 'Require the description.'
V '033-seller-address-omitted' 'Seller address is blank.' @('frm1606:txtSellerAddress') $null @('official-hta-runtime#validate:L3049-L3060') 'incorrect-official-behavior' 'Only buyer address is checked.' 'Require the legally applicable seller address.'
V '034-item18-omitted' 'Neither Item 18 multi-property option is selected.' @('frm1606:j_id394:_1','frm1606:j_id394:_2') $null @('official-hta-runtime#validate:L2991-L3091') 'incorrect-official-behavior' 'Validate never checks Item 18.' 'Require an explicit answer where applicable.'
V '035-transaction-omitted' 'No Item 20 transaction-description radio is selected, or Exempt/Others has no description.' @('frm1606:j_id395:_1','frm1606:j_id395:_2','frm1606:j_id395:_3','frm1606:j_id395:_4','frm1606:j_id395:_5','frm1606:txtOthers20') $null @('official-hta-runtime#validate:L2991-L3091','official-hta-runtime#control:L1086-L1110') 'incorrect-official-behavior' 'Validate never checks Item 20.' 'Require one transaction type and its conditional description.'
V '036-fmv-commented' 'Cash Sale is selected and an Item 27 FMV amount is blank or nonpositive.' @('frm1606:j_id395:_1','frm1606:txtFMVLand','frm1606:txtFMVImprovements','frm1606:txtFMVZonal','frm1606:txtFMVBIR') 'Please provide a valid value for 27A.' @('official-hta-runtime#validate:L3093-L3115') 'obsolete' 'All four branches are block-commented.' 'Apply evidence-backed FMV completeness rules.'
V '037-fmv-combination-commented' 'The disallowed Item 27 checkbox pair A+C or B+D is selected.' @('frm1606:opt27A','frm1606:opt27B','frm1606:opt27C','frm1606:opt27D') 'Selected combination on Item 27 is not allowed.' @('official-hta-runtime#validate:L3117-L3121') 'obsolete' 'The entire combination check is commented out.' 'Validate permitted valuation combinations explicitly.'
V '038-taxable-base-omitted' 'No Item 28 radio is selected or the applicable amount/description is blank, zero, stale, or disabled.' @('frm1606:opt28A','frm1606:opt28B','frm1606:opt28C','frm1606:opt28D','frm1606:opt28E','item-28-fields') $null @('official-hta-runtime#validate:L2991-L3135','official-hta-runtime#computeTaxableBase:L3443-L3473') 'incorrect-official-behavior' 'Validate never checks Item 28 or computed Item 30.' 'Require a compatible selection and authoritative source amount, then recompute.'
V '039-habitual-omitted' 'Neither Item 29 habitual-real-estate-business option is selected.' @('frm1606:Habitual_1','frm1606:Habitual_2') $null @('official-hta-runtime#validate:L2991-L3135','official-hta-runtime#control:L1247-L1263') 'incorrect-official-behavior' 'Validate never checks Item 29.' 'Require the legally applicable answer.'
V '040-tax-rate-omitted' 'Item 4 is Yes but Item 31 tax-rate selection is blank or incompatible.' @('frm1606:j_id252:_1','frm1606:txtTaxRate') $null @('official-hta-runtime#enablePart2:L3518-L3533','official-hta-runtime#validate:L2991-L3135') 'incorrect-official-behavior' 'Validate does not constrain the enabled tax-rate select.' 'Require a supported rate tied to ATC and transaction facts.'
V '041-overpayment-choice-omitted' 'Item 36 is negative and neither Item 37 option is selected.' @('frm1606:txtTotal','frm1606:opt37:_1','frm1606:opt37:_2') $null @('official-hta-runtime#computeOfTotalAmtDue:L3176-L3191','official-hta-runtime#validate:L2991-L3135') 'incorrect-official-behavior' 'Controls are enabled, but Validate never requires a choice.' 'Require one disposition when overremittance exists.'
V '042-tin-blur-nondigit' 'checkTIN receives a nondigit value.' @('TIN control passed to helper') 'Please enter a valid TIN' @('official-hta-runtime#checkTIN:L3195-L3201') 'official-bug-compatible' 'The helper clears the value.' 'Validate without destructive clearing.'
V '043-tin-blur-length' 'checkTIN receives fewer than 12 characters.' @('TIN control passed to helper') 'TIN should not be less 12 digits.' @('official-hta-runtime#checkTIN:L3202-L3207') 'incorrect-official-behavior' 'No checksum is checked.' 'Require exact shape and checksum.'
V '044-tin-length-helper' 'checkTINlength receives fewer than 12 characters.' @('TIN control passed to helper') 'TIN not valid' @('official-hta-runtime#checkTINlength:L3222-L3228') 'official-bug-compatible' 'The focus assignment after return is unreachable.' 'Return a structured error without hidden focus mutation.'
V '045-treaty-unreachable' 'enableSelTreaty finds neither Item 14 option selected.' @('frm1606:j_id392:_1','frm1606:j_id392:_2','frm1606:rdTreaty:_2') 'Please select an option for Item 14.' @('official-hta-runtime#enableSelTreaty:L3138-L3148') 'official-bug-compatible' 'The assignment after return is unreachable.' 'Return a typed dependency error.'
Rule '1606-final-001' 'final-copy' 1 'Final Copy is requested after local validation.' @('txtFinalFlag','return-body') $null @('official-hta-runtime#saveXML-and-final-copy','encrypted-field-audit-v796') 'verified-correct' 'The reviewed encrypted artifact decrypts to exactly 99 unique keys.' 'Preserve all 99 fields losslessly and keep finalization distinct from transport.'
Rule '1606-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body') $null @('official-hta-runtime#saveXMLsubmit','official-hta-runtime#sendEmail') 'unverified' 'Transport exists but was not exercised.' 'Keep local validation/finalization independently testable.' 'medium'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Save returns on the first of four checks. Validate is source-ordered and returns on the first active failure; success disables controls and shows the exact success alert.'
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
Calc '1606-fmv-pair-max' @('frm1606:txtFMVLI') @('frm1606:txtFMVLand','frm1606:txtFMVImprovements','frm1606:txtFMVZonal','frm1606:txtFMVBIR') 'Item 28B = max(Item 27A + 27B, Item 27C + 27D); ties take the C+D branch. The two cross-pair sums printed in the UI note are commented out in code.' 'computeFMVLI -> compareFMVLI' @() @('official-hta-runtime#computeFMVLI:L3393-L3400','official-hta-runtime#compareFMVLI:L3401-L3438') 'incorrect-official-behavior' 'Implement legally supported valuation combinations and make the printed-instruction discrepancy explicit.'
Calc '1606-taxable-base-cash' @('frm1606:txtTax') @('frm1606:txtGross','frm1606:txtFMVLI') 'Base Item 30 = max(Item 28A Gross, Item 28B FMVLI); equality ends as FMVLI because the second independent >= branch overwrites the first.' 'computeTaxableBase' @('1606-fmv-pair-max') @('official-hta-runtime#computeTaxableBase:L3443-L3459') 'official-bug-compatible' 'Use an explicit maximum and preserve source provenance.'
Calc '1606-taxable-base-foreclosure' @('frm1606:txtTax') @('frm1606:txtFMVLI','frm1606:txtBid') 'When Item 20 Foreclosure is selected, Item 30 is overwritten with max(Item 28B FMVLI, Item 28C Bid), ignoring Gross even if it is larger.' 'computeTaxableBase' @('1606-fmv-pair-max') @('official-hta-runtime#computeTaxableBase:L3460-L3465') 'incorrect-official-behavior' 'Use the legally authoritative foreclosure-base inputs.' 'Item 20 Foreclosure Sale.'
Calc '1606-taxable-base-installment' @('frm1606:txtTax','frm1606:txtSelling') @('frm1606:txtInstallment','frm1606:txtGross') 'When Item 20 Installment Sale is selected, Item 21 first copies Item 28A Gross and Item 30 then copies Item 28D Installment.' 'computeTaxableBase' @() @('official-hta-runtime#computeTaxableBase:L3443-L3446','official-hta-runtime#computeTaxableBase:L3466-L3468') 'official-bug-compatible' 'Copy authoritative typed amounts without unrelated side effects.' 'Item 20 Installment Sale.'
Calc '1606-taxable-base-exempt-other' @('frm1606:txtTax') @('frm1606:txtOthers28E') 'When Item 20 Exempt or Others is selected, Item 30 copies the numeric Item 28E amount regardless of which Item 28 radio is selected.' 'computeTaxableBase' @() @('official-hta-runtime#computeTaxableBase:L3469-L3471') 'incorrect-official-behavior' 'Require compatible transaction and base selections.' 'Item 20 Exempt or Others.'
Calc '1606-tax-required' @('frm1606:txtTaxR') @('frm1606:txtTax','frm1606:txtTaxRate') 'Item 32 = Item 30 taxable base multiplied by the user-selected Item 31 percentage and divided by 100.' 'computeOfTaxRequired' @('1606-taxable-base-cash') @('official-hta-runtime#computeOfTaxRequired:L3156-L3161')
Calc '1606-tax-still-due' @('frm1606:txtTaxDue') @('frm1606:txtTaxR','frm1606:txtLess') 'Item 34 = Item 32 - Item 33.' 'computeTaxDue' @('1606-tax-required') @('official-hta-runtime#computeTaxDue:L3162-L3167')
Calc '1606-penalties' @('frm1606:txtTotalPenalties') @('frm1606:txtSurcharge','frm1606:txtInterest','frm1606:txtCompromise') 'Item 35D = 35A + 35B + 35C.' 'computePenalties' @() @('official-hta-runtime#computePenalties:L3169-L3175')
Calc '1606-total-amount' @('frm1606:txtTotal') @('frm1606:txtTaxDue','frm1606:txtTotalPenalties') 'Item 36 = Item 34 + Item 35D.' 'computeOfTotalAmtDue' @('1606-tax-still-due','1606-penalties') @('official-hta-runtime#computeOfTotalAmtDue:L3176-L3191')
Calc '1606-overpayment-controls' @('frm1606:opt37:_1','frm1606:opt37:_2') @('frm1606:txtTotal') 'Enable Item 37 choices only when Item 36 < 0; otherwise disable and uncheck both.' 'computeOfTotalAmtDue' @('1606-total-amount') @('official-hta-runtime#computeOfTotalAmtDue:L3180-L3189')
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
        @{ case_id = 'fmv-unique-max'; calculation_id = '1606-fmv-pair-max'; inputs = @{ A=100; B=20; C=90; D=10 }; official_pair_sums = @(120,100); official_output = 120 },
        @{ case_id = 'fmv-tied-max'; calculation_id = '1606-fmv-pair-max'; inputs = @{ A=100; B=20; C=100; D=20 }; official_pair_sums = @(120,120); official_output = 120; tie_branch = 'C+D' },
        @{ case_id = 'cash-base'; calculation_id = '1606-taxable-base-cash'; inputs = @{ gross=1000000; fmv=1200000 }; official_output = 1200000 },
        @{ case_id = 'foreclosure-ignores-gross'; calculation_id = '1606-taxable-base-foreclosure'; inputs = @{ gross=1500000; fmv=1200000; bid=1300000 }; official_output = 1300000 },
        @{ case_id = 'selected-rate'; calculation_id = '1606-tax-required'; inputs = @{ taxable_base=1000000; rate=1.5 }; official_output = 15000 },
        @{ case_id = 'negative-total'; calculation_id = '1606-overpayment-controls'; inputs = @{ tax_due=-100; penalties=0 }; official_item36=-100; item37_enabled=$true }
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
        @{ phase='edit'; official_behavior='January 2018 monthly withholding-tax remittance return for onerous transfer of real property other than a capital asset, including taxable and exempt transfers.'; source_refs=@('official-form-pdf','official-help#scope:L84-L98'); confidence='high' },
        @{ phase='saved-draft'; official_behavior='Save checks only buyer TIN nonblankness, buyer RDO not 000, buyer name, and TCT/OCT/CCT number.'; source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3535-L3553'); confidence='high' },
        @{ phase='validated'; official_behavior='Validate runs its source-ordered date, identity, ATC, agent, classification, property, and treaty checks, then disables controls; it omits Items 18, 20, 28, 29, tax-rate compatibility, and overremittance disposition.'; source_refs=@('official-hta-runtime#validate:L2911-L3136'); confidence='high' },
        @{ phase='final-copy'; official_behavior='The reviewed encrypted artifact decrypts in memory to 99 unique flat keys; no plaintext editable save was available.'; source_refs=@('encrypted-field-audit-v796'); confidence='high' },
        @{ phase='submitted'; official_behavior='Online transport exists but was not exercised.'; source_refs=@('official-hta-runtime#saveXMLsubmit:L2419-L2632'); confidence='medium' }
    )
    transitions = @(
        @{ from='edit'; action='Save'; to='saved-draft'; guard='Four narrow Save checks pass.'; side_effects=@('Writes plaintext pseudo-XML in the official save directory.'); source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3535-L3553','official-hta-runtime#saveXML') },
        @{ from='edit'; action='Validate'; to='validated'; guard='All active source-ordered checks pass.'; side_effects=@('Disables controls.','Enables Edit and finalization actions through surrounding workflow.'); source_refs=@('official-hta-runtime#validate:L2911-L3136') },
        @{ from='validated'; action='Edit'; to='edit'; guard=$null; side_effects=@('Re-enables applicable controls, while identity/period fields may remain locked when reopening a saved file.'); source_refs=@('official-hta-runtime#enableAllControl:L2753-L2889') },
        @{ from='validated'; action='Final Copy'; to='final-copy'; guard='Official finalization flow permits progress.'; side_effects=@('Creates encrypted/compressed final copy; reviewed example has 99 keys.'); source_refs=@('encrypted-field-audit-v796') },
        @{ from='final-copy'; action='Transport'; to='submitted'; guard='Connectivity and remote acceptance succeed.'; side_effects=@('Attempts online submission; untested.'); source_refs=@('official-hta-runtime#sendEmail') }
    )
    prerequisites = @(
        'One monthly remittance return for applicable onerous transfers of real property other than capital assets',
        'Transaction date and amended-return selection',
        'Seller/buyer TIN, RDO, name, and registered-address information',
        'Property location, classification, title number, transaction description, and taxable-base method',
        'Overpayment disposition when Item 36 is negative'
    )
    required_attachments = @(
        @{ attachment_id='notarized-deed'; label='Notarized deed of sale or transfer.'; required_when='Every applicable filing.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' },
        @{ attachment_id='title-copy'; label='Owner copy for presentation with photocopy, or certified true copy, of TCT/CCT/OCT.'; required_when='Every applicable filing.'; official_ui_enforcement='Only the title number is locally required.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' },
        @{ attachment_id='tax-declaration'; label='Latest tax declaration.'; required_when='Every applicable filing.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' },
        @{ attachment_id='lot-only-certification'; label='Assessor certification when only a lot is transferred.'; required_when='Lot only.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' },
        @{ attachment_id='bir-ruling'; label='BIR ruling supporting exemption.'; required_when='Exemption is claimed.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' },
        @{ attachment_id='housing-agency-proof'; label='HLURB/HUDCC proof.'; required_when='Applicable housing transaction.'; official_ui_enforcement='Not checked by local Validate.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' },
        @{ attachment_id='amended-proof'; label='Proof of payment and previously filed return.'; required_when='Amended return.'; official_ui_enforcement='Item 33 is enabled, but attachment presence is not checked.'; source_refs=@('official-help#attachments:L177-L199'); confidence='high' }
    )
    filing_deadlines = @(
        @{ quarter='Q1'; due_date_rule='Monthly: on or before the 10th day following the end of the transaction month; large taxpayers use the 25th day of the following month.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' },
        @{ quarter='Q2'; due_date_rule='Monthly: the same 10th-day rule, or 25th for large taxpayers.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' },
        @{ quarter='Q3'; due_date_rule='Monthly: the same 10th-day rule, or 25th for large taxpayers.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' },
        @{ quarter='Q4'; due_date_rule='Monthly; December withholding is due January 25, and large taxpayers use the 25th-day rule.'; source_refs=@('official-help#deadline:L99-L116'); confidence='high' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1606; installed runtime used with January 2018 official PDF.'
    Asset 'official-help' 'official-runtime-help' $helpPath 'Content is Form 1606 guidance; HTA metadata is incorrectly labelled APPLICATIONNAME 0605.'
    Asset 'xml-final-v1' 'dummy-profile-encrypted-final-copy' $encryptedPath 'Reviewed 99-key final copy; decrypted in memory; values excluded.' (Join-Path $SaveDir '1606-final-copy-#email-redacted#.xml')
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1606.'
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version='1.0.0'; form_id=$formId; form_code='1606'; revision=$revision
    revision_label='January 2018'; package_version=$packageVersion; status='complete'
    official_assets=$assets
    counts=[ordered]@{
        concrete_fields=99; runtime_field_families=0; fields_total=$fields.Count; typed_fields=$fields.Count
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
        'The 99-key encrypted final copy is the only reviewed save artifact; no matching plaintext editable save was available.',
        'The exact January 2018 revision is pinned by the official PDF and source directory; the installed HTA identifies form code 1606 but carries no visible revision string.',
        'The help content is 1606-specific, but its HTA APPLICATIONNAME is incorrectly copied as 0605.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1606 - January 2018`n`nRevision-specific Offline eBIRForms rule package with 99 concrete final-copy keys and no active indexed field families. Source values and email-bearing filenames are excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- Installed HTA SHA-256: $($expected.hta); APPLICATIONNAME 1606.
- Official January 2018 PDF SHA-256: $($expected.pdf), valid PDF magic.
- Runtime help SHA-256: $($expected.help); content is 1606-specific, but HTA metadata incorrectly says APPLICATIONNAME 0605.
- Encrypted dummy final copy SHA-256: $($expected.encrypted); in-memory decrypted SHA-256 $($expected.decrypted); 99 unique keys; inventory SHA-256 $($expected.inventory); no values emitted.
- Runtime inventory: $($controls.Count) controls, $($serial.Count) serializer candidates, $($staticIds.Count) unique static IDs, and no active indexed/add-more field families.
- No existing typed 1606 model was found under crates/bir-core/src/forms; repository behavior was therefore not used as substitute official evidence.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. No matching plaintext editable save was available; the 99-key encrypted final-copy inventory is complete for the reviewed artifact, but editable-save subset differences are unobserved.
2. The installed HTA identifies form code 1606 but does not visibly state January 2018; revision binding relies on the pinned official PDF/source directory plus installed package provenance.
3. Online submission and external attachment presence were not exercised.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Revision/assets: **pass** - installed 1606 HTA/help, January 2018 PDF, package executable, and encrypted final copy are pinned.
- Fields: **pass** - all 99 decrypted final-copy keys are preserved; unknown/runtime-only fields remain lossless.
- Controls/functions: **pass** - $($controls.Count) controls, $($serial.Count) serializer candidates, $($staticIds.Count) static IDs, function inventories, and loaded resource hashes captured.
- Rules/workflow: **pass** - Save/Validate/Final Copy/Submit differences, exact first-error order, exact active messages, attachments, and monthly deadlines captured.
- Calculations: **pass** - two-pair FMV maximum, taxable-base branches, selected-rate tax, amended payment, penalties, total, and overpayment enablement recorded.
- Official defects: **pass** - $bugCount bug-compatible/incorrect/obsolete rules include zero/nonnumeric date components, future-date checks commented out, unvalidated Items 18/20/28/29/37, cross-pair FMV sums commented despite printed text, foreclosure Gross overwrite, and help metadata copied from 0605.
- Privacy: **pass** - no values or email-bearing filenames copied.
- Plaintext save, explicit HTA revision string, online transport, and external attachment presence: **unverified** and explicit gaps.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 19: 1606-v2018. Next: 1800.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -LiteralPath $indexPath -Raw | ConvertFrom-Json
$entry = [pscustomobject][ordered]@{
    form_id=$formId; form_code='1606'; revision=$revision; package_version=$packageVersion
    priority=19; status='complete'; path='forms/1606-v2018/manifest.json'
}
$index.forms = @(@($index.forms | Where-Object { $_.form_id -ne $formId }) + $entry | Sort-Object priority)
$index.updated = (Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bug_classifications=$bugCount, controls=$($controls.Count), serial=$($serial.Count), static_ids=$($staticIds.Count), final_only=$($finalOnly.Count), static_only=$($staticOnly.Count)"
