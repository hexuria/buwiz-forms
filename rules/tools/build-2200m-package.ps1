param(
    [string]$RepoRoot=(Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot='C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir='C:\Mac\Home\Downloads\forms\2200Mv2018',
    [string]$SampleDir='C:\Mac\Home\Downloads\forms\2200M'
)
$ErrorActionPreference='Stop'
$formId='2200m-v2018';$revision='2018-01-01';$packageVersion='7.9.6.0'
$htaPath=Join-Path $ExtractedRoot 'forms\BIR-Form2200Mv2018.hta'
$legacyPath=Join-Path $ExtractedRoot 'forms\BIR-Form2200M.hta'
$helpPath=Join-Path $ExtractedRoot 'helpfile\Help2200Mv2018.hta'
$legacyHelpPath=Join-Path $ExtractedRoot 'helpfile\Help2200M.hta'
$pdfPath=Join-Path $OfficialDir '2200-M Jan 2018 ENCS v2 final version.pdf'
$packagePath='C:\eBIRForms\BIRForms.exe'
$outDir=Join-Path $RepoRoot 'rules\forms\2200m-v2018';$fixtureDir=Join-Path $outDir 'fixtures'
$expected=@{
    hta='5f0f128a166da6429a237883f662fa36393c4a5ac1472be79db1d04a9e27eef5'
    legacy='5a9935fd72160013c0dd1468918bd7b0944dd669f19f71ce61638a2c8ba98e48'
    help='158b6d4cb145bb45e06e82e092f743852f0891372ad92daaa48aea0d17d3398c'
    legacy_help='69a984e199192f468eb5659597b210fd03dd079bbf8805032336f4c55b3cca89'
    pdf='94161600eef26177a311e09d7b7e233a9582e1d2e56211b5f81f59d1891d9a90'
    package='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher='c2fa440e5829c28fb7c7de157692f9e3c8572114aebf7fd23200bb57994fd4b1'
    plain='3d8f9bf6f196d40b6dacbb38e7e26d1f65a11783b94ebd1221b799f78422e058'
    inventory='e877bfaf9467ac34a9e32ea76b54b700dd6dd0bdf6a3f03c2ad4837870634656'
}
function WJ([string]$Path,$Value){[IO.File]::WriteAllText($Path,(($Value|ConvertTo-Json -Depth 60)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false))}
function WT([string]$Path,[string]$Value){[IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))}
function HL([string[]]$Lines){$s=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($s.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines-join"`n"))))).Replace('-','').ToLowerInvariant()}finally{$s.Dispose()}}
function Attr([string]$Tag,[string]$Name){$m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)));if($m.Success){$m.Groups[2].Value}else{$null}}
function Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding,[string]$Display=''){$i=Get-Item -LiteralPath $Path;[pscustomobject][ordered]@{asset_id=$Id;kind=$Kind;path=if($Display){$Display}else{$Path};sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant();size=$i.Length;revision_binding=$Binding}}
foreach($pair in @(@($htaPath,'hta'),@($legacyPath,'legacy'),@($helpPath,'help'),@($legacyHelpPath,'legacy_help'),@($pdfPath,'pdf'),@($packagePath,'package'))){
    if(-not(Test-Path -LiteralPath $pair[0] -PathType Leaf)){throw"Missing $($pair[0])"}
    if((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected[$pair[1]]){throw"Hash changed: $($pair[0])"}
}
$sample=@(Get-ChildItem -LiteralPath $SampleDir -File -Filter '*.xml');if($sample.Count-ne1){throw"Expected one sample; found $($sample.Count)."}
if((Get-FileHash -LiteralPath $sample[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected.cipher){throw'Sample hash changed.'}
$pb=[IO.File]::ReadAllBytes($pdfPath);if([Text.Encoding]::ASCII.GetString($pb[0..4])-ne'%PDF-'){throw'PDF magic mismatch.'}
$hta=[IO.File]::ReadAllText($htaPath);$legacy=[IO.File]::ReadAllText($legacyPath);$help=[IO.File]::ReadAllText($helpPath)
if($hta-notmatch'(?i)APPLICATIONNAME\s*=\s*["'']2200Mv2018["'']'-or$hta-notmatch'(?i)January\s+2018\s+\(ENCS\)'){throw'Current revision binding changed.'}
if($help-notmatch'(?i)excise tax return for mineral products'-or$help-notmatch'(?i)paid upon removal of the mineral products'){throw'Help binding changed.'}
New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null

$keyTool=Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1';$redactedSample=Join-Path $SampleDir '2200M-final-copy-#email-redacted#.xml'
$keyJson=&$keyTool -SourcePath $sample[0].FullName -RedactedSourcePath $redactedSample -FormId '2200m-legacy-excluded' `
    -ExpectedCiphertextSha256 $expected.cipher -ExpectedDecryptedSha256 $expected.plain -ExpectedFieldCount 139 -ExpectedFieldInventorySha256 $expected.inventory
$ka=$keyJson|ConvertFrom-Json;$legacyKeys=@($ka.keys)
$currentIds=@([regex]::Matches($hta,'(?i)\bid\s*=\s*(["''])(?<id>.*?)\1')|ForEach-Object{$_.Groups['id'].Value}|Where-Object{$_}|Sort-Object -Unique)
$legacyIds=@([regex]::Matches($legacy,'(?i)\bid\s*=\s*(["''])(?<id>.*?)\1')|ForEach-Object{$_.Groups['id'].Value}|Where-Object{$_}|Sort-Object -Unique)
$currentOverlap=@($legacyKeys|Where-Object{$currentIds-contains$_});$legacyOverlap=@($legacyKeys|Where-Object{$legacyIds-contains$_})
if($currentOverlap.Count-ne6-or$legacyOverlap.Count-ne139){throw"Sample discrimination changed: current/legacy overlap $($currentOverlap.Count)/$($legacyOverlap.Count)."}
WT (Join-Path $fixtureDir 'excluded-legacy-encrypted-field-keys-v796.json') ($keyJson-join[Environment]::NewLine)

$fm=[regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>');if(-not$fm.Success){throw'frmMain missing.'}
$body=$fm.Groups['body'].Value;$offset=$fm.Groups['body'].Index
$excluded=@(@([regex]::Matches($body,'(?is)<script\b.*?</script>'))+@([regex]::Matches($body,'(?is)<!--.*?-->')))
$controls=[Collections.Generic.List[object]]::new();$ordinal=0
foreach($m in [regex]::Matches($body,'(?is)<(input|select|textarea|button)\b[^>]*>')){
    $skip=$false;foreach($range in $excluded){if($m.Index-ge$range.Index-and$m.Index-lt($range.Index+$range.Length)){$skip=$true;break}};if($skip){continue}
    $ordinal++;$tag=$m.Value;$el=$m.Groups[1].Value.ToLowerInvariant();$kind=if($el-eq'input'){Attr $tag 'type'}else{$el};if(-not$kind){$kind='text'};$kind=$kind.ToLowerInvariant()
    $default=Attr $tag 'value';if($kind-in@('radio','checkbox')){$default=if($tag-match'(?i)\bchecked(?:\s*=|\s|>)'){'true'}else{'false'}}
    $controls.Add([pscustomobject][ordered]@{ordinal=$ordinal;id=Attr $tag 'id';name=Attr $tag 'name';element=$el;control_kind=$kind;source_line=1+[regex]::Matches($hta.Substring(0,$offset+$m.Index),"`n").Count;default_value=$default;maxlength=Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)';readonly=$tag-match'(?i)\breadonly(?:\s*=|\s|>)'})
}
$serial=@($controls|Where-Object{$_.control_kind-in@('text','select','select-one','textarea','radio','checkbox')-and$_.id})
$runtimeRdo='frm2200M:rdoPg1Pt1I6RDO'
if($controls.Count-ne277-or$serial.Count-ne250-or@($serial.id|Sort-Object -Unique).Count-ne250){throw"Control inventory changed: $($controls.Count)/$($serial.Count)/$(@($serial.id|Sort-Object -Unique).Count)."}
if($serial.id-contains$runtimeRdo-or$hta-notmatch[regex]::Escape("<select id='frm2200M:rdoPg1Pt1I6RDO'")){throw'Runtime RDO derivation changed.'}
$families=@(
    @{key='frm2200M:txtSched1ATC{n>=11}';kind='text';logical='code';computed=$false},
    @{key='frm2200M:chkBoxSched1Description{n>=11}';kind='checkbox';logical='boolean';computed=$false},
    @{key='frm2200M:txtSched1_description{n>=11}';kind='text';logical='string';computed=$false},
    @{key='frm2200M:txtSched1_PlaceOfRemoval{n>=11}';kind='text';logical='string';computed=$false},
    @{key='frm2200M:txtSched1_VOMRITaxableA{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_VOMRIExemptB{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_LocallyExtractedRateC{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_LocallyExtractedRateD{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_LocallyExtractedTaxRateE{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_LocallyExtractedTaxDueF{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_descriptionCont{n>=11}';kind='text';logical='string';computed=$true},
    @{key='frm2200M:txtSched1_ImportedTaxableG{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_ImportedExemptH{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_ImportedTaxRate{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_ImportedTaxDue{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_TaxDueAdjustment{n>=11}';kind='text';logical='decimal-amount';computed=$false},
    @{key='frm2200M:txtSched1_TotalTaxDue{n>=11}';kind='text';logical='decimal-amount';computed=$false}
)
$required=@('frm2200M:txtPg1I1Month','frm2200M:txtPg1I1Day','frm2200M:txtPg1I1Year','frm2200M:txtPg1I5RDO',$runtimeRdo,'frm2200M:txtPg1Pt1I6RegisteredName','frm2200M:txtPg1Pt1I7RegisteredAddress','frm2200M:txPg1I7ZipCode','frm2200M:txtPg1Pt1I8ContactNumber','frm2200M:txtPg1Pt1I9Email','frm2200M:txtPg1Pt1I10Region','frm2200M:txtPg1Pt1I10Province','frm2200M:txtPg1Pt1I10City','frm2200M:txtPg1Pt1I11Region','frm2200M:txtPg1Pt1I11Province','frm2200M:txtPg1Pt1I11City')
$computed=@('frm2200M:TotalExciseTaxDue','frm2200M:txtPg1P3I16ExciseTaxDue','frm2200M:txtPg1P3I17CTotal','frm2200M:txtPg1P3I18NetTaxDue','frm2200M:txtPg1P3I20TaxStillDue','frm2200M:txtPg1P3I21DTotPenalties','frm2200M:txtPg1P3I22AmountPayable','frm2200M:txtPg1P3I23BPenalties','frm2200M:txtPg1P3I23CTotPmntMade','frm2200M:txtPg1P3I24BalToCarryOver')
function MakeField([string]$Key,[string]$Serialized,$Occurrence,$Control,[string]$Logical=''){
    $kind=if($Control){$Control.control_kind}else{'runtime-generated-select'};if(-not$Logical){$Logical=if($kind-in@('radio','checkbox')){'boolean'}elseif($Key-match'(?i)(?:TIN|RDO|ATC|Zip|Month|Year|Region|Province|City)'){'code'}elseif($Key-match'(?i)(?:Tax|Bal|Credit|Pmnt|Surcharge|Interest|Compromise|Amount|Rate|Taxable|Exempt|Adjustment|Total)'){'decimal-amount'}else{'string'}}
    $isComp=$computed-contains$Serialized;$status=if($isComp){'computed'}elseif($required-contains$Serialized){'required'}else{'optional'};$req=$null
    if($Serialized-eq'frm2200M:txtPg1I12TaxReliefSpecify'){$status='conditional';$req='Tax Relief Yes is selected.'};if($Serialized-eq'frm2200M:txtPg1Pt2MOPOtherDesc'){$status='conditional';$req='Other manner of payment is selected.'}
    $cons=[ordered]@{};if($Control-and$Control.maxlength-match'^\d+$'){$cons.max_length=[int]$Control.maxlength};if($Logical-eq'decimal-amount'){$cons.precision=2;$cons.sign='source-dependent'}
    $enum=[object[]]::new(0);if($Logical-eq'boolean'){$enum=[object[]]@('true','false')};$norm=[string[]]::new(0);if($Logical-eq'decimal-amount'){$norm=[string[]]@('NumWithComma','formatCurrency','round(2)')}
    [pscustomobject][ordered]@{field_key=$Key;serialized_key=$Serialized;serialized_occurrence=$Occurrence;label=$Serialized;page=if($Serialized-match'(?i)Sched1|TotalExcise'){2}else{1};item_number=if($Serialized-match'P3I(\d+[A-D]?)'){$Matches[1]}elseif($Serialized-match'(?i)Sched1|TotalExcise'){'Schedule 1'}else{$null};control_kind=$kind;storage_type='string';logical_type=$Logical;required=$status;required_when=$req;enabled_when=if($Serialized-eq'frm2200M:txtPg1P3I19PmntOnRtrnPrevFiled'){'Amended Return Yes is selected.'}else{$null};visible_when=$null;default_value=if($Control){$Control.default_value}else{'000'};empty_representation='';constraints=[pscustomobject]$cons;enum_values=$enum;normalization=$norm;computed=$isComp;calculation_id=if($isComp){'See calculations.json'}else{$null};source_refs=@('official-hta-runtime#saveXML',"official-hta-runtime#control:L$(if($Control){$Control.source_line}else{9243})");confidence='high';notes=@('Source-derived from the hash-pinned January 2018 runtime; no revision-matched final copy is available.')}
}
$fields=[Collections.Generic.List[object]]::new();$occ=@{}
foreach($c in $serial){$k=$c.id;if(-not$occ.ContainsKey($k)){$occ[$k]=0};$occ[$k]++;$fk=if($occ[$k]-eq1){$k}else{"$k#occurrence-$($occ[$k])"};$fields.Add((MakeField $fk $k $occ[$k] $c))}
$fields.Add((MakeField $runtimeRdo $runtimeRdo 1 $null))
foreach($f in $families){
    $fenum=[object[]]::new(0);if($f.logical-eq'boolean'){$fenum=[object[]]@('true','false')};$fnorm=[string[]]::new(0);if($f.logical-eq'decimal-amount'){$fnorm=[string[]]@('NumWithComma','formatCurrency','round(2)')}
    $fields.Add([pscustomobject][ordered]@{field_key=$f.key;serialized_key=$f.key;serialized_occurrence=1;label=$f.key;page=2;item_number='Schedule 1 dynamic row';control_kind=$f.kind;storage_type='string';logical_type=$f.logical;required='conditional';required_when='A dynamic Schedule 1 row exists; description and place of removal plus at least one numeric column are required.';enabled_when=$null;visible_when='Dynamic row index n is created.';default_value='';empty_representation='';constraints=[pscustomobject]@{index_minimum=11;unbounded=$true};enum_values=$fenum;normalization=$fnorm;computed=[bool]$f.computed;calculation_id=if($f.computed){'2200m-schedule-description-copy'}else{$null};source_refs=@('official-hta-runtime#sched1RowTemplate:L4228-L4256','official-hta-runtime#saveXML');confidence='high';notes=@('Unbounded runtime family; row identifiers are reindexed after deletion.')})
}
$concrete=$serial.Count+1
if($concrete-ne251-or$families.Count-ne17-or$fields.Count-ne268){throw"Typed inventory changed: $concrete/$($families.Count)/$($fields.Count)."}
WJ (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=$concrete;inventory_sha256=HL @($fields.field_key|Sort-Object);fields=$fields})
WJ (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta;live_control_count=$controls.Count;static_serialized_occurrence_count=$serial.Count;static_unique_serialized_id_count=@($serial.id|Sort-Object -Unique).Count;duplicate_serialized_occurrence_count=$serial.Count-@($serial.id|Sort-Object -Unique).Count;runtime_generated_scalar_count=1;runtime_generated_scalars=@($runtimeRdo);runtime_family_count=$families.Count;runtime_families=@($families.key);excluded_legacy_sample_key_count=$legacyKeys.Count;excluded_sample_current_overlap=$currentOverlap.Count;excluded_sample_legacy_overlap=$legacyOverlap.Count;controls=$controls})
$fn=Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1';WT (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$fn -HtaPath $htaPath -ControlPrefix 'frm2200M:' -NamePattern '(?i)valid|check|save|date|submit|final|row')-join[Environment]::NewLine);WT (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$fn -HtaPath $htaPath -ControlPrefix 'frm2200M:' -NamePattern '(?i)comput|total|tax|penalt|balance|format')-join[Environment]::NewLine)

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',[string]$Recommended='Retain as a structured revision-aware error.'){$rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys;accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.';exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()})}
Rule '2200m-validate-001-month' validate 1 'Month is 00.' @('frm2200M:txtPg1I1Month') 'Month field on Page 1 Item 1 is required.' @('official-hta-runtime#validateAll:L4003-L4009')
Rule '2200m-validate-002-day' validate 2 'Day is blank.' @('frm2200M:txtPg1I1Day') 'Day field on Page 1 Item 1 is required.' @('official-hta-runtime#validateAll:L4011-L4016')
Rule '2200m-validate-003-year' validate 3 'Year is blank.' @('frm2200M:txtPg1I1Year') 'Year field on Page 1 Item 1 is required.' @('official-hta-runtime#validateAll:L4018-L4023')
Rule '2200m-validate-004-date' validate 4 'Return date fails component/calendar checks.' @('frm2200M:txtPg1I1Month','frm2200M:txtPg1I1Day','frm2200M:txtPg1I1Year') 'Please provide a valid date. (MM/DD/YYYY format) in Page 1 Item 1.' @('official-hta-runtime#validateDate:L4462-L4536')
Rule '2200m-validate-005-future' validate 5 'Return date is after today.' @('Return-period-fields') 'Page 1 Item 1 Date cannot be a future date ' @('official-hta-runtime#validateDate:L4525-L4529')
Rule '2200m-validate-006-pre-revision' validate 6 'Return date is earlier than January 1, 2018.' @('Return-period-fields') 'Page 1 Item 1 Date cannot be earlier than January 2018' @('official-hta-runtime#validateDate:L4530-L4534')
Rule '2200m-validate-007-rdo' validate 7 'Hidden RDO is 000.' @('frm2200M:txtPg1I5RDO') 'Please enter a valid RDO Code on Page 1 Item 6.' @('official-hta-runtime#validateAll:L4026')
$mandatory=@(
    @('name','frm2200M:txtPg1Pt1I6RegisteredName','Name field on Page 1 Item 7 is required.'),
    @('address','frm2200M:txtPg1Pt1I7RegisteredAddress','Registered Address field on Page 1 Item 8 is required.'),
    @('zip','frm2200M:txPg1I7ZipCode','Zip Code field on Page 1 Item 8A is required.'),
    @('contact','frm2200M:txtPg1Pt1I8ContactNumber','Contact Number field on Page 1 Item 9 is required.'),
    @('email','frm2200M:txtPg1Pt1I9Email','E-mail address on page 1 item 10 is required.')
)
$o=8;foreach($x in $mandatory){Rule "2200m-validate-$('{0:d3}'-f$o)-$($x[0])" validate $o 'Field is blank after trim.' @($x[1]) $x[2] @("official-hta-runtime#validateAll:L$((4019+$o))");$o++}
$locations=@(
    @('prod-region','frm2200M:txtPg1Pt1I10Region','Region field on Page 1 Item 10 (Place of Production) is required.'),
    @('prod-province','frm2200M:txtPg1Pt1I10Province','Province field on Page 1 Item 10 (Place of Production) is required.'),
    @('prod-city','frm2200M:txtPg1Pt1I10City','City field on Page 1 Item 10 (Place of Production) is required.'),
    @('rem-region','frm2200M:txtPg1Pt1I11Region','Region field on Page 1 Item 11 (Place of Removal) is required.'),
    @('rem-province','frm2200M:txtPg1Pt1I11Province','Province field on Page 1 Item 11 (Place of Removal) is required.'),
    @('rem-city','frm2200M:txtPg1Pt1I11City','City field on Page 1 Item 11 (Place of Removal) is required.')
)
foreach($x in $locations){Rule "2200m-validate-$('{0:d3}'-f$o)-$($x[0])" validate $o 'Location selector is 00.' @($x[1]) $x[2] @('official-hta-runtime#validateAll:L4032-L4042');$o++}
Rule '2200m-validate-019-relief-choice' validate 19 'Neither tax-relief radio is selected.' @('frm2200M:rdoPg1I12TaxReliefYes','frm2200M:rdoPg1I12TaxReliefNo') 'Availing of Tax Relief field on Page 1 Item 12 is required.' @('official-hta-runtime#validateAll:L4044-L4048')
Rule '2200m-validate-020-relief-specify' validate 20 'Tax Relief Yes is selected and specification is blank.' @('frm2200M:rdoPg1I12TaxReliefYes','frm2200M:txtPg1I12TaxReliefSpecify') 'Specify Tax Relief field on Page 1 Item 12A is required.' @('official-hta-runtime#validateAll:L4050-L4054')
Rule '2200m-validate-021-payment-choice' validate 21 'No manner-of-payment radio is selected.' @('frm2200M:rdoPg1Pt2MOPPaymentActual','frm2200M:rdoPg1Pt2MOPPrepayment','frm2200M:rdoPg1Pt2MOPOther') 'Manner of Payment on Page 1 Part II is required.' @('official-hta-runtime#validateAll:L4056-L4061')
Rule '2200m-validate-022-payment-other' validate 22 'Other manner is selected and description is blank.' @('frm2200M:rdoPg1Pt2MOPOther','frm2200M:txtPg1Pt2MOPOtherDesc') 'Specfy Manner of Payment field on Page 1 Part II Item 15 is required.' @('official-hta-runtime#validateAll:L4063-L4067')
Rule '2200m-validate-023-fund' validate 23 'Item 24 balance is positive.' @('frm2200M:txtPg1P3I24BalToCarryOver') 'YOU HAVE INSUFICIENT FUND. PLEASE APPLY DEPOSIT TO PROCEED' @('official-hta-runtime#validateAll:L4069-L4073')
Rule '2200m-schedule-024-description' validate 24 'Populated fixed row 9/10 or any dynamic row lacks description.' @('frm2200M:txtSched1_description{9..n}') 'Description in row #{row} is required.' @('official-hta-runtime#checkPartVFields:L4405-L4435')
Rule '2200m-schedule-025-place' validate 25 'A populated Schedule 1 row lacks place of removal.' @('frm2200M:txtSched1_PlaceOfRemoval{1..n}') 'Place of Removal in row #{row} is required.' @('official-hta-runtime#checkPartVRow:L4442-L4448')
Rule '2200m-schedule-026-one-column' validate 26 'A row has place of removal but every other tested numeric column is blank.' @('Schedule-1-row-fields') 'Input at least 1 more column in row #{row}' @('official-hta-runtime#checkPartVRow:L4450-L4458')
Rule '2200m-addrow-027-description' 'blur/change' 1 'Previous row description is blank when Add Row is pressed.' @('frm2200M:txtSched1_description{n}') 'Description is required in row #{row}' @('official-hta-runtime#isPrevRowValid:L4261-L4284')
Rule '2200m-input-028-email' 'blur/change' 2 'Nonblank email fails the source substring regex.' @('frm2200M:txtPg1Pt1I9Email') 'Please enter a valid e-mail address on page 1 item 10' @('official-hta-runtime#validateEmail:L4709-L4720')
Rule '2200m-input-029-year' 'blur/change' 3 'Nonblank year is greater than current year.' @('frm2200M:txtPg1I1Year') 'Year shall not be greater than the present year' @('official-hta-runtime#checkYear:L4793-L4805')
Rule '2200m-input-030-negative-balance' 'blur/change' 4 'Item 24 balance is negative.' @('frm2200M:txtPg1P3I24BalToCarryOver') 'YOU HAVE INSUFICIENT FUND. PLEASE APPLY DEPOSIT TO PROCEED' @('official-hta-runtime#checkI24BalToCarryOver:L4894-L4899') 'incorrect-official-behavior' 'The blur check alerts on a negative balance while Validate blocks a positive balance with the same message.' 'Apply the legally intended sign rule consistently.'
Rule '2200m-save-031-date' save 1 'Return date component is blank or month/day is 00.' @('Return-period-fields') 'Please enter correct date on Item 1.' @('official-hta-runtime#initialValidateBeforeSave:L10482-L10485')
Rule '2200m-save-032-rdo' save 2 'Runtime RDO selector is 000.' @($runtimeRdo) 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L10489')
Rule '2200m-save-033-name' save 3 'Name is blank.' @('frm2200M:txtPg1Pt1I6RegisteredName') 'Please enter a valid name on Page 1 Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L10490')
Rule '2200m-save-034-address' save 4 'Address is blank.' @('frm2200M:txtPg1Pt1I7RegisteredAddress') 'Please enter a valid Registered Address on Page 1 Item 8.' @('official-hta-runtime#initialValidateBeforeSave:L10491')
Rule '2200m-save-035-contact' save 5 'Contact number is blank.' @('frm2200M:txtPg1Pt1I8ContactNumber') 'Please enter a valid Contact Number on Page 1 Item 9.' @('official-hta-runtime#initialValidateBeforeSave:L10492')
Rule '2200m-defect-036-last-row' validate 27 'The final fixed or dynamic Schedule 1 row contains invalid data.' @('Schedule-1-last-row') $null @('official-hta-runtime#getTotalRow:L4356-L4363','official-hta-runtime#checkPartVFields:L4365-L4440') 'incorrect-official-behavior' 'getTotalRow returns the final zero-based jQuery index; loops start at 1 and end at that index, omitting the actual final row.' 'Return the actual row count/highest row identifier and validate every extant row.'
Rule '2200m-defect-037-total-last-row' 'blur/change' 5 'The final Schedule 1 row contains locally extracted tax due.' @('frm2200M:txtSched1_LocallyExtractedTaxDueF{last}') $null @('official-hta-runtime#getTotalRow:L4356-L4363','official-hta-runtime#computeTotalTaxDue:L4914-L4933') 'incorrect-official-behavior' 'The same off-by-one excludes the final row from TotalExciseTaxDue.' 'Sum every extant row.'
Rule '2200m-defect-038-total-wrong-column' 'blur/change' 6 'Imported tax due, adjustment, or total tax due differs from locally extracted Tax Due F.' @('frm2200M:txtSched1_LocallyExtractedTaxDueF{n}','frm2200M:txtSched1_ImportedTaxDue{n}','frm2200M:txtSched1_TaxDueAdjustment{n}','frm2200M:txtSched1_TotalTaxDue{n}') $null @('official-hta-runtime#computeTotalTaxDue:L4914-L4933') 'incorrect-official-behavior' 'TotalExciseTaxDue sums only locally extracted Tax Due F, ignoring the schedule total column, imported tax due, and adjustments.' 'Derive Item 16 from the authoritative Total Tax Due column.'
Rule '2200m-defect-039-manual-tax-due' 'blur/change' 7 'Entered tax-due values disagree with taxable/exempt/rate inputs.' @('Schedule-1-rate-and-tax-fields') $null @('official-hta-runtime#computeTaxDue:L4905-L4912','official-hta-runtime#sched1RowTemplate:L4228-L4256') 'incorrect-official-behavior' 'The row tax computation function is entirely commented out; tax-due and total fields remain manual.' 'Compute each row from a revision-pinned legal rate table.'
Rule '2200m-defect-040-nonnumeric-date' validate 28 'Day is two nonnumeric characters or year is four nonnumeric characters.' @('frm2200M:txtPg1I1Day','frm2200M:txtPg1I1Year') $null @('official-hta-runtime#validateDate:L4462-L4536') 'incorrect-official-behavior' 'isNaN(strmm || strdd || stryyyy) tests only the first truthy component; Invalid Date comparisons then fail open.' 'Strictly parse every component as digits.'
Rule '2200m-defect-041-email-substring' 'blur/change' 8 'The email contains a valid-looking address as a substring plus junk.' @('frm2200M:txtPg1Pt1I9Email') $null @('official-hta-runtime#validateEmail:L4709-L4720') 'official-bug-compatible' 'The regex is not anchored and accepts substring matches.' 'Validate the full normalized address.'
Rule '2200m-defect-042-save-sparse' save 6 'A Validate-only location, relief, payment, balance, email, ZIP, or schedule rule fails.' @('Validate-only-fields') $null @('official-hta-runtime#initialValidateBeforeSave:L10479-L10495','official-hta-runtime#validateAll:L4000-L4078') 'incorrect-official-behavior' 'Save checks only date, RDO, name, address, and contact.' 'Use a shared phase-aware validation graph.'
Rule '2200m-defect-043-row-index' 'blur/change' 9 'A dynamic row index reaches 100 or more.' @('Schedule-1-dynamic-families') $null @('official-hta-runtime#getFieldRowNum:L4158-L4169') 'incorrect-official-behavior' 'getFieldRowNum reads only the last two ID characters, so three-digit row indices are truncated.' 'Store row identity as typed data rather than parsing the last two characters.'
WJ (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='Validate and Save stop at the first source-ordered failure; Schedule 1 stops at the first tested row, but the final row is omitted by an off-by-one defect.';rules=$rules})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,[string]$Assessment='verified-correct'){$calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula;rounding='toFixed(2), then formatCurrency where arithmetic occurs.';trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior='Use typed decimals with two-decimal display and preserve source dependency order except where the official behavior is defective.';confidence='high'})}
Calc '2200m-schedule-description-copy' @('frm2200M:txtSched1_descriptionCont{n}') @('frm2200M:txtSched1_description{n}') 'Continuation description = primary description.' populateTable2Desc @() @('official-hta-runtime#populateTable2Desc:L4119-L4125')
Calc '2200m-schedule-total' @('frm2200M:TotalExciseTaxDue') @('frm2200M:txtSched1_LocallyExtractedTaxDueF{1..getTotalRow()}') 'Sum locally extracted Tax Due F for rows 1 through the off-by-one getTotalRow result.' computeTotalTaxDue @() @('official-hta-runtime#computeTotalTaxDue:L4914-L4933') 'incorrect-official-behavior'
Calc '2200m-item16' @('frm2200M:txtPg1P3I16ExciseTaxDue') @('frm2200M:TotalExciseTaxDue') 'Item 16 = TotalExciseTaxDue, or 0.00 when blank.' partThreeComputation @('2200m-schedule-total') @('official-hta-runtime#partThreeComputation:L4847-L4852')
Calc '2200m-item17c' @('frm2200M:txtPg1P3I17CTotal') @('frm2200M:txtPg1P3I17ABalCarriedOver','frm2200M:txtPg1P3I17BCredExciseTax') 'Item 17C = 17A + 17B.' partThreeComputation @() @('official-hta-runtime#partThreeComputation:L4854-L4858')
Calc '2200m-item18' @('frm2200M:txtPg1P3I18NetTaxDue') @('frm2200M:txtPg1P3I16ExciseTaxDue','frm2200M:txtPg1P3I17CTotal') 'Item 18 = Item 16 - Item 17C.' partThreeComputation @('2200m-item16','2200m-item17c') @('official-hta-runtime#partThreeComputation:L4860-L4864')
Calc '2200m-item20' @('frm2200M:txtPg1P3I20TaxStillDue') @('frm2200M:txtPg1P3I18NetTaxDue','frm2200M:txtPg1P3I19PmntOnRtrnPrevFiled') 'Item 20 = Item 18 - Item 19.' partThreeComputation @('2200m-item18') @('official-hta-runtime#partThreeComputation:L4866-L4870')
Calc '2200m-item21d' @('frm2200M:txtPg1P3I21DTotPenalties') @('frm2200M:txtPg1P3I21ASurcharge','frm2200M:txtPg1P3I21BInterest','frm2200M:txtPg1P3I21CCompromise') 'Item 21D = 21A + 21B + 21C.' partThreeComputation @() @('official-hta-runtime#partThreeComputation:L4872-L4877')
Calc '2200m-item22' @('frm2200M:txtPg1P3I22AmountPayable') @('frm2200M:txtPg1P3I20TaxStillDue','frm2200M:txtPg1P3I21DTotPenalties') 'Item 22 = Item 20 + Item 21D.' partThreeComputation @('2200m-item20','2200m-item21d') @('official-hta-runtime#partThreeComputation:L4879-L4882')
Calc '2200m-item23b' @('frm2200M:txtPg1P3I23BPenalties') @('frm2200M:txtPg1P3I21DTotPenalties') 'Item 23B copies Item 21D.' partThreeComputation @('2200m-item21d') @('official-hta-runtime#partThreeComputation:L4884-L4885')
Calc '2200m-item23c' @('frm2200M:txtPg1P3I23CTotPmntMade') @('frm2200M:txtPg1P3I23ATaxPmntDposit','frm2200M:txtPg1P3I23BPenalties') 'Item 23C = 23A + 23B.' partThreeComputation @('2200m-item23b') @('official-hta-runtime#partThreeComputation:L4887-L4890')
Calc '2200m-item24' @('frm2200M:txtPg1P3I24BalToCarryOver') @('frm2200M:txtPg1P3I22AmountPayable','frm2200M:txtPg1P3I23CTotPmntMade') 'Item 24 = Item 22 - Item 23C.' partThreeComputation @('2200m-item22','2200m-item23c') @('official-hta-runtime#partThreeComputation:L4892-L4896')
WJ (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})
$cases=@();$n=0;foreach($r in @($rules|Where-Object{$_.exact_message})){$n++;$cases+=[pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$r.rule_id);phase=$r.phase;mutations=@{synthetic_condition=$r.condition};expected_message=$r.exact_message;expected_behavior=$r.official_behavior;rule_id=$r.rule_id}}
WJ (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
WJ (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=@(@{case_id='item17c';calculation_id='2200m-item17c';a=100.25;b=20.10;official_output=120.35},@{case_id='last-row-omitted';calculation_id='2200m-schedule-total';rows=@(10,20,30);official_output=30;complete_output=60},@{case_id='item24';calculation_id='2200m-item24';item22=1200;item23c=1100;official_output=100})})
$resources=@();foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){$full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src));if(Test-Path -LiteralPath $full){$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}else{$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}};WJ (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})
$deadline='For each place of production, file a separate return and pay upon removal of the mineral products from the place of production.'
$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;phases=@(
    @{phase='edit';official_behavior='January 2018 mineral-products excise return with ten fixed Schedule 1 rows and unbounded add-more rows.';source_refs=@('official-hta-runtime','official-form-pdf','revision-help');confidence='high'},
    @{phase='saved-draft';official_behavior='Save uses a narrow date/RDO/name/address/contact graph before flat serialization.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L10479-L10495');confidence='high'},
    @{phase='validated';official_behavior='Validate checks identity, locations, relief/payment, positive balance, and Schedule 1 except its final row.';source_refs=@('official-hta-runtime#validateAll:L4000-L4078','official-hta-runtime#checkPartVFields:L4365-L4440');confidence='high'},
    @{phase='final-copy';official_behavior='Encryption exists, but the available 139-key artifact is from the predecessor field model and is excluded.';source_refs=@('excluded-legacy-encrypted-field-keys-v796');confidence='high'},
    @{phase='submitted';official_behavior='Online transport was not exercised.';source_refs=@('official-hta-runtime#sendEmail');confidence='medium'}
);transitions=@(
    @{from='edit';action='Save';to='saved-draft';guard='Sparse Save checks pass.';side_effects=@('Writes flat pseudo-XML.');source_refs=@('official-hta-runtime#saveXML')},
    @{from='edit';action='Validate';to='validated';guard='Active Validate graph passes.';side_effects=@('Disables controls.','Enables Final Copy.');source_refs=@('official-hta-runtime#validate')},
    @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables editable controls.');source_refs=@('official-hta-runtime#enableAllControl')},
    @{from='validated';action='Final Copy';to='final-copy';guard='Finalization succeeds.';side_effects=@('Compresses/encrypts the copy.');source_refs=@('official-hta-runtime#saveEncryptedProfile')},
    @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and acceptance succeed.';side_effects=@('Untested online attempt.');source_refs=@('official-hta-runtime#sendEmail')}
);prerequisites=@('Return date','RDO and taxpayer identity','Production/removal locations','Tax-relief choice','Manner of payment','Sufficient prepayment','Schedule 1 row completeness');required_attachments=@();filing_deadlines=@(@{quarter='Q1';due_date_rule=$deadline;source_refs=@('revision-help#L167-L170');confidence='high'},@{quarter='Q2';due_date_rule=$deadline;source_refs=@('revision-help#L167-L170');confidence='high'},@{quarter='Q3';due_date_rule=$deadline;source_refs=@('revision-help#L167-L170');confidence='high'},@{quarter='Q4';due_date_rule=$deadline;source_refs=@('revision-help#L167-L170');confidence='high'})}
WJ (Join-Path $outDir 'workflow.json') $workflow
$bugs=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.';Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'January 2018 ENCS runtime.';Asset 'legacy-runtime-excluded' 'runtime-extracted-hta' $legacyPath 'Predecessor used only for sample discrimination.';Asset 'revision-help' 'official-runtime-help' $helpPath 'January 2018 mineral-products instructions.';Asset 'legacy-help-excluded' 'official-runtime-help' $legacyHelpPath 'Predecessor help excluded.';Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 ENCS form PDF.';Asset 'legacy-encrypted-sample-excluded' 'dummy-profile-encrypted-final-copy' $sample[0].FullName 'Excluded predecessor-shaped dummy final copy; decrypted values omitted.' $redactedSample)
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2200M';revision=$revision;package_version=$packageVersion;status='complete';official_assets=$assets;counts=[ordered]@{concrete_fields=$concrete;runtime_field_families=$families.Count;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=2};artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';excluded_legacy_encrypted_keys='fixtures/excluded-legacy-encrypted-field-keys-v796.json';runtime_controls='fixtures/runtime-control-inventory-v796.json';validation_functions='fixtures/validation-function-inventory-v796.json';calculation_functions='fixtures/calculation-function-inventory-v796.json';resources='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research only; no renderer/release metadata changed.','No decrypted values or email-bearing filenames emitted.','Current static serialized occurrences plus runtime RDO and 17 unbounded Schedule 1 families are preserved.','Available 139-key predecessor artifact is excluded after current/legacy overlap comparison.')}
WJ (Join-Path $outDir 'manifest.json') $manifest
WT (Join-Path $outDir 'README.md') "# BIR Form 2200M - January 2018`n`nRevision-specific Offline eBIRForms rules with $concrete concrete serialized fields and 17 unbounded Schedule 1 families.`n"
WT (Join-Path $outDir 'evidence.md') "# Evidence`n`n- January 2018 runtime: $($expected.hta); predecessor: $($expected.legacy), excluded.`n- Revision help: $($expected.help); predecessor help: $($expected.legacy_help), excluded.`n- Form PDF: $($expected.pdf).`n- Encrypted sample: ciphertext $($expected.cipher), decrypted $($expected.plain), 139 keys, inventory $($expected.inventory); values never emitted.`n- Sample overlap: $($currentOverlap.Count) current IDs versus $($legacyOverlap.Count) predecessor IDs; sample excluded.`n- Inventory: $($serial.Count) static serialized occurrences, one runtime RDO selector, and 17 unbounded families.`n`nAll email-bearing filenames use `#email-redacted#`.`n"
WT (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No revision-matched January 2018 final copy is available; the 139-key predecessor artifact is excluded.`n2. Online submission was not exercised.`n"
WT (Join-Path $outDir 'audit.md') "# Audit`n`n- January 2018 revision separation: pass.`n- Sample discrimination: $($currentOverlap.Count) current overlaps / $($legacyOverlap.Count) predecessor overlaps.`n- Typed inventory: $concrete concrete plus $($families.Count) families.`n- Validations: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count); official defects: $bugs.`n- Full structural/schema audit must run after generation.`n- No renderer/release/capability/commit/push changes.`n"
$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json;$entry=$index.forms|Where-Object{$_.form_id-eq$formId};if($entry){$entry.form_code='2200M';$entry.revision=$revision;$entry.package_version=$packageVersion;$entry.priority=34;$entry.status='complete';$entry.path='forms/2200m-v2018/manifest.json'}else{$index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='2200M';revision=$revision;package_version=$packageVersion;priority=34;status='complete';path='forms/2200m-v2018/manifest.json'}};$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23';WJ $indexPath $index
[pscustomobject]@{form_id=$formId;live_controls=$controls.Count;static_serialized=$serial.Count;unique_static=@($serial.id|Sort-Object -Unique).Count;concrete_fields=$concrete;families=$families.Count;typed_fields=$fields.Count;sample_current_overlap=$currentOverlap.Count;sample_legacy_overlap=$legacyOverlap.Count;validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;confirmed_official_bugs=$bugs;next_form='2200P'}|ConvertTo-Json
