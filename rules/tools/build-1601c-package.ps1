param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir = 'C:\Mac\Home\Downloads\forms\1601Cv2018'
)

$ErrorActionPreference = 'Stop'
$formId = '1601c-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1601Cv2018.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1601Cv2018.hta'
$pdfPath = Join-Path $OfficialDir '1601C final Jan 2018 with DPA.pdf'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1601c-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'
$expected = @{
    hta = '3fb7b4185264e47c9d77b0def4301fa696e7b4424ad30b4e973c7c0b1f759879'
    help = '2395005a36fe8220e0d1fd5d6efa659456d0c4bc1e33a65ca5c67898ececf532'
    pdf = 'c8faaa71015337a73b4ceb96bfb265c539589ab5e10eb27899bb81f87f417397'
    package = 'de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher = '4501f3514a1883d0137d126101d02b3f0fa94daf7f6e39398b3729c9104c51d3'
    decrypted = '2c8595ffc2c96fd538e05a6458e3c9073a31133324e40bfcd1ed290fab042a93'
    encrypted_inventory = '4657f19d750aa81aaa8e8bc357627defc9e7bb1290bbac881dd5870777da750a'
    plain = '794892fc33c0fd7882a91327095f396fb1683d5b3c0d4cb1cb63916f981cad4c'
    plain_inventory = '06e7051ce1ba7104b180929a48afd7c481de547732217c92a47f3e9545533d9c'
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}
function Get-AttributeValue([string]$Tag, [string]$Name) {
    $match = [regex]::Match($Tag, ('(?i)\b{0}\s*=\s*([''"])(.*?)\1' -f [regex]::Escape($Name)))
    if ($match.Success) { return $match.Groups[2].Value }
    return $null
}
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
        $bytes = [Text.Encoding]::UTF8.GetBytes((@($Lines | Sort-Object) -join "`n"))
        ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant()
    }
    finally { $sha.Dispose() }
}
function New-Asset(
    [string]$AssetId,
    [string]$Kind,
    [string]$Path,
    [string]$Binding,
    [string]$DisplayPath = ''
) {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $AssetId
        kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = Get-Sha256 $Path
        size = $item.Length
        revision_binding = $Binding
    }
}

foreach ($asset in @(
    @($htaPath, 'hta'),
    @($helpPath, 'help'),
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
if (
    $hta -notmatch 'APPLICATIONNAME="1601Cv2018"' -or
    $hta -notmatch 'January 2018 \(ENCS\)'
) {
    throw 'January 2018 runtime binding changed.'
}
if ($help -notmatch 'BIR Form No\. 1601-C \[January 2018\(ENCS\)\]') {
    throw 'January 2018 help binding changed.'
}
if ($help -notmatch 'APPLICATIONNAME="0605"') {
    throw 'Known packaged-help APPLICATIONNAME defect changed.'
}
$pdfBytes = [IO.File]::ReadAllBytes($pdfPath)
if ([Text.Encoding]::ASCII.GetString($pdfBytes[0..4]) -ne '%PDF-') {
    throw '1601C PDF magic mismatch.'
}

$sampleFiles = @(Get-ChildItem -LiteralPath $OfficialDir -File -Filter '*.xml')
$sampleByHash = @{}
foreach ($file in $sampleFiles) {
    $sampleByHash[(Get-Sha256 $file.FullName)] = $file
}
foreach ($name in @('cipher', 'plain')) {
    if (-not $sampleByHash.ContainsKey($expected[$name])) {
        throw "Pinned 1601C sample missing: $name"
    }
}

$keyTool = Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson = & $keyTool `
    -SourcePath $sampleByHash[$expected.cipher].FullName `
    -RedactedSourcePath (Join-Path $OfficialDir '1601C-final-copy-#email-redacted#.xml') `
    -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher `
    -ExpectedDecryptedSha256 $expected.decrypted `
    -ExpectedFieldCount 101 `
    -ExpectedFieldInventorySha256 $expected.encrypted_inventory
$keyAudit = $keyJson | ConvertFrom-Json
$keys = @($keyAudit.keys)

$plainText = [IO.File]::ReadAllText($sampleByHash[$expected.plain].FullName)
$plainKeys = @(
    [regex]::Matches($plainText, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>') |
        ForEach-Object { $_.Groups['key'].Value }
)
if ($plainKeys.Count -ne 100 -or @($plainKeys | Sort-Object -Unique).Count -ne 100) {
    throw 'Plaintext 1601C inventory changed.'
}
if ((Get-LineInventoryHash $plainKeys) -ne $expected.plain_inventory) {
    throw 'Plaintext 1601C inventory hash changed.'
}
$encryptedOnly = @($keys | Where-Object { $plainKeys -notcontains $_ })
$plainOnly = @($plainKeys | Where-Object { $keys -notcontains $_ })
if (
    $encryptedOnly.Count -ne 1 -or
    $encryptedOnly[0] -ne 'frm1601c:txtAddress2' -or
    $plainOnly.Count -ne 0
) {
    throw '1601C save/final-copy field difference changed.'
}

New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null
Write-TextFile (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') (
    $keyJson -join [Environment]::NewLine
)
Write-JsonFile (Join-Path $fixtureDir 'plaintext-field-audit-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    source_path = (Join-Path $OfficialDir '1601C-save-#email-redacted#.xml')
    sha256 = $expected.plain
    field_count = $plainKeys.Count
    unique_field_count = @($plainKeys | Sort-Object -Unique).Count
    field_inventory_sha256 = $expected.plain_inventory
    encrypted_only_keys = $encryptedOnly
    plain_only_keys = $plainOnly
    values_emitted = $false
})

$controlTool = Join-Path $RepoRoot 'rules\tools\inspect-hta-controls.ps1'
$controlJson = & $controlTool -HtaPath $htaPath
$controlAudit = $controlJson | ConvertFrom-Json
$controls = @($controlAudit.controls)
$controlById = @{}
foreach ($control in $controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) {
        $controlById[$control.id] = $control
    }
}
$staticMatches = @($keys | Where-Object { $controlById.ContainsKey($_) })
$runtimeRdo = @($keys | Where-Object { $_ -eq 'frm1601c:txtRDOCode' })
$unexplained = @(
    $keys | Where-Object {
        -not $controlById.ContainsKey($_) -and $_ -ne 'frm1601c:txtRDOCode'
    }
)

$selectEnums = @{}
foreach ($selectMatch in [regex]::Matches($hta, '(?is)<select\b(?<open>[^>]*)>(?<body>.*?)</select>')) {
    $selectId = Get-AttributeValue $selectMatch.Groups['open'].Value 'id'
    if (-not $selectId) { continue }
    $values = [Collections.Generic.List[object]]::new()
    foreach ($option in [regex]::Matches($selectMatch.Groups['body'].Value, '(?is)<option\b(?<open>[^>]*)>(?<text>.*?)</option>')) {
        $entry = [pscustomobject][ordered]@{
            value = Get-AttributeValue $option.Groups['open'].Value 'value'
            label = [Net.WebUtility]::HtmlDecode(
                [regex]::Replace($option.Groups['text'].Value, '<[^>]+>', '')
            ).Trim()
        }
        [void]$values.Add($entry)
    }
    $selectEnums[$selectId] = @($values)
}

$familyDefinitions = @(
    @{
        key = 'chkScheduleDelete{N>=0}'
        label = 'Schedule I row N delete selector'
        kind = 'runtime-indexed-family'
        logical = 'boolean'
        default = 'false'
        computed = $false
        max = $null
        source = 'official-hta-runtime#addSchedule:L3369-L3384'
    },
    @{
        key = 'frm1601c:sched1:txtMonthYear{N>=0}'
        label = 'Schedule I row N applicable month/year'
        kind = 'runtime-indexed-family'
        logical = 'month-year'
        default = ''
        computed = $false
        max = 7
        source = 'official-hta-runtime#addSchedule:L3372'
    },
    @{
        key = 'frm1601c:sched1:txtDatePaid{N>=0}'
        label = 'Schedule I row N date paid'
        kind = 'runtime-indexed-family'
        logical = 'date'
        default = ''
        computed = $false
        max = 10
        source = 'official-hta-runtime#addSchedule:L3373'
    },
    @{
        key = 'frm1601c:sched1:txtBankCode{N>=0}'
        label = 'Schedule I row N bank code'
        kind = 'runtime-indexed-family'
        logical = 'code'
        default = ''
        computed = $false
        max = 20
        source = 'official-hta-runtime#addSchedule:L3374'
    },
    @{
        key = 'frm1601c:sched1:txtNumber{N>=0}'
        label = 'Schedule I row N payment number'
        kind = 'runtime-indexed-family'
        logical = 'string'
        default = ''
        computed = $false
        max = 20
        source = 'official-hta-runtime#addSchedule:L3375'
    },
    @{
        key = 'frm1601c:sched1:txtTaxPaid{N>=0}'
        label = 'Schedule I row N tax paid'
        kind = 'runtime-indexed-family'
        logical = 'decimal-amount'
        default = '0.00'
        computed = $false
        max = 25
        source = 'official-hta-runtime#addSchedule:L3376'
    },
    @{
        key = 'frm1601c:sched1:txtShouldTaxDue{N>=0}'
        label = 'Schedule I row N tax that should have been paid'
        kind = 'runtime-indexed-family'
        logical = 'decimal-amount'
        default = '0.00'
        computed = $false
        max = 25
        source = 'official-hta-runtime#addSchedule:L3381'
    },
    @{
        key = 'frm1601c:sched1:txtAdjustments{N>=0}'
        label = 'Schedule I row N adjustment'
        kind = 'runtime-indexed-family'
        logical = 'decimal-amount'
        default = '0.00'
        computed = $true
        max = 25
        source = 'official-hta-runtime#addSchedule:L3382'
    }
)

$requiredKeys = @(
    'frm1601c:txtMonth',
    'frm1601c:txtYear',
    'frm1601c:TaxWithheld_1',
    'frm1601c:TaxWithheld_2',
    'frm1601c:txtTIN1',
    'frm1601c:txtTIN2',
    'frm1601c:txtTIN3',
    'frm1601c:txtBranchCode',
    'frm1601c:txtRDOCode',
    'frm1601c:txtTaxpayerName',
    'frm1601c:txtAddress',
    'frm1601c:txtZipCode',
    'frm1601c:txtTelNum',
    'frm1601c:CatAgent_P',
    'frm1601c:CatAgent_G'
)
$computedKeys = @(
    'frm1601c:txtTax21',
    'frm1601c:txtTax22',
    'frm1601c:txtTax24',
    'frm1601c:txtTax26',
    'frm1601c:txtTax27',
    'frm1601c:txtTax30',
    'frm1601c:txtTax31',
    'frm1601c:txtTax35',
    'frm1601c:txtTax36',
    'frm1601c:sched1:txtAdjustments0',
    'frm1601c:sched1:txtAdjustments1',
    'frm1601c:sched1:txtAdjustments2',
    'frm1601c:sched1:txtTotal1'
)

function Get-ItemNumber([string]$Key) {
    if ($Key -match 'txtMonth$|txtYear$') { return '1' }
    if ($Key -match 'AmendedRtn') { return '2' }
    if ($Key -match 'TaxWithheld') { return '3' }
    if ($Key -match 'txtSheets') { return '4' }
    if ($Key -match 'txtATC') { return '5' }
    if ($Key -match 'txtTIN|txtBranchCode') { return '6' }
    if ($Key -match 'txtRDOCode') { return '7' }
    if ($Key -match 'txtTaxpayerName|txtLineBus') { return '8' }
    if ($Key -match 'txtAddress') { return '9' }
    if ($Key -match 'txtZipCode') { return '9A' }
    if ($Key -match 'txtTelNum') { return '10' }
    if ($Key -match 'CatAgent') { return '11' }
    if ($Key -eq 'txtEmail') { return '12' }
    if ($Key -match 'SpecialTax|selTreaty') { return '13' }
    if ($Key -match 'txtTax(?<item>1[4-9]|2[0-9]|3[0-6])$') { return $Matches.item }
    if ($Key -match 'txt20Other') { return '20' }
    if ($Key -match 'txt29Other') { return '29' }
    if ($Key -match 'txtAgency37|txtNumber37|txtDate37|txtAmount37') { return '37' }
    if ($Key -match 'txtAgency38|txtNumber38|txtDate38|txtAmount38') { return '38' }
    if ($Key -match 'txtNumber39|txtDate39|txtAmount39') { return '39' }
    if ($Key -match 'txtParticular40|txtAgency40|txtNumber40|txtDate40|txtAmount40') { return '40' }
    if ($Key -match 'sched1') { return 'Schedule I' }
    return $null
}
function Get-Label([string]$Key) {
    $labels = @{
        'frm1601c:txtMonth' = 'Return period month'
        'frm1601c:txtYear' = 'Return period year'
        'frm1601c:AmendedRtn_1' = 'Amended return: Yes'
        'frm1601c:AmendedRtn_2' = 'Amended return: No'
        'frm1601c:TaxWithheld_1' = 'Taxes withheld/remitted: Yes'
        'frm1601c:TaxWithheld_2' = 'Taxes withheld/remitted: No'
        'frm1601c:txtSheets' = 'Number of sheets attached'
        'frm1601c:txtATC' = 'Alphanumeric tax code'
        'frm1601c:txtTIN1' = 'TIN segment 1'
        'frm1601c:txtTIN2' = 'TIN segment 2'
        'frm1601c:txtTIN3' = 'TIN segment 3'
        'frm1601c:txtBranchCode' = 'TIN branch code'
        'frm1601c:txtRDOCode' = 'RDO code'
        'frm1601c:txtTaxpayerName' = 'Withholding agent name'
        'frm1601c:txtLineBus' = 'Registered name / line of business'
        'frm1601c:txtAddress' = 'Registered address line 1'
        'frm1601c:txtAddress2' = 'Registered address line 2'
        'frm1601c:txtZipCode' = 'ZIP code'
        'frm1601c:txtTelNum' = 'Telephone number'
        'frm1601c:CatAgent_P' = 'Withholding agent category: Private'
        'frm1601c:CatAgent_G' = 'Withholding agent category: Government'
        'txtEmail' = 'Email address'
        'frm1601c:SpecialTax_1' = 'Special tax rate: Yes'
        'frm1601c:SpecialTax_2' = 'Special tax rate: No'
        'frm1601c:selTreaty' = 'Treaty or international agreement'
        'frm1601c:txtTax14' = 'Total amount of compensation'
        'frm1601c:txtTax15' = 'Statutory minimum wage'
        'frm1601c:txtTax16' = 'MWE holiday/overtime/night/hazard pay'
        'frm1601c:txtTax17' = '13th month pay and other benefits'
        'frm1601c:txtTax18' = 'De minimis benefits'
        'frm1601c:txtTax19' = 'Mandatory contributions and union dues'
        'frm1601c:txt20Other' = 'Other non-taxable compensation description'
        'frm1601c:txtTax20' = 'Other non-taxable compensation amount'
        'frm1601c:txtTax21' = 'Total non-taxable compensation'
        'frm1601c:txtTax22' = 'Total taxable compensation'
        'frm1601c:txtTax23' = 'Taxable compensation not subject to withholding'
        'frm1601c:txtTax24' = 'Net taxable compensation'
        'frm1601c:txtTax25' = 'Total taxes withheld'
        'frm1601c:txtTax26' = 'Prior-month withholding adjustment'
        'frm1601c:txtTax27' = 'Taxes withheld for remittance'
        'frm1601c:txtTax28' = 'Tax remitted in previously filed return'
        'frm1601c:txt29Other' = 'Other remittance description'
        'frm1601c:txtTax29' = 'Other remittances made'
        'frm1601c:txtTax30' = 'Total tax remittances made'
        'frm1601c:txtTax31' = 'Tax still due or over-remittance'
        'frm1601c:txtTax32' = 'Surcharge'
        'frm1601c:txtTax33' = 'Interest'
        'frm1601c:txtTax34' = 'Compromise'
        'frm1601c:txtTax35' = 'Total penalties'
        'frm1601c:txtTax36' = 'Total amount still due or over-remittance'
        'txtTaxAgentNo' = 'Tax agent accreditation or attorney roll number'
        'txtDateIssue' = 'Tax agent accreditation date of issue'
        'txtDateExpiry' = 'Tax agent accreditation date of expiry'
        'frm1601c:txtPg2TIN1' = 'Page 2 TIN segment 1'
        'frm1601c:txtPg2TIN2' = 'Page 2 TIN segment 2'
        'frm1601c:txtPg2TIN3' = 'Page 2 TIN segment 3'
        'frm1601c:txtPg2BranchCode' = 'Page 2 branch code'
        'frm1601c:txtPg2TaxpayerName' = 'Page 2 withholding agent name'
        'frm1601c:sched1:txtTotal1' = 'Schedule I total adjustment'
        'frm1601c:txtCurrentPage' = 'Current form page'
        'frm1601c:txtMaxPage' = 'Maximum form page'
    }
    if ($labels.ContainsKey($Key)) { return $labels[$Key] }
    if ($Key -match '^chkScheduleDelete(?<row>\d+)$') { return "Schedule I row $($Matches.row) delete selector" }
    if ($Key -match '^frm1601c:sched1:txtMonthYear(?<row>\d+)$') { return "Schedule I row $($Matches.row) applicable month/year" }
    if ($Key -match '^frm1601c:sched1:txtDatePaid(?<row>\d+)$') { return "Schedule I row $($Matches.row) date paid" }
    if ($Key -match '^frm1601c:sched1:txtBankCode(?<row>\d+)$') { return "Schedule I row $($Matches.row) bank code" }
    if ($Key -match '^frm1601c:sched1:txtNumber(?<row>\d+)$') { return "Schedule I row $($Matches.row) payment number" }
    if ($Key -match '^frm1601c:sched1:txtTaxPaid(?<row>\d+)$') { return "Schedule I row $($Matches.row) tax paid" }
    if ($Key -match '^frm1601c:sched1:txtShouldTaxDue(?<row>\d+)$') { return "Schedule I row $($Matches.row) tax that should have been paid" }
    if ($Key -match '^frm1601c:sched1:txtAdjustments(?<row>\d+)$') { return "Schedule I row $($Matches.row) adjustment" }
    if ($Key -match 'txtAgency(?<item>3[7-9]|40)') { return "Item $($Matches.item) drawee bank or agency" }
    if ($Key -match 'txtNumber(?<item>3[7-9]|40)') { return "Item $($Matches.item) payment number" }
    if ($Key -match 'txtDate(?<item>3[7-9]|40)') { return "Item $($Matches.item) payment date" }
    if ($Key -match 'txtAmount(?<item>3[7-9]|40)') { return "Item $($Matches.item) payment amount" }
    if ($Key -match 'txtParticular40') { return 'Item 40 other payment particulars' }
    return $Key
}

function New-ConcreteField([string]$Key) {
    $control = if ($controlById.ContainsKey($Key)) { $controlById[$Key] } else { $null }
    $kind = if ($control) { $control.control_kind } else { 'runtime-generated-select' }
    $logical = if ($kind -in @('radio', 'checkbox')) {
        'boolean'
    }
    elseif ($Key -match 'txtMonthYear') {
        'month-year'
    }
    elseif ($Key -match 'txtDate|DateIssue|DateExpiry') {
        'date'
    }
    elseif ($Key -match 'txtTax\d|txtAmount|txtAdjustments|txtShouldTaxDue') {
        'decimal-amount'
    }
    elseif ($Key -match 'TIN|Branch|RDO|ATC|Zip|Month$|Year$|BankCode|CurrentPage|MaxPage') {
        'code'
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
    $enabledWhen = $null
    if ($Key -in @('frm1601c:txtTax14', 'frm1601c:txtTax25')) {
        $required = 'conditional'
        $requiredWhen = 'Taxes withheld/remitted Yes is selected; official Validate requires a value greater than zero.'
    }
    elseif ($Key -eq 'frm1601c:selTreaty') {
        $required = 'conditional'
        $requiredWhen = 'Special tax rate Yes is selected, although Validate does not enforce it.'
        $enabledWhen = 'Special tax rate Yes is selected.'
    }
    elseif ($Key -eq 'frm1601c:txtTax28') {
        $enabledWhen = 'Amended return Yes is selected.'
    }
    elseif ($Key -match '^frm1601c:sched1:') {
        $required = if ($computed) { 'computed' } else { 'conditional' }
        $requiredWhen = if ($computed) { $null } else { 'The corresponding Schedule I row exists and is completed.' }
        $enabledWhen = 'The corresponding Schedule I row exists.'
    }

    $constraints = [ordered]@{}
    if ($control -and $control.maxlength -match '^\d+$') {
        $constraints.max_length = [int]$control.maxlength
    }
    if ($logical -eq 'decimal-amount') {
        $constraints.precision = 2
        $constraints.sign = 'source-dependent; computed over-remittances may be negative'
    }
    elseif ($logical -eq 'date') {
        $constraints.format = 'MM/DD/YYYY'
    }
    elseif ($logical -eq 'month-year') {
        $constraints.format = 'MM/YYYY'
    }

    $enumValues = [object[]]::new(0)
    if ($kind -in @('radio', 'checkbox')) {
        $enumValues = [object[]]@('true', 'false')
    }
    elseif ($selectEnums.ContainsKey($Key)) {
        $enumValues = [object[]]@($selectEnums[$Key])
    }
    $normalization = [string[]]::new(0)
    if ($logical -eq 'decimal-amount') {
        $normalization = [string[]]@('NumWithComma', 'round(...,2)', 'formatCurrency', 'toFixed(2)')
    }
    elseif ($Key -match 'txtAddress') {
        $normalization = [string[]]@('Profile loading uppercases and splits at 127 characters.')
    }

    $notes = [Collections.Generic.List[string]]::new()
    [void]$notes.Add('Present in the revision-matched encrypted final-copy inventory; values excluded.')
    if ($Key -eq 'frm1601c:txtAddress2') {
        [void]$notes.Add('Present in encrypted final copy but omitted from the paired plaintext save.')
    }
    if ($Key -match 'sched1:\w+[0-2]$|^chkScheduleDelete[0-2]$') {
        [void]$notes.Add('Observed concrete member of an unbounded runtime-indexed family.')
    }

    [pscustomobject][ordered]@{
        field_key = $Key
        serialized_key = $Key
        serialized_occurrence = 1
        label = Get-Label $Key
        page = if ($Key -match 'txtPg2|sched1|CurrentPage|MaxPage') { 2 } else { 1 }
        item_number = Get-ItemNumber $Key
        control_kind = $kind
        storage_type = 'string'
        logical_type = $logical
        required = $required
        required_when = $requiredWhen
        enabled_when = $enabledWhen
        visible_when = $null
        default_value = if ($control) { $control.value } else { '000' }
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $enumValues
        normalization = $normalization
        computed = $computed
        calculation_id = if ($computed) { 'See calculations.json' } else { $null }
        source_refs = @(
            "xml-encrypted#decrypted-field:$Key",
            "official-hta-runtime#control:L$(if ($control) { $control.source_line } else { 2989 })"
        )
        confidence = 'high'
        notes = @($notes)
    }
}
function New-FamilyField($Definition) {
    $constraints = [ordered]@{ index = 'zero-based, source-unbounded' }
    if ($Definition.max) { $constraints.max_length = $Definition.max }
    if ($Definition.logical -eq 'date') { $constraints.format = 'MM/DD/YYYY' }
    if ($Definition.logical -eq 'month-year') { $constraints.format = 'MM/YYYY' }
    if ($Definition.logical -eq 'decimal-amount') {
        $constraints.precision = 2
        $constraints.sign = 'source-dependent'
    }
    $familyEnums = [object[]]::new(0)
    if ($Definition.logical -eq 'boolean') {
        $familyEnums = [object[]]@('true', 'false')
    }
    $familyNormalization = [string[]]::new(0)
    if ($Definition.logical -eq 'decimal-amount') {
        $familyNormalization = [string[]]@('NumWithComma', 'round(...,2)', 'formatCurrency')
    }
    [pscustomobject][ordered]@{
        field_key = $Definition.key
        serialized_key = $null
        serialized_occurrence = $null
        label = $Definition.label
        page = 2
        item_number = 'Schedule I'
        control_kind = $Definition.kind
        storage_type = 'string'
        logical_type = $Definition.logical
        required = if ($Definition.computed) { 'computed' } else { 'conditional' }
        required_when = if ($Definition.computed) { $null } else { 'The corresponding Schedule I row N exists and is completed.' }
        enabled_when = 'The Schedule I row exists.'
        visible_when = 'The Schedule I row exists.'
        default_value = $Definition.default
        empty_representation = ''
        constraints = [pscustomobject]$constraints
        enum_values = $familyEnums
        normalization = $familyNormalization
        computed = $Definition.computed
        calculation_id = if ($Definition.computed) { '1601c-schedule-row-adjustment' } else { $null }
        source_refs = @($Definition.source, 'official-hta-runtime#saveXML:L2201-L2493')
        confidence = 'high'
        notes = @('Source-derived unbounded serialized family; concrete indices 0-2 are proven by the paired final copy.')
    }
}

$fields = [Collections.Generic.List[object]]::new()
foreach ($key in $keys) {
    [void]$fields.Add((New-ConcreteField $key))
}
foreach ($definition in $familyDefinitions) {
    [void]$fields.Add((New-FamilyField $definition))
}
if ($fields.Count -ne 109) {
    throw "1601C typed inventory changed: $($fields.Count)."
}
Write-JsonFile (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema' = '../../schema/fields.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    field_count = $fields.Count
    runtime_serializable_element_count = $keys.Count
    inventory_sha256 = Get-LineInventoryHash @($fields.field_key)
    fields = $fields
})
Write-JsonFile (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    official_hta_sha256 = $expected.hta
    live_static_control_count = $controls.Count
    encrypted_key_count = $keys.Count
    plaintext_key_count = $plainKeys.Count
    static_match_count = $staticMatches.Count
    runtime_rdo_match_count = $runtimeRdo.Count
    unexplained_key_count = $unexplained.Count
    runtime_family_count = $familyDefinitions.Count
    observed_schedule_indices = @(0, 1, 2)
    encrypted_only_keys = $encryptedOnly
    controls = $controls
})
Write-JsonFile (Join-Path $fixtureDir 'schedule-family-inventory-v796.json') ([ordered]@{
    schema_version = '1.0.0'
    form_id = $formId
    observed_indices = @(0, 1, 2)
    source_unbounded = $true
    family_count = $familyDefinitions.Count
    families = $familyDefinitions
    source_refs = @(
        'official-hta-runtime#loadSchedule:L3297-L3347',
        'official-hta-runtime#addSchedule:L3349-L3385',
        'official-hta-runtime#deleteSchedule:L3387-L3439'
    )
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-TextFile (Join-Path $fixtureDir 'validation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm1601c:' `
        -NamePattern '(?i)valid|check|save|date|submit|final') -join [Environment]::NewLine
)
Write-TextFile (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') (
    (& $functionTool `
        -HtaPath $htaPath `
        -ControlPrefix 'frm1601c:' `
        -NamePattern '(?i)comput|total|tax|penalt|amount|schedule|format') -join [Environment]::NewLine
)

$rules = [Collections.Generic.List[object]]::new()
function Add-Rule(
    [string]$Id,
    [string]$Phase,
    [int]$Order,
    [string]$Condition,
    [string[]]$FieldKeys,
    $Message,
    [string[]]$Refs,
    [string]$Assessment = 'verified-correct',
    [string]$Official = 'The branch alerts and stops the active operation.',
    [string]$Recommended = 'Retain as a structured revision-aware error.',
    [string]$Confidence = 'high'
) {
    $rule = [pscustomobject][ordered]@{
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
        evidence_type = @('source')
        assessment = $Assessment
        official_behavior = $Official
        recommended_app_behavior = $Recommended
        confidence = $Confidence
        unresolved_questions = @()
    }
    [void]$rules.Add($rule)
}

Add-Rule '1601c-input-001-january-default-bug' input 1 `
    'The form initializes during January.' @('frm1601c:txtMonth', 'frm1601c:txtYear') $null `
    @('official-hta-runtime#init:L2767-L2778') 'incorrect-official-behavior' `
    'The source compares a Date object to zero, so the intended December/previous-year branch is unreachable.' `
    'Default to the most recently completed month and year.'
Add-Rule '1601c-input-002-month-year-format' 'blur/change' 2 `
    'A nonblank Schedule I month/year is not exactly MM/YYYY with numeric month 01..12 and four-digit year.' `
    @('frm1601c:sched1:txtMonthYear{N>=0}') 'Please provide a valid date. (MM/YYYY format)' `
    @('official-hta-runtime#validateMonthYear:L2810-L2845')
Add-Rule '1601c-input-003-month-year-future' 'blur/change' 3 `
    'A valid Schedule I MM/YYYY value is after the current date.' `
    @('frm1601c:sched1:txtMonthYear{N>=0}') 'This date cannot be a future date.' `
    @('official-hta-runtime#validateMonthYear:L2846-L2850')
Add-Rule '1601c-input-004-date-format' 'blur/change' 4 `
    'A nonblank Schedule I payment date is not a real MM/DD/YYYY date.' `
    @('frm1601c:sched1:txtDatePaid{N>=0}') 'Please provide a valid date. (MM/DD/YYYY format)' `
    @('official-hta-runtime#validateDate:L2855-L2912')
Add-Rule '1601c-input-005-date-future' 'blur/change' 5 `
    'A valid Schedule I payment date is after the current date.' `
    @('frm1601c:sched1:txtDatePaid{N>=0}') 'This date cannot be a future date.' `
    @('official-hta-runtime#validateDate:L2913-L2917')
Add-Rule '1601c-input-006-return-period-floor' 'blur/change' 6 `
    'Return-period year is before 2018.' @('frm1601c:txtYear') `
    'Please file using the old version of the form.' @('official-hta-runtime#validateRtnPeriod:L2922-L2929')
Add-Rule '1601c-input-007-return-period-month-future' 'blur/change' 7 `
    'Return month is later than current month in current year.' `
    @('frm1601c:txtMonth', 'frm1601c:txtYear') `
    'Invalid month. Month should not be later than the current month.' `
    @('official-hta-runtime#validateRtnPeriod:L2930-L2934')
Add-Rule '1601c-input-008-return-period-year-future' 'blur/change' 8 `
    'Return year exceeds current year.' @('frm1601c:txtYear') `
    'Invalid year. Year should not be later than the current year.' `
    @('official-hta-runtime#validateRtnPeriod:L2935-L2939')
Add-Rule '1601c-input-009-special-tax-yes' 'blur/change' 9 `
    'Special tax rate Yes is selected.' @('frm1601c:SpecialTax_1', 'frm1601c:selTreaty') $null `
    @('official-hta-runtime#enableSelTreaty:L3660-L3664') 'verified-correct' `
    'The treaty select is enabled and reset to its first option.' `
    'Preserve the conditional treaty selection.'
Add-Rule '1601c-input-010-special-tax-no' 'blur/change' 10 `
    'Special tax rate No is selected.' @('frm1601c:SpecialTax_2', 'frm1601c:selTreaty') $null `
    @('official-hta-runtime#disableSelTreaty:L3666-L3670') 'verified-correct' `
    'The treaty select is disabled and reset to its first option.' `
    'Preserve the reset and conditional disablement.'
Add-Rule '1601c-input-011-amended-item28' 'blur/change' 11 `
    'Amended Return changes.' @('frm1601c:AmendedRtn_1', 'frm1601c:AmendedRtn_2', 'frm1601c:txtTax28') $null `
    @('official-hta-runtime#changeAmended:L3648-L3656') 'verified-correct' `
    'Yes enables Item 28; otherwise Item 28 is disabled, reset to 0.00, and totals recompute.' `
    'Preserve the branch and recomputation.'
Add-Rule '1601c-input-012-withheld-no-reset' 'blur/change' 12 `
    'Taxes withheld/remitted changes to No while tax inputs or Schedule I adjustments are nonzero.' `
    @('frm1601c:TaxWithheld_1', 'frm1601c:TaxWithheld_2', 'tax-computation-fields', 'schedule-i-fields') `
    "You are about to change the value to 'No'. Doing this will clear all computation field. Do you wish to proceed? " `
    @('official-hta-runtime#cancelAllCompute:L3573-L3634') 'verified-correct' `
    'Confirmation clears tax inputs and Schedule I values, then recomputes; cancellation restores Yes.' `
    'Make destructive reset explicit and transactional.'

Add-Rule '1601c-save-013-month' save 13 `
    'Return month is blank.' @('frm1601c:txtMonth') 'Please enter Month on Item 1.' `
    @('official-hta-runtime#initialValidateBeforeSave:L3672-L3678')
Add-Rule '1601c-save-014-year' save 14 `
    'Return year is blank.' @('frm1601c:txtYear') 'Please enter Year on Item 1.' `
    @('official-hta-runtime#initialValidateBeforeSave:L3679-L3682')
Add-Rule '1601c-save-015-tin' save 15 `
    'Any TIN segment or branch code is blank.' @('TIN-fields') `
    'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L3683-L3687')
Add-Rule '1601c-save-016-rdo' save 16 `
    'RDO value is 000.' @('frm1601c:txtRDOCode') `
    'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L3688-L3691')
Add-Rule '1601c-save-017-name' save 17 `
    'Withholding-agent name is blank.' @('frm1601c:txtTaxpayerName') `
    "Please enter a valid Withholding Agent's Name on Item 8." `
    @('official-hta-runtime#initialValidateBeforeSave:L3692-L3696')
Add-Rule '1601c-save-018-sparse' save 18 `
    'Any field outside period/TIN/RDO/name is invalid.' @('all-other-form-fields') $null `
    @('official-hta-runtime#initialValidateBeforeSave:L3672-L3698') 'incorrect-official-behavior' `
    'Save ignores all other header, tax, schedule, and workflow rules.' `
    'Allow incomplete drafts explicitly, but never equate Save checks with validity.'
Add-Rule '1601c-save-019-amended-version' save 19 `
    'A finalized/versioned return exists and Amended Return is not Yes.' `
    @('frm1601c:AmendedRtn_1', 'frm1601c:AmendedRtn_2') `
    "If your intention is to make an amended return, kindly tick Amended Return to 'Yes' before hitting 'Save' or 'Final Copy' or 'Submit'." `
    @('official-hta-runtime#saveXML:L2201-L2493')

Add-Rule '1601c-validate-020-year-required' validate 20 `
    'Return year is blank.' @('frm1601c:txtYear') `
    'Please enter a valid year on Item 1.' @('official-hta-runtime#validate:L2942-L2948')
Add-Rule '1601c-validate-021-month-required' validate 21 `
    'Return month is blank.' @('frm1601c:txtMonth') `
    'Please enter a valid month on Item 1.' @('official-hta-runtime#validate:L2949-L2952')
Add-Rule '1601c-validate-022-year-future' validate 22 `
    'Return year exceeds current year.' @('frm1601c:txtYear') `
    'Invalid year. Year should not be later than the current year.' `
    @('official-hta-runtime#validate:L2963-L2969')
Add-Rule '1601c-validate-023-month-future' validate 23 `
    'Return month is later than current month in current year.' `
    @('frm1601c:txtMonth', 'frm1601c:txtYear') `
    'Invalid month. Month should not be later than the current month.' `
    @('official-hta-runtime#validate:L2970-L2976')
Add-Rule '1601c-validate-024-withheld-choice' validate 24 `
    'Neither taxes-withheld Yes nor No is selected.' `
    @('frm1601c:TaxWithheld_1', 'frm1601c:TaxWithheld_2') `
    'Please select an option for Item 3.' @('official-hta-runtime#validate:L2977-L2981')
Add-Rule '1601c-validate-025-tin' validate 25 `
    'Any TIN segment or branch code is blank.' @('TIN-fields') `
    'Please enter a valid TIN number on Item 6.' @('official-hta-runtime#validate:L2983-L2987')
Add-Rule '1601c-validate-026-tin-checksum-omitted' validate 26 `
    'TIN segments are nonblank but fail shared checksum/branch semantics.' @('TIN-fields') $null `
    @('official-hta-runtime#validate:L2983-L2987') 'incorrect-official-behavior' `
    'The source tests presence only.' 'Apply the shared evidence-backed TIN validation.'
Add-Rule '1601c-validate-027-rdo' validate 27 `
    'RDO selectedIndex is zero.' @('frm1601c:txtRDOCode') `
    'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#validate:L2988-L2993')
Add-Rule '1601c-validate-028-name-typo' validate 28 `
    'Withholding-agent name is blank.' @('frm1601c:txtTaxpayerName') `
    "Please enter a valid Witholding Agent's Name on Item 8." `
    @('official-hta-runtime#validate:L2999-L3003') 'official-bug-compatible' `
    'The exact message misspells Withholding as Witholding.' `
    'Use correctly spelled UI text while retaining the official message for diagnostics.'
Add-Rule '1601c-validate-029-telephone' validate 29 `
    'Telephone number is blank.' @('frm1601c:txtTelNum') `
    'Please enter a valid Telephone Number on Item 10.' @('official-hta-runtime#validate:L3004-L3008')
Add-Rule '1601c-validate-030-address' validate 30 `
    'Primary registered-address line is blank.' @('frm1601c:txtAddress') `
    "Please enter Taxpayer's Registered Address on Item 9." @('official-hta-runtime#validate:L3009-L3013')
Add-Rule '1601c-validate-031-zip' validate 31 `
    'ZIP code is blank.' @('frm1601c:txtZipCode') `
    "Please enter Taxpayer's Zip Code on Item 9A." @('official-hta-runtime#validate:L3014-L3018')
Add-Rule '1601c-validate-032-category' validate 32 `
    'Neither private nor government category is selected.' `
    @('frm1601c:CatAgent_P', 'frm1601c:CatAgent_G') `
    'Please select an option for Item 11.' @('official-hta-runtime#validate:L3019-L3022')
Add-Rule '1601c-validate-033-item14-positive' validate 33 `
    'Taxes withheld Yes is selected and Item 14 equals zero.' `
    @('frm1601c:TaxWithheld_1', 'frm1601c:txtTax14') `
    'Invalid amount in Item no.14. Value must be greater than zero(0).' `
    @('official-hta-runtime#validate:L3023-L3028')
Add-Rule '1601c-validate-034-item25-positive' validate 34 `
    'Taxes withheld Yes is selected and Item 25 equals zero.' `
    @('frm1601c:TaxWithheld_1', 'frm1601c:txtTax25') `
    'Invalid amount in Item no.25. Value must be greater than zero(0).' `
    @('official-hta-runtime#validate:L3029-L3033')
Add-Rule '1601c-validate-035-schedule-month-shape' validate 35 `
    'A nonblank Schedule I month/year does not contain exactly two slash-delimited parts.' `
    @('frm1601c:sched1:txtMonthYear{N>=0}') `
    'Invalid date entry on Section A, column 1 Record {N}.Format is mm/yyyy' `
    @('official-hta-runtime#validate:L3034-L3070')
Add-Rule '1601c-validate-036-schedule-month-numeric' validate 36 `
    'Schedule I month part is nonnumeric.' @('frm1601c:sched1:txtMonthYear{N>=0}') `
    'Invalid date entry on Section A, column 1 Record {N}.Format is mm/yyyy' `
    @('official-hta-runtime#validate:L3040-L3046')
Add-Rule '1601c-validate-037-schedule-month-range-zero-bug' validate 37 `
    'Schedule I month is above 12 or below 0.' @('frm1601c:sched1:txtMonthYear{N>=0}') `
    'Invalid date entry on Section A, column 1 Record {N}.Format is mm/yyyy' `
    @('official-hta-runtime#validate:L3047-L3052') 'incorrect-official-behavior' `
    'The comparison is month < 0, so month 00 passes Validate.' `
    'Require month 01 through 12.'
Add-Rule '1601c-validate-038-schedule-year-numeric' validate 38 `
    'Schedule I year part is nonnumeric.' @('frm1601c:sched1:txtMonthYear{N>=0}') `
    'Invalid date entry on Section A, column 1 Record {N}.Format is mm/yyyy' `
    @('official-hta-runtime#validate:L3054-L3058')
Add-Rule '1601c-validate-039-schedule-year-range-omitted' validate 39 `
    'Schedule I year is numeric but implausible or in the future.' `
    @('frm1601c:sched1:txtMonthYear{N>=0}') $null `
    @('official-hta-runtime#validate:L3059-L3065') 'incorrect-official-behavior' `
    'The year-range/future check is commented out.' `
    'Apply the revision-supported period range and reject future dates.'
Add-Rule '1601c-validate-040-schedule-date-shape' validate 40 `
    'A nonblank Schedule I payment date does not contain exactly three slash-delimited parts.' `
    @('frm1601c:sched1:txtDatePaid{N>=0}') `
    'Invalid date entry on Section A, column 2 Record {N}.Format is mm/dd/yyyy.' `
    @('official-hta-runtime#validate:L3072-L3109')
Add-Rule '1601c-validate-041-schedule-date-month-numeric' validate 41 `
    'Schedule I payment-date month is nonnumeric.' @('frm1601c:sched1:txtDatePaid{N>=0}') `
    'Invalid date entry on Section A, column 2 Record {N}.Format is mm/dd/yyyy.' `
    @('official-hta-runtime#validate:L3074-L3080')
Add-Rule '1601c-validate-042-schedule-date-month-zero-bug' validate 42 `
    'Schedule I payment-date month is above 12 or below 0.' `
    @('frm1601c:sched1:txtDatePaid{N>=0}') `
    'Invalid date entry on Section A, column 2 Record {N}.Format is mm/dd/yyyy.' `
    @('official-hta-runtime#validate:L3081-L3086') 'incorrect-official-behavior' `
    'The comparison is month < 0, so month 00 passes Validate.' 'Require month 01 through 12.'
Add-Rule '1601c-validate-043-schedule-date-day-numeric' validate 43 `
    'Schedule I payment-date day is nonnumeric.' @('frm1601c:sched1:txtDatePaid{N>=0}') `
    'Invalid date entry on Section A, column 2 Record {N}.Format is mm/dd/yyyy.' `
    @('official-hta-runtime#validate:L3088-L3092')
Add-Rule '1601c-validate-044-schedule-date-day-range' validate 44 `
    'Schedule I payment-date day is outside 1..31.' @('frm1601c:sched1:txtDatePaid{N>=0}') `
    'Invalid date entry on Section A, column 2 Record {N}.Format is mm/dd/yyyy.' `
    @('official-hta-runtime#validate:L3093-L3098')
Add-Rule '1601c-validate-045-schedule-date-year-numeric' validate 45 `
    'Schedule I payment-date year is nonnumeric.' @('frm1601c:sched1:txtDatePaid{N>=0}') `
    'Invalid date entry on Section A, column 2 Record {N}.Format is mm/dd/yyyy.' `
    @('official-hta-runtime#validate:L3100-L3104')
Add-Rule '1601c-validate-046-schedule-calendar-omitted' validate 46 `
    'Schedule I payment date has valid numeric components but is not a real calendar date.' `
    @('frm1601c:sched1:txtDatePaid{N>=0}') $null `
    @('official-hta-runtime#validate:L3072-L3110', 'official-hta-runtime#validateDate:L2855-L2920') `
    'incorrect-official-behavior' `
    'Validate accepts dates such as 02/31/yyyy if blur validation was bypassed.' `
    'Use one real-calendar-date validator in every phase.'
Add-Rule '1601c-validate-047-schedule-partial-row' validate 47 `
    'A Schedule I row has bank/number/amount data but blank month/year or date paid.' `
    @('schedule-i-fields') $null @('official-hta-runtime#validate:L3034-L3111') `
    'incorrect-official-behavior' `
    'The source validates each date only when that date field is nonblank and never requires row completeness.' `
    'When any row value is supplied, require the revision-supported row columns.'
Add-Rule '1601c-validate-048-amended-choice-omitted' validate 48 `
    'Neither Amended Return Yes nor No is selected.' `
    @('frm1601c:AmendedRtn_1', 'frm1601c:AmendedRtn_2') $null `
    @('official-hta-runtime#validate:L2942-L3142') 'incorrect-official-behavior' `
    'Validate never inspects the amended-return choice.' 'Require an explicit Yes/No state.'
Add-Rule '1601c-validate-049-special-tax-omitted' validate 49 `
    'Neither Special Tax Rate Yes nor No is selected, or Yes has no treaty selection.' `
    @('frm1601c:SpecialTax_1', 'frm1601c:SpecialTax_2', 'frm1601c:selTreaty') $null `
    @('official-hta-runtime#validate:L2942-L3142', 'official-hta-runtime#enableSelTreaty:L3660-L3670') `
    'incorrect-official-behavior' `
    'Validate does not inspect the choice or treaty.' `
    'Require an explicit choice and treaty only when revision-matched instructions require it.'
Add-Rule '1601c-validate-050-email-omitted' validate 50 `
    'Email is blank or malformed.' @('txtEmail') $null @('official-hta-runtime#validate:L2942-L3142') `
    'incorrect-official-behavior' 'Validate never inspects email.' `
    'Apply the shared evidence-backed email rule when the filing workflow requires email.'
Add-Rule '1601c-validate-051-address2-save-loss' validate 51 `
    'Second registered-address line exists in the final copy.' @('frm1601c:txtAddress2') $null `
    @('xml-encrypted', 'xml-plaintext', 'official-hta-runtime#saveXML:L2201-L2493') `
    'official-bug-compatible' `
    'The encrypted final copy retains the field, while the paired plaintext save omits it.' `
    'Preserve the field losslessly across every save/final-copy transition.'
Add-Rule '1601c-validate-052-help-application-name' validate 52 `
    'Packaged help is opened or identified by HTA metadata.' @('packaged-help') $null `
    @('packaged-help#APPLICATIONNAME:L7', 'packaged-help#revision-heading:L84-L85') `
    'official-bug-compatible' `
    'The 1601C January 2018 help content declares APPLICATIONNAME 0605.' `
    'Bind help by the pinned file and revision heading, not its defective APPLICATIONNAME.'
Add-Rule '1601c-validate-053-negative-derived-values' validate 53 `
    'Items 21 or 23 exceed their minuends, or remittances exceed withholding, producing negative derived values.' `
    @('frm1601c:txtTax22', 'frm1601c:txtTax24', 'frm1601c:txtTax31', 'frm1601c:txtTax36') $null `
    @('official-hta-runtime#computeTxt22:L3512-L3516', 'official-hta-runtime#computeTxt24:L3518-L3521', 'official-hta-runtime#computeTxt31:L3535-L3539') `
    'ambiguous' `
    'The source computes signed results and Validate imposes no cross-field bounds.' `
    'Preserve over-remittance where legal; reject impossible negative compensation values.'
Add-Rule '1601c-validate-054-success' validate 54 `
    'All active Validate branches pass.' @('frm1601c:btnValidate', 'frm1601c:btnEdit') `
    'Validation successful. Click on Edit if you wish to modify your entries.' `
    @('official-hta-runtime#validate:L3138-L3141') 'verified-correct' `
    'Validate disables controls and enables Edit, Print, Final Copy, and upload.' `
    'Tie validated state to the exact field snapshot.'

Write-JsonFile (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema' = '../../schema/validations.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    revision = $revision
    first_error_behavior = 'Save and Validate alert the first matching active branch and return; blur validators clear invalid dates.'
    rules = $rules
})

$calculations = [Collections.Generic.List[object]]::new()
function Add-Calculation(
    [string]$Id,
    [string[]]$Outputs,
    [string[]]$Inputs,
    [string]$Formula,
    [string]$Trigger,
    [string[]]$Dependencies,
    [string[]]$Refs,
    [string]$Assessment = 'verified-correct'
) {
    $calculation = [pscustomobject][ordered]@{
        calculation_id = $Id
        outputs = $Outputs
        inputs = $Inputs
        condition = $null
        official_formula = $Formula
        rounding = 'NumWithComma inputs, arithmetic, toFixed(2), then formatCurrency.'
        trigger = $Trigger
        depends_on = $Dependencies
        source_refs = $Refs
        assessment = $Assessment
        recommended_app_behavior = 'Use typed decimals and preserve the explicit source dependency order.'
        confidence = 'high'
    }
    [void]$calculations.Add($calculation)
}
Add-Calculation '1601c-item21-nontaxable-total' `
    @('frm1601c:txtTax21') `
    @('frm1601c:txtTax15', 'frm1601c:txtTax16', 'frm1601c:txtTax17', 'frm1601c:txtTax18', 'frm1601c:txtTax19', 'frm1601c:txtTax20') `
    '21 = 15 + 16 + 17 + 18 + 19 + 20.' 'computeTxt21' @() `
    @('official-hta-runtime#computeTxt21:L3505-L3510')
Add-Calculation '1601c-item22-taxable-compensation' `
    @('frm1601c:txtTax22') @('frm1601c:txtTax14', 'frm1601c:txtTax21') `
    '22 = 14 - 21.' 'computeTxt22' @('1601c-item21-nontaxable-total') `
    @('official-hta-runtime#computeTxt22:L3512-L3516')
Add-Calculation '1601c-item24-net-taxable-compensation' `
    @('frm1601c:txtTax24') @('frm1601c:txtTax22', 'frm1601c:txtTax23') `
    '24 = 22 - 23.' 'computeTxt24' @('1601c-item22-taxable-compensation') `
    @('official-hta-runtime#computeTxt24:L3518-L3521')
Add-Calculation '1601c-schedule-row-adjustment' `
    @('frm1601c:sched1:txtAdjustments{N>=0}') `
    @('frm1601c:sched1:txtShouldTaxDue{N>=0}', 'frm1601c:sched1:txtTaxPaid{N>=0}') `
    'Adjustment[N] = tax that should have been paid[N] - tax paid[N].' `
    'computeSchedule1' @() @('official-hta-runtime#computeSchedule1:L3549-L3554')
Add-Calculation '1601c-schedule-total' `
    @('frm1601c:sched1:txtTotal1') @('frm1601c:sched1:txtAdjustments{N>=0}') `
    'Schedule total = sum of every existing row adjustment.' `
    'computeSchedule1' @('1601c-schedule-row-adjustment') `
    @('official-hta-runtime#computeSchedule1:L3556-L3560')
Add-Calculation '1601c-item26-schedule-transfer' `
    @('frm1601c:txtTax26') @('frm1601c:sched1:txtTotal1') `
    '26 = Schedule I total adjustment.' 'computeSchedule1' @('1601c-schedule-total') `
    @('official-hta-runtime#computeSchedule1:L3561-L3563')
Add-Calculation '1601c-item27-remittance' `
    @('frm1601c:txtTax27') @('frm1601c:txtTax25', 'frm1601c:txtTax26') `
    '27 = 25 + 26.' 'computeTxt27' @('1601c-item26-schedule-transfer') `
    @('official-hta-runtime#computeTxt27:L3523-L3527')
Add-Calculation '1601c-item30-remittances-made' `
    @('frm1601c:txtTax30') @('frm1601c:txtTax28', 'frm1601c:txtTax29') `
    '30 = 28 + 29.' 'computeTxt30' @() @('official-hta-runtime#computeTxt30:L3529-L3533')
Add-Calculation '1601c-item31-tax-still-due' `
    @('frm1601c:txtTax31') @('frm1601c:txtTax27', 'frm1601c:txtTax30') `
    '31 = 27 - 30.' 'computeTxt31' @('1601c-item27-remittance', '1601c-item30-remittances-made') `
    @('official-hta-runtime#computeTxt31:L3535-L3539')
Add-Calculation '1601c-item35-penalties' `
    @('frm1601c:txtTax35') @('frm1601c:txtTax32', 'frm1601c:txtTax33', 'frm1601c:txtTax34') `
    '35 = 32 + 33 + 34.' 'computePenalties' @() `
    @('official-hta-runtime#computePenalties:L3541-L3547')
Add-Calculation '1601c-item36-total-still-due' `
    @('frm1601c:txtTax36') @('frm1601c:txtTax31', 'frm1601c:txtTax35') `
    '36 = 31 + 35.' 'computeTaxAmountStillDue' `
    @('1601c-item31-tax-still-due', '1601c-item35-penalties') `
    @('official-hta-runtime#computeTaxAmountStillDue:L3566-L3571')

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
    $negativeCase = [pscustomobject][ordered]@{
        case_id = ('case-{0:d2}-{1}' -f $caseNumber, $rule.rule_id)
        phase = $rule.phase
        mutations = @{ synthetic_condition = $rule.condition }
        expected_message = $rule.exact_message
        expected_behavior = $rule.official_behavior
        rule_id = $rule.rule_id
    }
    [void]$negativeCases.Add($negativeCase)
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
            case_id = 'non-taxable-total'
            calculation_id = '1601c-item21-nontaxable-total'
            inputs = @(100, 200, 300, 400, 500, 600)
            official_output = 2100
        },
        @{
            case_id = 'schedule-adjustment-overpayment'
            calculation_id = '1601c-schedule-row-adjustment'
            should_tax_due = 100
            tax_paid = 125
            official_output = -25
        },
        @{
            case_id = 'three-row-schedule-total'
            calculation_id = '1601c-schedule-total'
            adjustments = @(10, -5, 20)
            official_output = 25
        },
        @{
            case_id = 'total-still-due'
            calculation_id = '1601c-item36-total-still-due'
            item31 = 1000
            item35 = 60
            official_output = 1060
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
            official_behavior = 'January 2018 two-page monthly remittance return for income taxes withheld on compensation, with source-unbounded Schedule I rows.'
            source_refs = @('official-hta-runtime', 'official-form-pdf', 'packaged-help')
            confidence = 'high'
        },
        @{
            phase = 'saved-draft'
            official_behavior = 'Save checks only period, TIN, RDO, and withholding-agent name before serializing the current dynamic DOM.'
            source_refs = @('official-hta-runtime#initialValidateBeforeSave:L3672-L3698')
            confidence = 'high'
        },
        @{
            phase = 'validated'
            official_behavior = 'Validate runs ordered header, conditional tax amount, and partial Schedule I date checks.'
            source_refs = @('official-hta-runtime#validate:L2942-L3142')
            confidence = 'high'
        },
        @{
            phase = 'final-copy'
            official_behavior = 'Final Copy writes an encrypted/compressed copy; the paired sample has 101 concrete keys and three Schedule I rows.'
            source_refs = @('official-hta-runtime#saveEncryptedProfile:L2031-L2122', 'xml-encrypted')
            confidence = 'high'
        },
        @{
            phase = 'submitted'
            official_behavior = 'Online transport code exists but was not exercised.'
            source_refs = @('official-hta-runtime#sendEmail')
            confidence = 'medium'
        }
    )
    transitions = @(
        @{
            from = 'edit'
            action = 'Add Schedule I row'
            to = 'edit'
            guard = $null
            side_effects = @('Appends one unbounded indexed row and rebuilds both Schedule I tables.')
            source_refs = @('official-hta-runtime#addSchedule:L3349-L3385')
        },
        @{
            from = 'edit'
            action = 'Delete selected Schedule I rows'
            to = 'edit'
            guard = $null
            side_effects = @('Compacts surviving rows to zero-based indices and recomputes adjustments.')
            source_refs = @('official-hta-runtime#deleteSchedule:L3387-L3439')
        },
        @{
            from = 'edit'
            action = 'Save'
            to = 'saved-draft'
            guard = 'Sparse Save checks pass.'
            side_effects = @('Writes flat pseudo-XML for the current dynamic DOM.')
            source_refs = @('official-hta-runtime#saveXML:L2201-L2493')
        },
        @{
            from = 'edit'
            action = 'Validate'
            to = 'validated'
            guard = 'All ordered active validation branches pass.'
            side_effects = @('Disables controls.', 'Enables Edit, Print, Final Copy, and upload.')
            source_refs = @('official-hta-runtime#validate:L2942-L3142', 'official-hta-runtime#disableAllControl:L3228-L3295')
        },
        @{
            from = 'validated'
            action = 'Edit'
            to = 'edit'
            guard = $null
            side_effects = @('Re-enables editable controls according to amended/special-tax branches.')
            source_refs = @('official-hta-runtime#enableAllControl:L3144-L3227')
        },
        @{
            from = 'validated'
            action = 'Final Copy'
            to = 'final-copy'
            guard = 'Final-copy save succeeds.'
            side_effects = @('Writes encrypted/compressed copy.')
            source_refs = @('official-hta-runtime#saveEncryptedProfile:L2031-L2122', 'xml-encrypted')
        },
        @{
            from = 'final-copy'
            action = 'Online transport'
            to = 'submitted'
            guard = 'Connectivity and remote acceptance succeed.'
            side_effects = @('Untested online attempt.')
            source_refs = @('official-hta-runtime#sendEmail')
        }
    )
    prerequisites = @(
        'Return period',
        'Taxes-withheld choice',
        'TIN/RDO and identity',
        'Category/contact/address',
        'Conditional Items 14 and 25',
        'Valid populated Schedule I dates'
    )
    required_attachments = @()
    filing_deadlines = @(
        @{
            quarter = 'Q1'
            due_date_rule = 'Monthly filing deadline is not computed by this runtime; use revision-matched official guidance.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q2'
            due_date_rule = 'Monthly filing deadline is not computed by this runtime; use revision-matched official guidance.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q3'
            due_date_rule = 'Monthly filing deadline is not computed by this runtime; use revision-matched official guidance.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        },
        @{
            quarter = 'Q4'
            due_date_rule = 'Monthly filing deadline is not computed by this runtime; use revision-matched official guidance.'
            source_refs = @('packaged-help')
            confidence = 'medium'
        }
    )
}
Write-JsonFile (Join-Path $outDir 'workflow.json') $workflow

$bugCount = @(
    $rules | Where-Object {
        $_.assessment -in @(
            'official-bug-compatible',
            'incorrect-official-behavior',
            'obsolete'
        )
    }
).Count
$assets = @(
    New-Asset 'package-7.9.6' 'official-package-executable' $packagePath `
        'Installed Offline eBIRForms package 7.9.6.0.'
    New-Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath `
        'January 2018 ENCS runtime; selected instead of the co-installed legacy 1601C runtime.'
    New-Asset 'packaged-help' 'official-runtime-help' $helpPath `
        'Packaged January 2018 instructions; content is revision-bound despite defective APPLICATIONNAME 0605.'
    New-Asset 'official-form-pdf' 'official-form-pdf' $pdfPath `
        'January 2018 ENCS official form with DPA.'
    New-Asset 'xml-encrypted' 'dummy-profile-encrypted-final-copy' `
        $sampleByHash[$expected.cipher].FullName `
        'Revision-matched 101-key dummy final copy; values excluded.' `
        (Join-Path $OfficialDir '1601C-final-copy-#email-redacted#.xml')
    New-Asset 'xml-plaintext' 'dummy-profile-plaintext-save' `
        $sampleByHash[$expected.plain].FullName `
        'Revision-matched 100-key dummy save; values excluded.' `
        (Join-Path $OfficialDir '1601C-save-#email-redacted#.xml')
)
$manifest = [ordered]@{
    '$schema' = '../../schema/form-manifest.schema.json'
    schema_version = '1.0.0'
    form_id = $formId
    form_code = '1601C'
    revision = $revision
    package_version = $packageVersion
    status = 'complete'
    official_assets = $assets
    counts = [ordered]@{
        concrete_fields = $keys.Count
        runtime_field_families = $familyDefinitions.Count
        fields_total = $fields.Count
        typed_fields = $fields.Count
        validation_rules = $rules.Count
        confirmed_official_bugs = $bugCount
        calculations = $calculations.Count
        negative_fixtures = $negativeCases.Count
        unverified_gaps = 1
    }
    artifacts = [ordered]@{
        fields = 'fields.json'
        validations = 'validations.json'
        calculations = 'calculations.json'
        workflow = 'workflow.json'
        evidence = 'evidence.md'
        audit = 'audit.md'
        gaps = 'gaps.md'
        encrypted_field_audit = 'fixtures/encrypted-field-audit-v796.json'
        plaintext_field_audit = 'fixtures/plaintext-field-audit-v796.json'
        runtime_controls = 'fixtures/runtime-control-inventory-v796.json'
        schedule_families = 'fixtures/schedule-family-inventory-v796.json'
        validation_functions = 'fixtures/validation-function-inventory-v796.json'
        calculation_functions = 'fixtures/calculation-function-inventory-v796.json'
        negative_fixtures = 'fixtures/negative-cases.json'
        calculation_fixtures = 'fixtures/calculation-boundaries.json'
    }
    scope_notes = @(
        'Research only; no renderer or release metadata changed.',
        'No decrypted values or email-bearing filenames emitted.',
        'The 101-key final-copy inventory is lossless; the paired plaintext save omits txtAddress2.',
        'Eight source-unbounded Schedule I families preserve rows beyond observed indices 0-2.',
        'The co-installed legacy 1601C runtime is excluded from the January 2018 binding.'
    )
}
Write-JsonFile (Join-Path $outDir 'manifest.json') $manifest
Write-TextFile (Join-Path $outDir 'README.md') @"
# BIR Form 1601C - January 2018 (ENCS)

Revision-specific Offline eBIRForms rules with 101 observed concrete serialized
keys and eight source-unbounded Schedule I field families.
"@
Write-TextFile (Join-Path $outDir 'evidence.md') @"
# Evidence

- January 2018 runtime: $($expected.hta); help: $($expected.help); PDF: $($expected.pdf).
- Encrypted final copy: 101 unique keys, inventory $($expected.encrypted_inventory); values excluded.
- Plaintext save: 100 unique keys, inventory $($expected.plain_inventory); values excluded.
- The only paired-sample difference is ``frm1601c:txtAddress2``.
- Key accounting: 100 static controls + 1 runtime RDO; zero unexplained sample keys.
- Runtime DOM: 136 live static controls.
- Schedule I proves concrete indices 0-2 and eight source-unbounded zero-based families.
- Co-installed legacy ``BIR-Form1601C.hta`` is not merged into this revision.
- Packaged help has the correct January 2018 heading but a defective ``APPLICATIONNAME="0605"``.
- All email-bearing filenames use ``#email-redacted#``.
"@
Write-TextFile (Join-Path $outDir 'gaps.md') @"
# Gaps

1. Online submission was not exercised.
"@
Write-TextFile (Join-Path $outDir 'audit.md') @"
# Audit

- January 2018 ENCS binding: pass; legacy runtime excluded.
- Lossless final-copy inventory: 101 keys; plaintext Save omission documented.
- Typed inventory: 101 concrete keys + 8 source-unbounded families; zero unexplained sample keys.
- Validations $($rules.Count); calculations $($calculations.Count); negatives $($negativeCases.Count); defects $bugCount.
- Focused and full strict audits must run.
- No renderer/release/capability/commit/push changes.
"@

$indexPath = Join-Path $RepoRoot 'rules\index.json'
$index = Get-Content -Raw -LiteralPath $indexPath | ConvertFrom-Json
$entry = $index.forms | Where-Object { $_.form_id -eq $formId }
if ($entry) {
    $entry.form_code = '1601C'
    $entry.revision = $revision
    $entry.package_version = $packageVersion
    $entry.priority = 41
    $entry.status = 'complete'
    $entry.path = 'forms/1601c-v2018/manifest.json'
}
else {
    $index.forms += [pscustomobject][ordered]@{
        form_id = $formId
        form_code = '1601C'
        revision = $revision
        package_version = $packageVersion
        priority = 41
        status = 'complete'
        path = 'forms/1601c-v2018/manifest.json'
    }
}
$index.forms = @($index.forms | Sort-Object priority)
$index.updated = '2026-07-23'
Write-JsonFile $indexPath $index

$actual = [ordered]@{
    live_controls = $controls.Count
    encrypted_keys = $keys.Count
    plaintext_keys = $plainKeys.Count
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    unexplained = $unexplained.Count
    concrete_fields = $keys.Count
    runtime_families = $familyDefinitions.Count
    fields = $fields.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negatives = $negativeCases.Count
    bugs = $bugCount
}
$expectedCounts = [ordered]@{
    live_controls = 136
    encrypted_keys = 101
    plaintext_keys = 100
    static_matches = 100
    runtime_rdo = 1
    unexplained = 0
    concrete_fields = 101
    runtime_families = 8
    fields = 109
    validations = 54
    calculations = 11
    negatives = 39
    bugs = 14
}
foreach ($name in $expectedCounts.Keys) {
    if ($actual[$name] -ne $expectedCounts[$name]) {
        throw "1601C fail-closed count changed: $name expected $($expectedCounts[$name]), found $($actual[$name])."
    }
}

[pscustomobject][ordered]@{
    form_id = $formId
    live_controls = $controls.Count
    encrypted_keys = $keys.Count
    plaintext_keys = $plainKeys.Count
    static_matches = $staticMatches.Count
    runtime_rdo = $runtimeRdo.Count
    unexplained = $unexplained.Count
    concrete_fields = $keys.Count
    runtime_families = $familyDefinitions.Count
    typed_fields = $fields.Count
    validations = $rules.Count
    calculations = $calculations.Count
    negative_fixtures = $negativeCases.Count
    confirmed_official_bugs = $bugCount
    next_form = '2550Q'
} | ConvertTo-Json
