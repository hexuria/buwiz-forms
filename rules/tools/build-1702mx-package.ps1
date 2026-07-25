param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$SourceDir = 'C:\Mac\Home\Downloads\forms\1702MXv2018c'
)

$ErrorActionPreference = 'Stop'
$formId = '1702mx-v2018c'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1702MXv2018C.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1702MX.hta'
$pdfPath = Join-Path $SourceDir '1702-MX Jan 2018 ENCS Final with OSDv2.pdf'
$attachmentPdfPath = Join-Path $SourceDir '1702-MX Attachment Jan 2018 ENCS Final4.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1702mx-v2018c'
$fixtureDir = Join-Path $outDir 'fixtures'
$existingModelPath = Join-Path $RepoRoot 'crates\bir-core\src\forms\form_1702mx.rs'
$existingXmlPath = Join-Path $RepoRoot 'crates\bir-core\src\forms\form_1702mx_xml.rs'

$expected = @{
    hta = '34dbe79d6bf934718e86c73d8fdea4eb4a4e6a86c939ab0ecf231282d744acf7'
    help = '6a67074fe275a2584ada1d4a36a26e693bf3d17e824d5ce249e5edfb11b12f2d'
    pdf = '81c05fffadde6c0b4098aeba8547a9820a0806c6be9b0c6ceac5597cab4263d2'
    attachment_pdf = '36c02d4c84919d2e5b94cd31b339490019be80afa622f5681ce252c8ec3dec26'
    plain = 'ed96c5b56eecee68f1f73eef50dda00f69a42bd0dc5d0849e2cbe22c6b70b239'
    encrypted = 'ab4896a21603c7853985b6589a918c3d0189872b1817a4a55453ebea063a47b4'
    decrypted = '751685fe456f4040d09929aad3c51255e984c680ae7fde5d4aa19f4d6c8d23f9'
    plain_inventory = '49026b6af417cebf286a7fbc4edddfe2b90a66fde4d84f1de512f72b02e8b04b'
    encrypted_inventory = 'e2d1f26f6099e5cd050220561a60b7d8c31c282b785467309831af0affe8190c'
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

foreach ($path in @($htaPath, $helpPath, $pdfPath, $attachmentPdfPath, $packagePath, $existingModelPath, $existingXmlPath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
$plainPath = Find-FileByHash $SourceDir $expected.plain
$encryptedPath = Find-FileByHash $SourceDir $expected.encrypted
foreach ($pair in @(
    @($htaPath, 'hta'), @($helpPath, 'help'), @($pdfPath, 'pdf'),
    @($attachmentPdfPath, 'attachment_pdf'), @($plainPath, 'plain'),
    @($encryptedPath, 'encrypted'), @($packagePath, 'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
foreach ($pdf in @($pdfPath, $attachmentPdfPath)) {
    $bytes = [IO.File]::ReadAllBytes($pdf)
    if ([Text.Encoding]::ASCII.GetString($bytes[0..4]) -ne '%PDF-') { throw "PDF magic mismatch: $pdf" }
}
$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
$plain = [IO.File]::ReadAllText($plainPath)
if ($hta -notmatch '(?i)var\s+formType\s*=\s*["'']1702MXv2018C["'']') { throw 'Exact formType revision binding is absent.' }
if ($hta -notmatch '(?i)January\s+2018') { throw 'January 2018 printed revision is absent.' }
if ($help -notmatch '(?i)June\s+2013') { throw 'Expected legacy June 2013 help binding changed.' }
New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$plainEntries = Save-Entries $plain
$plainKeys = @($plainEntries | ForEach-Object { $_.Groups['key'].Value })
if ($plainKeys.Count -ne 210 -or ($plainKeys | Sort-Object -Unique).Count -ne 210) { throw "Expected 210 unique plaintext keys; found $($plainKeys.Count)." }
if ((Get-HashText @($plainKeys | Sort-Object)) -ne $expected.plain_inventory) { throw 'Plaintext inventory hash changed.' }

$decrypted = Decrypt-Save $encryptedPath
$encryptedKeys = @($decrypted.keys)
if ($decrypted.sha256 -ne $expected.decrypted) { throw 'Decrypted payload hash changed.' }
if ($encryptedKeys.Count -ne 588 -or ($encryptedKeys | Sort-Object -Unique).Count -ne 588) { throw "Expected 588 unique encrypted keys; found $($encryptedKeys.Count)." }
if ((Get-HashText @($encryptedKeys | Sort-Object)) -ne $expected.encrypted_inventory) { throw 'Encrypted inventory hash changed.' }
$plainOnly = @(Compare-Object @($encryptedKeys | Sort-Object -Unique) @($plainKeys | Sort-Object -Unique) -PassThru | Where-Object SideIndicator -eq '=>')
$encryptedOnly = @(Compare-Object @($encryptedKeys | Sort-Object -Unique) @($plainKeys | Sort-Object -Unique) -PassThru | Where-Object SideIndicator -eq '<=')
if ($plainOnly.Count -ne 0 -or $encryptedOnly.Count -ne 378) { throw "Expected plaintext to be an exact subset with 378 encrypted-only keys; found $($plainOnly.Count)/$($encryptedOnly.Count)." }
foreach ($key in @($plainKeys + $encryptedKeys)) {
    if ($key -match '@') { throw 'Email-like content appeared in a field key.' }
    if ($key -like 'frm1702MX:*' -eq $false -and $key -notin @('txtFinalFlag','txtVersion','txtEnabledInputsOnValidation','txtDisabledInputs','txtEnabledLinks','txtMaxPage','txtCurrentPage','txtScheduleIndex','txtScheduleCount','txtEnroll','ebirOnline','driveSelectTPExport')) {
        # Preserve unknown metadata keys; this branch is intentionally non-failing.
    }
}

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
if ($controls.Count -ne 835 -or $serial.Count -ne 696 -or $staticIds.Count -ne 696) {
    throw "Expected 835 controls/696 serializer candidates/696 unique IDs; found $($controls.Count)/$($serial.Count)/$($staticIds.Count)."
}
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) { $controlById[$control.id] = $control }
}

# Strip comments before deriving unbounded runtime families. Four Schedule 13 Item 13
# variants exist only inside comments and must not be promoted as active fields.
$activeHta = [regex]::Replace($hta, '(?is)<!--.*?-->', '')
$activeHta = [regex]::Replace($activeHta, '(?is)/\*.*?\*/', '')
$activeHta = [regex]::Replace($activeHta, '(?m)^\s*//.*$', '')
$familyMap = [ordered]@{}
$familyPattern = '(?is)(?<prefix>frm1702MX:[A-Za-z0-9:_-]+)["'']?\s*\+\s*\(\s*(?:i|x)\s*\+\s*1\s*\)\s*(?:\+\s*["''](?<suffix>[A-Za-z0-9:_-]+))?'
foreach ($match in [regex]::Matches($activeHta, $familyPattern)) {
    $pattern = $match.Groups['prefix'].Value + '{N>=1}' + $match.Groups['suffix'].Value
    $nonFieldPattern = (
        $pattern -match '^frm1702MX:(txtPg5Sc4I3_|txtPg5Sc5I4_|txtPg6Sc5I39_|txtPg3Sc5It17i_|txtPg2Sc3_|txtPg4Sc10Itm3_|txtPg4Sc10Itm6_|txtPg4Sc10Itm8_)\{N>=1\}Col5$' -or
        $pattern -eq 'frm1702MX:txtPg6Sc6_{N>=1}Col6'
    )
    if ($nonFieldPattern) { continue }
    if (-not $familyMap.Contains($pattern)) {
        $line = 1 + [regex]::Matches($activeHta.Substring(0, $match.Index), "`n").Count
        $familyMap[$pattern] = [pscustomobject][ordered]@{
            field_pattern = $pattern
            index_origin = 1
            source_line = $line
            source_expression = 'Runtime concatenation using (i + 1) or (x + 1).'
        }
    }
}
$dynamicFamilies = @($familyMap.Values)
if ($dynamicFamilies.Count -ne 83) {
    throw "Expected 83 active runtime field families after comment removal; found $($dynamicFamilies.Count)."
}
foreach ($commented in @(
    'frm1702MX:txtPg9Sc13I13CA-2-{N>=1}',
    'frm1702MX:txtPg9Sc13I13CA-3-{N>=1}',
    'frm1702MX:txtPg9Sc13I13CB-2-{N>=1}',
    'frm1702MX:txtPg9Sc13I13CB-3-{N>=1}'
)) {
    if ($familyMap.Contains($commented)) { throw "Commented family was incorrectly treated as active: $commented" }
}

$requiredKeys = @(
    'frm1702MX:drpPg1Pt1I7RDO',
    'frm1702MX:txtPg1Pt1I9RegisteredName',
    'frm1702MX:txtPg1Pt1I10RegisteredAddress',
    'frm1702MX:txtPg1Pt1I11ContactNumber',
    'frm1702MX:txtPg1Pt1I12Email',
    'frm1702MX:rdoPg1I1Calendar',
    'frm1702MX:rdoPg1I1Fiscal',
    'frm1702MX:ddlPg1I2Date',
    'frm1702MX:txtPg1I2YearEnd',
    'frm1702MX:txtPg1Pt1I8',
    'frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized',
    'frm1702MX:rdoPg1Pt1I13MethodOfDeducOptional'
)
function Get-FieldMeta([string]$Key, $Control, [bool]$IsFamily) {
    $page = $null
    if ($Key -match '(?i)Pg(?<page>\d+)') { $page = [int]$Matches.page }
    $item = $null
    $itemMatches = @([regex]::Matches($Key, '(?i)(?:Itm?|I)(?<item>\d+[a-z]?)'))
    if ($itemMatches.Count -gt 0) { $item = $itemMatches[-1].Groups['item'].Value }
    $logical = 'string'
    $normalization = [string[]]@()
    $enum = [object[]]@()
    if (($Control -and $Control.control_kind -in @('radio','checkbox')) -or $Key -match '(?i):(rdo|chk|Inst)[A-Za-z0-9]') {
        $logical = 'boolean'
        $enum = [object[]]@('true','false')
    } elseif ($Key -match '(?i)(Email|email)$') {
        $logical = 'email-string'
        $normalization = [string[]]@('emailAddress keypress filter', 'validateEmail on blur when the control receives focus')
    } elseif ($Key -match '(?i)(IssueDate|ExpiryDate|DateOf|txtPg1Pt1I8|Date$)') {
        $logical = 'date-string'
        $normalization = [string[]]@('MM/DD/YYYY')
    } elseif ($Key -match '(?i)(TIN|RDO|PSIC|ATC|branchCode)') {
        $logical = 'code'
    } elseif ($Key -match '(?i)(Year|year)$') {
        $logical = 'integer-year'
    } elseif (
        $Key -match '(?i)^frm1702MX:txtPg[2-9]' -and
        $Key -notmatch '(?i)(Desc|description|legal|name|other|year|Date|TIN|Title|Address|PSIC|ATC)'
    ) {
        $logical = 'whole-peso-amount'
        $normalization = [string[]]@('NumWithComma', 'NumWithParenthesis', 'NegativeValue', 'formatCurrencyWOC')
    }
    $computed = $false
    if ($Control -and ($Control.disabled -or $Control.readonly) -and $logical -eq 'whole-peso-amount') { $computed = $true }
    if ($Key -match '(?i)(Pt2I14TotalIncome|Pt2I15LessTotalTax|Pt2I16NetTaxPayable|Pt2I20TotalPenalties|Pt2I21TotalAmount|Sc2Itm(3|5|7|11|12|13)|Sc2It(15|17|19)D|Sc3It(32|33)|Sc4Itm(3|5|7)D|Sc10Itm(4|9)D)$') { $computed = $true }
    $status = 'optional'
    if ($requiredKeys -contains $Key) { $status = 'required' }
    if ($computed) { $status = 'computed' }
    if ($IsFamily) { $status = 'conditional'; $computed = $false }
    if ($Key -match '^(txtFinalFlag|txtVersion|txtEnabled|txtDisabled|txtMaxPage|txtCurrentPage|txtSchedule|txtEnroll|ebirOnline|driveSelectTPExport)') { $status = 'hidden' }
    $constraints = [ordered]@{}
    if ($Control -and $Control.maxlength -and $Control.maxlength -match '^\d+$') { $constraints.max_length = [int]$Control.maxlength }
    if ($logical -eq 'whole-peso-amount') {
        $constraints.precision = 0
        $constraints.sign = 'signed; parentheses parsed as negative'
    }
    [pscustomobject]@{
        page = $page; item = $item; logical = $logical; normalization = $normalization
        enum = $enum; computed = $computed; status = $status; constraints = [pscustomobject]$constraints
    }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $encryptedKeys) {
    $control = if ($controlById.ContainsKey($key)) { $controlById[$key] } else { $null }
    $meta = Get-FieldMeta $key $control $false
    $refs = @("xml-final-v1#field:$key")
    if ($plainKeys -contains $key) { $refs += "xml-editable-subset-v1#field:$key" }
    if ($control) { $refs += "official-hta-runtime#control:L$($control.source_line)" } else { $refs += 'official-hta-runtime#final-copy/runtime-serialization' }
    $requiredWhen = $null
    if ($key -match '(?i)rdoPg1Pt2I21(Refund|IssueTCC|CarriedOver)') { $requiredWhen = 'Part II Item 16 or Item 21 is negative.' }
    elseif ($key -match '(?i)Pt4I3[1-6]C[ABC]|Pt4I34SpecialTaxRate') { $requiredWhen = 'The corresponding Schedule 2 regime column has a nonzero amount and Instruction B is not selected.' }
    $notes = @('Observed in the reviewed 588-key decrypted final-copy inventory; source value is excluded.')
    if ($plainKeys -notcontains $key) { $notes += 'Absent from the reviewed 210-key editable plaintext subset; preserve losslessly.' }
    $fields.Add([pscustomobject][ordered]@{
        field_key = $key
        serialized_key = $key
        serialized_occurrence = 1
        label = if ($key -match '^frm1702MX:') { $key.Substring(10) } else { $key }
        page = $meta.page
        item_number = $meta.item
        control_kind = if ($control) { $control.control_kind } else { 'final-copy-only-or-runtime-generated-control' }
        storage_type = 'string'
        logical_type = $meta.logical
        required = $meta.status
        required_when = $requiredWhen
        enabled_when = $null
        visible_when = $null
        default_value = if ($control) { $control.value } else { $null }
        empty_representation = ''
        constraints = $meta.constraints
        enum_values = $meta.enum
        normalization = $meta.normalization
        computed = $meta.computed
        calculation_id = if ($meta.computed) { 'See calculations.json and function inventory.' } else { $null }
        source_refs = $refs
        confidence = if ($control -or $plainKeys -contains $key) { 'high' } else { 'medium' }
        notes = $notes
    })
}
foreach ($family in $dynamicFamilies) {
    $meta = Get-FieldMeta $family.field_pattern $null $true
    $fields.Add([pscustomobject][ordered]@{
        field_key = $family.field_pattern
        serialized_key = $null
        serialized_occurrence = $null
        label = "Runtime-indexed field family $($family.field_pattern)"
        page = $meta.page
        item_number = $meta.item
        control_kind = 'runtime-indexed-family'
        storage_type = 'string'
        logical_type = $meta.logical
        required = 'conditional'
        required_when = 'A corresponding add-more or mandatory-attachment row N exists.'
        enabled_when = 'The corresponding schedule or attachment row exists.'
        visible_when = 'The corresponding schedule or attachment row exists.'
        default_value = $null
        empty_representation = ''
        constraints = $meta.constraints
        enum_values = $meta.enum
        normalization = $meta.normalization
        computed = $false
        calculation_id = $null
        source_refs = @("official-hta-runtime#dynamic-id:L$($family.source_line)", 'official-hta-runtime#runtime-modal-serialization')
        confidence = 'high'
        notes = @('Unbounded source-derived family; no indexed instance appeared in the reviewed 588-key final-copy snapshot.', 'N begins at 1.')
    })
}
if ($fields.Count -ne 671) { throw "Expected 671 total fields (588 concrete + 83 families); found $($fields.Count)." }
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = 588
    inventory_sha256 = Get-HashText @($fields.field_key | Sort-Object)
    fields = $fields
})

$staticOnly = @(Compare-Object @($encryptedKeys | Sort-Object -Unique) $staticIds -PassThru | Where-Object SideIndicator -eq '=>')
$finalOnly = @(Compare-Object @($encryptedKeys | Sort-Object -Unique) $staticIds -PassThru | Where-Object SideIndicator -eq '<=')
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    form_control_count = $controls.Count
    static_serializer_candidate_count = $serial.Count
    static_serializer_unique_id_count = $staticIds.Count
    reviewed_editable_key_count = $plainKeys.Count
    reviewed_final_copy_key_count = $encryptedKeys.Count
    active_runtime_family_count = $dynamicFamilies.Count
    serializer_set_differences = [ordered]@{
        final_copy_not_in_static_dom = $finalOnly
        static_dom_not_in_final_copy_snapshot = $staticOnly
    }
    controls = $controls
    dynamic_families = $dynamicFamilies
})
Write-Json (Join-Path $fixtureDir 'editable-subset-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_path = $plainPath
    sha256 = $expected.plain
    field_count = $plainKeys.Count
    unique_field_count = ($plainKeys | Sort-Object -Unique).Count
    field_inventory_sha256 = $expected.plain_inventory
    exact_subset_of_final_copy = $true
    final_copy_only_field_count = $encryptedOnly.Count
    values_emitted = $false
})
Write-Json (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_path = (Join-Path $SourceDir '1702MXv2018C-final-copy-#email-redacted#.xml')
    ciphertext_sha256 = $expected.encrypted
    zlib_header = $decrypted.zlib_header
    decrypted_byte_count = $decrypted.bytes.Length
    decrypted_sha256 = $decrypted.sha256
    field_count = $encryptedKeys.Count
    unique_field_count = ($encryptedKeys | Sort-Object -Unique).Count
    field_inventory_sha256 = $expected.encrypted_inventory
    editable_subset_field_count = $plainKeys.Count
    final_copy_only_field_count = $encryptedOnly.Count
    values_emitted = $false
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1702MX:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final|year|date|email') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1702MX:' -NamePattern '(?i)compute|calculate|sum|difference|product|nolco|mcit|total') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule(
    [string]$Id, [string]$Phase, $Order, [string]$Condition, [string[]]$FieldKeys, $Message,
    [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.',
    [string]$Confidence = 'high',
    [string[]]$Evidence = @('source')
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
        exact_message = $Message
        source_refs = $Refs
        evidence_type = $Evidence
        assessment = $Assessment
        official_behavior = $Official
        recommended_app_behavior = $Recommended
        confidence = $Confidence
        unresolved_questions = @()
    })
}

# Save collects every failure but alerts only the first entry in this exact order.
Rule '1702mx-save-001-rdo' 'save' 1 'RDO code equals literal 000.' @('frm1702MX:drpPg1Pt1I7RDO') 'Please select an RDO Code (Part I Item 7).' @('official-hta-runtime#initialValidateBeforeSave:L21427-L21433')
Rule '1702mx-save-002-name' 'save' 2 'Registered name is blank.' @('frm1702MX:txtPg1Pt1I9RegisteredName') 'Please provide a Registered Name (Part I Item 9).' @('official-hta-runtime#initialValidateBeforeSave:L21434-L21437')
Rule '1702mx-save-003-address' 'save' 3 'Registered address is blank.' @('frm1702MX:txtPg1Pt1I10RegisteredAddress') 'Please provide a Registered Address (Part I Item 10).' @('official-hta-runtime#initialValidateBeforeSave:L21438-L21441')
Rule '1702mx-save-004-contact' 'save' 4 'Contact number is blank.' @('frm1702MX:txtPg1Pt1I11ContactNumber') 'Please provide a Contact Number (Part I Item 11).' @('official-hta-runtime#initialValidateBeforeSave:L21442-L21446')
Rule '1702mx-save-005-email' 'save' 5 'Email address is blank.' @('frm1702MX:txtPg1Pt1I12Email') 'Please provide an Email Address (Part I Item 12).' @('official-hta-runtime#initialValidateBeforeSave:L21447-L21451')
Rule '1702mx-save-006-first-error' 'save' 6 'Two or more save prerequisites fail.' @('return-body') $null @('official-hta-runtime#initialValidateBeforeSave:L21427-L21470') 'verified-correct' 'All failures are accumulated, but only errorList[0] is alerted; source order controls the first message.' 'Return every draft-completeness issue while preserving the official first-error order for compatibility.'
Rule '1702mx-save-007-omissions' 'save' 7 'TIN, filing basis/year, ATC, incorporation date, deduction method, schedules, or formats are incomplete.' @('return-body') $null @('official-hta-runtime#initialValidateBeforeSave:L21427-L21470') 'official-bug-compatible' 'Save ignores these fields and permits the draft when the five narrow checks pass.' 'Permit lossless drafts, but distinguish draft persistence from validation/finalization.'
Rule '1702mx-save-008-commented-profile' 'save' 8 'Main line of business or PSIC is blank/short.' @('frm1702MX:txtPg1Pt1I13MainLine','frm1702MX:txtPg1Pt1I14PSICCode') 'Please provide a valid PSIC code (Part I Item 14).' @('official-hta-runtime#initialValidateBeforeSave:L21452-L21460') 'obsolete' 'Both checks are inside one block comment.' 'Do not expose them as active official Save checks.'

$validateOrder = 0
function Validate-Rule(
    [string]$Suffix, [string]$Condition, [string[]]$FieldKeys, $Message, [string[]]$Refs,
    [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and returns from Validate.',
    [string]$Recommended = 'Retain with revision-aware wording.'
) {
    $script:validateOrder++
    Rule "1702mx-validate-$Suffix" 'validate' $script:validateOrder $Condition $FieldKeys $Message $Refs $Assessment $Official $Recommended
}

# Basis-of-tax-relief prerequisites are evaluated before every other Validate check.
$basis = @(
    @('A','1','frm1702MX:txtPg2Pt4I31CA'), @('A','2','frm1702MX:txtPg2Pt4I32CA'),
    @('A','3','frm1702MX:txtPg2Pt4I33CA'), @('A','5','frm1702MX:txtPg2Pt4I35CA'),
    @('A','6','frm1702MX:txtPg2Pt4I36CA'), @('B','1','frm1702MX:txtPg2Pt4I31CB'),
    @('B','2','frm1702MX:txtPg2Pt4I32CB'), @('B','3','frm1702MX:txtPg2Pt4I33CB'),
    @('B','4','frm1702MX:txtPg2Pt4I34SpecialTaxRate'), @('B','5','frm1702MX:txtPg2Pt4I35CB'),
    @('B','6','frm1702MX:txtPg2Pt4I36CB'), @('C','1','frm1702MX:txtPg2Pt4I31CC'),
    @('C','2','frm1702MX:txtPg2Pt4I32CC'), @('C','3','frm1702MX:txtPg2Pt4I33CC'),
    @('C','5','frm1702MX:txtPg2Pt4I35CC'), @('C','6','frm1702MX:txtPg2Pt4I36CC')
)
foreach ($entry in $basis) {
    $column = $entry[0]; $item = $entry[1]; $field = $entry[2]
    $condition = if ($column -eq 'B' -and $item -eq '4') {
        'Instruction B is unchecked, Schedule 2 column B has any nonzero amount, and the special tax rate is <= 0.'
    } else {
        "Instruction B is unchecked, Schedule 2 column $column has any nonzero amount, and Schedule 1 Item $item$column is blank."
    }
    Validate-Rule "basis-$column$item" $condition @($field,"Schedule 2 column $column") "Please provide value on Page 2 Schedule 1 Item $item$column." @('official-hta-runtime#validateBasistTaxtRelief:L20899-L21031')
}
Validate-Rule 'basis-else-if-skip' 'Two or more Schedule 2 regime columns have data, the first populated column has complete basis data, and a later populated column does not.' @('Schedule 2 columns A/B/C','Schedule 1 basis fields') $null @('official-hta-runtime#validateBasistTaxtRelief:L20902-L21027') 'incorrect-official-behavior' 'The outer A / else-if B / else-if C chain checks only the first populated regime, so incomplete later regimes pass.' 'Validate every populated regime independently.'
Validate-Rule 'tax-relief-helper-commented' 'Any condition formerly tested by validateTaxtRelief is present.' @('Schedule 2','Schedule 4') $null @('official-hta-runtime#validateTaxtRelief:L21033-L21074') 'obsolete' 'Every substantive branch is commented; the helper always returns true.' 'Do not treat the commented messages as active January 2018C rules.'

foreach ($entry in @(
    @('income-tax-1a','Schedule 2 Item 13A is positive and Schedule 4 Item 1A is <= 0.','frm1702MX:txtPg3Sc4Itm1A','Please provide value on Page 3 Schedule 4 Item 1A'),
    @('income-tax-1b','Schedule 2 Item 13B is positive and Schedule 4 Item 1B is <= 0.','frm1702MX:txtPg3Sc4Itm1B','Please provide value on Page 3 Schedule 4 Item 1B'),
    @('income-tax-2a','Schedule 2 Item 9A is positive and Schedule 4 Item 2A is <= 0.','frm1702MX:txtPg2Sc4Itm2A','Please provide value on Page 3 Schedule 4 Item 2A'),
    @('income-tax-2b','Schedule 2 Item 9B is positive and Schedule 4 Item 2B is <= 0.','frm1702MX:txtPg2Sc4Itm2B','Please provide value on Page 3 Schedule 4 Item 2B'),
    @('income-tax-2c','Schedule 2 Item 9C is positive and Schedule 4 Item 2C is <= 0.','frm1702MX:txtPg2Sc4Itm2C','Please provide value on Page 3 Schedule 4 Item 2C')
)) {
    Validate-Rule $entry[0] $entry[1] @($entry[2]) $entry[3] @('official-hta-runtime#validateComputationIncomeTax:L21076-L21101')
}

# initialValidateBeforeSave is called again here; preserve its five messages and order by reference.
Validate-Rule 'profile-prerequisites' 'Any of the five Save prerequisites fails.' @('frm1702MX:drpPg1Pt1I7RDO','frm1702MX:txtPg1Pt1I9RegisteredName','frm1702MX:txtPg1Pt1I10RegisteredAddress','frm1702MX:txtPg1Pt1I11ContactNumber','frm1702MX:txtPg1Pt1I12Email') $null @('official-hta-runtime#validate:L21116-L21119','official-hta-runtime#initialValidateBeforeSave:L21427-L21470') 'verified-correct' 'The same source-ordered first message used by Save is shown, then Validate returns.' 'Reuse the five structured rules without duplicating messages.'
Validate-Rule 'attachments' 'validateAttachments returns false.' @('mandatory-attachment-fields') $null @('official-hta-runtime#validate:L21121-L21124','official-hta-runtime#validateAttachments:L18881-L19065') 'official-bug-compatible' 'Attachment checks run before filing basis/year, ATC, and incorporation checks.' 'Keep deterministic ordering but present the affected attachment and row.'
Validate-Rule 'filing-basis' 'Neither Calendar nor Fiscal is selected.' @('frm1702MX:rdoPg1I1Calendar','frm1702MX:rdoPg1I1Fiscal') 'Please select if filing for Calendar or Fiscal Year (Page 1 Item 1).' @('official-hta-runtime#validate:L21133-L21138')
Validate-Rule 'period-required' 'Month or two-digit year ended is blank.' @('frm1702MX:ddlPg1I2Date','frm1702MX:txtPg1I2YearEnd') 'Please select Month and Year for Year Ended (Page 1 Item 2).' @('official-hta-runtime#validate:L21139-L21144')
foreach ($entry in @(
    @('year-fiscal-future','Fiscal period is later than the current month.','Date (Page 1 Item 2) cannot be greater than current date when filing for Fiscal Year.'),
    @('year-fiscal-december','Fiscal month is December.','Date (Page 1 Item 2) Month cannot be equal to December.'),
    @('year-before-effectivity','Period is earlier than September 2013.','Date (Page 1 Item 2) should not be earlier than September 2013.'),
    @('year-calendar-current','Calendar, not short-period, and year is current or future.','Year (Page 1 Item 2) cannot be greater than or equal to current year when filing for Calendar Year.'),
    @('year-short-future','Calendar short-period year is later than current year.','Year (Page 1 Item 2) cannot be greater than the current year when filing for Calendar Year.'),
    @('year-short-month-future','Calendar short-period month is later than current month in the current year.','Month (Page 1 Item 2) cannot be greater than  current month date when filing for Calendar Year  and  Short Period Return.'),
    @('year-short-december','Calendar short-period month is December in the current year.','Month (Page 1 Item 2) cannot be equal to december  when filing for Calendar Year and  Short Period Return.'),
    @('year-short-month-invalid','Calendar short-period month coerces below zero.','(Page 1 Item 2) Month is invalid.')
)) {
    Validate-Rule $entry[0] $entry[1] @('frm1702MX:ddlPg1I2Date','frm1702MX:txtPg1I2YearEnd') $entry[2] @('official-hta-runtime#validateYearEnd:L18352-L18433')
}
Validate-Rule 'year-empty-helper' 'validateYearEnd is called directly with blank year.' @('frm1702MX:txtPg1I2YearEnd') 'This field cannot be empty.' @('official-hta-runtime#validateYearEnd:L18423-L18426') 'official-bug-compatible' 'The helper alerts but fails to set isValid=false; normal Validate never reaches this branch because its preceding required check returns.' 'Return false and use the specific Item 2 required message.' 
Validate-Rule 'atc' 'Neither IC055 nor alternate ATC is checked.' @('frm1702MX:chkPg1I5ATCR1','frm1702MX:chkPg1I5ATCR2') 'Please tick at least one ATC option (Page 1 Item 5).' @('official-hta-runtime#validate:L21149-L21154')
Validate-Rule 'incorporation-required' 'Date of incorporation / organization is blank.' @('frm1702MX:txtPg1Pt1I8') 'Please provide Date of Incorporation / Organization (Part I Item 10).' @('official-hta-runtime#validate:L21157-L21162') 'incorrect-official-behavior' 'The message says Item 10, while the control ID and printed placement are Part I Item 8.' 'Report the actual printed item and preserve the official text as compatibility evidence.'
Validate-Rule 'incorporation-after-period' 'Incorporation month/year is after the filing month/year.' @('frm1702MX:txtPg1Pt1I8','frm1702MX:ddlPg1I2Date','frm1702MX:txtPg1I2YearEnd') 'Date of Incorporation cannot be greater than Page 1 Item 2 Date.' @('official-hta-runtime#checkDateOfIncorporation:L18504-L18531')
Validate-Rule 'incorporation-four-years-exact' 'Filing year minus incorporation year equals exactly four and IC055 is unchecked.' @('frm1702MX:txtPg1Pt1I8','frm1702MX:chkPg1I5ATCR1') 'The Year of Incorporation is <year>, ATC - IC 055 of Page 1 Item will be marked.' @('official-hta-runtime#checkDateOfIncorporation:L18532-L18547') 'official-bug-compatible' 'The message interpolates the incorporation year and automatically marks IC055.' 'Represent the automatic ATC transition explicitly and let the taxpayer review it.'
Validate-Rule 'incorporation-under-four' 'Filing year minus incorporation year is below four while IC055 is checked.' @('frm1702MX:txtPg1Pt1I8','frm1702MX:chkPg1I5ATCR1','frm1702MX:chkPg1I5ATCR2') 'Less than 4 years has passed since Date of Incorporation and the Filing Year, the mark in ATC - IC 055 of Page 1 Item 5 will be removed.' @('official-hta-runtime#checkDateOfIncorporation:L18548-L18565') 'official-bug-compatible' 'It unchecks/disables IC055, checks the alternate ATC, and returns false even after mutating the form.' 'Apply the derived eligibility change transparently, then validate the selected alternate code.'
Validate-Rule 'incorporation-four-plus' 'Elapsed time is at least four years and IC055 is unchecked.' @('frm1702MX:txtPg1Pt1I8','frm1702MX:chkPg1I5ATCR1') 'It has been 4 years or more since the Date of Incorporation and the Filing Year, ATC - IC 055 of Page 1 Item will be marked.' @('official-hta-runtime#checkDateOfIncorporation:L18566-L18577') 'official-bug-compatible' 'It marks IC055 but does not return false.' 'Expose the derived ATC change and validate consistently.'
Validate-Rule 'incorporation-year-only-bug' 'The calendar-year difference is four although fewer than four full years have elapsed.' @('frm1702MX:txtPg1Pt1I8','filing-period') $null @('official-hta-runtime#checkDateOfIncorporation:L18515-L18522','official-hta-runtime#checkDateOfIncorporation:L18532-L18577') 'incorrect-official-behavior' 'The exact-four branch uses filingYear-incorpYear and ignores months/days; the later elapsedTime branch is bypassed.' 'Calculate the statutory elapsed period using full dates.'
Validate-Rule 'overpayment-option' 'Part II Item 16 or Item 21 is negative and no overpayment disposition is checked.' @('frm1702MX:txtPg1Pt2I16NetTaxPayable','frm1702MX:txtPg1Pt2I21TotalAmount','frm1702MX:rdoPg1Pt2I21Refund','frm1702MX:rdoPg1Pt2I21IssueTCC','frm1702MX:rdoPg1Pt2I21CarriedOver') 'Please select an Overpayment option (Page 1 Part II Item 21).' @('official-hta-runtime#checkOverPayment:L18333-L18350')
Validate-Rule 'schedule2-schedule10-crosscheck' 'Any Schedule 2 Item 13 A-D value differs (loose inequality after removeCommaParenthesis) from Schedule 10 Item 10 A-D.' @('frm1702MX:txtPg2Sc2It13A','frm1702MX:txtPg2Sc2It13B','frm1702MX:txtPg2Sc2It13C','frm1702MX:txtPg2Sc2It13D','frm1702MX:txtPg2Sc2Itm10A','frm1702MX:txtPg2Sc2Itm10B','frm1702MX:txtPg2Sc2Itm10C','frm1702MX:txtPg2Sc2Itm10D') 'Page 2 Schedule 2 Item 13 Columns A, B, C & D must be equal to Page 4 Schedule 10 Item 10 Columns A, B, C & D.' @('official-hta-runtime#validate:L21174-L21180') 'ambiguous' 'The second key family is named Schedule 2 Item 10 even though the message calls it Schedule 10 Item 10; the calculation functions write Schedule 10 results into those keys.' 'Bind by printed schedule semantics and retain exact serialized aliases.'

$rowLabels = @(
    'Page 3 Schedule 5 Item 17d','Page 3 Schedule 5 Item 17e','Page 3 Schedule 5 Item 17f',
    'Page 3 Schedule 5 Item 17g','Page 3 Schedule 5 Item 17h','Page 3 Schedule 5 Item 17i',
    'Page 2 Schedule 3 Item 30','Page 2 Schedule 3 Item 31',
    'Page 3 Schedule 6 Item 1','Page 3 Schedule 6 Item 2','Page 3 Schedule 6 Item 3','Page 3 Schedule 6 Item 4',
    'Page 4 Schedule 7 Item 4','Page 4 Schedule 7 Item 5','Page 4 Schedule 7 Item 6','Page 4 Schedule 7 Item 7',
    'Page 4 Schedule 8.1 Item 4','Page 4 Schedule 8.1 Item 5','Page 4 Schedule 8.1 Item 6','Page 4 Schedule 8.1 Item 7',
    'Page 4 Schedule 9 Item 1','Page 4 Schedule 9 Item 2','Page 4 Schedule 9 Item 3',
    'Page 4 Schedule 10 Item 2','Page 4 Schedule 10 Item 3','Page 4 Schedule 10 Item 5',
    'Page 4 Schedule 10 Item 6','Page 4 Schedule 10 Item 7','Page 4 Schedule 10 Item 8'
)
$rowIndex = 0
foreach ($label in $rowLabels) {
    $rowIndex++
    Validate-Rule ("row-{0:d2}" -f $rowIndex) "The description/year side and positive-amount side of $label are not both populated." @($label) "Please provide complete data on $label.`n(Amount cannot be zero [0] if Description is not empty and vice versa)." @('official-hta-runtime#validate:L21182-L21338','official-hta-runtime#validate_nullDescription:L19116-L19151','official-hta-runtime#validate_nullDescription2:L19193-L19233')
}
Validate-Rule 'row-positive-only' 'A row amount is negative, or exactly zero, while its description is nonblank.' @('description/amount rows') $null @('official-hta-runtime#validate_nullDescription:L19123-L19147') 'incorrect-official-behavior' 'Only amounts > 0 increment the counter. Negative amounts are treated as absent and therefore rejected when a description is present, while zero-like strings are treated inconsistently by validate_nullDescription2.' 'Use explicit signed-domain rules per printed line and consistent numeric parsing.'
Validate-Rule 'method-of-deduction' 'Neither Itemized nor Optional Standard Deduction is checked.' @('frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized','frm1702MX:rdoPg1Pt1I13MethodOfDeducOptional') 'Please select a Method of Deduction in page 1 Item 13.' @('official-hta-runtime#check_MethodOfDeduction:L15564-L15574','official-hta-runtime#validate:L21342')

foreach ($entry in @(
    @('attachment-exempt-1','Exempt attachment Schedule B Item 57 is not literal "0" and Schedule A Item 1 is blank.','Please provide value on Exempt Attachment Page 1 Schedule A Item 1.'),
    @('attachment-exempt-2','Exempt attachment Schedule B Item 57 is not literal "0" and Schedule A Item 2 is blank.','Please provide value on Exempt Attachment Page 1 Schedule A Item 2.'),
    @('attachment-exempt-3','Exempt attachment Schedule B Item 57 is not literal "0" and Schedule A Item 3 is blank.','Please provide value on Exempt Attachment Page 1 Schedule A Item 3.'),
    @('attachment-exempt-5','Exempt attachment Schedule B Item 57 is not literal "0" and Schedule A Item 5 is blank.','Please provide value on Exempt Attachment Page 1 Schedule A Item 5.'),
    @('attachment-exempt-6','Exempt attachment Schedule B Item 57 is not literal "0" and Schedule A Item 6 is blank.','Please provide value on Exempt Attachment Page 1 Schedule A Item 6.'),
    @('attachment-special-1','Special-rate attachment Schedule B Item 57 is not literal "0" and Schedule A Item 1 is blank.','Please provide value on Special Rate Attachment Page 1 Schedule A Item 1.'),
    @('attachment-special-2','Special-rate attachment Schedule B Item 57 is not literal "0" and Schedule A Item 2 is blank.','Please provide value on Special Rate Attachment Page 1 Schedule A Item 2.'),
    @('attachment-special-3','Special-rate attachment Schedule B Item 57 is not literal "0" and Schedule A Item 3 is blank.','Please provide value on Special Rate Attachment Page 1 Schedule A Item 3.'),
    @('attachment-special-4','Special-rate attachment Schedule B Item 57 is not literal "0" and Schedule A Item 4 is literal "0".','Please provide value on Special Rate Attachment Page 1 Schedule A Item 4.'),
    @('attachment-special-5','Special-rate attachment Schedule B Item 57 is not literal "0" and Schedule A Item 5 is blank.','Please provide value on Special Rate Attachment Page 1 Schedule A Item 5.'),
    @('attachment-special-6','Special-rate attachment Schedule B Item 57 is not literal "0" and Schedule A Item 6 is blank.','Please provide value on Special Rate Attachment Page 1 Schedule A Item 6.')
)) {
    Validate-Rule $entry[0] $entry[1] @('mandatory-attachment-fields') $entry[2] @('official-hta-runtime#validateAttachments:L18881-L19065')
}
Validate-Rule 'attachment-exempt-wins' 'Both exempt and special-rate attachment arrays contain rows.' @('totalEXArray','totalSPArray') $null @('official-hta-runtime#validateAttachments:L18885-L19063') 'incorrect-official-behavior' 'The special-rate loop is an else-if, so it is skipped whenever any exempt attachment exists.' 'Validate both attachment collections independently.'
Validate-Rule 'attachment-multiple-alerts' 'A mandatory attachment has several missing Schedule A fields.' @('mandatory-attachment-fields') $null @('official-hta-runtime#validateAttachments:L18889-L19000') 'official-bug-compatible' 'The helper sets isValid=false but does not break or return after each alert; later rows/checks can produce multiple dialogs in one Validate action.' 'Collect all attachment errors and present them once.'
Validate-Rule 'attachment-nolco-uninvoked' 'NOLCO attachment year/amount rows are incomplete.' @('mandatory-attachment-NOLCO-fields') 'Please provide complete data on <attachment row>.`n(Amount cannot be zero [0] if Description is not empty and vice versa).' @('official-hta-runtime#validateNOLCOAttachments:L19068-L19115','official-hta-runtime#validate:L21103-L21425') 'obsolete' 'validateNOLCOAttachments is defined but never called.' 'Validate the same row completeness in the active attachment workflow.'
Validate-Rule 'description1-uninvoked' 'validate_nullDescription1 conditions fail.' @('description/amount rows') $null @('official-hta-runtime#validate_nullDescription1:L19153-L19189') 'obsolete' 'The helper is defined but never called.' 'Exclude from active compatibility rules.'
Validate-Rule 'email-format-blur' 'A nonblank email does not match the official regular expression.' @('frm1702MX:txtPg1Pt1I12Email') 'You have entered an invalid email address format!' @('official-hta-runtime#validateEmail:L18094-L18104','official-hta-runtime#control:L688-L689') 'official-bug-compatible' 'The disabled profile-prefilled email control has onblur validation; invalid input is cleared when the handler runs.' 'Validate nonblank email without destructive clearing.'
Validate-Rule 'success' 'All active checks pass.' @('return-body') 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L21402-L21417')
Validate-Rule 'success-unreachable-duplicate' 'Execution continues past the success else block.' @('return-body') 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L21402-L21423') 'obsolete' 'The success else block returns at line 21416, so the duplicate alert at line 21423 is unreachable.' 'Emit one success event.'
Validate-Rule 'commented-payment-and-accreditation' 'Details of payment, CTC/SEC, accreditation, or former Page 5 description checks fail.' @('payment/accreditation/former-schedule-fields') $null @('official-hta-runtime#validate:L21346-L21400') 'obsolete' 'All these branches are commented out.' 'Do not present them as active local Validate prerequisites.'

Rule '1702mx-final-001' 'final-copy' 1 'Final Copy is requested after successful validation.' @('txtFinalFlag','return-body') $null @('official-hta-runtime#final-copy-and-encryption','encrypted-field-audit-v796') 'verified-correct' 'The reviewed encrypted artifact decrypts to 588 unique keys: all 210 editable keys plus 378 additional final-copy keys.' 'Preserve all 588 concrete keys losslessly and keep finalization distinct from transport.' 'high' @('source','xml')
Rule '1702mx-final-002-model-gap' 'final-copy' 2 'The current repository serializer is used for this revision.' @('return-body') $null @('repository-xml-contract#editable-210-field:L1-L20','encrypted-field-audit-v796') 'incorrect-official-behavior' 'The repository contract explicitly covers the 210-field editable save, not the complete 588-field final-copy inventory.' 'Fail closed for final-copy/export until all 588 concrete keys and active indexed families have lossless coverage.' 'high' @('repository-code','xml')
Rule '1702mx-submit-001' 'submit' 1 'Online transport is invoked.' @('return-body','mandatory-attachment-fields') $null @('official-hta-runtime#saveXMLsubmit-and-sendEmail','repository-model#transition_to_queued:L728-L732') 'unverified' 'Transport and mandatory-attachment upload were not exercised; the repository model intentionally fails closed.' 'Keep local validation/finalization independently testable; never infer successful transport.' 'medium' @('source','repository-code')
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Validate order is basis-of-tax-relief, commented no-op tax-relief helper, five profile/save prerequisites, mandatory attachments, income-tax computation, filing basis, period, year-end, ATC, incorporation, overpayment, Schedule 2/10 cross-check, 29 source-ordered row-completeness checks, and deduction method. Most branches return on first failure; validateAttachments can display multiple dialogs before returning false. Save collects five failures and displays only the first.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Calc(
    [string]$Id, [string[]]$Outputs, [string[]]$Inputs, [string]$Formula, [string]$Trigger,
    [string[]]$Depends, [string[]]$Refs, [string]$Assessment = 'verified-correct',
    [string]$Recommended = 'Use checked signed whole-peso arithmetic and recompute from authoritative inputs.',
    [string]$Condition = $null, [string]$Confidence = 'high'
) {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id = $Id
        outputs = $Outputs
        inputs = $Inputs
        condition = $Condition
        official_formula = $Formula
        rounding = 'The January 2018C HTA parses commas/parentheses and generally writes whole-peso formatCurrencyWOC values; product helpers use Math.round in several paths.'
        trigger = $Trigger
        depends_on = $Depends
        source_refs = $Refs
        assessment = $Assessment
        recommended_app_behavior = $Recommended
        confidence = $Confidence
    })
}
Calc '1702mx-row-column-d' @('applicable schedule column D fields') @('same-row columns A','same-row column B','same-row column C') 'D = A + B + C, except rows whose printed applicability omits one or more columns.' 'schedule-specific compute...ColumnD functions' @() @('official-hta-runtime#getSum:L15580-L15593','official-hta-runtime#computePg2Sc2ColumnD:L14751-L14793','official-hta-runtime#computePg2Sc3ColumnD:L11095-L11113')
Calc '1702mx-part2-item14' @('frm1702MX:txtPg1Pt2I14TotalIncome') @('frm1702MX:txtPg2Sc2It19D') 'Part II Item 14 = Schedule 2 Item 19D.' 'computePg1Pt2It14' @('1702mx-schedule2-item19') @('official-hta-runtime#computePg1Pt2It14:L10908-L10912')
Calc '1702mx-part2-item15' @('frm1702MX:txtPg1Pt2I15LessTotalTax') @('frm1702MX:txtPg2Sc3It32D') 'Part II Item 15 = Schedule 3 Item 32D.' 'computePg1Pt2It15' @('1702mx-schedule3-item32') @('official-hta-runtime#computePg1Pt2It15:L10914-L10918')
Calc '1702mx-part2-item16' @('frm1702MX:txtPg1Pt2I16NetTaxPayable') @('frm1702MX:txtPg1Pt2I14TotalIncome','frm1702MX:txtPg1Pt2I15LessTotalTax') 'Item 16 = Item 14 - Item 15.' 'computePg1Pt2I18' @('1702mx-part2-item14','1702mx-part2-item15') @('official-hta-runtime#computePg1Pt2I18:L10902-L10906')
Calc '1702mx-part2-item20' @('frm1702MX:txtPg1Pt2I20TotalPenalties') @('frm1702MX:txtPg1Pt2I17','frm1702MX:txtPg1Pt2I18','frm1702MX:txtPg1Pt2I19') 'Item 20 = surcharge + interest + compromise.' 'computePg1Pt2I20' @() @('official-hta-runtime#computePg1Pt2I20:L11012-L11018')
Calc '1702mx-part2-item21-nonnegative' @('frm1702MX:txtPg1Pt2I21TotalAmount') @('frm1702MX:txtPg1Pt2I16NetTaxPayable','frm1702MX:txtPg1Pt2I20TotalPenalties') 'If Item 16 >= 0, Item 21 = Item 16 + Item 20.' 'computePg1Pt2I21' @('1702mx-part2-item16','1702mx-part2-item20') @('official-hta-runtime#computePg1Pt2I21:L11021-L11026') 'verified-correct' 'Retain the official conditional branch explicitly.' 'Item 16 is nonnegative.'
Calc '1702mx-part2-item21-negative-with-penalty' @('frm1702MX:txtPg1Pt2I21TotalAmount') @('frm1702MX:txtPg1Pt2I16NetTaxPayable','frm1702MX:txtPg1Pt2I20TotalPenalties') 'If Item 16 < 0 and Item 20 > 0, Item 21 is set to Item 20 only; the negative Item 16 is not netted against penalties.' 'computePg1Pt2I21' @('1702mx-part2-item16','1702mx-part2-item20') @('official-hta-runtime#computePg1Pt2I21:L11027-L11032','repository-model#recompute:L500-L536') 'incorrect-official-behavior' 'Preserve compatibility evidence, but obtain tax-domain review before reproducing this non-netting behavior.' 'Item 16 is negative and Item 20 is positive.'
Calc '1702mx-part2-item21-negative-no-penalty' @('frm1702MX:txtPg1Pt2I21TotalAmount') @('frm1702MX:txtPg1Pt2I16NetTaxPayable','frm1702MX:txtPg1Pt2I20TotalPenalties') 'If Item 16 < 0 and Item 20 <= 0, Item 21 = Item 16.' 'computePg1Pt2I21' @('1702mx-part2-item16','1702mx-part2-item20') @('official-hta-runtime#computePg1Pt2I21:L11033-L11036') 'verified-correct' 'Retain the official conditional branch explicitly.' 'Item 16 is negative and Item 20 is nonpositive.'
Calc '1702mx-schedule2-item3' @('Schedule 2 Item 3 A-D') @('Schedule 2 Item 1 A-D','Schedule 2 Item 2 A-D') 'Item 3 = Item 1 - Item 2 for each column.' 'computePg2Sc2Itm3' @() @('official-hta-runtime#computePg2Sc2Itm3:L14800-L14813','repository-model#recompute_schedule_2:L539-L552')
Calc '1702mx-schedule2-item5' @('Schedule 2 Item 5 A-D') @('Schedule 2 Item 3 A-D','Schedule 2 Item 4 A-D') 'Item 5 = Item 3 - Item 4 for each column.' 'computePg2Sc2Itm5' @('1702mx-schedule2-item3') @('official-hta-runtime#computePg2Sc2Itm5:L14816-L14826','repository-model#recompute_schedule_2:L553-L560')
Calc '1702mx-schedule2-item7' @('Schedule 2 Item 7 A-D') @('Schedule 2 Item 5 A-D','Schedule 2 Item 6 A-D') 'Item 7 = Item 5 + Item 6 for each column.' 'computePg2Sc2Itm7' @('1702mx-schedule2-item5') @('official-hta-runtime#computePg2Sc2Itm7:L14829-L14843','repository-model#recompute_schedule_2:L561-L568')
Calc '1702mx-schedule2-item8' @('Schedule 2 Item 8 A-D') @('Schedule 5 Item 18 A-D') 'Item 8 copies Schedule 5 Item 18.' 'computePg2Sc2Itm8' @('1702mx-schedule5-item18') @('official-hta-runtime#computePg2Sc2Itm8:L14845-L14866')
Calc '1702mx-schedule2-item9' @('Schedule 2 Item 9 A-D') @('Schedule 6 Item 5 A-D') 'Item 9 copies Schedule 6 Item 5.' 'computePg2Sc2Itm9' @('1702mx-schedule6-item5') @('official-hta-runtime#computePg2Sc2Itm9:L14868-L14884')
Calc '1702mx-schedule2-item10' @('Schedule 2 Item 10 B-D') @('Schedule 7.1 Item 8D','Schedule 8.1 Item 8D') 'Item 10B copies Schedule 8.1 total; Item 10C copies Schedule 7.1 total; Item 10D = B + C.' 'computePg2Sc2It10' @('1702mx-nolco-tables') @('official-hta-runtime#computePg2Sc2It10:L14888-L14906')
Calc '1702mx-schedule2-item11-itemized' @('Schedule 2 Item 11 A-D') @('Schedule 2 Items 8,9,10') 'When Itemized is selected: 11A=8A+9A; 11B=8B+9B+10B; 11C=8C+9C+10C; 11D=A+B+C.' 'computePg2Sc2It11 / computePg2Sch2Items' @('1702mx-schedule2-item8','1702mx-schedule2-item9','1702mx-schedule2-item10') @('official-hta-runtime#computePg2Sc2It11:L14908-L14956','official-hta-runtime#computePg2Sch2Items:L14957-L14968') 'verified-correct' 'Encode the column-specific applicability rather than a generic all-column sum.' 'Itemized deduction selected.'
Calc '1702mx-schedule2-item12-osd' @('Schedule 2 Item 12 C-D') @('Schedule 2 Item 7C') 'When OSD is selected: Item 12C = 40% of Item 7C; Item 12D copies C; Item 11C/D are zeroed.' 'computePg2Sc2It12' @('1702mx-schedule2-item7') @('official-hta-runtime#computePg2Sc2It12:L14969-L14982','repository-model#recompute_schedule_2:L577-L600') 'incorrect-official-behavior' 'Use the official column-specific applicability. The current repository model applies 40% to every column and should not be treated as official evidence.' 'Optional Standard Deduction selected.'
Calc '1702mx-schedule2-item13' @('Schedule 2 Item 13 A-D') @('Schedule 2 Item 7','selected Item 11 or Item 12') 'Itemized paths subtract Item 11 by applicable column; OSD path computes only 13C = 7C - 12C and 13D copies C.' 'computePg2Sc2It11 / computePg2Sch2Items / computePg2Sc2It12' @('1702mx-schedule2-item11-itemized','1702mx-schedule2-item12-osd') @('official-hta-runtime#computePg2Sc2It11:L14908-L14968','official-hta-runtime#computePg2Sc2It12:L14969-L14982') 'verified-correct' 'Represent applicability by regime and deduction method.'
Calc '1702mx-schedule2-item15-special' @('Schedule 2 Item 15B') @('Schedule 2 Item 1B','Item 3B','Item 7B','Item 14B rate') 'If rounded Item 3B >= rounded Item 7B, Item 15B = Item 1B × rate; otherwise Item 15B = Item 7B × rate.' 'computePg2Sc2It15' @('1702mx-schedule2-item3','1702mx-schedule2-item7') @('official-hta-runtime#computePg2Sc2It15:L14984-L15018') 'ambiguous' 'Preserve the exact comparison/base selection and flag it for domain review; comments show earlier alternative bases.' 
Calc '1702mx-schedule2-item15-regular' @('Schedule 2 Item 15C') @('Schedule 2 Item 13C','Item 14C rate') 'The direct product helper exists in computePg2Sc2It14C; another former product line in computePg2Sc2It15C is commented, and that function only copies Item 15C to Item 17C.' 'computePg2Sc2It14C / computePg2Sc2It15C' @('1702mx-schedule2-item13') @('official-hta-runtime#computePg2Sc2It14C:L15019-L15021','official-hta-runtime#computePg2Sc2It15C:L15022-L15027') 'official-bug-compatible' 'Bind calculations to the active event wiring and do not infer execution from a commented line.'
Calc '1702mx-schedule2-item17' @('Schedule 2 Item 17B-C') @('Schedule 2 Items 15B/C','Item 16B') '17B = 15B - 16B and is copied to 19B; 17C copies 15C.' 'computePg2Sc2It17B / computePg2Sc2It15C' @('1702mx-schedule2-item15-special','1702mx-schedule2-item15-regular') @('official-hta-runtime#computePg2Sc2It17B:L15029-L15033','official-hta-runtime#computePg2Sc2It15C:L15022-L15027')
Calc '1702mx-schedule2-item18-mcit' @('Schedule 2 Item 18C-D') @('Schedule 2 Item 7C') 'The source line that would compute 2% of Item 7C is commented. computePg2Sc2It18C only triggers Item 19C; Item 18C remains an enabled/input value when IC055 applies.' 'computePg2Sc2It18C' @('1702mx-schedule2-item7') @('official-hta-runtime#computePg2Sc2It18C:L15035-L15039','official-hta-runtime#enableMCITFields:L10921-L10988','repository-model#recompute_schedule_2:L616-L621') 'incorrect-official-behavior' 'Do not silently synthesize 2% as official runtime behavior. The current repository model does so and requires separate legal/product review.'
Calc '1702mx-schedule2-item19' @('Schedule 2 Item 19B-D') @('Schedule 2 Item 17B','Item 15C','Item 18C') '19B copies 17B; 19C = max(15C,18C); 19D = 19B + 19C.' 'computePg2Sc2It17B / computePg2Sc2It19C / computePg2Sc2ColumnD' @('1702mx-schedule2-item17','1702mx-schedule2-item18-mcit') @('official-hta-runtime#computePg2Sc2It17B:L15029-L15033','official-hta-runtime#computePg2Sc2It19C:L15042-L15053','official-hta-runtime#computePg2Sc2ColumnD:L14783-L14787')
Calc '1702mx-schedule3-item32' @('Schedule 3 Item 32 A-D') @('Schedule 3 Items 20-31 by applicable column') 'Item 32 is the sum of Items 20-31, with A/B omitting inapplicable Items 21 and 23 and C/D including them.' 'computePg2Schd3It32A-D' @() @('official-hta-runtime#computePg2Schd3It32:L11200-L11243','repository-model#recompute_schedule_3:L631-L650') 'incorrect-official-behavior' 'Model the source column applicability; the current repository helper generically sums the first 12 rows for every column.'
Calc '1702mx-schedule3-item33' @('Schedule 3 Item 33 B-D') @('Schedule 2 Item 19 B-D','Schedule 3 Item 32 B-D') 'Item 33B-D = Schedule 2 Item 19B-D - Schedule 3 Item 32B-D. The A calculation is commented.' 'computePg2Sc3It33' @('1702mx-schedule2-item19','1702mx-schedule3-item32') @('official-hta-runtime#computePg2Sc3It33:L11115-L11125','repository-model#recompute_schedule_3:L631-L650') 'incorrect-official-behavior' 'Preserve that A is not actively computed; do not generalize across all four columns.'
Calc '1702mx-schedule4-item3' @('Schedule 4 Item 3 A-D') @('Schedule 4 Items 1 and 2') '3A=1A+2A; 3B=1B+2B; 3C=2C; 3D=A+B+C.' 'computePg3Sc4Itm3' @() @('official-hta-runtime#computePg3Sc4Itm3:L15238-L15249','repository-model#recompute_schedule_4:L652-L665') 'incorrect-official-behavior' 'Use the source column applicability; the repository model currently sums Item 1 + Item 2 in every column.'
Calc '1702mx-schedule4-item5' @('Schedule 4 Item 5 A-D') @('Schedule 4 Items 3 and 4') '5A copies 3A; 5B=3B-4B; 5C copies 3C; 5D=A+B+C.' 'computePg3Sc4Itm5' @('1702mx-schedule4-item3') @('official-hta-runtime#computePg3Sc4Itm5:L15251-L15257','repository-model#recompute_schedule_4:L666-L673') 'incorrect-official-behavior' 'Use the source column applicability; the repository model subtracts Item 4 in every column.'
Calc '1702mx-schedule4-item7' @('Schedule 4 Item 7 A-D') @('Schedule 4 Items 5 and 6') 'Item 7 = Item 5 + Item 6 by A-C; D=A+B+C.' 'computePg3Sc4Itm7' @('1702mx-schedule4-item5') @('official-hta-runtime#computePg3Sc4Itm7:L15259-L15264','repository-model#recompute_schedule_4:L674-L678')
Calc '1702mx-schedule5-item18' @('Schedule 5 Item 18 A-D') @('Schedule 5 Items 1-17i') 'Item 18 is the sum of the 25 printed ordinary-itemized-deduction rows in each column.' 'computePg3Schd5It18' @() @('official-hta-runtime#computePg3Schd5It18:L11244-L11250','repository-model#recompute_schedule_5_and_6:L680-L689')
Calc '1702mx-schedule6-item5' @('Schedule 6 Item 5 A-D') @('Schedule 6 Items 1-4') 'Item 5 is the sum of the four special-allowable-deduction rows by column.' 'schedule 6 compute functions' @() @('official-hta-runtime#schedule6-computation','repository-model#recompute_schedule_5_and_6:L690-L701')
Calc '1702mx-nolco-seed' @('Schedule 7/8 Item 1-3 values') @('Schedule 2 Item 7B/C','Schedule 5 Item 18B/C') 'For B, when Item 7B < Item 18B, Schedule 8 Item 1=7B, Item 2=18B, Item 3=1-2. For C, when Item 18C > Item 7C, Schedule 7 Item 1=7C, Item 2=18C, Item 3=1-2; otherwise the negative handler runs.' 'computeNolco(type)' @('1702mx-schedule2-item7','1702mx-schedule5-item18') @('official-hta-runtime#computeNolco:L14522-L14553') 'official-bug-compatible' 'Preserve the active branch semantics but remove fallthrough in a typed implementation.'
Calc '1702mx-nolco-switch-fallthrough' @('Schedule 7/8 seed values') @('computeNolco type B') 'The switch has no break after case B, so a B call also executes the C case.' 'computeNolco(type)' @('1702mx-nolco-seed') @('official-hta-runtime#computeNolco:L14524-L14551') 'incorrect-official-behavior' 'Evaluate B and C independently without implicit switch fallthrough.'
Calc '1702mx-nolco-tables' @('Schedule 7.1/8.1 unapplied and totals') @('amount','applied previous years','expired','applied current year') 'Per row, unapplied = amount - (applied previous years + expired + applied current year); total current-year application sums the row current-year applications.' 'schedule 7.1/8.1 compute functions' @('1702mx-nolco-seed') @('official-hta-runtime#schedule7a:L14555-L14640','repository-model#recompute_nolco_table:L937-L967') 'official-bug-compatible' 'Enforce B+C+D <= A before subtraction and retain signed audit evidence.'
Calc '1702mx-mcit-table' @('Schedule 9 columns C/G and Item 4') @('MCIT','normal income tax','prior applications','expired','current application') 'C = max(MCIT - normal income tax, 0); G = C - (D+E+F), resetting D/E/F/G to zero if negative; Item 4 = sum of current-year applications (column F).' 'computePg7Sc9I1CC..I4' @() @('official-hta-runtime#computePg7Sc9I1CC:L14669-L14677','official-hta-runtime#computePg7Sc9I1CG:L14699-L14709','official-hta-runtime#computePg7Sc9I4:L14736-L14739','repository-model#recompute_mcit:L969-L1006') 'official-bug-compatible' 'Return a structured limit error instead of destructive zeroing.'
Calc '1702mx-schedule10-item4' @('Schedule 10 Item 4 A-D') @('Schedule 10 Items 1-3 A-D') 'Item 4 = sum of Items 1-3 by column.' 'computePg4Sc10I4' @() @('official-hta-runtime#computePg4Sc10I4:L15271-L15277','repository-model#recompute_schedule_10:L703-L717')
Calc '1702mx-schedule10-item9' @('Schedule 10 Item 9 A-D') @('Schedule 10 Items 5-8 A-D') 'Item 9 = sum of Items 5-8 by column.' 'computePg4Sc10I9' @() @('official-hta-runtime#computePg4Sc10I9:L15280-L15285','repository-model#recompute_schedule_10:L718-L726')
Calc '1702mx-schedule10-item10' @('Schedule 10 Item 10 aliases in Schedule 2 Item 10 keys') @('Schedule 10 Item 4 A-D','Schedule 10 Item 9 A-D') 'Item 10 = Item 4 - Item 9 by column, written into txtPg2Sc2Itm10A-D.' 'computePg4Sc10I10' @('1702mx-schedule10-item4','1702mx-schedule10-item9') @('official-hta-runtime#computePg4Sc10I10:L15287-L15292','repository-model#recompute_schedule_10:L727-L735') 'ambiguous' 'Keep printed semantics distinct from the historical serialized key names.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema' = '../../schema/calculations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    evaluation_order = @($calculations.calculation_id)
    calculations = $calculations
})

$negativeRules = @($rules | Where-Object { $_.exact_message } | Select-Object -First 50)
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
        @{ case_id = 'part21-positive'; calculation_id = '1702mx-part2-item21-nonnegative'; inputs = @{ item16 = 100; item20 = 5 }; official_output = 105 },
        @{ case_id = 'part21-overpayment-with-penalty'; calculation_id = '1702mx-part2-item21-negative-with-penalty'; inputs = @{ item16 = -100; item20 = 5 }; official_output = 5; repository_model_output = -95 },
        @{ case_id = 'part21-overpayment-no-penalty'; calculation_id = '1702mx-part2-item21-negative-no-penalty'; inputs = @{ item16 = -100; item20 = 0 }; official_output = -100 },
        @{ case_id = 'osd-column-c'; calculation_id = '1702mx-schedule2-item12-osd'; inputs = @{ item7C = 700000 }; official_output_item12C = 280000 },
        @{ case_id = 'mcit-not-auto-computed'; calculation_id = '1702mx-schedule2-item18-mcit'; inputs = @{ item7C = 700000; item18C = 0 }; official_output_item18C = 0; repository_model_output_item18C = 14000 },
        @{ case_id = 'schedule4-column-a'; calculation_id = '1702mx-schedule4-item5'; inputs = @{ item3A = 100; item4A = 30 }; official_output_item5A = 100; repository_model_output_item5A = 70 },
        @{ case_id = 'nolco-switch-fallthrough'; calculation_id = '1702mx-nolco-switch-fallthrough'; inputs = @{ call_type = 'B' }; official_behavior = 'B branch then C branch' }
    )
})

$resources = @()
foreach ($src in @([regex]::Matches($hta, '(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1') | ForEach-Object { $_.Groups['v'].Value } | Sort-Object -Unique)) {
    $full = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if (Test-Path -LiteralPath $full) {
        $resources += [pscustomobject][ordered]@{
            src = $src; path = $full; present = $true; size = (Get-Item -LiteralPath $full).Length
            sha256 = (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()
        }
    } else {
        $resources += [pscustomobject][ordered]@{ src = $src; path = $full; present = $false; size = $null; sha256 = $null }
    }
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{
    schema_version = '1.0.0'; form_id = $formId; resources = $resources
})
Write-Json (Join-Path $fixtureDir 'repository-contract-diff.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    repository_model_path = $existingModelPath
    repository_xml_path = $existingXmlPath
    repository_xml_contract_scope = '210-field editable-save contract'
    official_editable_subset_count = 210
    official_final_copy_count = 588
    uncovered_final_copy_key_count = 378
    calculation_differences = @(
        @{ id = 'part-ii-item-21'; official = 'Conditional branch; negative Item 16 with positive Item 20 yields Item 20 only.'; repository = 'Always Item 16 + Item 20.' },
        @{ id = 'schedule-2-item-12-osd'; official = '40% is applied to column C only.'; repository = '40% is applied generically to all four columns.' },
        @{ id = 'schedule-2-item-18-mcit'; official = '2% computation line is commented; Item 18C is not auto-computed by computePg2Sc2It18C.'; repository = 'Computes 2% of Item 7 regular.' },
        @{ id = 'schedule-3-item-32-33'; official = 'Column-specific applicability; Item 33A is not computed.'; repository = 'Generic four-column sums/differences.' },
        @{ id = 'schedule-4-items-3-5'; official = 'Column-specific applicability/copy behavior.'; repository = 'Generic sum/subtract in all columns.' }
    )
    renderer_or_model_modified = $false
})

$workflow = [ordered]@{
    '$schema' = '../../schema/workflow.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    phases = @(
        @{ phase = 'edit'; official_behavior = 'January 2018 1702-MX four-page annual return with a separate two-page mandatory attachment and runtime add-more/modal field families.'; source_refs = @('official-form-pdf','official-attachment-pdf','official-hta-runtime#frmMain'); confidence = 'high' },
        @{ phase = 'saved-draft'; official_behavior = 'Save checks only RDO, registered name/address, contact number, and email, shows the first failure, and the reviewed plaintext artifact contains a 210-key editable subset.'; source_refs = @('official-hta-runtime#initialValidateBeforeSave:L21427-L21470','xml-editable-subset-v1'); confidence = 'high' },
        @{ phase = 'validated'; official_behavior = 'Validate uses the source order recorded in validations.json; most branches stop at first error, while mandatory-attachment validation can alert repeatedly before returning false. Success disables form controls and enables Edit, Upload, and Final Copy.'; source_refs = @('official-hta-runtime#validate:L21103-L21425'); confidence = 'high' },
        @{ phase = 'final-copy'; official_behavior = 'The reviewed encrypted companion decrypts in memory to 588 concrete keys: the 210 editable keys plus 378 final-copy-only keys. Values are never emitted.'; source_refs = @('encrypted-field-audit-v796'); confidence = 'high' },
        @{ phase = 'submitted'; official_behavior = 'Online return/mandatory-attachment transport exists but was not exercised.'; source_refs = @('official-hta-runtime#saveXMLsubmit-and-sendEmail','repository-model#transition_to_queued:L728-L732'); confidence = 'medium' }
    )
    transitions = @(
        @{ from = 'edit'; action = 'Save'; to = 'saved-draft'; guard = 'Five narrow profile checks pass.'; side_effects = @('Writes a plaintext pseudo-XML editable save.'); source_refs = @('official-hta-runtime#initialValidateBeforeSave:L21427-L21470') },
        @{ from = 'edit'; action = 'Validate'; to = 'validated'; guard = 'All active source-ordered checks pass.'; side_effects = @('Disables controls.','Enables Edit, Upload, Final Copy, Previous, and Next.'); source_refs = @('official-hta-runtime#validate:L21103-L21417') },
        @{ from = 'validated'; action = 'Edit'; to = 'edit'; guard = $null; side_effects = @('Re-enables applicable controls.'); source_refs = @('official-hta-runtime#edit-workflow') },
        @{ from = 'validated'; action = 'Final Copy'; to = 'final-copy'; guard = 'Official finalization/profile flow permits progress.'; side_effects = @('Creates an encrypted/compressed artifact with 588 concrete keys in the reviewed example.'); source_refs = @('encrypted-field-audit-v796') },
        @{ from = 'final-copy'; action = 'Transport return and mandatory attachment'; to = 'submitted'; guard = 'Connectivity, attachment, and remote acceptance succeed.'; side_effects = @('Attempts online transport; untested.'); source_refs = @('official-hta-runtime#saveXMLsubmit-and-sendEmail') }
    )
    prerequisites = @(
        'January 2018C revision and applicable annual filing period',
        'Taxpayer profile fields required by initialValidateBeforeSave',
        'At least one applicable ATC',
        'Date of incorporation / organization',
        'Method of deduction',
        'Complete applicable schedules and mandatory-attachment rows',
        'Overpayment disposition when Item 16 or Item 21 is negative'
    )
    required_attachments = @(
        @{ attachment_id = '1702mx-mandatory-attachment'; label = 'Separate two-page 1702-MX mandatory attachment for multiple exempt and/or special-rate activities.'; required_when = 'Instruction B / multiple exempt or special activities applies.'; official_ui_enforcement = 'Runtime modal rows are locally checked by validateAttachments, but transport was not exercised.'; source_refs = @('official-attachment-pdf','official-hta-runtime#validateAttachments:L18881-L19065'); confidence = 'high' },
        @{ attachment_id = 'audited-financial-statements'; label = 'Audited Financial Statements and related schedules/reconciliation when required.'; required_when = 'Applicable under the official filing instructions.'; official_ui_enforcement = 'Presence is not verified by the local Validate code reviewed.'; source_refs = @('legacy-help-supporting-only#attachments:L674-L693'); confidence = 'medium' },
        @{ attachment_id = 'tax-credit-proof'; label = 'Proof of tax credits/payments claimed in Schedule 3.'; required_when = 'A credit/payment line is claimed.'; official_ui_enforcement = 'Local row calculations and completeness checks do not verify external proof files.'; source_refs = @('official-form-pdf#Schedule-3','legacy-help-supporting-only#attachments:L674-L693'); confidence = 'medium' }
    )
    filing_deadlines = @(
        @{ quarter = 'Q1'; due_date_rule = 'Annual return: legacy June 2013 runtime help states the 15th day of the fourth month following close of the taxable year; exact January 2018 revision-matched instruction remains unverified.'; source_refs = @('legacy-help-supporting-only#deadline:L177-L197'); confidence = 'medium' },
        @{ quarter = 'Q2'; due_date_rule = 'Not quarterly; same annual deadline caveat applies.'; source_refs = @('legacy-help-supporting-only#deadline:L177-L197'); confidence = 'medium' },
        @{ quarter = 'Q3'; due_date_rule = 'Not quarterly; same annual deadline caveat applies.'; source_refs = @('legacy-help-supporting-only#deadline:L177-L197'); confidence = 'medium' },
        @{ quarter = 'Q4'; due_date_rule = 'Not quarterly; same annual deadline caveat applies.'; source_refs = @('legacy-help-supporting-only#deadline:L177-L197'); confidence = 'medium' }
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @($rules | Where-Object { $_.assessment -in @('official-bug-compatible','incorrect-official-behavior','obsolete') }).Count
$encryptedDisplay = Join-Path $SourceDir '1702MXv2018C-final-copy-#email-redacted#.xml'
$assets = @(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'Exact formTyp and final-copy/transport identifiers bind 1702MXv2018C; printed January 2018.'
    Asset 'legacy-help-supporting-only' 'official-runtime-help-legacy' $helpPath 'June 2013 help; supporting only, not silently revision-matched.'
    Asset 'xml-editable-subset-v1' 'dummy-profile-editable-save' $plainPath 'Reviewed 210-key editable subset; values excluded.'
    Asset 'xml-final-v1' 'dummy-profile-encrypted-final-copy' $encryptedPath 'Reviewed 588-key final-copy artifact; decrypted in memory; values excluded.' $encryptedDisplay
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1702-MX.'
    Asset 'official-attachment-pdf' 'official-mandatory-attachment-pdf' $attachmentPdfPath 'January 2018 two-page mandatory attachment.'
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    form_code = '1702MX'
    revision = $revision
    revision_label = 'January 2018C'
    package_version = $packageVersion
    status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = 588
        editable_subset_fields = 210
        final_copy_only_fields = 378
        runtime_field_families = 83
        fields_total = $fields.Count
        typed_fields = $fields.Count
        validation_rules = $rules.Count
        confirmed_official_bugs = $bugCount
        calculations = $calculations.Count
        negative_fixtures = $cases.Count
        unverified_gaps = 3
    }
    artifacts = [ordered]@{
        fields = 'fields.json'
        validations = 'validations.json'
        calculations = 'calculations.json'
        workflow = 'workflow.json'
        evidence = 'evidence.md'
        audit = 'audit.md'
        gaps = 'gaps.md'
        runtime_control_fixture = 'fixtures/runtime-control-inventory-v796.json'
        editable_subset_audit = 'fixtures/editable-subset-audit-v796.json'
        encrypted_field_audit = 'fixtures/encrypted-field-audit-v796.json'
        validation_function_fixture = 'fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture = 'fixtures/calculation-function-inventory-v796.json'
        resource_hash_fixture = 'fixtures/official-resource-hashes-v796.json'
        repository_contract_diff = 'fixtures/repository-contract-diff.json'
        negative_fixtures = 'fixtures/negative-cases.json'
        calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer, typed model, migration, capability, or release metadata changed.',
        'No source values or email-bearing filenames are copied.',
        'The 588-key final-copy inventory is authoritative for concrete-field preservation; the 210-key plaintext save is an exact editable subset.',
        'The 83 active indexed families are source-derived capacity patterns and had no concrete instance in the reviewed final-copy snapshot.',
        'June 2013 help is explicitly supporting-only because no revision-matched January 2018 help file was found.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest

Write-Utf8 (Join-Path $outDir 'README.md') @"
# BIR Form 1702-MX - January 2018C

Revision-specific Offline eBIRForms rule package for `1702MXv2018C`.

- 588 concrete final-copy keys
- 210-key editable subset
- 378 final-copy-only keys
- 83 active unbounded runtime families

All source values and email-bearing filenames are excluded.
"@
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- Exact HTA SHA-256: $($expected.hta); internal `formTyp`, final-copy names, and transport identifiers bind `1702MXv2018C`; printed revision is January 2018.
- Main form PDF SHA-256: $($expected.pdf).
- Mandatory attachment PDF SHA-256: $($expected.attachment_pdf).
- Legacy runtime help SHA-256: $($expected.help); it is June 2013 and is used only as supporting evidence.
- Plaintext dummy save SHA-256: $($expected.plain); 210 unique keys; inventory SHA-256 $($expected.plain_inventory).
- Encrypted dummy final copy SHA-256: $($expected.encrypted); in-memory decrypted SHA-256 $($expected.decrypted); 588 unique keys; inventory SHA-256 $($expected.encrypted_inventory).
- The 210 plaintext keys are an exact subset of the 588 decrypted keys; 378 keys occur only in the final-copy artifact.
- Runtime inventory: 835 controls, 696 serializer candidates/unique IDs, 83 active indexed families after comments are removed, and 518 inline functions.
- Existing repository XML code explicitly documents a 210-field editable-save contract; the 378 final-copy-only keys are therefore a recorded coverage gap, not discarded unknown data.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. Online return and mandatory-attachment transport were not exercised.
2. The installed help file is June 2013, not revision-matched January 2018; its deadline and attachment prose are supporting-only.
3. No indexed runtime-family instance appeared in the reviewed 588-key final-copy snapshot, so family capacity is source-proven but concrete save/reopen behavior remains unobserved.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Revision/assets: **pass** - exact v2018C HTA, January 2018 main/attachment PDFs, package executable, plaintext subset, and encrypted final copy are pinned.
- Fields: **pass** - all 588 concrete final-copy keys are preserved, the 210 editable keys are proved an exact subset, and 83 active indexed families are retained separately.
- Controls/functions: **pass** - 835 controls, 696 serializer candidates/unique static IDs, validation/calculation function inventories, and loaded resource hashes are captured.
- Rules/workflow: **pass** - Save/Validate/Final Copy/Submit differences, first-error order, exact active messages, attachment behavior, and obsolete/commented branches are separated.
- Calculations: **pass** - $($calculations.Count) source-bound calculation groups include column applicability, conditional overpayment behavior, commented MCIT calculation, NOLCO fallthrough, and dependency order.
- Official defects: **pass** - $bugCount bug-compatible/incorrect/obsolete findings remain distinct from recommended app behavior.
- Repository comparison: **pass** - the existing 210-field XML scope and five material formula differences are recorded without changing application code.
- Privacy: **pass** - no values or email-bearing filenames copied.
- Transport/help/family replay: **unverified** and explicit gaps.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 17: 1702mx-v2018c. Next: 1706.`n"

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -LiteralPath $indexPath -Raw | ConvertFrom-Json
$entry = [pscustomobject][ordered]@{
    form_id = $formId
    form_code = '1702MX'
    revision = $revision
    package_version = $packageVersion
    priority = 17
    status = 'complete'
    path = 'forms/1702mx-v2018c/manifest.json'
}
$index.forms = @(@($index.forms | Where-Object { $_.form_id -ne $formId }) + $entry | Sort-Object priority)
$index.updated = (Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), concrete=$($encryptedKeys.Count), editable=$($plainKeys.Count), families=$($dynamicFamilies.Count), rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bug_classifications=$bugCount, static_unique_ids=$($staticIds.Count), final_only=$($encryptedOnly.Count)"
