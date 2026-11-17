param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot = 'C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$PdfDir = 'C:\Mac\Home\Downloads\forms\1702EXv2018',
    [string]$SampleDir = 'C:\Mac\Home\Downloads\forms\1702EX'
)

$ErrorActionPreference = 'Stop'
$formId='1702ex-v2018c';$revision='2018-01-01';$packageVersion='7.9.6.0'
$htaPath=Join-Path $ExtractedRoot 'forms\BIR-Form1702EXv2018C.hta'
$predecessorPath=Join-Path $ExtractedRoot 'forms\BIR-Form1702EXv2018.hta'
$legacyPath=Join-Path $ExtractedRoot 'forms\BIR-Form1702EX.hta'
$helpPath=Join-Path $ExtractedRoot 'helpfile\Help1702EX.hta'
$releaseNotesPath=Join-Path $ExtractedRoot 'Release Notes 7.9.5.0.txt'
$pdfPath=Join-Path $PdfDir '1702-EX Jan 2018 ENCS v2.pdf'
$packagePath='C:\eBIRForms\BIRForms.exe'
$outDir=Join-Path $RepoRoot 'rules\forms\1702ex-v2018c';$fixtureDir=Join-Path $outDir 'fixtures'
$expected=@{
    hta='1c2f990e08d6ad02bd488bffdcd1b6cf1666d3c173ed0b5617f5faa0c546a7ed'
    predecessor='6bad84a723c0e4bd8ec3e2488f11946bd9d5117f0c7bfba7231edfb68e71d641'
    legacy='5dbc2357e5c46b82688396cc0eeaf48aeb76e6ceda95e5e38577fe3d0ce91960'
    help='eecc062d879b8417cd0d3d78a70eef8bff80235b99ee82d75bfd410d91c092bf'
    pdf='e2dece405bbf48d4be8f50fc73f124b90e574ddaf79361b2f81bacf8831efcfb'
    cipher='524d4b122e252fb090045a898f1c84245ece2ee250751b3433591c686e488a44'
    plain='d7e2395a67b6a8df20577e8fe742833c1bb5ecc68d754da867d4557f10420c87'
    inventory='734d5d65c0cb102100dd8e940d7980622e3dbd00b66e33ecf8bb277458ba2a86'
    package='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
}
function Write-Json([string]$Path,$Value){[IO.File]::WriteAllText($Path,(($Value|ConvertTo-Json -Depth 60)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false))}
function Write-Utf8([string]$Path,[string]$Value){[IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))}
function Hash-Lines([string[]]$Lines){$s=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($s.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines-join"`n"))))).Replace('-','').ToLowerInvariant()}finally{$s.Dispose()}}
function Attr([string]$Tag,[string]$Name){$m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)));if($m.Success){$m.Groups[2].Value}else{$null}}
function Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding,[string]$Display=''){$i=Get-Item -LiteralPath $Path;[pscustomobject][ordered]@{asset_id=$Id;kind=$Kind;path=if($Display){$Display}else{$Path};sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant();size=$i.Length;revision_binding=$Binding}}

foreach($p in @($htaPath,$predecessorPath,$legacyPath,$helpPath,$releaseNotesPath,$pdfPath,$packagePath)){if(-not(Test-Path -LiteralPath $p)){throw "Missing source: $p"}}
foreach($pair in @(@($htaPath,'hta'),@($predecessorPath,'predecessor'),@($legacyPath,'legacy'),@($helpPath,'help'),@($pdfPath,'pdf'),@($packagePath,'package'))){
    if((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected[$pair[1]]){throw "Hash changed: $($pair[0])"}
}
$sample=@(Get-ChildItem -LiteralPath $SampleDir -File|Where-Object{$_.Extension-eq'.xml'})
if($sample.Count-ne1){throw "Expected one encrypted sample; found $($sample.Count)."}
if((Get-FileHash -LiteralPath $sample[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected.cipher){throw 'Encrypted sample hash changed.'}
$pdfBytes=[IO.File]::ReadAllBytes($pdfPath);if([Text.Encoding]::ASCII.GetString($pdfBytes[0..4])-ne'%PDF-'){throw 'PDF magic mismatch.'}
$hta=[IO.File]::ReadAllText($htaPath);$help=[IO.File]::ReadAllText($helpPath);$release=[IO.File]::ReadAllText($releaseNotesPath)
if($hta-notmatch'(?i)var\s+formType\s*=\s*["'']1702EXv2018C["'']' -or $hta-notmatch'(?i)January\s+2018'){throw 'CREATE runtime binding changed.'}
if($release-notmatch'(?is)CREATE\s+LAW.*1702EXv2018C'){throw 'Release-note CREATE binding changed.'}
if($help-notmatch'(?i)June\s+2013'){throw 'Legacy-help binding changed.'}
New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null

$keyTool=Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson=&$keyTool -SourcePath $sample[0].FullName -RedactedSourcePath (Join-Path $SampleDir '1702EX-final-copy-#email-redacted#.xml') -FormId '1702ex-v2013' `
    -ExpectedCiphertextSha256 $expected.cipher -ExpectedDecryptedSha256 $expected.plain -ExpectedFieldCount 484 -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit=$keyJson|ConvertFrom-Json;$legacyKeys=@($keyAudit.keys)
if($legacyKeys.Count-ne484){throw 'Keys-only extractor count changed.'}
Write-Utf8 (Join-Path $fixtureDir 'excluded-legacy-encrypted-field-keys-v796.json') ($keyJson-join[Environment]::NewLine)

$legacyAllIds=@([regex]::Matches([IO.File]::ReadAllText($legacyPath),'(?i)\bid\s*=\s*(["''])(?<id>.*?)\1')|ForEach-Object{$_.Groups['id'].Value}|Where-Object{$_}|Sort-Object -Unique)
$createAllIds=@([regex]::Matches($hta,'(?i)\bid\s*=\s*(["''])(?<id>.*?)\1')|ForEach-Object{$_.Groups['id'].Value}|Where-Object{$_}|Sort-Object -Unique)
$legacyOverlap=@($legacyKeys|Where-Object{$legacyAllIds-contains$_})
$createOverlap=@($legacyKeys|Where-Object{$createAllIds-contains$_})
if($legacyOverlap.Count-ne484-or$createOverlap.Count-ne46){throw "Encrypted-sample revision discrimination changed: legacy=$($legacyOverlap.Count), CREATE=$($createOverlap.Count)."}

$fm=[regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>');if(-not$fm.Success){throw 'frmMain missing.'}
$body=$fm.Groups['body'].Value;$offset=$fm.Groups['body'].Index
$excluded=@(@([regex]::Matches($body,'(?is)<script\b.*?</script>'))+@([regex]::Matches($body,'(?is)<!--.*?-->')))
$controls=[Collections.Generic.List[object]]::new();$ordinal=0
foreach($m in [regex]::Matches($body,'(?is)<(input|select|textarea|button)\b[^>]*>')){
    $skip=$false;foreach($r in $excluded){if($m.Index-ge$r.Index-and$m.Index-lt($r.Index+$r.Length)){$skip=$true;break}};if($skip){continue}
    $ordinal++;$tag=$m.Value;$element=$m.Groups[1].Value.ToLowerInvariant();$kind=if($element-eq'input'){Attr $tag 'type'}else{$element};if(-not$kind){$kind='text'}
    $controls.Add([pscustomobject][ordered]@{ordinal=$ordinal;id=Attr $tag 'id';name=Attr $tag 'name';element=$element;control_kind=$kind.ToLowerInvariant();source_line=1+[regex]::Matches($hta.Substring(0,$offset+$m.Index),"`n").Count;value=Attr $tag 'value';maxlength=Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)';readonly=$tag-match'(?i)\breadonly(?:\s*=|\s|>)'})
}
$serial=@($controls|Where-Object{$_.control_kind-in@('text','select','select-one','textarea','radio','checkbox')})
$staticIds=@($serial.id|Where-Object{$_}|Sort-Object -Unique)
if($controls.Count-ne246-or$staticIds.Count-ne186){throw "Expected 246 live controls/186 source-serializable IDs; found $($controls.Count)/$($staticIds.Count)."}
$byId=@{};foreach($c in $controls){if($c.id -and -not $byId.ContainsKey($c.id)){$byId[$c.id]=$c}}

$active=[regex]::Replace($hta,'(?is)<!--.*?-->','');$active=[regex]::Replace($active,'(?is)/\*.*?\*/','');$active=[regex]::Replace($active,'(?m)^\s*//.*$','')
$familyMap=[ordered]@{};$familyPattern='(?is)(?<prefix>frm1702EX:[A-Za-z0-9:_-]+)["'']?\s*\+\s*\(\s*(?:i|x)\s*\+\s*1\s*\)\s*(?:\+\s*["''](?<suffix>[A-Za-z0-9:_-]+))?'
foreach($m in [regex]::Matches($active,$familyPattern)){
    $pattern=$m.Groups['prefix'].Value+'{N>=1}'+$m.Groups['suffix'].Value
    if(-not$familyMap.Contains($pattern)){$familyMap[$pattern]=[pscustomobject][ordered]@{field_pattern=$pattern;index_origin=1;source_line=1+[regex]::Matches($active.Substring(0,$m.Index),"`n").Count}}
}
$families=@($familyMap.Values);if($families.Count-ne11){throw "Expected 11 active runtime families; found $($families.Count)."}

$required=@('frm1702EX:txtPg1I2YearEnd','frm1702EX:rdoPg1Pt1I7RDO','frm1702EX:txtPg1Pt1I8RegisteredName','frm1702EX:txtPg1Pt1I9RegisteredAddress','frm1702EX:txPg1I9ZipCode','frm1702EX:txtPg1Pt1I10DateofIncorporation','frm1702EX:txtPg1Pt1I11ContactNumber','frm1702EX:txtPg1Pt1I12Email','frm1702EX:txtPg1Pt1I14LegalBasis','frm1702EX:txtPg1Pt1I15Investment','frm1702EX:txtPg1Pt1I16RegisteredActivity','frm1702EX:txtPg1Pt1I17EffectivityFrom','frm1702EX:txtPg1Pt1I17EffectivityTo')
function Meta([string]$Key,$Control,[bool]$Family){
    $page=$null;if($Key-match'(?i)Pg(?<p>\d+)'){$page=[int]$Matches.p}
    $item=$null;$ims=@([regex]::Matches($Key,'(?i)(?:Itm?|I)(?<i>\d+[a-z]?)'));if($ims.Count){$item=$ims[-1].Groups['i'].Value}
    $logical='string';$enum=[object[]]@();$norm=[string[]]@()
    if(($Control-and$Control.control_kind-in@('radio','checkbox'))-or$Key-match'(?i):(rdo|chk)'){$logical='boolean';$enum=[object[]]@('true','false')}
    elseif($Key-match'(?i)Email$'){$logical='email-string'}
    elseif($Key-match'(?i)(Date|Effectivity|Issue|Expiry)'){$logical='date-string-mm-dd-yyyy';$norm=[string[]]@('MM/DD/YYYY')}
    elseif($Key-match'(?i)(TIN|RDO|PSIC|ATC|ZIP)'){$logical='code'}
    elseif($Key-match'(?i)Year'){$logical='integer-year'}
    elseif($Key-match'(?i)^frm1702EX:txtPg[1-9]' -and $Key-notmatch'(?i)(Desc|Name|Legal|Address|Date|TIN|Title|Email|MainLine|Activity|Agency|PSIC|ATC|Mode|OCT|CAR|Shares)'){$logical='whole-peso-amount';$norm=[string[]]@('NumWithComma','formatCurrency','toFixed(0)')}
    $computed=$false;if($Control-and($Control.disabled-or$Control.readonly)-and$logical-eq'whole-peso-amount'){$computed=$true}
    if($Key-match'(?i)(Total|NetTaxable|TaxDue|TaxWithheld|OrdinaryAllowable|SpecialAllowable|OptionalStandard|Overpmt|AmountPayable|SubTotal)$'){$computed=$true}
    $status=if($required-contains$Key){'required'}elseif($computed){'computed'}else{'optional'}
    if($Family){$status='conditional';$computed=$false}
    if($Key-match'^(txtFinalFlag|txtEnroll|ebirOnline|driveSelect|Pg\d+.*PopLength)'){$status='hidden'}
    $constraints=[ordered]@{};if($Control-and$Control.maxlength-match'^\d+$'){$constraints.max_length=[int]$Control.maxlength};if($logical-eq'whole-peso-amount'){$constraints.precision=0;$constraints.sign='signed; negative values supported'}
    [pscustomobject]@{page=$page;item=$item;logical=$logical;enum=$enum;norm=$norm;computed=$computed;status=$status;constraints=[pscustomobject]$constraints}
}
$fields=[Collections.Generic.List[object]]::new()
foreach($key in $staticIds){
    $control=$byId[$key];$meta=Meta $key $control $false
    $refs=@('official-hta-runtime#saveXML:L7242-L7557',"official-hta-runtime#control:L$($control.source_line)")
    $fields.Add([pscustomobject][ordered]@{field_key=$key;serialized_key=$key;serialized_occurrence=1;label=if($key-like'frm1702EX:*'){$key.Substring(10)}else{$key};page=$meta.page;item_number=$meta.item;control_kind=$control.control_kind;storage_type='string';logical_type=$meta.logical;required=$meta.status;required_when=$null;enabled_when=$null;visible_when=$null;default_value=$control.value;empty_representation='';constraints=$meta.constraints;enum_values=$meta.enum;normalization=$meta.norm;computed=$meta.computed;calculation_id=if($meta.computed){'See calculations.json'}else{$null};source_refs=$refs;confidence='high';notes=@('Source-derived from the hash-pinned CREATE runtime; no revision-matched encrypted final copy was available.')})
}
foreach($family in $families){
    $meta=Meta $family.field_pattern $null $true
    $fields.Add([pscustomobject][ordered]@{field_key=$family.field_pattern;serialized_key=$null;serialized_occurrence=$null;label="Runtime-indexed family $($family.field_pattern)";page=$meta.page;item_number=$meta.item;control_kind='runtime-indexed-family';storage_type='string';logical_type=$meta.logical;required='conditional';required_when='A corresponding add-more popup row N exists.';enabled_when='The row exists.';visible_when='The row exists.';default_value=$null;empty_representation='';constraints=[pscustomobject]@{index='one-based, source-unbounded'};enum_values=@();normalization=$meta.norm;computed=$false;calculation_id=$null;source_refs=@("official-hta-runtime#dynamic-id:L$($family.source_line)",'official-hta-runtime#popup-serialization');confidence='high';notes=@('Source-derived unbounded family; no revision-matched final-copy snapshot was available.')})
}
if($fields.Count-ne197){throw "Expected 197 fields; found $($fields.Count)."}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=186;inventory_sha256=Hash-Lines @($fields.field_key|Sort-Object);fields=$fields})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta;live_control_count=$controls.Count;static_serialized_id_count=$staticIds.Count;revision_matched_final_copy_key_count=0;excluded_legacy_sample_key_count=$legacyKeys.Count;excluded_legacy_sample_overlap_with_legacy_runtime=$legacyOverlap.Count;excluded_legacy_sample_overlap_with_create_runtime=$createOverlap.Count;active_runtime_family_count=$families.Count;controls=$controls;dynamic_families=$families})
$functionTool=Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1702EX:' -NamePattern '(?i)valid|check|mandatory|save|enable|disable|date|email|submit|final')-join[Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$functionTool -HtaPath $htaPath -ControlPrefix 'frm1702EX:' -NamePattern '(?i)compute|amount|sum|format|tax|relief')-join[Environment]::NewLine)

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',[string]$Recommended='Retain as a structured revision-aware error.'){
    $rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys;accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.';exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()})
}
Rule '1702ex-validate-001-year' validate 1 'Year End is blank.' @('frm1702EX:txtPg1I2YearEnd') 'Please enter a valid Year End on Page 1 Item 2' @('official-hta-runtime#validateAll:L4721-L4724')
Rule '1702ex-validate-002-period' validate 2 'Calendar/fiscal/short-period rules fail, including a year before 2018 or disallowed current/future period.' @('frm1702EX:rdoPg1I1Calendar','frm1702EX:rdoPg1I1Fiscal','frm1702EX:ddlPg1I2Date','frm1702EX:txtPg1I2YearEnd','frm1702EX:rdoPg1I4ShortPeriodYes','frm1702EX:rdoPg1I4ShortPeriodNo') 'Period-specific message from checkFilingYear.' @('official-hta-runtime#checkFilingYear:L5294-L5438')
Rule '1702ex-validate-003-atc' validate 3 'Neither IC021 nor IC011 ATC is selected.' @('frm1702EX:rdoPg1I5ATCR1C1','frm1702EX:rdoPg1I5ATCR1C2') 'Please select an ATC on Page 1 Item 5' @('official-hta-runtime#Check_rdoPg1I5ATC:L5460-L5477')
Rule '1702ex-validate-004-rdo' validate 4 'RDO value is 000.' @('frm1702EX:rdoPg1Pt1I7RDO') 'Please enter a valid RDO Code on Page 1 Item 7.' @('official-hta-runtime#validateAll:L4726-L4728')
Rule '1702ex-validate-005-name' validate 5 'Registered name is blank.' @('frm1702EX:txtPg1Pt1I8RegisteredName') 'Please enter a valid name on Page 1 Item 8' @('official-hta-runtime#validateAll:L4729')
Rule '1702ex-validate-006-address' validate 6 'Registered address is blank.' @('frm1702EX:txtPg1Pt1I9RegisteredAddress') 'Please enter a valid Registered Address on Page 1 Item 9.' @('official-hta-runtime#validateAll:L4730')
Rule '1702ex-validate-007-zip' validate 7 'Zip code is blank.' @('frm1702EX:txPg1I9ZipCode') 'Please enter a valid Zip Code on Page 1 Item 9A.' @('official-hta-runtime#validateAll:L4731')
Rule '1702ex-validate-008-incorporation' validate 8 'Date of incorporation is blank.' @('frm1702EX:txtPg1Pt1I10DateofIncorporation') 'Please enter a valid Date of Incorporation on Page 1 Item 10' @('official-hta-runtime#validateAll:L4732')
Rule '1702ex-validate-009-incorporation-after-filing' validate 9 'Date of incorporation is after the filing year/month.' @('frm1702EX:txtPg1Pt1I10DateofIncorporation','frm1702EX:ddlPg1I2Date','frm1702EX:txtPg1I2YearEnd') 'The date of incorporation should not be more than the Filing date ' @('official-hta-runtime#checkDateOfIncorporation:L4769-L4787')
Rule '1702ex-validate-010-contact' validate 10 'Contact number is blank.' @('frm1702EX:txtPg1Pt1I11ContactNumber') 'Please enter a valid Contact Number on Page 1 Item 11' @('official-hta-runtime#validateAll:L4734')
Rule '1702ex-validate-011-email-required' validate 11 'Email is blank.' @('frm1702EX:txtPg1Pt1I12Email') 'Please enter a valid e-mail address on page 1 item 12' @('official-hta-runtime#validateAll:L4735')
Rule '1702ex-validate-012-email-format' validate 12 'Email fails the source regex.' @('frm1702EX:txtPg1Pt1I12Email') 'Please enter a valid e-mail address on page 1 item 12' @('official-hta-runtime#validateEmail:L5047-L5057')
Rule '1702ex-validate-013-deduction' validate 13 'Neither itemized nor optional standard deduction is selected.' @('frm1702EX:rdoPg1Pt1I13MethodOfDeducItemized','frm1702EX:rdoPg1Pt1I13MethodOfDeducOptional') 'Please select a Method of Deduction in page 1 Item 13.' @('official-hta-runtime#check_MethodOfDeduction:L5479-L5487')
Rule '1702ex-validate-014-relief-basis' validate 14 'Legal basis is blank.' @('frm1702EX:txtPg1Pt1I14LegalBasis') 'Please enter a value Legal Bases of Tax Relief/Exemption on Page 1 Item 14' @('official-hta-runtime#validateAll:L4739')
Rule '1702ex-validate-015-ipa' validate 15 'IPA/government agency is blank.' @('frm1702EX:txtPg1Pt1I15Investment') 'Please enter a value for Investment Promotion Agency (IPA) Government Agency on Page 1 Item 15' @('official-hta-runtime#validateAll:L4740')
Rule '1702ex-validate-016-activity' validate 16 'Registered activity/program is blank.' @('frm1702EX:txtPg1Pt1I16RegisteredActivity') 'Please enter a value for Registered Activity/Program (Reg. No.) on Page 1 Item 16' @('official-hta-runtime#validateAll:L4741')
Rule '1702ex-validate-017-effectivity-from' validate 17 'Effectivity From is blank.' @('frm1702EX:txtPg1Pt1I17EffectivityFrom') 'Please enter a valid date for Effectivity Date of Tax Relief/Exemption(FROM) on Page 1 Item 17' @('official-hta-runtime#validateAll:L4742')
Rule '1702ex-validate-018-effectivity-to' validate 18 'Effectivity To is blank.' @('frm1702EX:txtPg1Pt1I17EffectivityTo') 'Please enter a valid date for Effectivity Date of Tax Relief/Exemption(TO) on Page 1 Item 17' @('official-hta-runtime#validateAll:L4743')
Rule '1702ex-validate-019-effectivity-order' validate 19 'Effectivity From is after Effectivity To.' @('frm1702EX:txtPg1Pt1I17EffectivityFrom','frm1702EX:txtPg1Pt1I17EffectivityTo') 'Date range message from DateCompare_Page1_Item17.' @('official-hta-runtime#DateCompare_Page1_Item17:L4546-L4568')
Rule '1702ex-validate-020-overpayment' validate 20 'Item 20 is negative and no overpayment disposition is selected.' @('frm1702EX:txtPg1Pt2I20TotalOverpmt','frm1702EX:rdoPg1OverpaymentRefund','frm1702EX:rdoPg1OverpaymentTCC','frm1702EX:rdoPg1OverpaymentCarryOver') 'Please select an Overpayment option in Page 1 after Item 22.' @('official-hta-runtime#check_Overpayment:L5491-L5500')
Rule '1702ex-validate-021-reconciliation' validate 21 'Page 2 Item 39 string value differs from Page 3 Schedule 3 Item 10 string value.' @('frm1702EX:txtPg2Pt4I39NetTaxable','frm1702EX:txtPg3Pt6S3I10NetTaxableIncome') 'Page 2 Part IV Item 39 should be equal to Page 3 Schedule 3 Item 10.' @('official-hta-runtime#validateNetTaxInc:L5503-L5510') 'official-bug-compatible' 'Direct formatted-string comparison can reject numerically equal representations.' 'Compare normalized decimal values.'
Rule '1702ex-validate-022-description-pairs' validate 22 'An amount/description pair in Part IV/V or Page 3 has only one side populated.' @('schedule-description-and-amount-controls') 'Dynamic pair-specific message.' @('official-hta-runtime#validateAmountDescription:L5096-L5124','official-hta-runtime#validateAmountDescription_pt2:L5125-L5292')
Rule '1702ex-validate-023-item53' validate 23 'Item 36 is positive while Item 53 is blank or zero.' @('frm1702EX:txtPg2Pt4I36SpecialAllowable','frm1702EX:txtPg2Pt5I53SpecialAllowableItemizedDeduc') 'Please provide value on Page 2 Item 53.' @('official-hta-runtime#validateAll:L4757-L4760')
Rule '1702ex-validate-024-item52' validate 24 'Item 39 is positive while Item 52 is zero.' @('frm1702EX:txtPg2Pt4I39NetTaxable','frm1702EX:txtPg2Pt5I52RegularIncomeOtherwiseDue') 'Please provide value on Page 2 Item 52.' @('official-hta-runtime#validateAll:L4762-L4765')
Rule '1702ex-validate-025-unused-schedule8' validate $null 'Schedule 8 contains incomplete TIN/name/contribution/percent rows.' @('frm1702EX:txtPg6S8I{index}Col1Name','frm1702EX:txtPg6S8I{index}Col2TIN{segment}','frm1702EX:txtPg6S8I{index}Col3CapContri','frm1702EX:txtPg6S8I{index}Col4PTotal') $null @('official-hta-runtime#validateSchedule8TIN:L4790-L4827','official-hta-runtime#validateAll:L4721-L4767') 'incorrect-official-behavior' 'A detailed validator exists but validateAll never calls it.' 'Invoke the row validator during Validate.'
Rule '1702ex-validate-026-unused-ctc-reg' validate $null 'Neither CTC nor SEC registration is selected.' @('frm1702EX:rdoPg1CTC','frm1702EX:rdoPg1Reg') 'Please select CTC or SEC Reg on Page 1 Item 23' @('official-hta-runtime#Check_CTCorReg:L5441-L5457','official-hta-runtime#validateAll:L4721-L4767') 'incorrect-official-behavior' 'The function is defined but never called.' 'Invoke it during Validate if Item 23 remains required.'
Rule '1702ex-validate-027-success' validate 25 'All invoked checks pass.' @() 'Validation successful. Click on Edit if you wish to modify your entries.' @('official-hta-runtime#validate:L7784-L7806')
Rule '1702ex-save-001-rdo' save 1 'RDO value is 000.' @('frm1702EX:rdoPg1Pt1I7RDO') 'Please enter a valid RDO Code on Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L7818-L7822')
Rule '1702ex-save-002-name' save 2 'Registered name is blank.' @('frm1702EX:txtPg1Pt1I8RegisteredName') 'Please enter a valid name on Page 1 Item 8' @('official-hta-runtime#initialValidateBeforeSave:L7822')
Rule '1702ex-save-003-address' save 3 'Registered address is blank.' @('frm1702EX:txtPg1Pt1I9RegisteredAddress') 'Please enter a valid Registered Address on Page 1 Item 9.' @('official-hta-runtime#initialValidateBeforeSave:L7823')
Rule '1702ex-save-004-contact' save 4 'Contact number is blank.' @('frm1702EX:txtPg1Pt1I11ContactNumber') 'Please enter a valid Contact Number on Page 1 Item 11' @('official-hta-runtime#initialValidateBeforeSave:L7824')
Rule '1702ex-save-005-sparse' save 5 'Any other Validate rule fails.' @() $null @('official-hta-runtime#initialValidateBeforeSave:L7818-L7828','official-hta-runtime#validateAll:L4721-L4767') 'incorrect-official-behavior' 'Save ignores period, ATC, email, deduction, relief, reconciliation, and schedule failures.' 'Use a shared validation graph with explicit phase exceptions.'
Rule '1702ex-date-future-return' 'blur/change' 1 'A valid date is after today.' @('date-controls') 'This date cannot be a future date.' @('official-hta-runtime#validateDate:L4872-L4935') 'incorrect-official-behavior' 'The field clears but isValid remains true.' 'Set false and return false after clearing.'
Rule '1702ex-ctc-reg-message' 'blur/change' 2 'SEC registration date is more than 50 years old.' @('frm1702EX:txtPg1Pt3I24DateofIssue') 'CTC year should not be less than {year} for Page 1 Item 24' @('official-hta-runtime#validateCTCDate:L5060-L5093') 'incorrect-official-behavior' 'The SEC-registration branch still labels its message CTC year.' 'Name the selected certificate type correctly.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='Validate and Save stop at the first invoked source-ordered failure; two defined validators are unreachable.';rules=$rules})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Recommended='Implement with typed decimals and the official whole-peso rounding order.'){
    $calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula;rounding='Most monetary results use toFixed(0) before formatCurrency; popup totals use source-specific formatting.';trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior=$Recommended;confidence='high'})
}
Calc '1702ex-popup-s1-17' @('frm1702EX:txtPg3Pt6S1I17Others') @('frm1702EX:txtPg3Pt6S1I17_{N>=1}Col2') 'Sum Schedule 1 Item 17 popup amounts.' Sum_Pg3Pt6S1I17 @() @('official-hta-runtime#Sum_Pg3Pt6S1I17:L3561-L3579')
Calc '1702ex-popup-s2-4' @('frm1702EX:txtPg3Pt6S2I4SpecialAllowable') @('frm1702EX:txtPg3Pt6S2I4_{N>=1}Col3') 'Sum Schedule 2 Item 4 popup amounts.' Sum_Pg3Pt6S2I4 @() @('official-hta-runtime#Sum_Pg3Pt6S2I4:L3727-L3744')
Calc '1702ex-popup-s3-3' @('frm1702EX:txtPg3Pt6S3I3OtherIncome') @('frm1702EX:txtPg3Pt6S3I3_{N>=1}Col2') 'Sum Schedule 3 Item 3 popup amounts.' Sum_Pg3Pt6S3I3 @() @('official-hta-runtime#Sum_Pg3Pt6S3I3:L3860-L3878')
Calc '1702ex-popup-s3-6' @('frm1702EX:txtPg3Pt6S3I6NonDeductible') @('frm1702EX:txtPg3Pt6S3I6_{N>=1}Col2') 'Sum Schedule 3 Item 6 popup amounts.' Sum_Pg3Pt6S3I6 @() @('official-hta-runtime#Sum_Pg3Pt6S3I6:L3993-L4012')
Calc '1702ex-popup-s3-8' @('frm1702EX:txtPg3Pt6S3I8SpecialDeductions') @('frm1702EX:txtPg3Pt6S3I8_{N>=1}Col2') 'Sum Schedule 3 Item 8 popup amounts.' Sum_Pg3Pt6S3I8 @() @('official-hta-runtime#Sum_Pg3Pt6S3I8:L4127-L4141')
Calc '1702ex-net-sales' @('frm1702EX:txtPg2Pt4I30NetSalesReceiptsRevFees') @('frm1702EX:txtPg2Pt4I28SalesReceiptsRevFees','frm1702EX:txtPg2Pt4I29SalesRetAllowanceDisc') 'Item 30 = Item 28 - Item 29.' FormatAmount @() @('official-hta-runtime#FormatAmount:L5528-L5535')
Calc '1702ex-total-gross' @('frm1702EX:txtPg2Pt4I34TotalGross') @('frm1702EX:txtPg2Pt4I30NetSalesReceiptsRevFees','frm1702EX:txtPg2Pt4I31CostSales','frm1702EX:txtPg2Pt4I32GrossIncome','frm1702EX:txtPg2Pt4I33AddOther') 'Item 34 aggregates net sales less cost plus other gross income components.' FormatAmount @('1702ex-net-sales') @('official-hta-runtime#FormatAmount:L5528-L5628')
Calc '1702ex-itemized-total' @('frm1702EX:txtPg2Pt4I37TotalItemized') @('frm1702EX:txtPg3Pt6S1I18TotOrdinaryAllowableItemDeduc','frm1702EX:txtPg3Pt6S2I5TotSpecialAllowedItemDeduc') 'Items 35/36 copy schedule totals; Item 37 is their sum.' amountComputation @('1702ex-popup-s1-17','1702ex-popup-s2-4') @('official-hta-runtime#amountComputation:L5630-L5649')
Calc '1702ex-osd' @('frm1702EX:txtPg2Pt4I38OptionalStandardDeduc') @('frm1702EX:txtPg2Pt4I34TotalGross') 'When OSD is selected, Item 38 = Item 34 × 40%.' amountComputation @('1702ex-total-gross') @('official-hta-runtime#amountComputation:L5651-L5661')
Calc '1702ex-net-taxable' @('frm1702EX:txtPg2Pt4I39NetTaxable') @('frm1702EX:txtPg2Pt4I34TotalGross','frm1702EX:txtPg2Pt4I37TotalItemized','frm1702EX:txtPg2Pt4I38OptionalStandardDeduc') 'Net taxable = total gross less the selected deduction method.' amountComputation @('1702ex-itemized-total','1702ex-osd') @('official-hta-runtime#amountComputation:L5639-L5666')
Calc '1702ex-tax-due' @('frm1702EX:txtPg2Pt4I41TaxDue') @('frm1702EX:txtPg2Pt4I39NetTaxable','frm1702EX:txtPg2Pt4I40TaxRate') 'Item 41 = Item 39 × Item 40 / 100.' amountComputation @('1702ex-net-taxable') @('official-hta-runtime#amountComputation:L5668-L5672')
Calc '1702ex-overpayment' @('frm1702EX:txtPg2Pt4I51TotalOverpayment') @('frm1702EX:txtPg2Pt4I41TaxDue','frm1702EX:txtPg2Pt4I50TotalTaxCrPmt') 'Item 51 = Item 41 - Item 50.' amountComputation @('1702ex-tax-due') @('official-hta-runtime#amountComputation:L5673-L5677')
Calc '1702ex-relief-total' @('frm1702EX:txtPg2Pt5I54TotalTaxReliefAvailment') @('frm1702EX:txtPg2Pt5I52RegularIncomeOtherwiseDue','frm1702EX:txtPg2Pt5I53SpecialAllowableItemizedDeduc') 'Item 54 = Item 52 + Item 53.' compTaxReliefAvail @() @('official-hta-runtime#compTaxReliefAvail:L5725-L5736')
Calc '1702ex-page1-copies' @('frm1702EX:txtPg1Pt2I18TaxDue','frm1702EX:txtPg1Pt2I19TotalTaxCrPmt','frm1702EX:txtPg1Pt2I20TotalOverpmt') @('frm1702EX:txtPg2Pt4I41TaxDue','frm1702EX:txtPg2Pt4I50TotalTaxCrPmt','frm1702EX:txtPg2Pt4I51TotalOverpayment') 'Copy Items 41, 50, and 51 to Page 1 Items 18, 19, and 20.' amountComputation @('1702ex-tax-due','1702ex-overpayment') @('official-hta-runtime#amountComputation:L5681-L5692')
Calc '1702ex-total-payable' @('frm1702EX:txtPg1Pt2I22TotalAmtPayable') @('frm1702EX:txtPg1Pt2I20TotalOverpmt','frm1702EX:txtPg1Pt2I21PenaltyCompromise') 'If Item 20 is nonnegative, add penalties; if negative with positive penalties, payable is penalties; otherwise payable retains the negative amount.' amountComputation @('1702ex-page1-copies') @('official-hta-runtime#amountComputation:L5694-L5708') 'incorrect-official-behavior' 'Do not expose a negative amount payable; represent overpayment separately and payable as applicable positive penalties.'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})
$cases=@();$n=0;foreach($r in @($rules|Where-Object{$_.exact_message})){$n++;$cases+=[pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$r.rule_id);phase=$r.phase;mutations=@{synthetic_condition=$r.condition};expected_message=$r.exact_message;expected_behavior=$r.official_behavior;rule_id=$r.rule_id}}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=@(
    @{case_id='osd-40-percent';calculation_id='1702ex-osd';gross=1000;official_output=400},
    @{case_id='tax-due';calculation_id='1702ex-tax-due';net_taxable=1000;rate=20;official_output=200},
    @{case_id='overpayment-with-penalty';calculation_id='1702ex-total-payable';overpayment=-100;penalty=25;official_output=25},
    @{case_id='negative-payable-defect';calculation_id='1702ex-total-payable';overpayment=-100;penalty=0;official_output=-100;recommended_output=0}
)})
$resources=@();foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){$full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src));if(Test-Path -LiteralPath $full){$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}else{$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision
phases=@(
    @{phase='edit';official_behavior='CREATE-law January 2018 corporate exempt return with nine pages and source-unbounded popup schedule rows.';source_refs=@('official-hta-runtime','official-form-pdf','package-release-notes#create-law');confidence='high'},
    @{phase='saved-draft';official_behavior='Save checks only RDO, name, address, and contact number before serializing current static and runtime-generated controls.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L7818-L7828','official-hta-runtime#saveXML:L7242-L7557');confidence='high'},
    @{phase='validated';official_behavior='Validate invokes validateAll, but defined Schedule 8 and CTC/SEC selection validators are unreachable.';source_refs=@('official-hta-runtime#validateAll:L4721-L4767','official-hta-runtime#validate:L7784-L7816');confidence='high'},
    @{phase='final-copy';official_behavior='The CREATE runtime defines final-copy encryption and 11 source-unbounded popup families, but no revision-matched final-copy sample is available.';source_refs=@('official-hta-runtime#saveXML:L7242-L7557');confidence='medium'},
    @{phase='submitted';official_behavior='Online/EFPS transports exist but were not exercised.';source_refs=@('official-hta-runtime#sendEmail','official-hta-runtime#submitToEFPS');confidence='medium'}
)
transitions=@(
    @{from='edit';action='Save';to='saved-draft';guard='Four narrow Save checks and file-version guards pass.';side_effects=@('Writes flat pseudo-XML.','Serializes runtime popup rows and UI-state keys.');source_refs=@('official-hta-runtime#saveXML:L7242-L7557')},
    @{from='edit';action='Validate';to='validated';guard='All invoked validateAll checks pass.';side_effects=@('Captures disabled text state.','Disables controls.','Enables print/final-copy/transport actions.');source_refs=@('official-hta-runtime#validate:L7784-L7806')},
    @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables controls subject to deduction and period conditions.');source_refs=@('official-hta-runtime#enableAllControl:L8291-L8306')},
    @{from='validated';action='Final Copy';to='final-copy';guard='Finalization/version flow succeeds.';side_effects=@('Encrypts/compresses the final copy.');source_refs=@('official-hta-runtime#saveXML:L7242-L7557')},
    @{from='final-copy';action='Transport';to='submitted';guard='Connectivity and remote acceptance succeed.';side_effects=@('Online/EFPS attempt; untested.');source_refs=@('official-hta-runtime#sendEmail','official-hta-runtime#submitToEFPS')}
)
prerequisites=@('Return period and ATC','RDO and corporation identity','Contact and valid email','Deduction method','Tax-relief basis/agency/activity/effectivity','Reconciled Part IV and Schedule 3 amounts','Overpayment disposition when applicable')
required_attachments=@(
    @{attachment_id='financial-statements';label='Audited financial statements and schedules required by the official instructions.';required_when='Applicable annual return filing.';official_ui_enforcement='Not fully enforced by local Validate.';source_refs=@('legacy-help-runtime#required-attachments');confidence='medium'},
    @{attachment_id='tax-relief-support';label='Proof and schedules supporting exemption or tax-relief availment.';required_when='Tax relief/exemption is claimed.';official_ui_enforcement='Local UI requires descriptive basis fields but not attachment presence.';source_refs=@('legacy-help-runtime#required-attachments','official-hta-runtime#validateAll:L4739-L4765');confidence='medium'}
)
filing_deadlines=@(
    @{quarter='Q1';due_date_rule='Annual return; revision-matched deadline text is not pinned because the packaged help is June 2013.';source_refs=@('legacy-help-runtime');confidence='low'},
    @{quarter='Q2';due_date_rule='Annual return; revision-matched deadline text is not pinned because the packaged help is June 2013.';source_refs=@('legacy-help-runtime');confidence='low'},
    @{quarter='Q3';due_date_rule='Annual return; revision-matched deadline text is not pinned because the packaged help is June 2013.';source_refs=@('legacy-help-runtime');confidence='low'},
    @{quarter='Q4';due_date_rule='Annual return; revision-matched deadline text is not pinned because the packaged help is June 2013.';source_refs=@('legacy-help-runtime');confidence='low'}
)}
Write-Json (Join-Path $outDir 'workflow.json') $workflow
$bugs=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'formType 1702EXv2018C; CREATE-law variant.'
    Asset 'pre-create-hta-excluded' 'runtime-extracted-hta' $predecessorPath 'January 2018 predecessor; excluded from CREATE calculations.'
    Asset 'legacy-hta-excluded' 'runtime-extracted-hta' $legacyPath 'June 2013 predecessor; excluded.'
    Asset 'legacy-help-runtime' 'official-runtime-help' $helpPath 'Packaged help is June 2013 and is not revision-matched.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 ENCS v2 printed form used by both 2018 runtime variants.'
    Asset 'package-release-notes' 'official-package-release-notes' $releaseNotesPath 'v7.9 explicitly introduces 1702EXv2018C for CREATE law.'
    Asset 'legacy-encrypted-sample-excluded' 'dummy-profile-encrypted-final-copy' $sample[0].FullName 'Excluded from CREATE inventory: all 484 keys match the June 2013 runtime, while only 46 match the CREATE runtime.' (Join-Path $SampleDir '1702EX-final-copy-#email-redacted#.xml')
)
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='1702EX';revision=$revision;package_version=$packageVersion;status='complete';official_assets=$assets;counts=[ordered]@{concrete_fields=186;runtime_field_families=11;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=3};artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';excluded_legacy_encrypted_keys='fixtures/excluded-legacy-encrypted-field-keys-v796.json';runtime_controls='fixtures/runtime-control-inventory-v796.json';validation_functions='fixtures/validation-function-inventory-v796.json';calculation_functions='fixtures/calculation-function-inventory-v796.json';resources='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research only; no renderer/release metadata changed.','No decrypted values or email-bearing filenames are emitted.','CREATE runtime is separated from the plain v2018 predecessor.','The available encrypted sample is proven legacy and excluded from the CREATE field inventory.','186 source-serializable controls plus 11 source-unbounded families are preserved.')}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 1702-EX - January 2018 CREATE variant`n`nRevision-specific rule package for runtime formType 1702EXv2018C with 186 source-serializable controls and 11 unbounded popup families.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') @"
# Evidence

- CREATE runtime HTA SHA-256: $($expected.hta); `formType = "1702EXv2018C"` and January 2018 print identity.
- Plain January 2018 predecessor SHA-256: $($expected.predecessor); excluded from CREATE calculations.
- Package v7.9 release notes explicitly list 1702EXv2018C under RA 11534 CREATE Law.
- January 2018 ENCS v2 PDF SHA-256: $($expected.pdf).
- The available encrypted dummy final copy has ciphertext $($expected.cipher), decrypted payload $($expected.plain), 484 unique keys, and inventory $($expected.inventory). Values were never emitted.
- Revision discrimination is decisive: all 484 keys occur in the June 2013 runtime, but only 46 occur in the CREATE runtime. The sample is excluded from the CREATE inventory.
- The CREATE inventory therefore comes from 186 controls serialized by `saveXML` plus eleven source-unbounded popup families.
- Packaged help SHA-256 $($expected.help) is June 2013 and is treated as legacy guidance only.

All email-bearing filenames are represented as `#email-redacted#`.
"@
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. The packaged help is June 2013, so CREATE-specific deadline and attachment claims remain unverified.`n2. No revision-matched CREATE final copy is available; the supplied 484-key sample is proven to match the legacy runtime and is excluded.`n3. Online and EFPS submission were deliberately not exercised.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- CREATE revision and predecessor separation: pass.`n- Legacy-sample mismatch detected and excluded: 484/484 legacy overlap versus 46/484 CREATE overlap.`n- Official assets and encrypted inventory hashes: pass.`n- Typed inventory: 186 concrete + 11 families = $($fields.Count).`n- Validation rules: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count).`n- Confirmed official defects: $bugs.`n- Full JSON structural/schema audit must run after generation.`n- No renderer/release/capability/commit/push changes.`n"
$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json;$entry=$index.forms|Where-Object{$_.form_id-eq$formId}
if($entry){$entry.form_code='1702EX';$entry.revision=$revision;$entry.package_version=$packageVersion;$entry.priority=26;$entry.status='complete';$entry.path='forms/1702ex-v2018c/manifest.json'}else{$index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='1702EX';revision=$revision;package_version=$packageVersion;priority=26;status='complete';path='forms/1702ex-v2018c/manifest.json'}}
$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23';Write-Json $indexPath $index
[pscustomobject]@{form_id=$formId;concrete_fields=186;families=11;typed_fields=$fields.Count;validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;confirmed_official_bugs=$bugs;next_form='1707'}|ConvertTo-Json
