param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

$ErrorActionPreference = 'Stop'

$formRoot = Join-Path $RepoRoot 'rules\forms\2550q-v2024'
$fixtureRoot = Join-Path $formRoot 'fixtures'
$ruleSetPath = Join-Path $RepoRoot 'rules\ir\v2\2550q-v2024-p7.9.6.0\rule-set.json'
$outputPath = Join-Path $fixtureRoot 'serialization-binding-inventory-v796.json'

$paths = [ordered]@{
    plaintext = Join-Path $fixtureRoot 'plaintext-field-audit-v796.json'
    encrypted = Join-Path $fixtureRoot 'encrypted-field-audit-v796.json'
    controls = Join-Path $fixtureRoot 'runtime-control-inventory-v796.json'
    families = Join-Path $fixtureRoot 'schedule-family-inventory-v796.json'
    fields = Join-Path $formRoot 'fields.json'
    rule_set = $ruleSetPath
}
$expectedHashes = @{
    plaintext = '4f3cb5f5f273def4ce4956e1c16e8215edff79c71f69a01c4c363a3e3eba67b2'
    encrypted = 'a66a1fc5162b5275b245d0c564dba5ddac5f09da6b2e274e9fed57b8b6fd843a'
    controls = '89de4bd2be69884b02908bc9162802224d5c7385eaf0e0ad2e286644b384ddce'
    families = '5889d5a7809ac5bf961ae20f35f1ffb4a06c0a602ad04a24620297e31efd3985'
    fields = '5e7b5a3bfe175c4ca1cf1ffc33db216174926ae97b89ddde0bcd9e4513c1ed06'
}

function Get-Sha256([string]$Path) {
    (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Read-Json([string]$Path) {
    Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 80) + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )
}

foreach ($name in $paths.Keys) {
    if (-not (Test-Path -LiteralPath $paths[$name] -PathType Leaf)) {
        throw "Missing 2550Q serialization binding input: $($paths[$name])"
    }
}
foreach ($name in $expectedHashes.Keys) {
    $actual = Get-Sha256 $paths[$name]
    if ($actual -ne $expectedHashes[$name]) {
        throw "Pinned 2550Q serialization binding input changed: $name ($actual)"
    }
}

$plaintext = Read-Json $paths.plaintext
$encrypted = Read-Json $paths.encrypted
$controls = Read-Json $paths.controls
$families = Read-Json $paths.families
$fields = Read-Json $paths.fields
$ruleSet = Read-Json $paths.rule_set

$plainKeys = @($plaintext.keys)
$encryptedKeys = @($encrypted.keys)
if ($plainKeys.Count -ne 160 -or $encryptedKeys.Count -ne 159) {
    throw 'Pinned 2550Q occurrence counts changed.'
}
if (($plainKeys[0..158] -join "`n") -ne ($encryptedKeys -join "`n")) {
    throw 'Encrypted occurrence order is no longer the exact plaintext prefix.'
}
if ($plainKeys[159] -ne 'dateFiled') {
    throw 'Expected generated plaintext dateFiled as occurrence 160.'
}
if ($plaintext.values_emitted -ne $false -or $encrypted.values_emitted -ne $false) {
    throw 'Serialization binding inputs must remain value-free.'
}

$controlById = @{}
foreach ($control in $controls.static_controls) {
    if ($control.id -and -not $controlById.ContainsKey($control.id)) {
        $controlById[$control.id] = $control
    }
}
foreach ($key in $encryptedKeys) {
    if (-not $controlById.ContainsKey($key)) {
        throw "Observed occurrence does not resolve to one static source control: $key"
    }
}

$v2FieldIds = @{}
foreach ($field in $ruleSet.fields) {
    $v2FieldIds[$field.field_id] = $true
}

$groupSpecs = @(
    [ordered]@{
        group_id = 'schedule-1-capital-good-row'
        source_lines = '6089-6217'
        observed_indices = @(0, 1)
        insert_after_key = $null
        families = @(
            'txtDatePurchase1', 'txtSourceCode1', 'txtDescription1',
            'txtAmountPurchase1', 'txtInputTax1', 'txtEstimatedLife1',
            'txtRecognizedLife1', 'txtAllowedInputTax1', 'txtBalanceInputTax1'
        )
    },
    [ordered]@{
        group_id = 'schedule-3-creditable-vat-row'
        source_lines = '6390-6473'
        observed_indices = @(0, 1)
        insert_after_key = $null
        families = @(
            'txtDateCovered3', 'txtDateCovered3To',
            'txtNameWithHoldingAgent3', 'txtIncomePayment3',
            'txtTotalTaxWithHeld3'
        )
    },
    [ordered]@{
        group_id = 'schedule-4-advance-vat-row'
        source_lines = '6618-6703'
        observed_indices = @(0, 1)
        insert_after_key = $null
        families = @(
            'txtDate4', 'txtDate4To', 'txtNameOfMiller4',
            'txtNameOfTaxpayer4', 'txtOfficialReceiptNumber4',
            'txtAmountPaid4'
        )
    },
    [ordered]@{
        group_id = 'item-19-additional-row'
        source_lines = '6789-7104'
        observed_indices = @()
        insert_after_key = 'resultOtherCreditsNo19'
        families = @(
            'frm2550qv2024:totalTaxPayableNo19Description',
            'frm2550qv2024:totalTaxPayableNo19Amount'
        )
    },
    [ordered]@{
        group_id = 'item-42-additional-row'
        source_lines = '7213-7529'
        observed_indices = @()
        insert_after_key = 'resultOtherCreditsNo42'
        families = @(
            'frm2550qv2024:totalTaxPayableNo42Description',
            'frm2550qv2024:totalTaxPayableNo42Amount'
        )
    },
    [ordered]@{
        group_id = 'item-47-additional-row'
        source_lines = '7609-7907'
        observed_indices = @()
        insert_after_key = 'resultOtherCreditsNo47'
        families = @(
            'frm2550qv2024:totalTaxPayableNo47Description',
            'frm2550qv2024:totalTaxPayableNo47Amount'
        )
    },
    [ordered]@{
        group_id = 'item-56-additional-row'
        source_lines = '7991-8283'
        observed_indices = @()
        insert_after_key = 'resultOtherCreditsNo56'
        families = @(
            'frm2550qv2024:totalTaxPayableNo56Description',
            'frm2550qv2024:totalTaxPayableNo56Amount'
        )
    }
)

$declaredPrefixes = @($families.families | ForEach-Object { $_.prefix })
$plannedPrefixes = @($groupSpecs | ForEach-Object { $_.families })
$declaredPrefixText = (($declaredPrefixes | Sort-Object) -join "`n")
$plannedPrefixText = (($plannedPrefixes | Sort-Object) -join "`n")
if ($declaredPrefixText -ne $plannedPrefixText) {
    throw "The seven serialization groups do not cover the 28 extracted dynamic families exactly (declared=$($declaredPrefixes.Count), planned=$($plannedPrefixes.Count))."
}

$prefixBindings = @()
foreach ($group in $groupSpecs) {
    for ($familyOrdinal = 0; $familyOrdinal -lt $group.families.Count; $familyOrdinal++) {
        $prefixBindings += [pscustomobject]@{
            group_id = $group.group_id
            family_ordinal = $familyOrdinal + 1
            prefix = $group.families[$familyOrdinal]
        }
    }
}
$prefixBindings = @($prefixBindings | Sort-Object { $_.prefix.Length } -Descending)

function Resolve-GroupBinding([string]$Key) {
    foreach ($binding in $prefixBindings) {
        $escaped = [regex]::Escape($binding.prefix)
        if ($Key -match "^$escaped(?<index>[0-9]+)$") {
            return [pscustomobject]@{
                group_id = $binding.group_id
                family_ordinal = $binding.family_ordinal
                prefix = $binding.prefix
                observed_index = [int]$Matches.index
                observed_legacy_index_key = "legacy-index-$($Matches.index)"
            }
        }
    }
    $null
}

$occurrenceBindings = [Collections.Generic.List[object]]::new()
for ($index = 0; $index -lt $plainKeys.Count; $index++) {
    $key = $plainKeys[$index]
    $plainOrdinal = $index + 1
    if ($key -eq 'dateFiled') {
        $occurrenceBindings.Add([pscustomobject][ordered]@{
            plaintext_ordinal = $plainOrdinal
            encrypted_ordinal = $null
            key = $key
            source_projection = [ordered]@{
                kind = 'generated-local-date'
                pattern = 'yyyy-mm-dd-slash'
                v2_context_value_id = 'local-current-date'
            }
            control_kind = 'generated-metadata'
            raw_value_kind = 'generated-local-date'
            candidate_v2_field_id = $null
            plaintext_save = [ordered]@{
                placement = 'pseudo-div'
                semantic_format = 'date-yyyy-mm-dd-slash'
                body_codec = 'raw-literal'
            }
            encrypted_staging = [ordered]@{
                placement = 'standalone-metadata-after-marker'
                semantic_format = 'date-yyyy-mm-dd-slash'
                body_codec = 'raw-literal'
            }
            source_refs = @(
                'official-hta-runtime#saveXML:L5620-L5628',
                'official-hta-runtime#saveEncryptedProfile:L5240-L5267'
            )
        })
        continue
    }

    $control = $controlById[$key]
    $groupBinding = Resolve-GroupBinding $key
    $semanticFormat = if ($control.control_kind -in @('radio', 'checkbox')) {
        'javascript-boolean'
    } else {
        'raw-text'
    }
    $sourceProjection = if ($groupBinding) {
        [ordered]@{
            kind = 'raw-group-field'
            group_id = $groupBinding.group_id
            family_ordinal = $groupBinding.family_ordinal
            family_prefix = $groupBinding.prefix
            observed_index = $groupBinding.observed_index
            observed_legacy_index_key = $groupBinding.observed_legacy_index_key
        }
    } else {
        [ordered]@{
            kind = 'raw-static-control'
            control_id = $key
        }
    }
    $candidateFieldId = if (
        $groupBinding -and $v2FieldIds.ContainsKey($groupBinding.prefix)
    ) {
        $groupBinding.prefix
    } elseif ($v2FieldIds.ContainsKey($key)) {
        $key
    } else {
        # Every observed static control has an exact candidate identity even
        # when its value projection remains documented-only and is therefore
        # intentionally absent from the executable field surface.
        $key
    }
    $plainCodec = if ($key -in @(
        'frm2550qv2024:taxpayerName',
        'frm2550qv2024:taxpayerAddress'
    )) {
        'legacy-javascript-escape'
    } else {
        'raw-literal'
    }
    $occurrenceBindings.Add([pscustomobject][ordered]@{
        plaintext_ordinal = $plainOrdinal
        encrypted_ordinal = $plainOrdinal
        key = $key
        source_projection = $sourceProjection
        control_kind = $control.control_kind
        raw_value_kind = $semanticFormat
        candidate_v2_field_id = $candidateFieldId
        plaintext_save = [ordered]@{
            placement = 'pseudo-div'
            semantic_format = $semanticFormat
            body_codec = $plainCodec
        }
        encrypted_staging = [ordered]@{
            placement = 'pseudo-div'
            semantic_format = $semanticFormat
            body_codec = 'raw-literal'
        }
        source_refs = @(
            "official-hta-runtime#control:L$($control.source_line)",
            'official-hta-runtime#saveXML:L5516-L5628',
            'official-hta-runtime#saveEncryptedProfile:L5194-L5267'
        )
    })
}

$groupContracts = @()
foreach ($group in $groupSpecs) {
    $observed = @($occurrenceBindings | Where-Object {
        $_.source_projection.kind -eq 'raw-group-field' -and
        $_.source_projection.group_id -eq $group.group_id
    })
    $observedIndices = @($observed | ForEach-Object {
        $_.source_projection.observed_index
    } | Sort-Object -Unique)
    if (($observedIndices -join ',') -ne ($group.observed_indices -join ',')) {
        throw "Observed instance set changed for group $($group.group_id)."
    }
    foreach ($instance in $observedIndices) {
        $instanceBindings = @($observed | Where-Object {
            $_.source_projection.observed_index -eq $instance
        })
        if ($instanceBindings.Count -ne $group.families.Count) {
            throw "Incomplete observed row $instance for group $($group.group_id)."
        }
        $actualPrefixes = @($instanceBindings | ForEach-Object {
            $_.source_projection.family_prefix
        })
        if (($actualPrefixes -join "`n") -ne ($group.families -join "`n")) {
            throw "Observed child order changed for group $($group.group_id), row $instance."
        }
    }
    $groupContracts += [pscustomobject][ordered]@{
        group_id = $group.group_id
        instance_order = 'legacy-zero-based-index-ascending'
        instance_identity = 'assigned-stable-id'
        serialization_instance_order_relation = 'unresolved'
        min_occurs = 0
        max_occurs = $null
        observed_indices = $observedIndices
        insert_after_key = $group.insert_after_key
        families = @(
            for ($familyOrdinal = 0; $familyOrdinal -lt $group.families.Count; $familyOrdinal++) {
                [ordered]@{
                    family_ordinal = $familyOrdinal + 1
                    key_prefix = $group.families[$familyOrdinal]
                    index_base = 0
                    index_step = 1
                    padding = 0
                }
            }
        )
        source_refs = @(
            "official-hta-runtime#dynamic-row-builders:L$($group.source_lines)",
            'v1-schedule-family-inventory#/families'
        )
    }
}

$fieldBoundCount = @($occurrenceBindings | Where-Object candidate_v2_field_id).Count
if ($fieldBoundCount -ne 159) {
    throw "Expected the current candidate to identify all 159 live observed controls; got $fieldBoundCount."
}
$contextBoundCount = @(
    $occurrenceBindings |
        Where-Object { $_.source_projection.v2_context_value_id }
).Count
if ($contextBoundCount -ne 1) {
    throw "Expected generated dateFiled to have one v2 context projection; got $contextBoundCount."
}
$modeledCount = $fieldBoundCount + $contextBoundCount

$output = [ordered]@{
    schema_version = '1.0.0'
    form_id = '2550q-v2024'
    package_version = '7.9.6.0'
    status = 'documented-only'
    values_emitted = $false
    input_sha256 = [ordered]@{
        plaintext_field_audit = $expectedHashes.plaintext
        encrypted_field_audit = $expectedHashes.encrypted
        runtime_control_inventory = $expectedHashes.controls
        schedule_family_inventory = $expectedHashes.families
        fields = $expectedHashes.fields
    }
    observed_contract = [ordered]@{
        plaintext_occurrence_count = $plainKeys.Count
        encrypted_pseudo_div_occurrence_count = $encryptedKeys.Count
        candidate_v2_bound_occurrence_count = $modeledCount
        plaintext_ordered_inventory_sha256 = $plaintext.ordered_field_inventory_sha256
        encrypted_ordered_inventory_sha256 = $encrypted.ordered_field_inventory_sha256
        encrypted_is_exact_plaintext_prefix = $true
        plaintext_only_suffix = 'dateFiled'
    }
    occurrence_bindings = $occurrenceBindings
    dynamic_groups = $groupContracts
    artifact_boundaries = @(
        [ordered]@{
            artifact_id = 'official-editable-save'
            control_sequence = 'occurrence_bindings plaintext order with dynamic groups'
            marker = 'All Rights Reserved BIR 2012.'
            marker_sample_status = 'source-established-no-separate-sample'
            date_filed_placement = 'final-pseudo-div-before-marker'
            executable = $false
        },
        [ordered]@{
            artifact_id = 'official-finalized-save'
            control_sequence = 'occurrence_bindings plaintext order with dynamic groups'
            marker = 'All Rights Reserved BIR 2012.0'
            marker_sample_status = 'source-and-sample-established'
            date_filed_placement = 'final-pseudo-div-before-marker'
            executable = $false
        },
        [ordered]@{
            artifact_id = 'official-encrypted-final-copy'
            control_sequence = 'first 159 occurrence bindings with dynamic groups'
            marker = 'All Rights Reserved BIR 2012.0'
            marker_sample_status = 'source-and-decrypted-sample-established'
            date_filed_placement = 'standalone-metadata-after-marker'
            encryption_provider_sha256 = '429337f44f84b93cd1095df48c8f3265e5ede7c646d1b48d9b80f4f92de74d2c'
            encryption_contract = 'opaque-unresolved'
            executable = $false
        }
    )
    unresolved_execution_boundaries = @(
        'Forty-four derived/alias controls and nine workflow/credential/UI-state controls remain documented-only rather than executable value projections.',
        'Executable serialization still lacks a reviewed mapping from assigned stable instance order to official live DOM/display order.',
        'The runtime separator, text encoding, newline conversion, and non-ASCII behavior are not byte-bound.',
        'The dateFiled context projection is reviewed, but its production clock and timezone provider remains unresolved.',
        'Filename construction, sanitization, overwrite behavior, path confinement, and custody remain unresolved.',
        'The external encryption container, success criteria, failure handling, and determinism remain unresolved.',
        'No filing-safe branch or reviewed production registry entry exists.'
    )
}

Write-Json $outputPath $output
Write-Output "Wrote value-free 2550Q serialization binding inventory: $outputPath"
Write-Output "SHA-256: $(Get-Sha256 $outputPath)"
