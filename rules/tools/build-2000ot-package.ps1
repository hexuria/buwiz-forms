param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\2000-OTv2018',
    [string]$LegacySampleDir = 'C:\Mac\Home\Downloads\forms\2000OT'
)

$ErrorActionPreference = 'Stop'
$formId='2000ot-v2018';$revision='2018-01-01';$packageVersion='7.9.6.0'
$htaPath=Join-Path $ExtractedRoot 'forms\BIR-Form2000OTv2018.hta'
$legacyHtaPath=Join-Path $ExtractedRoot 'forms\BIR-Form2000OT.hta'
$helpPath=Join-Path $ExtractedRoot 'helpfile\Help2000-OTv2018.hta'
$legacyHelpPath=Join-Path $ExtractedRoot 'helpfile\Help2000-OT.hta'
$pdfPath=Join-Path $PdfDir '2000-OT January 2018 ENCS v3.pdf'
$packagePath='C:\eBIRForms\BIRForms.exe'
$outDir=Join-Path $RepoRoot 'rules\forms\2000ot-v2018';$fixtureDir=Join-Path $outDir 'fixtures'
$expected=@{
    hta='e44331e99dd8edb8d6fd01529f82e5d6612a9640307c7421b8be397ac87ebe33'
    legacy_hta='cb989655cc9161a7791e95a6a535604d16419bb38cfc69411d64a32b23ab19f6'
    help='e8dfb6463dacd15794db7029fa6032ab772292dd5af02b4b697915df6dcc4011'
    legacy_help='62ebbd7a285d0f3f9a26732b21d5c9642761c583df1f9af66ef2983b23f52a74'
    pdf='64d987ef79ed57005c1f13f8c9a5732bde2bf40f57dd4ec9f2067ef96c3c492d'
    legacy_cipher='1ec8c2829f0fac2e57e55d119d10e25d9755e6c53955010c073211cb59f6a11a'
    legacy_plain='75d040d198e76be1dc9c69d29673eefb23fdcde3fae826c89a672921fe6b2075'
    legacy_inventory='4e979a609c41fa0a603ebfa7adc5640b0aa7066fa6a40eae7a0ecec66c94a539'
    package='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}
function Write-Json([string]$Path,$Value){[IO.File]::WriteAllText($Path,(($Value|ConvertTo-Json -Depth 60)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false))}
function Write-Utf8([string]$Path,[string]$Value){[IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))}
function Hash-Lines([string[]]$Lines){$s=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($s.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines-join"`n"))))).Replace('-','').ToLowerInvariant()}finally{$s.Dispose()}}
function Attr([string]$Tag,[string]$Name){$m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)));if($m.Success){$m.Groups[2].Value}else{$null}}
function Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding,[string]$Display=''){$i=Get-Item -LiteralPath $Path;[pscustomobject][ordered]@{asset_id=$Id;kind=$Kind;path=if($Display){$Display}else{$Path};sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant();size=$i.Length;revision_binding=$Binding}}

foreach($p in @($htaPath,$legacyHtaPath,$helpPath,$legacyHelpPath,$pdfPath,$packagePath)){if(-not(Test-Path -LiteralPath $p)){throw"Missing source: $p"}}
foreach($pair in @(@($htaPath,'hta'),@($legacyHtaPath,'legacy_hta'),@($helpPath,'help'),@($legacyHelpPath,'legacy_help'),@($pdfPath,'pdf'),@($packagePath,'package'))){
    if((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected[$pair[1]]){throw"Hash changed: $($pair[0])"}
}
$legacySample=@(Get-ChildItem -LiteralPath $LegacySampleDir -File|Where-Object{$_.Extension-eq'.xml'})
if($legacySample.Count-ne1){throw"Expected one legacy sample; found $($legacySample.Count)."}
if((Get-FileHash -LiteralPath $legacySample[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected.legacy_cipher){throw'Legacy sample hash changed.'}
$bytes=[IO.File]::ReadAllBytes($pdfPath);if([Text.Encoding]::ASCII.GetString($bytes[0..4])-ne'%PDF-'){throw'PDF magic mismatch.'}
$hta=[IO.File]::ReadAllText($htaPath);$help=[IO.File]::ReadAllText($helpPath)
if($hta-notmatch'(?i)APPLICATIONNAME\s*=\s*["'']2000OTv2018["'']' -or $hta-notmatch'(?i)January\s+2018\s+\(ENCS\)'){throw'HTA revision binding changed.'}
if($help-notmatch'(?i)2000-OT.*January\s+2018' -or $help-notmatch'(?i)within\s+five\s+\(5\)\s+days'){throw'Help revision/deadline binding changed.'}
New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null

$fm=[regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>');if(-not$fm.Success){throw'frmMain missing.'}
$body=$fm.Groups['body'].Value;$offset=$fm.Groups['body'].Index
$excluded=@(@([regex]::Matches($body,'(?is)<script\b.*?</script>'))+@([regex]::Matches($body,'(?is)<!--.*?-->')))
$controls=[Collections.Generic.List[object]]::new();$ordinal=0
foreach($m in [regex]::Matches($body,'(?is)<(input|select|textarea|button)\b[^>]*>')){
    $skip=$false;foreach($r in $excluded){if($m.Index-ge$r.Index-and$m.Index-lt($r.Index+$r.Length)){$skip=$true;break}};if($skip){continue}
    $ordinal++;$tag=$m.Value;$element=$m.Groups[1].Value.ToLowerInvariant();$kind=if($element-eq'input'){Attr $tag 'type'}else{$element};if(-not$kind){$kind='text'}
    $controls.Add([pscustomobject][ordered]@{ordinal=$ordinal;id=Attr $tag 'id';name=Attr $tag 'name';element=$element;control_kind=$kind.ToLowerInvariant();source_line=1+[regex]::Matches($hta.Substring(0,$offset+$m.Index),"`n").Count;value=Attr $tag 'value';maxlength=Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)';readonly=$tag-match'(?i)\breadonly(?:\s*=|\s|>)'})
}
if($controls.Count-ne138){throw"Expected 138 live controls; found $($controls.Count)."}
$static=@($controls|Where-Object{$_.control_kind-in@('text','select','select-one','textarea','radio','checkbox')})
if($static.Count-ne125-or@($static.id|Sort-Object -Unique).Count-ne125){throw"Static serializer inventory changed: $($static.Count)."}
$projected=@($static|Where-Object{$_.id-notin@('frm2000OT:txtAddress2','frm2000OT:txtOtherName2')})
if($projected.Count-ne123){throw'Address/name collapse projection changed.'}
$families=@(
    @{pattern='chkSchedule1ADelete{index}';kind='boolean';control='runtime-indexed-checkbox';source='addSchedule1A:L5307-L5363'},
    @{pattern='chkSchedule1A1Delete{index}';kind='boolean';control='runtime-indexed-checkbox';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtOCTNo{index}';kind='string';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtTaxDecNo{index}';kind='string';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtLoc{index}';kind='string';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtLot{index}';kind='string';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtClassification{index}';kind='string';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtArea{index}';kind='decimal-or-string';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtFMVCol1{index}';kind='decimal-amount';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtFMVCol2{index}';kind='decimal-amount';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='frm2000OT:sched1A:txtFMVSubTot{index}';kind='decimal-amount';control='runtime-indexed-text';source='addSchedule1A:L5307-L5363'},
    @{pattern='chkSchedule1BDelete{index}';kind='boolean';control='runtime-indexed-checkbox';source='addSchedule1B:L5365-L5394'},
    @{pattern='frm2000OT:sched1B:txtNameOfCorpStock{index}';kind='string';control='runtime-indexed-text';source='addSchedule1B:L5365-L5394'},
    @{pattern='frm2000OT:sched1B:txtNoOfSharesSold{index}';kind='decimal-or-string';control='runtime-indexed-text';source='addSchedule1B:L5365-L5394'},
    @{pattern='frm2000OT:sched1B:txtStockCertNo{index}';kind='string';control='runtime-indexed-text';source='addSchedule1B:L5365-L5394'},
    @{pattern='frm2000OT:sched1B:txtParValOfShares{index}';kind='decimal-amount';control='runtime-indexed-text';source='addSchedule1B:L5365-L5394'},
    @{pattern='frm2000OT:sched1B:txtDSTPaid{index}';kind='decimal-amount';control='runtime-indexed-text';source='addSchedule1B:L5365-L5394'}
)
if($families.Count-ne17){throw'Dynamic family count changed.'}
$required=@('frm2000OT:txtTransactionDate','frm2000OT:txtTIN1','frm2000OT:txtTIN2','frm2000OT:txtTIN3','frm2000OT:txtBranchCode','frm2000OT:txtRDOCode','frm2000OT:txtTaxpayerName','frm2000OT:txtAddress','frm2000OT:txtZipCode','frm2000OT:txtTelNum','frm2000OT:txtOtherName','frm2000OT:txtOtherTIN')
$item=@{txtTransactionDate='1';AmendedRtn_1='2';AmendedRtn_2='2';ATC_1='3';ATC_2='3';ATC_3='3';txtSheets='4';txtTIN1='5';txtTIN2='5';txtTIN3='5';txtBranchCode='5';txtRDOCode='6';txtTaxpayerName='7';txtAddress='8';txtZipCode='8A';txtTelNum='9';optParty_1='11';optParty_2='11';txtOtherName='11A';txtOtherTIN='11B';rbTransNature_1='12';rbTransNature_2='12';rbTransNature_3='12';txtRealLocation='13';txtTax14='14';txtTax15='15';txtTax16='16';txtTax17='17';txtTax18='18';txtTax19='19';txtTax20A='20A';txtTax20B='20B';txtTax20C='20C';txtTax20D='20D';txtTax21='21'}
$computed='(?i)(txtTax14|txtTax15|txtTax16|txtTax17|txtTax19|txtTax20D|txtTax21|txtFMVTotal|txtTotalFairMarket|txtTaxableBase|txtFMVSubTot)'
$amount='(?i)(txtTax1[4-9]|txtTax20|txtTax21|FMV|FairMarket|GrossSelling|OthersAmount|TaxableBase|ParVal|DSTPaid|Amt$)'
$fields=[Collections.Generic.List[object]]::new()
foreach($c in $projected){
    $key=$c.id;$short=if($key-like'frm2000OT:*'){$key.Substring(10)}else{$key}
    $logical='string';$enum=[object[]]@();$norm=[string[]]@()
    if($c.control_kind-in@('radio','checkbox')){$logical='boolean';$enum=[object[]]@('true','false')}
    elseif($key-match'(?i)(TIN|RDO|BranchCode|ATC_)'){$logical='code'}
    elseif($key-match'(?i)Date'){$logical='date-string-mm-dd-yyyy'}
    elseif($key-match$amount){$logical='decimal-amount';$norm=[string[]]@('NumWithComma','formatCurrency','round(...,2)')}
    elseif($key-eq'txtEmail'){$logical='email-string'}
    $isComputed=$key-match$computed;$status=if($required-contains$key){'required'}elseif($isComputed){'computed'}else{'optional'}
    if($key-match'(?i)(txtCurrentPage|txtMaxPage)'){$status='hidden'}
    if($key-eq'frm2000OT:txtRealLocation'){$status='conditional'}
    $constraints=[ordered]@{};if($c.maxlength-and$c.maxlength-match'^\d+$'){$constraints.max_length=[int]$c.maxlength};if($logical-eq'decimal-amount'){$constraints.precision=2}
    $notes=@('Source-derived from the exact January 2018 DOM and Save serializer.')
    if($key-eq'frm2000OT:txtAddress'){$notes+='Serialized value concatenates txtAddress and txtAddress2 under this key.'}
    if($key-eq'frm2000OT:txtOtherName'){$notes+='Serialized value concatenates txtOtherName and txtOtherName2 under this key.'}
    if($key-eq'txtEmail'){$notes+='Official unprefixed control ID is serialized literally.'}
    $fields.Add([pscustomobject][ordered]@{field_key=$key;serialized_key=$key;serialized_occurrence=1;label=$short;page=if($key-match'(?i)(sched|Sched2)'){2}else{1};item_number=if($item.ContainsKey($short)){$item[$short]}else{$null};control_kind=$c.control_kind;storage_type='string';logical_type=$logical;required=$status;required_when=if($key-eq'frm2000OT:txtRealLocation'){'Nature of transaction is either real-property option.'}else{$null};enabled_when=if($key-eq'frm2000OT:txtTax18'){'Amended Return Yes.'}elseif($key-eq'frm2000OT:txtRealLocation'){'Nature is real property.'}else{$null};visible_when=$null;default_value=$c.value;empty_representation='';constraints=[pscustomobject]$constraints;enum_values=$enum;normalization=$norm;computed=$isComputed;calculation_id=if($isComputed){'See calculations.json'}else{$null};source_refs=@("official-hta-runtime#control:L$($c.source_line)",'official-hta-runtime#saveXML:L2412-L2757');confidence='high';notes=$notes})
}
foreach($f in $families){
    $fields.Add([pscustomobject][ordered]@{field_key=$f.pattern;serialized_key=$f.pattern;serialized_occurrence=$null;label=$f.pattern;page=2;item_number=$null;control_kind=$f.control;storage_type='string';logical_type=$f.kind;required='optional';required_when=$null;enabled_when=$null;visible_when=$null;default_value=if($f.kind-eq'decimal-amount'){'0.00'}else{$null};empty_representation='';constraints=[pscustomobject]@{index='zero-based contiguous; index 0 exists in static markup; Add is source-unbounded'};enum_values=@(if($f.kind-eq'boolean'){'true';'false'});normalization=@(if($f.kind-eq'decimal-amount'){'round(...,2)';'formatCurrency'});computed=$f.pattern-like'*FMVSubTot*';calculation_id=if($f.pattern-like'*FMVSubTot*'){'2000ot-fmv-row'}else{$null};source_refs=@("official-hta-runtime#$($f.source)",'official-hta-runtime#saveXML:L2412-L2757');confidence='high';notes=@('Initial index 0 is already represented in the concrete baseline; this family preserves added/reindexed rows.')})
}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=123;inventory_sha256=Hash-Lines @($fields.field_key|Sort-Object);fields=$fields})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta;live_static_control_count=$controls.Count;static_serializer_control_count=$static.Count;projected_baseline_serializer_entry_count=$projected.Count;collapsed_controls=@('frm2000OT:txtAddress2','frm2000OT:txtOtherName2');dynamic_family_count=$families.Count;initial_index=0;controls=$controls;dynamic_families=$families})
$decryptTool=Join-Path $RepoRoot 'rules\tools\audit-1701-encrypted-fields.ps1'
Write-Utf8 (Join-Path $fixtureDir 'legacy-artifact-exclusion.json') ((&$decryptTool -SourceDir $LegacySampleDir -FormId $formId -FilePattern '*.xml' -RedactedFileName '2000OT-final-copy-#email-redacted#.xml' -ExpectedCiphertextSha256 $expected.legacy_cipher -ExpectedDecryptedSha256 $expected.legacy_plain -ExpectedFieldCount 78 -ExpectedFieldInventorySha256 $expected.legacy_inventory -ExpectedExtraField 'frm2000_OT:txtDateMonth' -VersionField '*' -ExpectedXmlVersion '*')-join[Environment]::NewLine)
$functionTool=Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm2000OT:' -NamePattern '(?i)valid|save|enable|disable|date|process|final|submit')-join[Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm2000OT:' -NamePattern '(?i)compute|tax|fmv|atc')-join[Environment]::NewLine)

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',[string]$Recommended='Retain as a structured revision-aware error.'){
    $rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys;accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.';exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()})
}
Rule '2000ot-validate-001-date' validate 1 'Transaction date is blank.' @('frm2000OT:txtTransactionDate') 'Please enter a valid date on Item 1.' @('official-hta-runtime#validate:L3681-L3687')
Rule '2000ot-validate-002-atc' validate 2 'No ATC radio is selected.' @('frm2000OT:ATC_1','frm2000OT:ATC_2','frm2000OT:ATC_3') 'Please choose an ATC on item 3.' @('official-hta-runtime#validate:L3688-L3692')
Rule '2000ot-validate-003-tin' validate 3 'Any taxpayer TIN segment or branch code is blank.' @('frm2000OT:txtTIN1','frm2000OT:txtTIN2','frm2000OT:txtTIN3','frm2000OT:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#validate:L3694-L3698') 'official-bug-compatible' 'Only blankness is checked.' 'Apply shared TIN checksum and segment constraints.'
Rule '2000ot-validate-004-rdo' validate 4 'RDO value is blank.' @('frm2000OT:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#validate:L3700-L3704') 'official-bug-compatible' 'Validate rejects blank but not the placeholder 000 that Save rejects.' 'Reject both blank and 000 consistently.'
Rule '2000ot-validate-005-name' validate 5 'Taxpayer name is blank.' @('frm2000OT:txtTaxpayerName') 'Please enter a valid Taxpayer Name on Item 7.' @('official-hta-runtime#validate:L3706-L3710')
Rule '2000ot-validate-006-phone' validate 6 'Telephone is blank.' @('frm2000OT:txtTelNum') 'Please enter a valid Telephone Number on Item 9.' @('official-hta-runtime#validate:L3711-L3715') 'official-bug-compatible' 'Only blankness is checked.' 'Validate the accepted telephone syntax.'
Rule '2000ot-validate-007-address' validate 7 'Registered address first line is blank.' @('frm2000OT:txtAddress') "Please enter Taxpayer's Registered Address on Item 8." @('official-hta-runtime#validate:L3716-L3720')
Rule '2000ot-validate-008-zip' validate 8 'Zip code is blank.' @('frm2000OT:txtZipCode') "Please enter Taxpayer's Zip Code on Item 8A." @('official-hta-runtime#validate:L3722-L3726')
Rule '2000ot-validate-009-other-party' validate 9 'Neither other-party classification is selected.' @('frm2000OT:optParty_1','frm2000OT:optParty_2') 'Please choose one (1) Other Party to the transaction on item 11.' @('official-hta-runtime#validate:L3728-L3731')
Rule '2000ot-validate-010-other-name' validate 10 'Other-party name is blank.' @('frm2000OT:txtOtherName') 'Please enter a name for item 11A. The name that you will enter corresponds to your choice in item 11.' @('official-hta-runtime#validate:L3733-L3737')
Rule '2000ot-validate-011-other-tin' validate 11 'Other-party TIN is blank.' @('frm2000OT:txtOtherTIN') 'Please enter TIN for item 11B. The TIN that you will enter is the TIN of your entry in item 11A.' @('official-hta-runtime#validate:L3739-L3743') 'official-bug-compatible' 'Only blankness is checked.' 'Validate the counterparty TIN format/checksum.'
Rule '2000ot-validate-012-nature' validate 12 'No nature-of-transaction radio is selected.' @('frm2000OT:rbTransNature_1','frm2000OT:rbTransNature_2','frm2000OT:rbTransNature_3') 'Please choose one (1) Nature of Transaction on item 12.' @('official-hta-runtime#validate:L3745-L3748')
Rule '2000ot-validate-013-real-location' validate 13 'Either real-property nature is selected and location is blank.' @('frm2000OT:rbTransNature_2','frm2000OT:rbTransNature_3','frm2000OT:txtRealLocation') 'Please enter the address of the Location of Real Property in item 13.' @('official-hta-runtime#validate:L3750-L3758')
Rule '2000ot-validate-014-success' validate 14 'All prior checks pass.' @() "Validation successful. Click on 'Edit' if you wish to modify your entries." @('official-hta-runtime#validate:L3760-L3763') 'verified-correct' 'Controls are disabled and the success alert is shown.' 'Model validated state explicitly.'
Rule '2000ot-save-001-date' save 1 'Transaction date is blank.' @('frm2000OT:txtTransactionDate') 'Please enter a valid Transaction Date on Item 1.' @('official-hta-runtime#initialValidateBeforeSave:L3982-L3987')
Rule '2000ot-save-002-tin' save 2 'Any taxpayer TIN segment or branch code is blank.' @('frm2000OT:txtTIN1','frm2000OT:txtTIN2','frm2000OT:txtTIN3','frm2000OT:txtBranchCode') 'Please enter a valid TIN number on Item 5.' @('official-hta-runtime#initialValidateBeforeSave:L3989-L3993') 'official-bug-compatible' 'Only blankness is checked.' 'Apply shared checksum and format rules.'
Rule '2000ot-save-003-rdo' save 3 'RDO is 000.' @('frm2000OT:txtRDOCode') 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L3994-L3997')
Rule '2000ot-save-004-name' save 4 'Taxpayer name is blank.' @('frm2000OT:txtTaxpayerName') "Please enter a valid Taxpayer's Name on Item 7." @('official-hta-runtime#initialValidateBeforeSave:L3998-L4003')
Rule '2000ot-save-005-sparse' save 5 'ATC, phone, address, zip, counterparty, nature, or conditional real-property location is invalid.' @() $null @('official-hta-runtime#initialValidateBeforeSave:L3982-L4004','official-hta-runtime#validate:L3681-L3764') 'incorrect-official-behavior' 'Save ignores these Validate failures.' 'Use a shared validation graph with documented phase exceptions.'
Rule '2000ot-date-001-format' 'blur/change' 1 'A date control is not a real MM/DD/YYYY date.' @('date-controls') 'Please provide a valid date. (MM/DD/YYYY format)' @('official-hta-runtime#validateDate:L4980-L5038')
Rule '2000ot-date-002-future' 'blur/change' 2 'Date is after today.' @('date-controls') 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L5039-L5043') 'incorrect-official-behavior' 'The field clears but the function still returns true.' 'Return false after clearing.'
Rule '2000ot-date-003-pre2018' 'blur/change' 3 'Date year is before 2018.' @('date-controls') 'This date cannot be prior to 2018.' @('official-hta-runtime#validateDate:L5044-L5050') 'incorrect-official-behavior' 'The field clears but the function still returns true.' 'Return false after clearing.'
Rule '2000ot-serialization-001-collapses' save $null 'Save reaches two-line taxpayer address or other-party name.' @('frm2000OT:txtAddress','frm2000OT:txtAddress2','frm2000OT:txtOtherName','frm2000OT:txtOtherName2') $null @('official-hta-runtime#saveXML:L2608-L2615','official-hta-runtime#saveXML:L2670-L2675') 'official-bug-compatible' 'Each pair becomes one key with no inserted separator.' 'Preserve exact compatibility while retaining typed source-line components.'
Rule '2000ot-serialization-002-unprefixed-email' save $null 'Generic serializer reaches email control.' @('txtEmail') $null @('official-hta-runtime#control','official-hta-runtime#saveXML:L2594-L2693') 'official-bug-compatible' 'The unprefixed ID txtEmail becomes the XML key.' 'Preserve the literal key with a typed alias.'
Rule '2000ot-legacy-sample-excluded' final-copy $null 'The available encrypted sample is considered as v2018 evidence.' @('frm2000_OT:txtDateMonth') $null @('legacy-artifact-exclusion','legacy-hta-excluded#control') 'incorrect-official-behavior' 'The sample contains the legacy split-date key and is not revision-matched.' 'Exclude it from the January 2018 contract.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='Validate and Save alert and return on the first source-ordered failure.';rules=$rules})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Implement with typed decimals and deterministic two-decimal formatting.'){
    $calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula;rounding='formatCurrency uses toFixed(2) and thousands separators unless formula states otherwise.';trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'})
}
Calc '2000ot-fmv-row' @('frm2000OT:sched1A:txtFMVSubTot{index}') @('frm2000OT:sched1A:txtFMVCol1{index}','frm2000OT:sched1A:txtFMVCol2{index}') 'Row subtotal is max(zonal/Commissioner FMV, assessor FMV).' ComputeFMV @() @('official-hta-runtime#ComputeFMV:L5289-L5304','official-help-v2018#taxable-base')
Calc '2000ot-fmv-total' @('frm2000OT:sched1A:txtFMVTotal','frm2000OT:Sched2:txtTotalFairMarket') @('frm2000OT:sched1A:txtFMVSubTot{index}') 'Sum row FMV subtotals and copy to Schedule 2.' ComputeFMV @('2000ot-fmv-row') @('official-hta-runtime#ComputeFMV:L5299-L5304')
Calc '2000ot-stock-par-base' @('frm2000OT:txtTax14') @('frm2000OT:sched1B:txtParValOfShares{index}') 'For DO102, Item 14 is the sum of par values.' ComputeSharesOfStock @() @('official-hta-runtime#ComputeSharesOfStock:L5187-L5204')
Calc '2000ot-stock-no-par-base' @('frm2000OT:txtTax14') @('frm2000OT:sched1B:txtDSTPaid{index}') 'For DO125, Item 14 is the sum of DST paid on original issue.' ComputeSharesOfStock @() @('official-hta-runtime#ComputeSharesOfStock:L5187-L5214')
Calc '2000ot-real-property-base' @('frm2000OT:Sched2:txtTaxableBase','frm2000OT:txtTax15') @('frm2000OT:Sched2:txtGrossSellingPrice','frm2000OT:Sched2:txtTotalFairMarket','frm2000OT:Sched2:txtOthers','frm2000OT:Sched2:txtOthersAmount') 'For DO122, if Others is blank and amount is 0, base=max(gross selling price,total FMV); otherwise base=Others amount.' ComputeRealProperty @('2000ot-fmv-total') @('official-hta-runtime#ComputeRealProperty:L5217-L5264','official-help-v2018#taxable-base') 'official-bug-compatible' 'Model the third valuation basis explicitly rather than switching solely on blank description/zero amount.'
Calc '2000ot-do102-tax' @('frm2000OT:txtTax17') @('frm2000OT:txtTax14') 'For stock transfer with par value: floor(base / 200) × PHP 1.50.' TaxComputation @('2000ot-stock-par-base') @('official-hta-runtime#TaxComputation:L5120-L5130') 'incorrect-official-behavior' 'Use the legally required per-PHP-200-or-fraction rule (ceiling) after confirming the pinned form text; floor drops every fractional block.'
Calc '2000ot-do122-tax' @('frm2000OT:txtTax17') @('frm2000OT:txtTax15') 'For real property: round(base / 1000) × PHP 15.' TaxComputation @('2000ot-real-property-base') @('official-hta-runtime#TaxComputation:L5142-L5151') 'incorrect-official-behavior' 'Use the legally required per-PHP-1,000-or-fraction rule (ceiling); Math.round undercharges fractions below half a block.'
Calc '2000ot-do125-tax' @('frm2000OT:txtTax17') @('frm2000OT:txtTax14') 'For shares without par value: base × 50%.' TaxComputation @('2000ot-stock-no-par-base') @('official-hta-runtime#TaxComputation:L5137-L5140','official-help-v2018#shares-without-par')
Calc '2000ot-tax-still-due' @('frm2000OT:txtTax19') @('frm2000OT:txtTax17','frm2000OT:txtTax18') 'Tax still due/overpayment = tax due - previously paid on amended return.' TaxComputation @('2000ot-do102-tax','2000ot-do122-tax','2000ot-do125-tax') @('official-hta-runtime#TaxComputation:L5160-L5162')
Calc '2000ot-penalties' @('frm2000OT:txtTax20D') @('frm2000OT:txtTax20A','frm2000OT:txtTax20B','frm2000OT:txtTax20C') 'Total penalties = surcharge + interest + compromise.' TaxComputation @() @('official-hta-runtime#TaxComputation:L5164-L5166')
Calc '2000ot-total-payable' @('frm2000OT:txtTax21') @('frm2000OT:txtTax19','frm2000OT:txtTax20D') 'Total payable = max(tax still due,0) + penalties.' TaxComputation @('2000ot-tax-still-due','2000ot-penalties') @('official-hta-runtime#TaxComputation:L5172-L5180')
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})
$cases=@();$n=0;foreach($r in @($rules|Where-Object{$_.exact_message})){$n++;$cases+=[pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$r.rule_id);phase=$r.phase;mutations=@{synthetic_condition=$r.condition};expected_message=$r.exact_message;expected_behavior=$r.official_behavior;rule_id=$r.rule_id}}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=@(
    @{case_id='do102-exact-block';calculation_id='2000ot-do102-tax';base=200;official_output=1.5;recommended_output=1.5},
    @{case_id='do102-fraction';calculation_id='2000ot-do102-tax';base=201;official_output=1.5;recommended_output=3.0},
    @{case_id='do122-below-half';calculation_id='2000ot-do122-tax';base=1001;official_output=15;recommended_output=30},
    @{case_id='do125-half';calculation_id='2000ot-do125-tax';base=100;official_output=50},
    @{case_id='overpayment-plus-penalty';calculation_id='2000ot-total-payable';tax_still_due=-100;penalties=25;official_output=25}
)})
$resources=@();foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){$full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src));if(Test-Path -LiteralPath $full){$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}else{$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
phases=@(
    @{phase='edit';official_behavior='January 2018 monthly DST return for one-time stock and real-property transactions, with expandable valuation schedules.';source_refs=@('official-form-pdf','official-help-v2018');confidence='high'},
    @{phase='saved-draft';official_behavior='Save applies four narrow identity/date guards and serializes 123 baseline entries plus added schedule rows.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L3982-L4004','official-hta-runtime#saveXML:L2412-L2757');confidence='high'},
    @{phase='validated';official_behavior='Validate checks transaction date, ATC, identity, counterparty, transaction nature, and conditional real-property location, but not schedule completeness or tax-base consistency.';source_refs=@('official-hta-runtime#validate:L3681-L3764');confidence='high'},
    @{phase='final-copy';official_behavior='Final-copy encryption is source-present; the only available encrypted sample is legacy and excluded.';source_refs=@('official-hta-runtime#saveXML','legacy-artifact-exclusion');confidence='medium'},
    @{phase='submitted';official_behavior='Online transport exists but was not exercised.';source_refs=@('official-hta-runtime#sendEmail');confidence='medium'}
)
transitions=@(
    @{from='edit';action='Save';to='saved-draft';guard='Narrow Save guards and file-version guards pass.';side_effects=@('Writes flat pseudo-XML.','Collapses two address/name continuation controls.','Preserves added zero-based schedule rows.');source_refs=@('official-hta-runtime#saveXML:L2412-L2757')},
    @{from='edit';action='Validate';to='validated';guard='All source-ordered checks pass.';side_effects=@('Disables controls.','Enables print/edit/final-copy paths.');source_refs=@('official-hta-runtime#validate:L3681-L3764')},
    @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables controls and restores conditional states.');source_refs=@('official-hta-runtime#enableAllControl:L3846-L3918')},
    @{from='validated';action='Final Copy';to='final-copy';guard='Finalization/version flow succeeds.';side_effects=@('Encrypts/compresses final copy.');source_refs=@('official-hta-runtime#saveXML')},
    @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and remote acceptance succeed.';side_effects=@('Online submission attempt; untested.');source_refs=@('official-hta-runtime#sendEmail')}
)
prerequisites=@('Transaction date and ATC','Taxpayer identity/RDO','Counterparty identity','Nature of transaction and location if real property','Applicable stock or real-property valuation schedules')
required_attachments=@(
    @{attachment_id='taxable-document';label='Document to which the documentary stamp is affixed.';required_when='All applicable filings.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#attachments');confidence='high'},
    @{attachment_id='original-issue-dst-proof';label='Proof of DST paid on original issue of shares without par value.';required_when='DO125 applies.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#attachments');confidence='high'},
    @{attachment_id='valuation-documents';label='Documents supporting stock or real-property valuation and selling price.';required_when='Applicable transaction.';official_ui_enforcement='Not checked by local Validate.';source_refs=@('official-help-v2018#taxable-base');confidence='high'}
)
filing_deadlines=@(
    @{quarter='Q1';due_date_rule='Monthly/event-related: file and pay within five days after the close of the month of the taxable document.';source_refs=@('official-help-v2018#deadline');confidence='high'},
    @{quarter='Q2';due_date_rule='Not quarterly; the same monthly deadline applies.';source_refs=@('official-help-v2018#deadline');confidence='high'},
    @{quarter='Q3';due_date_rule='Not quarterly; the same monthly deadline applies.';source_refs=@('official-help-v2018#deadline');confidence='high'},
    @{quarter='Q4';due_date_rule='Not quarterly; the same monthly deadline applies.';source_refs=@('official-help-v2018#deadline');confidence='high'}
)}
Write-Json (Join-Path $outDir 'workflow.json') $workflow
$bugs=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 2000OTv2018; January 2018 ENCS.'
    Asset 'official-help-v2018' 'official-runtime-help' $helpPath 'Revision-matched January 2018 guide and deadline.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 Form 2000-OT.'
    Asset 'legacy-hta-excluded' 'runtime-extracted-hta-legacy' $legacyHtaPath 'Legacy split-date form; excluded.'
    Asset 'legacy-help-excluded' 'official-runtime-help-legacy' $legacyHelpPath 'Legacy help; excluded.'
    Asset 'legacy-final-copy-excluded' 'dummy-profile-encrypted-final-copy-legacy' $legacySample[0].FullName 'Contains legacy frm2000_OT split-date key; excluded.' (Join-Path $LegacySampleDir '2000OT-final-copy-#email-redacted#.xml')
)
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2000OT';revision=$revision;revision_label='January 2018 ENCS';package_version=$packageVersion;status='complete';official_assets=$assets;counts=[ordered]@{concrete_fields=123;runtime_field_families=17;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=3};artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';runtime_control_fixture='fixtures/runtime-control-inventory-v796.json';legacy_artifact_exclusion='fixtures/legacy-artifact-exclusion.json';validation_function_fixture='fixtures/validation-function-inventory-v796.json';calculation_function_fixture='fixtures/calculation-function-inventory-v796.json';resource_hash_fixture='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research only; no renderer/release metadata changed.','No values or email-bearing filenames copied.','Exact serializer projects 125 static controls to 123 baseline entries by collapsing address and counterparty-name continuation controls.','Seventeen zero-based indexed families preserve expandable Schedule 1A/1B rows; index 0 is already in the baseline.','The available 78-key encrypted sample is legacy because it contains frm2000_OT:txtDateMonth; it is excluded.')}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 2000-OT - January 2018 ENCS`n`nRevision-specific rule package with 123 baseline serializer entries and 17 expandable indexed families. The available legacy split-date final copy is excluded.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- January 2018 HTA SHA-256: $($expected.hta); APPLICATIONNAME 2000OTv2018; printed January 2018 ENCS.
- Revision-matched help SHA-256: $($expected.help); five-day-after-month-close deadline and taxable-base/attachment guidance.
- Official PDF SHA-256: $($expected.pdf); valid PDF magic.
- DOM: 138 live controls, 125 static serializer controls, 123 baseline entries after two continuation collapses, and 17 indexed families.
- Legacy encrypted sample SHA-256: $($expected.legacy_cipher); decrypted SHA-256 $($expected.legacy_plain); 78 fields; inventory $($expected.legacy_inventory). Presence of frm2000_OT:txtDateMonth proves revision mismatch, so values and fields are excluded.
- No existing typed 2000-OT model was found under crates/bir-core/src/forms.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No saved/final-copy sample produced by the January 2018 single-date runtime was available.`n2. Online submission was not exercised.`n3. Schedule completeness, attachment presence, and the corrected per-block tax rounding were not black-box exercised.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- Revision/assets: **pass**.`n- Fields: **pass with explicit observation gap** - 123 baseline entries and 17 families are source-derived; legacy 78-key sample excluded.`n- Rules/workflow: **pass** - exact Save/Validate order and messages captured.`n- Calculations: **pass** - valuation, three ATCs, credits, penalties, and payable total captured.`n- Official defects: **pass** - $bugs bug-compatible/incorrect rules include sparse Save, weak identity checks, inconsistent RDO placeholder handling, misleading date return, collapsed keys, unprefixed email, and incorrect floor/round per-block tax formulas.`n- Privacy: **pass**.`n"
Write-Utf8 (Join-Path $outDir 'HANDOFF.md') "Completed priority 22: 2000ot-v2018. Next: 2000.`n"
$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -LiteralPath $indexPath -Raw|ConvertFrom-Json
$entry=[pscustomobject][ordered]@{form_id=$formId;form_code='2000OT';revision=$revision;package_version=$packageVersion;priority=22;status='complete';path='forms/2000ot-v2018/manifest.json'}
$index.forms=@(@($index.forms|Where-Object{$_.form_id-ne$formId})+$entry|Sort-Object priority);$index.updated=(Get-Date).ToString('yyyy-MM-dd');Write-Json $indexPath $index
"Generated ${formId}: fields=$($fields.Count), concrete=123, families=17, rules=$($rules.Count), calculations=$($calcs.Count), negative_cases=$($cases.Count), bugs=$bugs"
