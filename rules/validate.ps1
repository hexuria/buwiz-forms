param([switch]$RequireJsonSchema)
$ErrorActionPreference = 'Stop'
$rulesRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$jsonFiles = Get-ChildItem -LiteralPath $rulesRoot -Recurse -File -Filter '*.json'
$v2IrRoot = (Join-Path $rulesRoot 'ir\v2').TrimEnd('\') + '\'
$v2SchemaRoot = (Join-Path $rulesRoot 'schema\v2').TrimEnd('\') + '\'
$legacyJsonFiles = @($jsonFiles | Where-Object {
    -not $_.FullName.StartsWith($v2IrRoot, [System.StringComparison]::OrdinalIgnoreCase) -and
    -not $_.FullName.StartsWith($v2SchemaRoot, [System.StringComparison]::OrdinalIgnoreCase)
})
$documents = @{}
foreach ($file in $jsonFiles) {
    try { $documents[$file.FullName] = Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json }
    catch { throw "Invalid JSON: $($file.FullName): $($_.Exception.Message)" }
}
$indexPath = Join-Path $rulesRoot 'index.json'
if (-not $documents.ContainsKey($indexPath)) { throw "Missing rules index: $indexPath" }
$index = $documents[$indexPath]
$indexedForms = @($index.forms)
if (($indexedForms.form_id | Sort-Object -Unique).Count -ne $indexedForms.Count) { throw 'rules/index.json has duplicate form_id entries' }
if (($indexedForms.priority | Sort-Object -Unique).Count -ne $indexedForms.Count) { throw 'rules/index.json has duplicate priority entries' }
if (($indexedForms.path | Sort-Object -Unique).Count -ne $indexedForms.Count) { throw 'rules/index.json has duplicate manifest paths' }
if ($index.priority_queue.Count -ne $indexedForms.Count) { throw 'rules/index.json priority_queue length does not match forms length' }
$orderedIndexForms = @($indexedForms | Sort-Object priority)
for ($indexOffset = 0; $indexOffset -lt $orderedIndexForms.Count; $indexOffset++) {
    $entry = $orderedIndexForms[$indexOffset]
    $expectedPriority = $indexOffset + 1
    if ($entry.priority -ne $expectedPriority) { throw "rules/index.json priorities are not contiguous at $expectedPriority" }
    if ($entry.form_code -ne $index.priority_queue[$indexOffset]) {
        throw "rules/index.json priority_queue mismatch at priority $expectedPriority"
    }
    $entryManifestPath = Join-Path $rulesRoot ($entry.path -replace '/', '\')
    if (-not $documents.ContainsKey($entryManifestPath)) { throw "Indexed manifest is missing: $($entry.path)" }
    $entryManifest = $documents[$entryManifestPath]
    if ($entryManifest.form_id -ne $entry.form_id) { throw "Indexed form_id does not match manifest: $($entry.form_id)" }
    if ($entryManifest.revision -ne $entry.revision) { throw "Indexed revision does not match manifest: $($entry.form_id)" }
    if ($entryManifest.status -ne $entry.status) { throw "Indexed status does not match manifest: $($entry.form_id)" }
}
$formResults = @()
$totalFields = 0
$totalValidations = 0
$totalCalculations = 0
$totalNegativeFixtures = 0
$formsRoot = Join-Path $rulesRoot 'forms'
foreach ($formDirectory in Get-ChildItem -LiteralPath $formsRoot -Directory | Sort-Object Name) {
    $manifestPath = Join-Path $formDirectory.FullName 'manifest.json'
    if (-not $documents.ContainsKey($manifestPath)) { continue }
    $manifest = $documents[$manifestPath]
    $requiredPaths = @{
        fields = Join-Path $formDirectory.FullName 'fields.json'
        validations = Join-Path $formDirectory.FullName 'validations.json'
        calculations = Join-Path $formDirectory.FullName 'calculations.json'
        negative = Join-Path $formDirectory.FullName 'fixtures\negative-cases.json'
    }
    $missing = @($requiredPaths.Values | Where-Object { -not $documents.ContainsKey($_) })
    if ($missing.Count -gt 0) {
        if ($manifest.status -eq 'complete') { throw "Complete form $($manifest.form_id) is missing required JSON: $($missing -join ', ')" }
        continue
    }
    $fields = $documents[$requiredPaths.fields]
    $validations = $documents[$requiredPaths.validations]
    $calculations = $documents[$requiredPaths.calculations]
    $negativeCases = $documents[$requiredPaths.negative]
    $prefix = $manifest.form_id
    if ($fields.field_count -ne $fields.fields.Count) { throw "$prefix fields.field_count does not match fields array length" }
    if (($fields.fields.field_key | Sort-Object -Unique).Count -ne $fields.fields.Count) { throw "$prefix has duplicate field_key" }
    if (($validations.rules.rule_id | Sort-Object -Unique).Count -ne $validations.rules.Count) { throw "$prefix has duplicate validation rule_id" }
    if (($calculations.calculations.calculation_id | Sort-Object -Unique).Count -ne $calculations.calculations.Count) { throw "$prefix has duplicate calculation_id" }
    if (($calculations.evaluation_order | Sort-Object -Unique).Count -ne $calculations.calculations.Count) { throw "$prefix calculation evaluation_order mismatch" }
    foreach ($entry in $fields.fields) { if (-not $entry.source_refs -or $entry.source_refs.Count -eq 0) { throw "$prefix field without source_refs: $($entry.field_key)" } }
    foreach ($rule in $validations.rules) { if (-not $rule.source_refs -or $rule.source_refs.Count -eq 0) { throw "$prefix rule without source_refs: $($rule.rule_id)" } }
    if ($manifest.counts.typed_fields -ne $fields.fields.Count) { throw "$prefix manifest typed_fields count mismatch" }
    if ($manifest.counts.validation_rules -ne $validations.rules.Count) { throw "$prefix manifest validation_rules count mismatch" }
    if ($manifest.counts.calculations -ne $calculations.calculations.Count) { throw "$prefix manifest calculations count mismatch" }
    $negativeIdProperty = if ($negativeCases.cases.Count -gt 0 -and $negativeCases.cases[0].PSObject.Properties.Name -contains 'case_id') { 'case_id' } else { 'id' }
    $negativeIds = @($negativeCases.cases | ForEach-Object { $_.$negativeIdProperty })
    if (($negativeIds | Sort-Object -Unique).Count -ne $negativeCases.cases.Count) { throw "$prefix has duplicate negative fixture $negativeIdProperty" }
    $ruleIds = @($validations.rules.rule_id)
    foreach ($case in $negativeCases.cases) {
        if (-not $case.rule_id -or $ruleIds -notcontains $case.rule_id) { throw "$prefix negative fixture has unknown rule_id: $($case.$negativeIdProperty): $($case.rule_id)" }
    }
    $totalFields += $fields.fields.Count
    $totalValidations += $validations.rules.Count
    $totalCalculations += $calculations.calculations.Count
    $totalNegativeFixtures += $negativeCases.cases.Count
    $formResults += [pscustomobject]@{
        form_id = $manifest.form_id
        fields = $fields.fields.Count
        validations = $validations.rules.Count
        calculations = $calculations.calculations.Count
        negative_fixtures = $negativeCases.cases.Count
    }
}
$schemaCommand = Get-Command Test-Json -ErrorAction SilentlyContinue
$schemaValidated = $false
if ($schemaCommand -and $schemaCommand.Parameters.ContainsKey('SchemaFile')) {
    foreach ($file in $legacyJsonFiles | Where-Object { $_.FullName -notmatch '\\schema\\' }) {
        $doc = $documents[$file.FullName]
        if ($doc.'$schema') {
            $schemaPath = [IO.Path]::GetFullPath((Join-Path $file.DirectoryName $doc.'$schema'))
            if (-not (Test-Path -LiteralPath $schemaPath)) { throw "Missing schema for $($file.FullName): $schemaPath" }
            if (-not (Get-Content -Raw -LiteralPath $file.FullName | Test-Json -SchemaFile $schemaPath)) { throw "Schema validation failed: $($file.FullName)" }
        }
    }
    $schemaValidated = $true
}
if (-not $schemaValidated) {
    $portableSchemaAudit = (& (Join-Path $rulesRoot 'validate-json-schema.ps1')) | ConvertFrom-Json
    if ($portableSchemaAudit.result -ne 'pass') { throw 'Tracked JSON Schema keyword validation failed' }
    $schemaValidated = $true
}
if ($RequireJsonSchema -and -not $schemaValidated) { throw 'JSON Schema validation is unavailable' }
[pscustomobject]@{
    json_files = $legacyJsonFiles.Count
    total_json_files = $jsonFiles.Count
    v2_json_files = $jsonFiles.Count - $legacyJsonFiles.Count
    forms_audited = $formResults.Count
    fields = $totalFields
    validations = $totalValidations
    calculations = $totalCalculations
    negative_fixtures = $totalNegativeFixtures
    form_results = $formResults
    structural_audit = 'pass'
    json_schema_validation = if ($schemaValidated) { 'pass' } else { 'unavailable' }
    schema_documents = if ($portableSchemaAudit) { $portableSchemaAudit.schema_documents } else { $null }
    v2_schema_validation = 'separate-bir-rules-codegen'
} | ConvertTo-Json
