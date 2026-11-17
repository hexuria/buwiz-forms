param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\1801v2018',
    [string]$LegacySaveDir = 'C:\eBIRForms\savefile',
    [string]$LegacyFinalDir = 'C:\eBIRForms\IAF_RDO_Copy'
)

$ErrorActionPreference = 'Stop'
$formId = '1801-v2018'
$revision = '2018-01-01'
$packageVersion = '7.9.6.0'
$htaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1801v2018.hta'
$legacyHtaPath = Join-Path $ExtractedRoot 'forms\BIR-Form1801.hta'
$helpPath = Join-Path $ExtractedRoot 'helpfile\Help1801v2018.hta'
$legacyHelpPath = Join-Path $ExtractedRoot 'helpfile\Help1801.hta'
$pdfPath = Join-Path $PdfDir '1801 Jan 2018 ENCS.pdf'
$guidelinesPath = Join-Path $PdfDir '1801 Guidelines.pdf'
$legacySavePath = Join-Path $LegacySaveDir '00000000000000-1801-07222026.xml'
$packagePath = 'C:\eBIRForms\BIRForms.exe'
$outDir = Join-Path $RepoRoot 'rules\forms\1801-v2018'
$fixtureDir = Join-Path $outDir 'fixtures'

$expected = @{
    hta = 'd19c4ebe40ed109094ef43687ed5486a003bd11a67a251bdd5545a41c2f46d8d'
    legacy_hta = 'c102cf9b62a73dbe491e6e181e33384668d5fd8152e49b4005ee4563f4cb908b'
    help = '21fc91fffadb99b78f9495cca3c25e8ba14e76a4467a181adbd73ef5607c68f5'
    legacy_help = 'c0f36cb2700668dd1cba093afe02f614bdb561e4bb01932f2d21bca1a0c9fcb9'
    pdf = 'ec49207aab9b035d1913d41091b677d9df690e01b391ed2c2f4c34cf43a524c6'
    guidelines = '06cbd878536d2960ef556fbfb29a23e9a58896f1ae4e43623a6f389c916f7e0a'
    legacy_save = 'cd1ceb8cfb2e1daac21f0e948c25b0ba62e7d4cdf8a0e4a73710daaf96ac7001'
    legacy_final = '6ab3445921227b0537d9370bd08fdf92672fbada6edf781fcac86f74684ea603'
    legacy_inventory = 'c82bc9762711506ac134187ca823581528fd4c163efbe31ddabd73ee01dde9ef'
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
function Asset([string]$Id, [string]$Kind, [string]$Path, [string]$Binding, [string]$DisplayPath = '') {
    $item = Get-Item -LiteralPath $Path
    [pscustomobject][ordered]@{
        asset_id = $Id; kind = $Kind
        path = if ($DisplayPath) { $DisplayPath } else { $Path }
        sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
        size = $item.Length; revision_binding = $Binding
    }
}
function Find-FileByHash([string]$Directory, [string]$Hash) {
    $matches = @(Get-ChildItem -LiteralPath $Directory -File | Where-Object {
        (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -eq $Hash
    })
    if ($matches.Count -ne 1) { throw "Expected one file with SHA-256 $Hash; found $($matches.Count)." }
    $matches[0].FullName
}

foreach ($path in @($htaPath,$legacyHtaPath,$helpPath,$legacyHelpPath,$pdfPath,$guidelinesPath,$legacySavePath,$packagePath)) {
    if (-not (Test-Path -LiteralPath $path)) { throw "Missing source: $path" }
}
$legacyFinalPath = Find-FileByHash $LegacyFinalDir $expected.legacy_final
foreach ($pair in @(
    @($htaPath,'hta'),@($legacyHtaPath,'legacy_hta'),@($helpPath,'help'),@($legacyHelpPath,'legacy_help'),
    @($pdfPath,'pdf'),@($guidelinesPath,'guidelines'),@($legacySavePath,'legacy_save'),
    @($legacyFinalPath,'legacy_final'),@($packagePath,'package')
)) {
    $actual = (Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $expected[$pair[1]]) { throw "Hash changed: $($pair[0])" }
}
foreach ($pdf in @($pdfPath,$guidelinesPath)) {
    $bytes = [IO.File]::ReadAllBytes($pdf)
    if ([Text.Encoding]::ASCII.GetString($bytes[0..4]) -ne '%PDF-') { throw "PDF magic mismatch: $pdf" }
}

$hta = [IO.File]::ReadAllText($htaPath)
$help = [IO.File]::ReadAllText($helpPath)
if ($hta -notmatch '(?i)APPLICATIONNAME\s*=\s*["'']1801v2018["'']') { throw 'APPLICATIONNAME mismatch.' }
if ($hta -notmatch '(?i)January\s+2018\s+\(ENCS\)') { throw 'Printed revision label is absent.' }
if ($help -notmatch '(?i)BIR\s+Form\s+No\.\s*1801.*January\s+2018') { throw 'Revision-matched help binding changed.' }
if ($help -notmatch '(?i)six\s+percent\s+\(6%\)') { throw 'Revision-matched help tax rate changed.' }
if ($help -notmatch '(?i)within\s+one\s+\(1\)\s+year') { throw 'Revision-matched help filing deadline changed.' }

New-Item -ItemType Directory -Force -Path $fixtureDir | Out-Null

$legacyText = [IO.File]::ReadAllText($legacySavePath)
$legacyMatches = @([regex]::Matches($legacyText, '<div>(?<key>[^=<>]+)=(?<value>[^<]*?)\k<key>=</div>'))
$legacyKeys = @($legacyMatches | ForEach-Object { $_.Groups['key'].Value })
if ($legacyKeys.Count -ne 111 -or ($legacyKeys | Sort-Object -Unique).Count -ne 111) { throw 'Legacy save key count changed.' }
if ((Get-HashText @($legacyKeys | Sort-Object)) -ne $expected.legacy_inventory) { throw 'Legacy save inventory changed.' }
if (@($legacyKeys | Where-Object { $_ -like 'frm1801v2018:*' }).Count -gt 0 -or
    @($legacyKeys | Where-Object { $_ -like 'frm1801:*' }).Count -ne 104) {
    throw 'Legacy sample revision-prefix classification changed.'
}

$formMatch = [regex]::Match($hta, '(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if (-not $formMatch.Success) { throw 'frmMain missing.' }
$body = $formMatch.Groups['body'].Value
$bodyOffset = $formMatch.Groups['body'].Index
$excludedRanges = @(
    @([regex]::Matches($body, '(?is)<script\b.*?</script>'))
    @([regex]::Matches($body, '(?is)<!--.*?-->'))
)
$controls = [Collections.Generic.List[object]]::new()
$ordinal = 0
foreach ($match in [regex]::Matches($body, '(?is)<(input|select|textarea|button)\b[^>]*>')) {
    $excluded = $false
    foreach ($range in $excludedRanges) {
        if ($match.Index -ge $range.Index -and $match.Index -lt ($range.Index + $range.Length)) { $excluded = $true; break }
    }
    if ($excluded) { continue }
    $ordinal++
    $tag = $match.Value
    $element = $match.Groups[1].Value.ToLowerInvariant()
    $kind = if ($element -eq 'input') { Get-Attr $tag 'type' } else { $element }
    if (-not $kind) { $kind = 'text' }
    $controls.Add([pscustomobject][ordered]@{
        ordinal=$ordinal; id=Get-Attr $tag 'id'; name=Get-Attr $tag 'name'; element=$element
        control_kind=$kind.ToLowerInvariant()
        source_line=1+[regex]::Matches($hta.Substring(0,$bodyOffset+$match.Index),"`n").Count
        value=Get-Attr $tag 'value'; maxlength=Get-Attr $tag 'maxlength'
        disabled=$tag -match '(?i)\bdisabled(?:\s*=|\s|>)'; readonly=$tag -match '(?i)\breadonly(?:\s*=|\s|>)'
    })
}
if ($controls.Count -ne 157) { throw "Expected 157 live controls; found $($controls.Count)." }
$staticSerial = @($controls | Where-Object { $_.control_kind -in @('text','select','select-one','textarea','radio','checkbox') })
if ($staticSerial.Count -ne 137 -or @($staticSerial.id | Sort-Object -Unique).Count -ne 137) {
    throw "Expected 137 unique static serializer controls; found $($staticSerial.Count)."
}
$rdoControl = [pscustomobject][ordered]@{
    ordinal=158; id='frm1801v2018:txtRDOCode_1'; name='frm1801v2018:txtRDOCode_1'
    element='select'; control_kind='select-one'; source_line=3788; value='000'; maxlength=$null
    disabled=$false; readonly=$false
}
$concreteControls = @($staticSerial) + @($rdoControl)

$families = @(
    @{name='sched1Oct';kind='string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Td';kind='string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Loc';kind='string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Lot';kind='string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Area';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Class';kind='enum-string';control='runtime-indexed-select';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Fmv';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Zonal';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Exc';kind='decimal-amount';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1Conj';kind='decimal-amount';control='runtime-indexed-text';source='addRow_RealProp:L5135-L5177'},
    @{name='sched1AOct';kind='string';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1ATd';kind='string';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1ALoc';kind='string';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1AArea';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1AClass';kind='enum-string';control='runtime-indexed-select';source='addRow_Fam:L5195-L5226'},
    @{name='sched1AFmv';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1AZonal';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1AExc';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched1AConj';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Fam:L5195-L5226'},
    @{name='sched2Corp';kind='string';control='runtime-indexed-text';source='addRow_Stock:L5239-L5263'},
    @{name='sched2Class';kind='enum-string';control='runtime-indexed-select';source='addRow_Stock:L5239-L5263'},
    @{name='sched2Stock';kind='string';control='runtime-indexed-text';source='addRow_Stock:L5239-L5263'},
    @{name='sched2Shares';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_Stock:L5239-L5263'},
    @{name='sched2FmvPerShare';kind='decimal-or-string';control='runtime-indexed-text';source='addRow_Stock:L5239-L5263'},
    @{name='sched2Exc';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Stock:L5239-L5263'},
    @{name='sched2Conj';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Stock:L5239-L5263'},
    @{name='sched2AParticulars';kind='string';control='runtime-indexed-text';source='addRow_Other:L5276-L5286'},
    @{name='sched2AExc';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Other:L5276-L5286'},
    @{name='sched2AConj';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Other:L5276-L5286'},
    @{name='sched3Particulars';kind='string';control='runtime-indexed-text';source='addRow_Transfer:L5299-L5309'},
    @{name='sched3Exc';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Transfer:L5299-L5309'},
    @{name='sched3Conj';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Transfer:L5299-L5309'},
    @{name='sched4Name';kind='string';control='runtime-indexed-text';source='addRow_Business:L5322-L5340'},
    @{name='sched4Address';kind='string';control='runtime-indexed-text';source='addRow_Business:L5322-L5340'},
    @{name='sched4Rdo';kind='rdo-code';control='runtime-indexed-select';source='addRow_Business:L5322-L5340'},
    @{name='sched4Exc';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Business:L5322-L5340'},
    @{name='sched4Conj';kind='decimal-amount';control='runtime-indexed-text';source='addRow_Business:L5322-L5340'}
)
if ($families.Count -ne 37) { throw 'Dynamic family inventory changed.' }
$runtimeBaseline = $concreteControls.Count + (2 * $families.Count)
if ($concreteControls.Count -ne 138 -or $runtimeBaseline -ne 212) { throw 'Runtime baseline projection changed.' }

$requiredKeys = @(
    'frm1801v2018:txtDateMonth','frm1801v2018:txtDateDay','frm1801v2018:txtDateYear',
    'frm1801v2018:tinA_1','frm1801v2018:tinB_1','frm1801v2018:tinC_1','frm1801v2018:branchCode_1',
    'frm1801v2018:txtRDOCode_1','frm1801v2018:registeredName_1','frm1801v2018:registeredAddress_1',
    'frm1801v2018:categoryNonResident_1','frm1801v2018:categoryNonResident_2','frm1801v2018:telephoneNumber'
)
$itemMap = @{
    txtDateMonth='1';txtDateDay='1';txtDateYear='1';amendedRtn_1='2';amendedRtn_2='2';txtSheets='3';txtAtc='4'
    tinA_1='5';tinB_1='5';tinC_1='5';branchCode_1='5';txtRDOCode_1='6';registeredName_1='7'
    registeredAddress_1='8';categoryNonResident_1='9';categoryNonResident_2='9';adminName='10'
    txtTINE1='11';txtTINE2='11';txtTINE3='11';txtBranchCodeE='11';telephoneNumber='12';txtEmail='13'
    optTreaty_1='14';optTreaty_2='14';treatyY='14';fileGranted_1='15A';fileGranted_2='15A'
    instGranted_1='15B';instGranted_2='15B';settled_1='15C';settled_2='15C';extGranted_1='15D';extGranted_2='15D'
    freq_Mon='15D';freq_Qtr='15D';freq_Semi='15D';freq_Other='15D';freqOtherInput='15D'
    txtTaxableEstate='17';txtTaxRate='18';txtEstTaxDue='19A';txtCredits_ForeignEst='19B';txtCredits_TaxPrev='19B'
    txtCredits_Tot='19C';txtTaxPayable='20';txtInstallmentYear='21';txtInstallment='21';txtTaxPayable_1st='22'
    txtPen_Surcharge='23A';txtPen_Interest='23B';txtPen_Compromise='23C';txtPen_Tot='23D';txtTotPayable='24'
    cashBank='25';cashNum='25';cashDate='25';cashAmt='25';checkBank='26';checkNum='26';checkDate='26';checkAmt='26'
    taxDate='27';taxAmt='27';othersDetails='28';othersBank='28';othersNum='28';othersDate='28';othersAmt='28'
    txtStandardDed_C='37A';txtFamDed_C='37B';txtOtherDed_Specify='37C';txtOtherDed_C='37C'
}
$computedPattern = '(?i)(txtTaxableEstate|txtTaxRate|txtEstTaxDue|txtCredits_Tot|txtTaxPayable|txtTaxPayable_1st|txtPen_Tot|txtTotPayable|txtRealProp_|txtFam_|txtPersonalProp_|txtTaxable_|txtBusiness_|txtGrossEst_|txtOrdDed_|txtEstAfterDed_|txtTotalDed_|txtNetEst_|txtShareSpouse_|txtNetTaxEst_|_Total$)'
$amountPattern = '(?i)(Taxable|TaxRate|Tax|Credit|Installment$|Pen_|Payable|Amt$|Prop_|Fam_|Business_|GrossEst|OrdDed|AfterDed|TotalDed|NetEst|ShareSpouse|sched5|_Total)'

$fields = [Collections.Generic.List[object]]::new()
foreach ($control in $concreteControls) {
    $key = $control.id
    $short = if ($key -like 'frm1801v2018:*') { $key.Substring(15) } else { $key }
    $logical = 'string'; $enum = [object[]]@(); $normalization = [string[]]@()
    if ($control.control_kind -in @('radio','checkbox')) { $logical='boolean'; $enum=[object[]]@('true','false') }
    elseif ($key -match '(?i)(TIN|RDO|branchCode|txtAtc)') { $logical='code' }
    elseif ($key -match '(?i)(txtDateMonth|txtDateDay|txtDateYear)$') { $logical='date-component-string' }
    elseif ($key -match '(?i)(cashDate|checkDate|taxDate|othersDate)$') { $logical='date-string-mm-dd-yyyy' }
    elseif ($key -match $amountPattern) { $logical='decimal-amount'; $normalization=[string[]]@('NumWithComma','formatCurrency','round(...,2)') }
    elseif ($key -match '(?i)Email') { $logical='email-string' }
    $computed = $key -match $computedPattern
    $status = if ($requiredKeys -contains $key) { 'required' } elseif ($computed) { 'computed' } else { 'optional' }
    if ($key -match '(?i)(txtCurrentPage|txtMaxPage)') { $status='hidden' }
    if ($key -eq 'frm1801v2018:treatyY' -or $key -eq 'frm1801v2018:freqOtherInput') { $status='conditional' }
    $constraints = [ordered]@{}
    if ($control.maxlength -and $control.maxlength -match '^\d+$') { $constraints.max_length=[int]$control.maxlength }
    if ($logical -eq 'decimal-amount') { $constraints.precision=2; $constraints.sign='Most editable amount fields clear negative values on blur; computed values are not consistently floored.' }
    $item = if ($itemMap.ContainsKey($short)) { $itemMap[$short] } elseif ($key -eq 'frm1801v2018taxkNum') { '27' } else { $null }
    $page = if ($key -match '(?i)(_2$|txtRealProp_|txtFam_|txtPersonalProp_|txtTaxable_|txtBusiness_|txtGrossEst_|txtOrdDed_|txtEstAfterDed_|txtStandardDed|txtFamDed|txtOtherDed|txtTotalDed|txtNetEst|txtShareSpouse|txtNetTaxEst|sched)') { 2 } else { 1 }
    $notes = @('Source-derived from the exact January 2018 live DOM and generic Save serializer.')
    if ($key -eq 'frm1801v2018taxkNum') { $notes += 'Official malformed ID omits the colon while name is frm1801v2018:taxNum; generic Save serializes the malformed ID as the XML key.' }
    $fields.Add([pscustomobject][ordered]@{
        field_key=$key;serialized_key=$key;serialized_occurrence=1;label=$short;page=$page;item_number=$item
        control_kind=$control.control_kind;storage_type='string';logical_type=$logical;required=$status
        required_when=if($key-eq'frm1801v2018:treatyY'){'Item 14 Yes.'}elseif($key-eq'frm1801v2018:freqOtherInput'){'Installment granted and frequency Others.'}else{$null}
        enabled_when=if($key-eq'frm1801v2018:treatyY'){'Item 14 Yes.'}elseif($key-eq'frm1801v2018:freqOtherInput'){'Item 15B Yes and frequency Others.'}elseif($key-eq'frm1801v2018:txtCredits_TaxPrev'){'Amended Return Yes.'}else{$null}
        visible_when=$null;default_value=$control.value;empty_representation='';constraints=[pscustomobject]$constraints
        enum_values=$enum;normalization=$normalization;computed=$computed
        calculation_id=if($computed){'See calculations.json'}else{$null}
        source_refs=@("official-hta-runtime#control:L$($control.source_line)",'official-hta-runtime#saveXML:L2647-L2908')
        confidence='high';notes=$notes
    })
}
foreach ($family in $families) {
    $pattern = "frm1801v2018:$($family.name)_{index}"
    $fields.Add([pscustomobject][ordered]@{
        field_key=$pattern;serialized_key=$pattern;serialized_occurrence=$null;label=$family.name;page=2;item_number=$null
        control_kind=$family.control;storage_type='string';logical_type=$family.kind;required='optional'
        required_when='If any non-zero/nonblank cell in the same schedule row is populated, all cells in that row must be populated.'
        enabled_when=$null;visible_when=$null;default_value=if($family.kind-eq'decimal-amount'){'0.00'}else{$null}
        empty_representation='';constraints=[pscustomobject]@{index='one-based, contiguous; two rows initially; Add has no source upper bound'}
        enum_values=@(if($family.name-like'*Class'){'official select choices; see runtime function fixture'})
        normalization=@(if($family.kind-eq'decimal-amount'){'round(...,2)';'blockNegativeNumber';'formatCurrency'})
        computed=$false;calculation_id=$null
        source_refs=@("official-hta-runtime#$($family.source)",'official-hta-runtime#saveXML:L2647-L2908')
        confidence='high';notes=@('sleeptime creates indices 1 and 2 for a new form; Add/Delete retain contiguous one-based indices.')
    })
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{
    '$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    field_count=$fields.Count;runtime_serializable_element_count=$runtimeBaseline
    inventory_sha256=Get-HashText @($fields.field_key|Sort-Object);fields=$fields
})

Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta
    live_static_control_count=$controls.Count;static_serializer_control_count=$staticSerial.Count
    runtime_injected_control_count=1;concrete_control_definition_count=$concreteControls.Count
    dynamic_family_count=$families.Count;initial_rows_per_family=2;projected_baseline_serializer_entry_count=$runtimeBaseline
    controls=@($controls)+@($rdoControl);dynamic_families=$families
})
Write-Json (Join-Path $fixtureDir 'legacy-artifact-exclusion.json') ([ordered]@{
    schema_version='1.0.0';target_form_id=$formId;plaintext_sha256=$expected.legacy_save
    encrypted_sha256=$expected.legacy_final;plaintext_key_count=111;plaintext_inventory_sha256=$expected.legacy_inventory
    observed_prefix='104 frm1801: keys plus seven unprefixed legacy metadata keys';required_prefix='frm1801v2018:'
    values_emitted=$false;disposition='excluded from target revision field evidence'
    source_paths=@((Join-Path $LegacySaveDir '00000000000000-1801-07222026.xml'),(Join-Path $LegacyFinalDir '1801-final-copy-#email-redacted#.xml'))
})

$functionTool = Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1801v2018:' -NamePattern '(?i)valid|check|enable|disable|save|submit|final|date|tin') -join [Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((& $functionTool -HtaPath $htaPath -ControlPrefix 'frm1801v2018:' -NamePattern '(?i)compute|calculate|get.*total|installment|credit|tax|penalt') -join [Environment]::NewLine)

$rules = [Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,
    [string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',
    [string]$Recommended='Retain as a structured revision-aware error.',[string]$Confidence='high') {
    $rules.Add([pscustomobject][ordered]@{
        rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys
        accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.'
        exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official
        recommended_app_behavior=$Recommended;confidence=$Confidence;unresolved_questions=@()
    })
}
$dateKeys=@('frm1801v2018:txtDateMonth','frm1801v2018:txtDateDay','frm1801v2018:txtDateYear')
$tinKeys=@('frm1801v2018:tinA_1','frm1801v2018:tinB_1','frm1801v2018:tinC_1','frm1801v2018:branchCode_1')
Rule '1801-validate-001-date-required' validate 1 'Any return-date component is blank.' $dateKeys 'Please enter a valid Return Date' @('official-hta-runtime#validateForm:L3281-L3287')
Rule '1801-validate-002-month' validate 2 'Parsed month is below 1, above 12, or NaN.' $dateKeys 'Please enter a valid month on Item 1.' @('official-hta-runtime#validateForm:L3361-L3384')
Rule '1801-validate-003-nonleap-february' validate 3 'Non-leap February day exceeds 28.' $dateKeys 'Please enter a valid date on Item 1. Filing year is not a leap year.' @('official-hta-runtime#validateForm:L3386-L3389')
Rule '1801-validate-004-day' validate 4 'Day is below 1 or exceeds the computed month maximum.' $dateKeys 'Please enter a valid day on Item 1.' @('official-hta-runtime#validateForm:L3391-L3394')
Rule '1801-validate-005-year' validate 5 'Year is blank.' $dateKeys 'Please enter a valid year on Item 1.' @('official-hta-runtime#validateForm:L3396-L3399')
Rule '1801-validate-006-min-year' validate 6 'Year is below 1904.' $dateKeys 'Invalid date entry on Item 1. Entry should not be lower than 1904.' @('official-hta-runtime#validateForm:L3401-L3404') 'incorrect-official-behavior' 'The active Validate path accepts death dates from 1904 through 2017 even though payment-date validation and the January 2018 revision require 2018 or later.' 'Apply the revision/legal effective-date rule consistently and distinguish death date from filing date.'
Rule '1801-validate-007-future-date' validate 7 'Constructed return date is after today.' $dateKeys 'Invalid date entry on Item 1. Date cannot be after the current date.' @('official-hta-runtime#validateForm:L3406-L3409')
Rule '1801-validate-008-tin-required' validate 8 'Any Item 5 TIN segment or branch code is blank.' $tinKeys 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#validateForm:L3413-L3416') 'official-bug-compatible' 'Only blankness is checked; checksum and format validity are not checked here.' 'Apply the shared TIN checksum and exact segment constraints in addition to blankness.'
Rule '1801-validate-009-rdo' validate 9 'RDO select remains at index zero.' @('frm1801v2018:txtRDOCode_1') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#validateForm:L3417-L3421')
Rule '1801-validate-010-name' validate 10 'Taxpayer/decedent name is blank.' @('frm1801v2018:registeredName_1') "Please enter a valid Taxpayer's Name on Item 7." @('official-hta-runtime#validateForm:L3422-L3425')
Rule '1801-validate-011-address' validate 11 'Taxpayer/decedent residence is blank.' @('frm1801v2018:registeredAddress_1') "Please enter Taxpayer's Resident of Decedent on Item 8." @('official-hta-runtime#validateForm:L3426-L3429') 'official-bug-compatible' 'The source uses the grammatically incorrect phrase Resident of Decedent.' 'Use a clear label while preserving the official message for compatibility diagnostics.'
Rule '1801-validate-012-residency' validate 12 'Neither non-resident status radio is checked.' @('frm1801v2018:categoryNonResident_1','frm1801v2018:categoryNonResident_2') 'Please select an option for Item 9.' @('official-hta-runtime#validateForm:L3430-L3433')
Rule '1801-validate-013-telephone' validate 13 'Telephone number is blank.' @('frm1801v2018:telephoneNumber') 'Please enter a valid Telephone Number on Item 12.' @('official-hta-runtime#validateForm:L3434-L3437') 'official-bug-compatible' 'Only blankness is checked; syntactic validity is not checked.' 'Validate the accepted telephone character set and length.'
Rule '1801-validate-014-installment-frequency' validate 14 'Installment is granted but no payment frequency is selected.' @('frm1801v2018:instGranted_1','frm1801v2018:freq_Mon','frm1801v2018:freq_Qtr','frm1801v2018:freq_Semi','frm1801v2018:freq_Other') 'Please select a frequency of payment on Item 15D.' @('official-hta-runtime#validateForm:L3439-L3447') 'official-bug-compatible' 'The message cites Item 15D, but frequency is subordinate to installment permission and the printed subitem association is ambiguous.' 'Bind the error to the actual frequency group and retain the official item citation as source behavior.'
Rule '1801-validate-015-other-frequency' validate 15 'Installment is granted, Others is selected, and its description is blank.' @('frm1801v2018:freq_Other','frm1801v2018:freqOtherInput') 'Please specify a frequency of payment on Item 15D - Others.' @('official-hta-runtime#validateForm:L3449-L3452')
Rule '1801-validate-016-standard-ded-nonresident' validate 16 'Item 9 Yes and standard deduction exceeds PHP 500,000.' @('frm1801v2018:categoryNonResident_1','frm1801v2018:txtStandardDed_C') 'If Yes is selected on Item 9, value should not exceed Php500 Thousand on Item 37A.' @('official-hta-runtime#validateForm:L3455-L3458','official-help-v2018#deductions')
Rule '1801-validate-017-standard-ded-resident' validate 17 'Item 9 No and standard deduction exceeds PHP 5,000,000.' @('frm1801v2018:categoryNonResident_2','frm1801v2018:txtStandardDed_C') 'If No is selected on Item 9, value should not exceed Php5 Million on Item 37A.' @('official-hta-runtime#validateForm:L3458-L3461','official-help-v2018#deductions')
Rule '1801-validate-018-family-home' validate 18 'Family-home deduction exceeds PHP 10,000,000.' @('frm1801v2018:txtFamDed_C') 'Value should not exceed Php10 Million on Item 37B.' @('official-hta-runtime#validateForm:L3463-L3466','official-help-v2018#deductions')
$scheduleMessages=@(
    @('schedule-1','Schedule 1','frm1801v2018:sched1{column}_{index}','official-hta-runtime#validateForm:L3468-L3505'),
    @('schedule-1a','Schedule 1A','frm1801v2018:sched1A{column}_{index}','official-hta-runtime#validateForm:L3507-L3528'),
    @('schedule-2','Schedule 2','frm1801v2018:sched2{column}_{index}','official-hta-runtime#validateForm:L3530-L3548'),
    @('schedule-2a','Schedule 2A','frm1801v2018:sched2A{column}_{index}','official-hta-runtime#validateForm:L3550-L3568'),
    @('schedule-3','Schedule 3','frm1801v2018:sched3{column}_{index}','official-hta-runtime#validateForm:L3570-L3588'),
    @('schedule-4','Schedule 4','frm1801v2018:sched4{column}_{index}','official-hta-runtime#validateForm:L3590-L3611')
)
$order=19
foreach($s in $scheduleMessages){
    Rule "1801-validate-$('{0:d3}' -f $order)-$($s[0])" validate $order "Within a row, non-zero cells mix blank and populated values in $($s[1])." @($s[2]) "Incomplete values on $($s[1]), Row {row}." @($s[3]) 'official-bug-compatible' 'The source treats 0.00 as absent and requires all other cells to be uniformly blank or populated; for Schedule 1 the state is shared across its continuation table.' 'Use row-schema validation with explicit required columns and preserve zero as a legitimate numeric value where legally meaningful.'
    $order++
}
Rule '1801-validate-025-success' validate 25 'All preceding source-ordered checks pass.' @() 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validateForm:L3613-L3621') 'verified-correct' 'Controls are disabled, Print/Edit/Upload are enabled, and success is alerted.' 'Model validation state explicitly and retain the source ordering.'
Rule '1801-save-001-date-all-blank' save 1 'All three return-date components are blank.' $dateKeys 'Please enter a valid Return Date' @('official-hta-runtime#initialValidateBeforeSave:L3800-L3805') 'incorrect-official-behavior' 'Save uses AND, so a partially populated date bypasses this guard; it performs no calendar/future-date checks.' 'Require all date components and apply the same full date validation used by Validate.'
Rule '1801-save-002-tin-required' save 2 'Any Item 5 TIN segment or branch code is blank.' $tinKeys 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L3806-L3809') 'official-bug-compatible' 'Only blankness is checked.' 'Apply shared TIN format and checksum validation.'
Rule '1801-save-003-rdo' save 3 'RDO value is 000 or blank.' @('frm1801v2018:txtRDOCode_1') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L3810-L3813')
Rule '1801-save-004-name' save 4 'Taxpayer/decedent name is blank.' @('frm1801v2018:registeredName_1') 'Please enter a valid Taxpayer Name on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L3814-L3817') 'official-bug-compatible' "Save omits the apostrophe-s used by Validate's otherwise equivalent message." 'Use one structured error with phase-specific official text retained for diagnostics.'
Rule '1801-save-005-sparse-guard' save 5 'Address, residency, phone, installment frequency, deduction caps, and schedule completeness are invalid.' @() $null @('official-hta-runtime#initialValidateBeforeSave:L3800-L3820','official-hta-runtime#validateForm:L3426-L3611') 'incorrect-official-behavior' 'Save ignores every listed condition even though Validate rejects it.' 'Use a shared validation graph with explicitly documented phase exceptions.'
Rule '1801-input-001-negative-clear' 'blur/change' 1 'A numeric field passed to blockNegativeNumber parses below zero.' @('amount-fields') $null @('official-hta-runtime#blockNegativeNumber:L3252-L3258') 'official-bug-compatible' 'The field is silently cleared without an error message.' 'Return a field-level error instead of silently deleting input.'
Rule '1801-date-001-debug-alert' 'blur/change' 1 'Any enabled payment-date field loses focus and calls validateDate.' @('frm1801v2018:cashDate','frm1801v2018:checkDate','frm1801v2018:taxDate','frm1801v2018:othersDate') 'in' @('official-hta-runtime#validateDate:L4811-L4817') 'incorrect-official-behavior' 'An unconditional debugging alert appears before validation.' 'Remove the debug alert.'
Rule '1801-date-002-format' 'blur/change' 2 'Payment date is not a real MM/DD/YYYY date.' @('frm1801v2018:cashDate','frm1801v2018:checkDate','frm1801v2018:taxDate','frm1801v2018:othersDate') 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L4818-L4870')
Rule '1801-date-003-future' 'blur/change' 3 'Payment date is after today.' @('frm1801v2018:cashDate','frm1801v2018:checkDate','frm1801v2018:taxDate','frm1801v2018:othersDate') 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L4871-L4875') 'incorrect-official-behavior' 'The field is cleared, but isValid remains true and the function returns true.' 'Return false after clearing the field.'
Rule '1801-date-004-pre2018' 'blur/change' 4 'Payment date year is before 2018.' @('frm1801v2018:cashDate','frm1801v2018:checkDate','frm1801v2018:taxDate','frm1801v2018:othersDate') 'This date cannot be prior to 2018.' @('official-hta-runtime#validateDate:L4876-L4882') 'incorrect-official-behavior' 'The field is cleared, but isValid remains true and the function returns true.' 'Return false after clearing the field.'
Rule '1801-serialization-001-malformed-tax-debit-key' save $null 'The generic serializer reaches the Tax Debit Memo number control.' @('frm1801v2018taxkNum') $null @('official-hta-runtime#control:L1153','official-hta-runtime#saveXML:L2807-L2876') 'incorrect-official-behavior' 'The XML key is the malformed id frm1801v2018taxkNum, not the name frm1801v2018:taxNum.' 'Preserve the malformed key for lossless compatibility while exposing a typed alias and migration warning.'
Rule '1801-validate-026-treaty-description-unchecked' validate 26 'Tax treaty Yes is selected while treaty description is blank.' @('frm1801v2018:optTreaty_1','frm1801v2018:treatyY') $null @('official-hta-runtime#changeTreaty:L4938-L4940','official-hta-runtime#validateForm:L3281-L3622') 'incorrect-official-behavior' 'The text box is enabled but Validate never requires it.' 'Require treaty identification when Yes is selected.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{
    '$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    first_error_behavior='validateForm and initialValidateBeforeSave alert and return at the first source-ordered failure.';rules=$rules
})

$calculations=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Outputs,[string[]]$Inputs,[string]$Formula,[string]$Trigger,[string[]]$Depends,[string[]]$Refs,
    [string]$Assessment='verified-correct',[string]$Recommended='Implement as a typed decimal calculation with deterministic two-decimal formatting.') {
    $calculations.Add([pscustomobject][ordered]@{
        calculation_id=$Id;outputs=$Outputs;inputs=$Inputs;condition=$null;official_formula=$Formula
        rounding='formatCurrency calls Number.toFixed(2) and inserts thousands separators.';trigger=$Trigger;depends_on=$Depends
        source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'
    })
}
Calc '1801-schedule-real-property' @('frm1801v2018:sched1Exc_Total','frm1801v2018:sched1Conj_Total') @('frm1801v2018:sched1Exc_{index}','frm1801v2018:sched1Conj_{index}') 'Sum each ownership column across Schedule 1 rows.' getRealProp_totals @() @('official-hta-runtime#getRealProp_totals:L5032-L5045')
Calc '1801-schedule-family-home' @('frm1801v2018:sched1AExc_Total','frm1801v2018:sched1AConj_Total') @('frm1801v2018:sched1AExc_{index}','frm1801v2018:sched1AConj_{index}') 'Sum each ownership column across Schedule 1A rows.' getFam_totals @() @('official-hta-runtime#getFam_totals:L5046-L5059')
Calc '1801-schedule-shares' @('frm1801v2018:sched2Exc_Total','frm1801v2018:sched2Conj_Total') @('frm1801v2018:sched2Exc_{index}','frm1801v2018:sched2Conj_{index}') 'Sum each ownership column across Schedule 2 rows.' getStock_totals @() @('official-hta-runtime#getStock_totals:L5060-L5073')
Calc '1801-schedule-other-personal' @('frm1801v2018:sched2AExc_Total','frm1801v2018:sched2AConj_Total') @('frm1801v2018:sched2AExc_{index}','frm1801v2018:sched2AConj_{index}') 'Sum each ownership column across Schedule 2A rows.' getOther_totals @() @('official-hta-runtime#getOther_totals:L5074-L5087')
Calc '1801-schedule-taxable-transfers' @('frm1801v2018:sched3Exc_Total','frm1801v2018:sched3Conj_Total') @('frm1801v2018:sched3Exc_{index}','frm1801v2018:sched3Conj_{index}') 'Sum each ownership column across Schedule 3 rows.' getTransfer_totals @() @('official-hta-runtime#getTransfer_totals:L5088-L5101')
Calc '1801-schedule-business' @('frm1801v2018:sched4Exc_Total','frm1801v2018:sched4Conj_Total') @('frm1801v2018:sched4Exc_{index}','frm1801v2018:sched4Conj_{index}') 'Sum each ownership column across Schedule 4 rows.' getBusiness_totals @() @('official-hta-runtime#getBusiness_totals:L5102-L5115')
Calc '1801-schedule-ordinary-deductions' @('frm1801v2018:sched5Exc_Total','frm1801v2018:sched5Conj_Total') @('frm1801v2018:sched5*Exc','frm1801v2018:sched5*Conj') 'Sum the seven fixed Schedule 5 ordinary-deduction rows by ownership column.' getOrdDed_totals @() @('official-hta-runtime#getOrdDed_totals:L5116-L5130')
Calc '1801-gross-estate-components' @('frm1801v2018:txtRealProp_C','frm1801v2018:txtFam_C','frm1801v2018:txtPersonalProp_C','frm1801v2018:txtTaxable_C','frm1801v2018:txtBusiness_C') @('schedule totals') 'For each category, C = exclusive A + conjugal B; personal property combines Schedules 2 and 2A.' calculate_Part4 @('1801-schedule-real-property','1801-schedule-family-home','1801-schedule-shares','1801-schedule-other-personal','1801-schedule-taxable-transfers','1801-schedule-business') @('official-hta-runtime#calculate_Part4:L4977-L4996')
Calc '1801-gross-estate-total' @('frm1801v2018:txtGrossEst_A','frm1801v2018:txtGrossEst_B','frm1801v2018:txtGrossEst_C') @('category A/B values') 'Gross A and B are category sums; Gross C = Gross A + Gross B.' calculate_Part4 @('1801-gross-estate-components') @('official-hta-runtime#calculate_Part4:L4998-L5008')
Calc '1801-estate-after-ordinary-deductions' @('frm1801v2018:txtEstAfterDed_A','frm1801v2018:txtEstAfterDed_B','frm1801v2018:txtEstAfterDed_C') @('gross estate','ordinary deductions') 'A = gross A - ordinary deductions A; B likewise; C = A + B.' calculate_Part4 @('1801-gross-estate-total','1801-schedule-ordinary-deductions') @('official-hta-runtime#calculate_Part4:L5010-L5016')
Calc '1801-special-deductions-total' @('frm1801v2018:txtTotalDed_C') @('frm1801v2018:txtStandardDed_C','frm1801v2018:txtFamDed_C','frm1801v2018:txtOtherDed_C') 'Total special deductions = standard + family home + other.' calculate_Part4 @() @('official-hta-runtime#calculate_Part4:L5018')
Calc '1801-net-estate' @('frm1801v2018:txtNetEst_C') @('frm1801v2018:txtEstAfterDed_C','frm1801v2018:txtTotalDed_C') 'Net estate = estate after ordinary deductions - special deductions.' calculate_Part4 @('1801-estate-after-ordinary-deductions','1801-special-deductions-total') @('official-hta-runtime#calculate_Part4:L5020')
Calc '1801-surviving-spouse-share' @('frm1801v2018:txtShareSpouse_C') @('frm1801v2018:txtEstAfterDed_B') 'Surviving-spouse share = one half of conjugal estate after ordinary deductions.' calculate_Part4 @('1801-estate-after-ordinary-deductions') @('official-hta-runtime#calculate_Part4:L5022') 'ambiguous' 'Verify the legal base and ownership treatment against the revision-matched guide before implementation; retain the source formula separately.'
Calc '1801-net-taxable-estate' @('frm1801v2018:txtNetTaxEst_C','frm1801v2018:txtTaxableEstate') @('frm1801v2018:txtNetEst_C','frm1801v2018:txtShareSpouse_C') 'Net taxable estate = net estate - surviving-spouse share; Item 17 copies it.' calculate_Part4 @('1801-net-estate','1801-surviving-spouse-share') @('official-hta-runtime#calculate_Part4:L5024-L5026')
Calc '1801-estate-tax-due' @('frm1801v2018:txtEstTaxDue') @('frm1801v2018:txtTaxableEstate','frm1801v2018:txtTaxRate') 'If taxable estate is positive, tax due = taxable estate × parsed percentage / 100; otherwise 0.00.' computeNo18 @('1801-net-taxable-estate') @('official-hta-runtime#computeNo18:L5359-L5377','official-help-v2018#six-percent')
Calc '1801-tax-credits-total' @('frm1801v2018:txtCredits_Tot') @('frm1801v2018:txtCredits_ForeignEst','frm1801v2018:txtCredits_TaxPrev') 'Credits total = foreign estate-tax credit + previous-payment credit.' calculate_Part2 @() @('official-hta-runtime#calculate_Part2:L4946-L4951')
Calc '1801-tax-payable' @('frm1801v2018:txtTaxPayable') @('frm1801v2018:txtEstTaxDue','frm1801v2018:txtCredits_Tot') 'Tax payable = estate tax due - total credits; no zero floor.' calculate_Part2 @('1801-estate-tax-due','1801-tax-credits-total') @('official-hta-runtime#calculate_Part2:L4952') 'official-bug-compatible' 'Represent overpayment separately or floor payable at zero according to the filing specification.'
Calc '1801-installment-balance' @('frm1801v2018:txtTaxPayable_1st') @('frm1801v2018:txtTaxPayable','frm1801v2018:txtInstallment') 'Balance = tax payable - installment amount; no zero floor.' calculate_Part2 @('1801-tax-payable') @('official-hta-runtime#calculate_Part2:L4953-L4954') 'official-bug-compatible' 'Reject installments exceeding payable tax or represent the resulting credit explicitly.'
Calc '1801-penalties-total' @('frm1801v2018:txtPen_Tot') @('frm1801v2018:txtPen_Surcharge','frm1801v2018:txtPen_Interest','frm1801v2018:txtPen_Compromise') 'Total penalties = surcharge + interest + compromise.' calculate_Part2 @() @('official-hta-runtime#calculate_Part2:L4955-L4958')
Calc '1801-total-payable' @('frm1801v2018:txtTotPayable') @('frm1801v2018:txtTaxPayable_1st','frm1801v2018:txtPen_Tot') 'Normally balance + penalties; if balance is negative and penalties positive, output penalties only.' calculate_Part2 @('1801-installment-balance','1801-penalties-total') @('official-hta-runtime#calculate_Part2:L4960-L4972') 'incorrect-official-behavior' 'Floor the tax component at zero before adding penalties and represent any credit separately; source can emit a negative total when penalties are zero.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{
    '$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    evaluation_order=@($calculations.calculation_id);calculations=$calculations
})

$cases=@();$n=0
foreach($rule in @($rules|Where-Object{$_.exact_message})){
    $n++;$cases += [pscustomobject][ordered]@{
        case_id=('case-{0:d2}-{1}'-f$n,$rule.rule_id);phase=$rule.phase;mutations=@{synthetic_condition=$rule.condition}
        expected_message=$rule.exact_message;expected_behavior=$rule.official_behavior;rule_id=$rule.rule_id
    }
}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{
    schema_version='1.0.0';form_id=$formId;cases=@(
        @{case_id='six-percent';calculation_id='1801-estate-tax-due';inputs=@{taxable_estate=1000000;rate_percent=6};official_output=60000},
        @{case_id='nonpositive-estate';calculation_id='1801-estate-tax-due';inputs=@{taxable_estate=-1;rate_percent=6};official_output=0},
        @{case_id='resident-deduction-cap';rule_id='1801-validate-017-standard-ded-resident';input=5000000;accepted=$true},
        @{case_id='nonresident-deduction-cap';rule_id='1801-validate-016-standard-ded-nonresident';input=500000;accepted=$true},
        @{case_id='negative-balance-no-penalty';calculation_id='1801-total-payable';inputs=@{balance=-100;penalties=0};official_output=-100;recommended_output=0},
        @{case_id='negative-balance-with-penalty';calculation_id='1801-total-payable';inputs=@{balance=-100;penalties=25};official_output=25}
    )
})

$resources=@()
foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){
    $full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if(Test-Path -LiteralPath $full){$resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}
    else{$resources += [pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{
    '$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
    phases=@(
        @{phase='edit';official_behavior='January 2018 estate-tax return with two-page estate and deduction schedules; new forms initialize two rows in each expandable schedule.';source_refs=@('official-form-pdf','official-help-v2018','official-hta-runtime#sleeptime:L2226-L2270');confidence='high'},
        @{phase='saved-draft';official_behavior='Save runs only date-all-blank, TIN blankness, RDO, and name checks, then generically serializes every non-hidden form element.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3800-L3820','official-hta-runtime#saveXML:L2647-L2908');confidence='high'},
        @{phase='validated';official_behavior='Validate performs source-ordered date, identity, residency, installment, deduction-cap, and schedule-row checks before disabling editable controls.';source_refs=@('official-hta-runtime#validateForm:L3281-L3622');confidence='high'},
        @{phase='final-copy';official_behavior='Final-copy paths reuse the generic serializer and encrypt/compress the copy; no revision-matched saved artifact was available for black-box comparison.';source_refs=@('official-hta-runtime#saveXML:L2647-L2908','legacy-artifact-exclusion');confidence='medium'},
        @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#saveXMLsubmit:L2910-L3090','official-hta-runtime#sendEmail:L4444-L4558');confidence='medium'}
    )
    transitions=@(
        @{from='edit';action='Save';to='saved-draft';guard='Narrow initialValidateBeforeSave checks and version-file guards pass.';side_effects=@('Writes flat pseudo-XML.','Serializes malformed tax-debit ID literally.','Includes two initial rows per dynamic schedule.');source_refs=@('official-hta-runtime#saveXML:L2647-L2908')},
        @{from='edit';action='Validate';to='validated';guard='All source-ordered Validate checks pass.';side_effects=@('Disables editable controls.','Enables Print/Edit/Upload.','Shows success alert.');source_refs=@('official-hta-runtime#validateForm:L3281-L3622')},
        @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables controls, then restores treaty/installment/amended conditional state.');source_refs=@('official-hta-runtime#editForm:L3633-L3640','official-hta-runtime#enabledDisabledControls:L3644-L3784')},
        @{from='validated';action='Final Copy';to='final-copy';guard='File-version and finalization flow permits progress.';side_effects=@('Creates encrypted/compressed final copy.');source_refs=@('official-hta-runtime#saveXML','official-hta-runtime#openAlertEmail')},
        @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and remote acceptance succeed.';side_effects=@('Attempts online submission; untested.');source_refs=@('official-hta-runtime#sendEmail')}
    )
    prerequisites=@('Date of death','Decedent TIN, RDO, identity, residence and residency status','Estate schedules, deductions, credits and installment information as applicable')
    required_attachments=@(
        @{attachment_id='death-certificate';label='Certified true copy of the death certificate.';required_when='All filings.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#mandatory-requirements');confidence='high'},
        @{attachment_id='estate-settlement';label='Affidavit of self-adjudication, deed of extrajudicial settlement, or court order/schedule of partition, as applicable.';required_when='According to the mode of estate settlement.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#mandatory-requirements');confidence='high'},
        @{attachment_id='real-property-documents';label='Titles, tax declarations, and assessor certifications for real property.';required_when='Estate includes real property.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#real-properties');confidence='high'},
        @{attachment_id='personal-property-documents';label='Ownership and valuation evidence for shares, deposits, vehicles, and other personal property.';required_when='Estate includes applicable personal property.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#personal-properties');confidence='high'},
        @{attachment_id='deduction-support';label='Invoices, statements, judicial/administrative evidence, and other documents supporting claimed deductions.';required_when='Applicable deductions are claimed.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#mandatory-requirements');confidence='high'}
    )
    filing_deadlines=@(
        @{quarter='Q1';due_date_rule='Event-based: within one year from death; approved meritorious extension may not exceed 30 days.';source_refs=@('official-help-v2018#filing-deadline');confidence='high'},
        @{quarter='Q2';due_date_rule='Not quarterly; the same death-relative deadline applies.';source_refs=@('official-help-v2018#filing-deadline');confidence='high'},
        @{quarter='Q3';due_date_rule='Not quarterly; the same death-relative deadline applies.';source_refs=@('official-help-v2018#filing-deadline');confidence='high'},
        @{quarter='Q4';due_date_rule='Not quarterly; the same death-relative deadline applies.';source_refs=@('official-help-v2018#filing-deadline');confidence='high'}
    )
}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugCount=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 1801v2018 and printed January 2018 ENCS.'
    Asset 'official-help-v2018' 'official-runtime-help' $helpPath 'Revision-matched January 2018 help; six-percent rate, one-year deadline, and documentary requirements.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 1801.'
    Asset 'official-guidelines-pdf' 'official-guidelines-pdf' $guidelinesPath 'Local guidelines distributed with the January 2018 form assets.'
    Asset 'legacy-hta-excluded' 'runtime-extracted-hta-legacy' $legacyHtaPath 'Legacy APPLICATIONNAME 1801; excluded.'
    Asset 'legacy-help-excluded' 'official-runtime-help-legacy' $legacyHelpPath 'Legacy help; excluded from revision-specific claims.'
    Asset 'legacy-editable-save-excluded' 'dummy-profile-editable-save-legacy' $legacySavePath '111-key legacy artifact; values excluded.'
    Asset 'legacy-final-copy-excluded' 'dummy-profile-encrypted-final-copy-legacy' $legacyFinalPath 'Legacy final copy; values excluded.' (Join-Path $LegacyFinalDir '1801-final-copy-#email-redacted#.xml')
)
$manifest=[ordered]@{
    '$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1801';revision=$revision
    revision_label='January 2018 ENCS';package_version=$packageVersion;status='complete';official_assets=$assets
    counts=[ordered]@{
        concrete_fields=$concreteControls.Count;runtime_field_families=$families.Count;fields_total=$fields.Count;typed_fields=$fields.Count
        validation_rules=$rules.Count;confirmed_official_bugs=$bugCount;calculations=$calculations.Count
        negative_fixtures=$cases.Count;unverified_gaps=3
    }
    artifacts=[ordered]@{
        fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json'
        evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json'
        legacy_artifact_exclusion='fixtures/legacy-artifact-exclusion.json';validation_function_fixture='fixtures/validation-function-inventory-v796.json'
        calculation_function_fixture='fixtures/calculation-function-inventory-v796.json';resource_hash_fixture='fixtures/official-resource-hashes-v796.json'
        negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'
    }
    scope_notes=@(
        'Research only; no renderer, typed model, migration, capability, or release metadata changed.',
        'No source values or email-bearing filenames are copied.',
        'The source-derived serializer model has 138 concrete control definitions and 37 indexed families; two initial rows per family yield 212 baseline entries.',
        'The available 111-key plaintext save and encrypted final copy are legacy frm1801 artifacts and are explicitly excluded.',
        'The malformed official Tax Debit Memo number ID is preserved literally as frm1801v2018taxkNum.'
    )
}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1801 - January 2018 ENCS`n`nRevision-specific Offline eBIRForms rule package with 138 concrete serializer controls, 37 expandable indexed families, and a 212-entry new-form baseline. Legacy frm1801 saves are excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- January 2018 HTA SHA-256: $($expected.hta); APPLICATIONNAME 1801v2018; printed January 2018 ENCS.
- Revision-matched help SHA-256: $($expected.help); identifies January 2018, the 6% rate, one-year deadline, and documentary requirements.
- Official form PDF SHA-256: $($expected.pdf); guidelines PDF SHA-256: $($expected.guidelines); valid PDF magic.
- Live DOM inventory: 157 static controls; 137 static serializable controls; one runtime-injected RDO select.
- Runtime projection: 138 concrete definitions plus 37 indexed families; two new-form rows per family produce 212 baseline serializer entries.
- Legacy plaintext SHA-256: $($expected.legacy_save); 111 unique keys, 104 with frm1801 prefix; inventory $($expected.legacy_inventory). Legacy encrypted SHA-256: $($expected.legacy_final). Both are excluded.
- The malformed Item 27 control ID frm1801v2018taxkNum is source-proven at line 1153 and is serialized literally.
- No existing typed 1801 model was found under crates/bir-core/src/forms.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') @"
# Gaps

1. No plaintext or encrypted final-copy sample with frm1801v2018 keys was available; the 212-entry baseline and 37 families are proven from the exact serializer/runtime source but not compared with a saved January 2018 artifact.
2. Online submission was not exercised.
3. Documentary attachment presence, payment controls populated by external processing, and ambiguous surviving-spouse-share legal treatment were not black-box exercised.
"@
Write-Utf8 (Join-Path $outDir 'audit.md') @"
# Audit

- Revision/assets: **pass** - exact 1801v2018 HTA, revision-matched help, January 2018 PDF/guidelines, package, and excluded legacy assets are pinned.
- Fields: **pass with explicit observation gap** - 138 concrete controls and 37 indexed families yield a 212-entry new-form baseline; revision-mismatched legacy saves are excluded.
- Controls/functions: **pass** - comment/script filtering, runtime RDO injection, dynamic rows, function inventories, and resource hashes captured.
- Rules/workflow: **pass** - exact Validate and Save order/messages, sparse Save behavior, conditional controls, version guards, deadline, and attachments captured.
- Calculations: **pass** - schedule sums, estate/deduction chain, 6% tax, credits, installment, penalties, and payable total recorded.
- Official defects: **pass** - $bugCount bug-compatible/incorrect rules include sparse Save, weak TIN/phone checks, mismatched date floors, row completeness semantics, debug alert, misleading date return value, negative payable paths, and malformed XML key.
- Privacy: **pass** - no values or email-bearing filenames copied.
- Revision-matched saved artifact, online transport, and black-box attachment/payment behavior: **unverified** and explicit gaps.
"@
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 21: 1801-v2018. Next: 2000OT.`n"

$indexPath=Join-Path $RepoRoot 'rules\index.json'
$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json
$entry=[pscustomobject][ordered]@{form_id=$formId;form_code='1801';revision=$revision;package_version=$packageVersion;priority=21;status='complete';path='forms/1801-v2018/manifest.json'}
$index.forms=@(@($index.forms|Where-Object{$_.form_id-ne$formId})+$entry|Sort-Object priority)
$index.updated=(Get-Date).ToString('yyyy-MM-dd')
Write-Json $indexPath $index

"Generated ${formId}: fields=$($fields.Count), concrete=$($concreteControls.Count), families=$($families.Count), baseline=$runtimeBaseline, rules=$($rules.Count), calculations=$($calculations.Count), negative_cases=$($cases.Count), bugs=$bugCount"
