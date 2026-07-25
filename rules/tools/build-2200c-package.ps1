param(
    [string]$RepoRoot=(Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$ExtractedRoot='C:\Users\uriah\AppData\Local\Temp\{68379049-6E6C-4478-A2C5-EF7B77ED7E38}',
    [string]$OfficialDir='C:\Mac\Home\Downloads\forms\2200Cv2018'
)
$ErrorActionPreference='Stop'
$formId='2200c-v2018';$revision='2018-01-01';$packageVersion='7.9.6.0'
$htaPath=Join-Path $ExtractedRoot 'forms\BIR-Form2200Cv2018.hta'
$pdfPath=Join-Path $OfficialDir '2200-C Jan 2018 final version3.pdf'
$packagePath='C:\eBIRForms\BIRForms.exe'
$outDir=Join-Path $RepoRoot 'rules\forms\2200c-v2018';$fixtureDir=Join-Path $outDir 'fixtures'
$expected=@{
    hta='d9f47fba52eafa26f03fedb113db4ef1a0074635359d991f1cd7a160383a705c'
    pdf='7b60d517ac6f3697e351aa89c124423d03dd7cac0961c4319b6507dd0ae64ce2'
    package='de8ef0815509d65189e6794e1f8135a5ecf5f2800005d1fc5c87043efd96dbca'
    cipher='560b603f71cea0b00ab5151b8aba9620e7a943770d0ec84b086f112a87d31a48'
    plain='8cf668017e2ddd5ab1ac59cee56704356fe6f1b3018e7938693b7ffef93b1a87'
    inventory='6d80afb233101bba37518617aa71d5fc1852febc8afa6d00120883170fed689a'
}
function Write-Json([string]$Path,$Value){[IO.File]::WriteAllText($Path,(($Value|ConvertTo-Json -Depth 60)+[Environment]::NewLine),[Text.UTF8Encoding]::new($false))}
function Write-Utf8([string]$Path,[string]$Value){[IO.File]::WriteAllText($Path,$Value,[Text.UTF8Encoding]::new($false))}
function Hash-Lines([string[]]$Lines){$s=[Security.Cryptography.SHA256]::Create();try{([BitConverter]::ToString($s.ComputeHash([Text.Encoding]::UTF8.GetBytes(($Lines-join"`n"))))).Replace('-','').ToLowerInvariant()}finally{$s.Dispose()}}
function Attr([string]$Tag,[string]$Name){$m=[regex]::Match($Tag,('(?i)\b{0}\s*=\s*([''"])(.*?)\1'-f[regex]::Escape($Name)));if($m.Success){$m.Groups[2].Value}else{$null}}
function Asset([string]$Id,[string]$Kind,[string]$Path,[string]$Binding,[string]$Display=''){$i=Get-Item -LiteralPath $Path;[pscustomobject][ordered]@{asset_id=$Id;kind=$Kind;path=if($Display){$Display}else{$Path};sha256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant();size=$i.Length;revision_binding=$Binding}}

foreach($pair in @(@($htaPath,'hta'),@($pdfPath,'pdf'),@($packagePath,'package'))){
    if(-not(Test-Path -LiteralPath $pair[0] -PathType Leaf)){throw"Missing $($pair[0])"}
    if((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected[$pair[1]]){throw"Hash changed: $($pair[0])"}
}
$sample=@(Get-ChildItem -LiteralPath $OfficialDir -File -Filter '*.xml')
if($sample.Count-ne1){throw"Expected one encrypted sample; found $($sample.Count)."}
if((Get-FileHash -LiteralPath $sample[0].FullName -Algorithm SHA256).Hash.ToLowerInvariant()-ne$expected.cipher){throw'Sample hash changed.'}
$pdfBytes=[IO.File]::ReadAllBytes($pdfPath);if([Text.Encoding]::ASCII.GetString($pdfBytes[0..4])-ne'%PDF-'){throw'PDF magic mismatch.'}
$hta=[IO.File]::ReadAllText($htaPath)
if($hta-notmatch'(?i)APPLICATIONNAME\s*=\s*["'']2200Cv2018["'']'-or$hta-notmatch'(?i)January\s+2018'){throw'January 2018 binding changed.'}
New-Item -ItemType Directory -Force -Path $fixtureDir|Out-Null

$redactedSample=Join-Path $OfficialDir '2200Cv2018-final-copy-#email-redacted#.xml'
$keyTool=Join-Path $RepoRoot 'rules\tools\extract-encrypted-field-keys.ps1'
$keyJson=&$keyTool -SourcePath $sample[0].FullName -RedactedSourcePath $redactedSample -FormId $formId `
    -ExpectedCiphertextSha256 $expected.cipher -ExpectedDecryptedSha256 $expected.plain `
    -ExpectedFieldCount 181 -ExpectedFieldInventorySha256 $expected.inventory
$keyAudit=$keyJson|ConvertFrom-Json;$keys=@($keyAudit.keys)
Write-Utf8 (Join-Path $fixtureDir 'encrypted-field-audit-v796.json') ($keyJson-join[Environment]::NewLine)

$fm=[regex]::Match($hta,'(?is)<form\b[^>]*(?:id|name)\s*=\s*["'']frmMain["''][^>]*>(?<body>.*?)</form>')
if(-not$fm.Success){throw'frmMain missing.'}
$body=$fm.Groups['body'].Value;$offset=$fm.Groups['body'].Index
$excluded=@(@([regex]::Matches($body,'(?is)<script\b.*?</script>'))+@([regex]::Matches($body,'(?is)<!--.*?-->')))
$controls=[Collections.Generic.List[object]]::new();$ordinal=0
foreach($m in [regex]::Matches($body,'(?is)<(input|select|textarea|button)\b[^>]*>')){
    $skip=$false;foreach($range in $excluded){if($m.Index-ge$range.Index-and$m.Index-lt($range.Index+$range.Length)){$skip=$true;break}}
    if($skip){continue};$ordinal++;$tag=$m.Value;$el=$m.Groups[1].Value.ToLowerInvariant()
    $kind=if($el-eq'input'){Attr $tag 'type'}else{$el};if(-not$kind){$kind='text'};$kind=$kind.ToLowerInvariant()
    $default=Attr $tag 'value';if($kind-in@('radio','checkbox')){$default=if($tag-match'(?i)\bchecked(?:\s*=|\s|>)'){'true'}else{'false'}}
    $controls.Add([pscustomobject][ordered]@{ordinal=$ordinal;id=Attr $tag 'id';name=Attr $tag 'name';element=$el;control_kind=$kind;source_line=1+[regex]::Matches($hta.Substring(0,$offset+$m.Index),"`n").Count;default_value=$default;maxlength=Attr $tag 'maxlength';disabled=$tag-match'(?i)\bdisabled(?:\s*=|\s|>)';readonly=$tag-match'(?i)\breadonly(?:\s*=|\s|>)'})
}
$serial=@($controls|Where-Object{$_.control_kind-in@('text','select','select-one','textarea','radio','checkbox')-and$_.id})
$byId=@{};foreach($c in $serial){if(-not$byId.ContainsKey($c.id)){$byId[$c.id]=$c}}
$runtimeRdo='frm2200C:rdoPg1Pt1I6RDO'
$staticSample=@($keys|Where-Object{$_-ne$runtimeRdo})
$missingStatic=@($staticSample|Where-Object{-not$byId.ContainsKey($_)})
$extraStatic=@($serial.id|Where-Object{$keys-notcontains$_})
if($missingStatic.Count-or$extraStatic.Count){throw"Sample/DOM mismatch: missing $($missingStatic.Count), extra $($extraStatic.Count)."}
if($hta-notmatch[regex]::Escape("<select id='frm2200C:rdoPg1Pt1I6RDO'")){throw'Runtime RDO derivation changed.'}

$required=@('frm2200C:txtPg1I1Month','frm2200C:txtPg1I1Day','frm2200C:txtPg1I1Year','frm2200C:txtPg1I6RDO',$runtimeRdo,'frm2200C:txtPg1Pt1I7RegisteredName','frm2200C:txtPg1Pt1I8RegisteredAddress','frm2200C:txPg1I8ZipCode','frm2200C:txtPg1Pt1I9ContactNumber','frm2200C:txtPg1Pt1I10Email','frm2200C:txtPg1Pt1I11Region','frm2200C:txtPg1Pt1I11Province','frm2200C:txtPg1Pt1I11City','frm2200C:rdoPg1I12TaxReliefYes','frm2200C:rdoPg1I12TaxReliefNo','frm2200C:rdoPg1Pt2MOPPaymentActual','frm2200C:rdoPg1Pt2MOPPrepayment','frm2200C:rdoPg1Pt2MOPOther')
$computed=@('frm2200C:ACexciseTotal','frm2200C:txtPg1P3I16ExciseTaxDue','frm2200C:txtPg1P3I17CTotal','frm2200C:txtPg1P3I18NetTaxDue','frm2200C:txtPg1P3I20TaxStillDue','frm2200C:txtPg1P3I21DTotPenalties','frm2200C:txtPg1P3I22AmountPayable','frm2200C:txtPg1P3I23BPenalties','frm2200C:txtPg1P3I23CTotPmntMade','frm2200C:txtPg1P3I24BalToCarryOver')
for($i=1;$i-le10;$i++){$computed+=@("frm2200C:ACexcise$i","frm2200C:ACvat$i","frm2200C:ACtotAmountBill$i")}
function Item([string]$k){
    if($k-match'Pg1I1(?:Month|Day|Year)'){return '1'};if($k-match'Pg1I2Amended'){return '2'};if($k-match'Pg1I3ATC'){return '3'};if($k-match'Pg1I4NoSheets'){return '4'}
    if($k-match'Pg1TIN'){return '5'};if($k-match'Pg1I6RDO|Pg1Pt1I6RDO'){return '6'};if($k-match'Pt1I7'){return '7'};if($k-match'Pt1I8|I8Zip'){return '8'}
    if($k-match'Pt1I9'){return '9'};if($k-match'Pt1I10'){return '10'};if($k-match'Pt1I11'){return '11'};if($k-match'Pg1I12'){return '12'}
    if($k-match'P3I(\d+[A-D]?)') {return $Matches[1]};if($k-match'CPP|GR|AC') {return 'Part V'};$null
}
function Field([string]$k){
    $c=if($byId.ContainsKey($k)){$byId[$k]}else{[pscustomobject]@{control_kind='runtime-generated-select';source_line=8115;default_value='000';maxlength=$null;disabled=$false;readonly=$false}}
    $logical='string';$norm=[string[]]@();$enum=[object[]]@()
    if($c.control_kind-in@('radio','checkbox')){$logical='boolean';$enum=[object[]]@('true','false')}
    elseif($k-match'(?i)(?:TIN|RDO|ATC|Zip|Month|Year|Region|Province|City)'){$logical='code'}
    elseif($k-match'(?i)(?:CPP|GR|AC|Tax|Bal|Credit|Pmnt|Surcharge|Interest|Compromise|Amount|Cash|Check)'){$logical='decimal-amount';$norm=[string[]]@('NumWithComma','formatCurrency')}
    $isComp=$computed-contains$k;$status=if($isComp){'computed'}elseif($required-contains$k){'required'}else{'optional'};$req=$null
    if($k-eq'frm2200C:txtPg1I12TaxReliefSpecify'){$status='conditional';$req='Tax Relief Yes is selected.'}
    if($k-eq'frm2200C:txtPg1Pt2MOPOtherDesc'){$status='conditional';$req='Other manner of payment is selected.'}
    $cons=[ordered]@{};if($c.maxlength-match'^\d+$'){$cons.max_length=[int]$c.maxlength};if($logical-eq'decimal-amount'){$cons.precision=2;$cons.sign='source-dependent'}
    [pscustomobject][ordered]@{field_key=$k;serialized_key=$k;serialized_occurrence=1;label=$k;page=if($k-match'Pg2|CPP|GR|AC'){2}else{1};item_number=Item $k;control_kind=$c.control_kind;storage_type='string';logical_type=$logical;required=$status;required_when=$req;enabled_when=if($k-eq'frm2200C:txtPg1P3I19PmntOnRtrnPrevFiled'){'Amended Return Yes is selected.'}else{$null};visible_when=$null;default_value=$c.default_value;empty_representation='';constraints=[pscustomobject]$cons;enum_values=$enum;normalization=$norm;computed=$isComp;calculation_id=if($isComp){'See calculations.json'}else{$null};source_refs=@("xml-encrypted-v1#decrypted-field:$k","official-hta-runtime#control:L$($c.source_line)");confidence='high';notes=[string[]]@('Present in the revision-matched encrypted dummy final-copy inventory; value excluded.')}
}
$fields=@($keys|ForEach-Object{Field $_})
if($fields.Count-ne181){throw"Expected 181 fields; found $($fields.Count)."}
Write-Json (Join-Path $outDir 'fields.json') ([ordered]@{'$schema'='../../schema/fields.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;field_count=$fields.Count;runtime_serializable_element_count=181;inventory_sha256=Hash-Lines @($fields.field_key|Sort-Object);fields=$fields})
Write-Json (Join-Path $fixtureDir 'runtime-control-inventory-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;official_hta_sha256=$expected.hta;live_control_count=$controls.Count;static_serialized_control_count=$serial.Count;runtime_generated_scalar_count=1;runtime_generated_scalars=@($runtimeRdo);runtime_family_count=0;encrypted_final_copy_key_count=$keys.Count;sample_dom_missing_count=$missingStatic.Count;dom_sample_extra_count=$extraStatic.Count;controls=$controls})
$fn=Join-Path $RepoRoot 'rules\tools\extract-hta-function-inventory.ps1'
Write-Utf8 (Join-Path $fixtureDir 'validation-function-inventory-v796.json') ((&$fn -HtaPath $htaPath -ControlPrefix 'frm2200C:' -NamePattern '(?i)valid|check|save|date|submit|final|process')-join[Environment]::NewLine)
Write-Utf8 (Join-Path $fixtureDir 'calculation-function-inventory-v796.json') ((&$fn -HtaPath $htaPath -ControlPrefix 'frm2200C:' -NamePattern '(?i)comput|total|tax|penalt|balance|format')-join[Environment]::NewLine)

$rules=[Collections.Generic.List[object]]::new()
function Rule([string]$Id,[string]$Phase,$Order,[string]$Condition,[string[]]$Keys,$Message,[string[]]$Refs,[string]$Assessment='verified-correct',[string]$Official='The branch alerts and stops the active operation.',[string]$Recommended='Retain as a structured revision-aware error.'){
    $rules.Add([pscustomobject][ordered]@{rule_id=$Id;form_id=$formId;revision=$revision;phase=$Phase;order=$Order;condition=$Condition;fields=$Keys;accepted_behavior='Condition is false; processing continues.';rejected_behavior='The active operation stops unless official_behavior states otherwise.';exact_message=$Message;source_refs=$Refs;evidence_type=@('source');assessment=$Assessment;official_behavior=$Official;recommended_app_behavior=$Recommended;confidence='high';unresolved_questions=@()})
}
Rule '2200c-validate-001-month' validate 1 'Month is 00.' @('frm2200C:txtPg1I1Month') 'Month field on Page 1 Item 1 is required.' @('official-hta-runtime#validateAll:L3180-L3186')
Rule '2200c-validate-002-day' validate 2 'Day is blank.' @('frm2200C:txtPg1I1Day') 'Day field on Page 1 Item 1 is required.' @('official-hta-runtime#validateAll:L3188-L3193')
Rule '2200c-validate-003-year' validate 3 'Year is blank.' @('frm2200C:txtPg1I1Year') 'Year field on Page 1 Item 1 is required.' @('official-hta-runtime#validateAll:L3195-L3200')
Rule '2200c-validate-004-date-shape' validate 4 'Return date fails source calendar and length checks.' @('frm2200C:txtPg1I1Month','frm2200C:txtPg1I1Day','frm2200C:txtPg1I1Year') 'Please provide a valid date. (MM/DD/YYYY format) in Page 1 Item 1.' @('official-hta-runtime#validateDate:L3313-L3382')
Rule '2200c-validate-005-future-date' validate 5 'Return date is after the current date.' @('frm2200C:txtPg1I1Month','frm2200C:txtPg1I1Day','frm2200C:txtPg1I1Year') 'Page 1 Item 1 Date cannot be a future date ' @('official-hta-runtime#validateDate:L3374-L3380')
Rule '2200c-validate-006-rdo' validate 6 'Hidden RDO code is 000.' @('frm2200C:txtPg1I6RDO') 'Please enter a valid RDO Code on Page 1 Item 6.' @('official-hta-runtime#validateAll:L3201')
Rule '2200c-validate-007-name' validate 7 'Taxpayer name is blank after trim.' @('frm2200C:txtPg1Pt1I7RegisteredName') 'Name field on Page 1 Item 7 is required.' @('official-hta-runtime#validateAll:L3202')
Rule '2200c-validate-008-address' validate 8 'Registered address line 1 is blank after trim.' @('frm2200C:txtPg1Pt1I8RegisteredAddress') 'Registered Address field on Page 1 Item 8 is required.' @('official-hta-runtime#validateAll:L3203')
Rule '2200c-validate-009-zip' validate 9 'ZIP code is blank after trim.' @('frm2200C:txPg1I8ZipCode') 'Zip Code field on Page 1 Item 8A is required.' @('official-hta-runtime#validateAll:L3204')
Rule '2200c-validate-010-contact' validate 10 'Contact number is blank after trim.' @('frm2200C:txtPg1Pt1I9ContactNumber') 'Contact Number field on Page 1 Item 9 is required.' @('official-hta-runtime#validateAll:L3205')
Rule '2200c-validate-011-email-required' validate 11 'Email is blank after trim.' @('frm2200C:txtPg1Pt1I10Email') 'E-mail address on page 1 item 10 is required.' @('official-hta-runtime#validateAll:L3206')
Rule '2200c-validate-012-region' validate 12 'Region value is 00.' @('frm2200C:txtPg1Pt1I11Region') 'Region field on Page 1 Item 11 is required.' @('official-hta-runtime#validateAll:L3207')
Rule '2200c-validate-013-province' validate 13 'Province value is 00.' @('frm2200C:txtPg1Pt1I11Province') 'Province field on Page 1 Item 11 is required.' @('official-hta-runtime#validateAll:L3208')
Rule '2200c-validate-014-city' validate 14 'City value is 00.' @('frm2200C:txtPg1Pt1I11City') 'City field on Page 1 Item 11 is required.' @('official-hta-runtime#validateAll:L3209')
Rule '2200c-validate-015-relief-choice' validate 15 'Neither tax-relief radio is selected.' @('frm2200C:rdoPg1I12TaxReliefYes','frm2200C:rdoPg1I12TaxReliefNo') 'Availing of Tax Relief field on Page 1 Item 12 is required.' @('official-hta-runtime#validateAll:L3211-L3215')
Rule '2200c-validate-016-relief-specify' validate 16 'Tax-relief Yes is selected and specification is blank.' @('frm2200C:rdoPg1I12TaxReliefYes','frm2200C:txtPg1I12TaxReliefSpecify') 'Specify Tax Relief field on Page 1 Item 12A is required.' @('official-hta-runtime#validateAll:L3217-L3221')
Rule '2200c-validate-017-payment-choice' validate 17 'No manner-of-payment radio is selected.' @('frm2200C:rdoPg1Pt2MOPPaymentActual','frm2200C:rdoPg1Pt2MOPPrepayment','frm2200C:rdoPg1Pt2MOPOther') 'Manner of Payment on Page 1 Part II is required.' @('official-hta-runtime#validateAll:L3223-L3228')
Rule '2200c-validate-018-payment-other' validate 18 'Other manner of payment is selected and description is blank.' @('frm2200C:rdoPg1Pt2MOPOther','frm2200C:txtPg1Pt2MOPOtherDesc') 'Specfy Manner of Payment field on Page 1 Part II Item 15 is required.' @('official-hta-runtime#validateAll:L3230-L3234')
Rule '2200c-validate-019-part-v-row' validate 19 'For a row 1..10, any CPP input is zero while computed bill is nonzero, or any CPP input is nonzero while bill is zero.' @('frm2200C:CPPexmpt{1..10}','frm2200C:CPPexcise{1..10}','frm2200C:CPPnonexcise{1..10}','frm2200C:ACtotAmountBill{1..10}') 'Please complete row #{row} in Page 2 Part V.' @('official-hta-runtime#checkPartVFields:L3286-L3310')
Rule '2200c-input-020-email-format' 'blur/change' 1 'Nonblank email does not contain a source-regex address with a 2-4 letter final label.' @('frm2200C:txtPg1Pt1I10Email') 'Please enter a valid e-mail address on page 1 item 10' @('official-hta-runtime#validateEmail:L3554-L3565')
Rule '2200c-input-021-year-range' 'blur/change' 2 'Nonblank year is earlier than 2018 or later than current year.' @('frm2200C:txtPg1I1Year') 'Year shall not be greater than the present year and not earlier than 2018.' @('official-hta-runtime#checkYear:L3638-L3651')
Rule '2200c-save-022-date-required' save 1 'Month/day/year is blank, or month/day is 00.' @('frm2200C:txtPg1I1Month','frm2200C:txtPg1I1Day','frm2200C:txtPg1I1Year') 'Please enter correct date on Item 1.' @('official-hta-runtime#initialValidateBeforeSave:L9322-L9325')
Rule '2200c-save-023-rdo' save 2 'Runtime RDO selector value is 000.' @($runtimeRdo) 'Please enter a valid RDO Code on Item 6.' @('official-hta-runtime#initialValidateBeforeSave:L9329')
Rule '2200c-save-024-name' save 3 'Taxpayer name is blank.' @('frm2200C:txtPg1Pt1I7RegisteredName') 'Please enter a valid name on Page 1 Item 7.' @('official-hta-runtime#initialValidateBeforeSave:L9330')
Rule '2200c-save-025-address' save 4 'Registered address line 1 is blank.' @('frm2200C:txtPg1Pt1I8RegisteredAddress') 'Please enter a valid Registered Address on Page 1 Item 8.' @('official-hta-runtime#initialValidateBeforeSave:L9331')
Rule '2200c-save-026-contact' save 5 'Contact number is blank.' @('frm2200C:txtPg1Pt1I9ContactNumber') 'Please enter a valid Contact Number on Page 1 Item 9.' @('official-hta-runtime#initialValidateBeforeSave:L9332')
Rule '2200c-defect-027-nonnumeric-date' validate 20 'Day is a two-character nonnumeric string or year is a four-character nonnumeric string.' @('frm2200C:txtPg1I1Day','frm2200C:txtPg1I1Year') $null @('official-hta-runtime#validateDate:L3326-L3373') 'incorrect-official-behavior' 'isNaN(strmm || strdd || stryyyy) tests only the first truthy component; coercive comparisons and Invalid Date allow nonnumeric day/year to pass.' 'Parse each component strictly as digits before calendar construction.'
Rule '2200c-defect-028-whole-peso-rounding' 'blur/change' 4 'Any Part III or Part V computation has a nonzero centavo component.' @('Part-III-amounts','Part-V-amounts') $null @('official-hta-runtime#partThreeComputation:L3676-L3745','official-hta-runtime#partFiveComputation:L3782-L3811') 'incorrect-official-behavior' 'Every formula calls toFixed(0), discarding centavos before currency formatting.' 'Calculate and round to two decimal places.'
Rule '2200c-defect-029-email-substring' 'blur/change' 3 'Email field contains a valid-looking address as a substring plus surrounding junk.' @('frm2200C:txtPg1Pt1I10Email') $null @('official-hta-runtime#validateEmail:L3554-L3565') 'official-bug-compatible' 'The regular expression is not anchored, so substring matches pass.' 'Require the entire normalized value to be a valid address.'
Rule '2200c-defect-030-save-sparse' save 6 'A Validate-only field such as ZIP, email, location, relief choice, payment choice, or Part V completeness is invalid.' @('frm2200C:txPg1I8ZipCode','frm2200C:txtPg1Pt1I10Email','frm2200C:txtPg1Pt1I11City','frm2200C:rdoPg1I12TaxReliefYes','frm2200C:rdoPg1Pt2MOPPaymentActual','Part-V-fields') $null @('official-hta-runtime#initialValidateBeforeSave:L9319-L9335','official-hta-runtime#validateAll:L3177-L3240') 'incorrect-official-behavior' 'Save checks only date, RDO, name, address, and contact number.' 'Use one phase-aware validation graph.'
Rule '2200c-defect-031-part-v-zero' validate 21 'A legitimate populated Part V row contains zero in any of the three CPP columns while computed bill is nonzero.' @('frm2200C:CPPexmpt{1..10}','frm2200C:CPPexcise{1..10}','frm2200C:CPPnonexcise{1..10}') 'Please complete row #{row} in Page 2 Part V.' @('official-hta-runtime#checkPartVFields:L3286-L3310') 'official-bug-compatible' 'The completeness predicate treats numeric zero as missing, so legitimate zero-valued columns cannot coexist with a nonzero row.' 'Track presence separately from numeric value.'
Rule '2200c-defect-032-stale-submit' submit 1 'The eFPS path dereferences IDs and arrays copied from unrelated income-tax schedules.' @('unrelated-submit-identifiers') $null @('official-hta-runtime#submitToEFPS:L3826-L4300') 'incorrect-official-behavior' 'The source references many controls and AddRow functions absent from 2200C; the path is internally stale and was not exercised.' 'Use a revision-specific typed transport mapping after separate online certification.'
Write-Json (Join-Path $outDir 'validations.json') ([ordered]@{'$schema'='../../schema/validations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;first_error_behavior='Validate and Save stop on the first source-ordered branch, except Part V loops until the first incomplete row.';rules=$rules})

$calcs=[Collections.Generic.List[object]]::new()
function Calc([string]$Id,[string[]]$Out,[string[]]$In,[string]$Formula,[string]$Trigger,[string[]]$Deps,[string[]]$Refs,[string]$Assessment='verified-correct'){
    $calcs.Add([pscustomobject][ordered]@{calculation_id=$Id;outputs=$Out;inputs=$In;condition=$null;official_formula=$Formula;rounding='Source applies toFixed(0), then formatCurrency; whole-peso rounding is defect-compatible.';trigger=$Trigger;depends_on=$Deps;source_refs=$Refs;assessment=$Assessment;recommended_app_behavior='Use typed decimals and preserve two decimal places; retain the source dependency order.';confidence='high'})
}
Calc '2200c-partv-excise' @('frm2200C:ACexcise{1..10}') @('frm2200C:GRexciseNet{1..10}','frm2200C:GRexciseExmpt{1..10}') 'H[row] = (E[row] + F[row]) x 0.05.' partFiveComputation @() @('official-hta-runtime#partFiveComputation:L3782-L3790') 'incorrect-official-behavior'
Calc '2200c-partv-vat' @('frm2200C:ACvat{1..10}') @('frm2200C:GRnetVat{1..10}','frm2200C:GRexciseNet{1..10}','frm2200C:GRnonexcise{1..10}','frm2200C:ACexcise{1..10}','vatColumnI') 'I[row] = (D + E + G + H) x runtime vatColumnI.' partFiveComputation @('2200c-partv-excise') @('official-hta-runtime#partFiveComputation:L3792-L3801') 'incorrect-official-behavior'
Calc '2200c-partv-bill' @('frm2200C:ACtotAmountBill{1..10}') @('frm2200C:GRnetVat{1..10}','frm2200C:GRexciseNet{1..10}','frm2200C:GRexciseExmpt{1..10}','frm2200C:GRnonexcise{1..10}','frm2200C:ACexcise{1..10}','frm2200C:ACvat{1..10}') 'J[row] = D + E + F + G + H + I.' partFiveComputation @('2200c-partv-excise','2200c-partv-vat') @('official-hta-runtime#partFiveComputation:L3803-L3810') 'incorrect-official-behavior'
Calc '2200c-excise-total' @('frm2200C:ACexciseTotal') @('frm2200C:ACexcise{1..10}') 'Excise total = sum of Part V Column H rows 1..10.' totalExciseTax @('2200c-partv-excise') @('official-hta-runtime#totalExciseTax:L3748-L3758') 'incorrect-official-behavior'
Calc '2200c-item16' @('frm2200C:txtPg1P3I16ExciseTaxDue') @('frm2200C:ACexciseTotal') 'Item 16 = ACexciseTotal.' partThreeComputation @('2200c-excise-total') @('official-hta-runtime#partThreeComputation:L3676-L3680')
Calc '2200c-item17c' @('frm2200C:txtPg1P3I17CTotal') @('frm2200C:txtPg1P3I17ABalCarriedOver','frm2200C:txtPg1P3I17BCredExciseTax') 'Item 17C = 17A + 17B.' partThreeComputation @() @('official-hta-runtime#partThreeComputation:L3682-L3686') 'incorrect-official-behavior'
Calc '2200c-item18' @('frm2200C:txtPg1P3I18NetTaxDue') @('frm2200C:txtPg1P3I16ExciseTaxDue','frm2200C:txtPg1P3I17CTotal') 'Item 18 = Item 16 - Item 17C.' partThreeComputation @('2200c-item16','2200c-item17c') @('official-hta-runtime#partThreeComputation:L3688-L3692') 'incorrect-official-behavior'
Calc '2200c-item20' @('frm2200C:txtPg1P3I20TaxStillDue') @('frm2200C:txtPg1P3I18NetTaxDue','frm2200C:txtPg1P3I19PmntOnRtrnPrevFiled') 'Item 20 = Item 18 - Item 19.' partThreeComputation @('2200c-item18') @('official-hta-runtime#partThreeComputation:L3694-L3698') 'incorrect-official-behavior'
Calc '2200c-item21d' @('frm2200C:txtPg1P3I21DTotPenalties') @('frm2200C:txtPg1P3I21ASurcharge','frm2200C:txtPg1P3I21BInterest','frm2200C:txtPg1P3I21CCompromise') 'Item 21D = 21A + 21B + 21C.' partThreeComputation @() @('official-hta-runtime#partThreeComputation:L3700-L3705') 'incorrect-official-behavior'
Calc '2200c-item22' @('frm2200C:txtPg1P3I22AmountPayable') @('frm2200C:txtPg1P3I20TaxStillDue','frm2200C:txtPg1P3I21DTotPenalties') 'Item 22 = Item 20 + Item 21D.' partThreeComputation @('2200c-item20','2200c-item21d') @('official-hta-runtime#partThreeComputation:L3707-L3711') 'incorrect-official-behavior'
Calc '2200c-item23b' @('frm2200C:txtPg1P3I23BPenalties') @('frm2200C:txtPg1P3I21DTotPenalties') 'Item 23B = Item 21D.' partThreeComputation @('2200c-item21d') @('official-hta-runtime#partThreeComputation:L3728-L3731') 'incorrect-official-behavior'
Calc '2200c-item23c' @('frm2200C:txtPg1P3I23CTotPmntMade') @('frm2200C:txtPg1P3I23ATaxPmntDposit','frm2200C:txtPg1P3I23BPenalties') 'Item 23C = Item 23A + Item 23B.' partThreeComputation @('2200c-item23b') @('official-hta-runtime#partThreeComputation:L3733-L3737') 'incorrect-official-behavior'
Calc '2200c-item24' @('frm2200C:txtPg1P3I24BalToCarryOver') @('frm2200C:txtPg1P3I22AmountPayable','frm2200C:txtPg1P3I23CTotPmntMade') 'Item 24 = Item 22 - Item 23C.' partThreeComputation @('2200c-item22','2200c-item23c') @('official-hta-runtime#partThreeComputation:L3739-L3744') 'incorrect-official-behavior'
Write-Json (Join-Path $outDir 'calculations.json') ([ordered]@{'$schema'='../../schema/calculations.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;evaluation_order=@($calcs.calculation_id);calculations=$calcs})

$cases=@();$n=0;foreach($r in @($rules|Where-Object{$_.exact_message})){$n++;$cases+=[pscustomobject][ordered]@{case_id=('case-{0:d2}-{1}'-f$n,$r.rule_id);phase=$r.phase;mutations=@{synthetic_condition=$r.condition};expected_message=$r.exact_message;expected_behavior=$r.official_behavior;rule_id=$r.rule_id}}
Write-Json (Join-Path $fixtureDir 'negative-cases.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;synthetic_only=$true;cases=$cases})
Write-Json (Join-Path $fixtureDir 'calculation-boundaries.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;cases=@(
    @{case_id='partv-centavo-loss';calculation_id='2200c-partv-excise';excise_net=100.10;excise_exempt=0;rate=0.05;official_output=5.00;two_decimal_output=5.01},
    @{case_id='partv-row';calculation_id='2200c-partv-bill';d=100;e=200;f=300;g=400;h=25;i=87;official_output=1112},
    @{case_id='item24';calculation_id='2200c-item24';item22=1200;item23c=1100;official_output=100}
)})
$resources=@();foreach($src in @([regex]::Matches($hta,'(?i)<script[^>]+src\s*=\s*(["''])(?<v>.*?)\1')|ForEach-Object{$_.Groups['v'].Value}|Sort-Object -Unique)){
    $full=[IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $htaPath) $src))
    if(Test-Path -LiteralPath $full){$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$true;size=(Get-Item -LiteralPath $full).Length;sha256=(Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash.ToLowerInvariant()}}
    else{$resources+=[pscustomobject][ordered]@{src=$src;path=$full;present=$false;size=$null;sha256=$null}}
}
Write-Json (Join-Path $fixtureDir 'official-resource-hashes-v796.json') ([ordered]@{schema_version='1.0.0';form_id=$formId;resources=$resources})

$workflow=[ordered]@{'$schema'='../../schema/workflow.schema.json';schema_version='1.0.0';form_id=$formId;revision=$revision;phases=@(
    @{phase='edit';official_behavior='January 2018 excise-tax return for cosmetic procedures with ten fixed Part V rows.';source_refs=@('official-hta-runtime','official-form-pdf');confidence='high'},
    @{phase='saved-draft';official_behavior='Save checks date, RDO, name, address, and contact number before serializing 181 controls.';source_refs=@('official-hta-runtime#initialValidateBeforeSave:L9319-L9335','encrypted-field-audit-v796');confidence='high'},
    @{phase='validated';official_behavior='Validate runs validateAll and disables the form after the first-error graph passes.';source_refs=@('official-hta-runtime#validateAll:L3177-L3240','official-hta-runtime#validate:L9289-L9317');confidence='high'},
    @{phase='final-copy';official_behavior='The revision-matched dummy final copy decrypts in memory to exactly 181 unique keys; values are excluded.';source_refs=@('encrypted-field-audit-v796');confidence='high'},
    @{phase='submitted';official_behavior='Online transport was not exercised; its eFPS mapping contains absent identifiers copied from another form.';source_refs=@('official-hta-runtime#submitToEFPS:L3826-L4300');confidence='low'}
);transitions=@(
    @{from='edit';action='Save';to='saved-draft';guard='Sparse Save checks pass.';side_effects=@('Writes flat pseudo-XML.');source_refs=@('official-hta-runtime#saveXML')},
    @{from='edit';action='Validate';to='validated';guard='validateAll passes.';side_effects=@('Disables controls.','Enables Final Copy.');source_refs=@('official-hta-runtime#validate')},
    @{from='validated';action='Edit';to='edit';guard=$null;side_effects=@('Re-enables editable controls.');source_refs=@('official-hta-runtime#enableAllControl')},
    @{from='validated';action='Final Copy';to='final-copy';guard='Finalization succeeds.';side_effects=@('Compresses/encrypts the serialized copy.');source_refs=@('official-hta-runtime#saveEncryptedProfile')},
    @{from='final-copy';action='Transport';to='submitted';guard='Not certified in this audit.';side_effects=@('Untested online attempt.');source_refs=@('official-hta-runtime#submitToEFPS')}
);prerequisites=@('Return date','RDO and taxpayer identity','Registered address/contact/email','Location','Tax relief choice','Manner of payment','Part V row completeness');required_attachments=@();filing_deadlines=@()}
Write-Json (Join-Path $outDir 'workflow.json') $workflow

$bugs=@($rules|Where-Object{$_.assessment-in@('official-bug-compatible','incorrect-official-behavior','obsolete')}).Count
$assets=@(
    Asset 'package-7.9.6' 'official-package-executable' $packagePath 'Installed Offline eBIRForms package 7.9.6.0.'
    Asset 'official-hta-runtime' 'runtime-extracted-hta' $htaPath 'APPLICATIONNAME 2200Cv2018; printed January 2018.'
    Asset 'official-form-pdf' 'official-form-pdf' $pdfPath 'January 2018 form PDF.'
    Asset 'xml-encrypted-v1' 'dummy-profile-encrypted-final-copy' $sample[0].FullName 'Revision-matched 181-key dummy final copy; decrypted values excluded.' $redactedSample
)
$manifest=[ordered]@{'$schema'='../../schema/form-manifest.schema.json';schema_version='1.0.0';form_id=$formId;form_code='2200C';revision=$revision;package_version=$packageVersion;status='complete';official_assets=$assets;counts=[ordered]@{concrete_fields=181;runtime_field_families=0;fields_total=$fields.Count;typed_fields=$fields.Count;validation_rules=$rules.Count;confirmed_official_bugs=$bugs;calculations=$calcs.Count;negative_fixtures=$cases.Count;unverified_gaps=2};artifacts=[ordered]@{fields='fields.json';validations='validations.json';calculations='calculations.json';workflow='workflow.json';evidence='evidence.md';audit='audit.md';gaps='gaps.md';encrypted_field_audit='fixtures/encrypted-field-audit-v796.json';runtime_controls='fixtures/runtime-control-inventory-v796.json';validation_functions='fixtures/validation-function-inventory-v796.json';calculation_functions='fixtures/calculation-function-inventory-v796.json';resources='fixtures/official-resource-hashes-v796.json';negative_fixtures='fixtures/negative-cases.json';calculation_fixtures='fixtures/calculation-boundaries.json'};scope_notes=@('Research only; no renderer/release metadata changed.','No decrypted values or email-bearing filenames emitted.','180 static serialized controls plus one runtime RDO selector; no active runtime field families.','Stale unreachable AddRow references are not modeled as 2200C families.')}
Write-Json (Join-Path $outDir 'manifest.json') $manifest
Write-Utf8 (Join-Path $outDir 'README.md') "# BIR Form 2200C - January 2018`n`nRevision-specific Offline eBIRForms validation package with 181 concrete serialized fields and no active dynamic families.`n"
Write-Utf8 (Join-Path $outDir 'evidence.md') "# Evidence`n`n- January 2018 runtime SHA-256: $($expected.hta).`n- January 2018 form PDF SHA-256: $($expected.pdf).`n- Encrypted sample: ciphertext $($expected.cipher), decrypted $($expected.plain), 181 unique keys, inventory $($expected.inventory); values never emitted.`n- The sample matches all 180 static serialized controls plus the runtime-generated RDO selector.`n- Source formulas use whole-peso `toFixed(0)` rounding in Parts III and V.`n`nAll email-bearing filenames use `#email-redacted#`.`n"
Write-Utf8 (Join-Path $outDir 'gaps.md') "# Gaps`n`n1. No revision-matched packaged help file was located; the form PDF and runtime bind the January 2018 revision, but filing deadline/attachment facts are intentionally omitted.`n2. Online submission was not exercised; the eFPS function contains absent identifiers copied from unrelated forms.`n"
Write-Utf8 (Join-Path $outDir 'audit.md') "# Audit`n`n- January 2018 revision binding: pass.`n- Revision-matched encrypted sample: 181 unique keys; values excluded.`n- DOM/sample binding: 180 static controls plus one runtime RDO selector; zero live families.`n- Validations: $($rules.Count); calculations: $($calcs.Count); negative fixtures: $($cases.Count); confirmed official defects: $bugs.`n- Full structural/schema audit must run after generation.`n- No renderer/release/capability/commit/push changes.`n"
$indexPath=Join-Path $RepoRoot 'rules\index.json';$index=Get-Content -Raw -LiteralPath $indexPath|ConvertFrom-Json
$entry=$index.forms|Where-Object{$_.form_id-eq$formId}
if($entry){$entry.form_code='2200C';$entry.revision=$revision;$entry.package_version=$packageVersion;$entry.priority=33;$entry.status='complete';$entry.path='forms/2200c-v2018/manifest.json'}
else{$index.forms+=[pscustomobject][ordered]@{form_id=$formId;form_code='2200C';revision=$revision;package_version=$packageVersion;priority=33;status='complete';path='forms/2200c-v2018/manifest.json'}}
$index.forms=@($index.forms|Sort-Object priority);$index.updated='2026-07-23';Write-Json $indexPath $index
[pscustomobject]@{form_id=$formId;concrete_fields=181;families=0;typed_fields=$fields.Count;live_controls=$controls.Count;static_serialized=$serial.Count;validations=$rules.Count;calculations=$calcs.Count;negative_fixtures=$cases.Count;confirmed_official_bugs=$bugs;next_form='2200M'}|ConvertTo-Json
