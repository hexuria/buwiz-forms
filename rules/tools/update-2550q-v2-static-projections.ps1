param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

$ErrorActionPreference = 'Stop'

$ruleSetPath = Join-Path $RepoRoot 'rules\ir\v2\2550q-v2024-p7.9.6.0\rule-set.json'
$bindingPath = Join-Path $RepoRoot 'rules\forms\2550q-v2024\fixtures\serialization-binding-inventory-v796.json'
$runtimePath = Join-Path $RepoRoot 'rules\forms\2550q-v2024\fixtures\runtime-control-inventory-v796.json'
$v1FieldsPath = Join-Path $RepoRoot 'rules\forms\2550q-v2024\fields.json'
$coreAdapterPath = Join-Path $RepoRoot 'crates\bir-core\src\form_rules\form_2550q.rs'

function Read-Json([string]$Path) {
    Get-Content -Raw -LiteralPath $Path | ConvertFrom-Json
}

function Write-Json([string]$Path, $Value) {
    [IO.File]::WriteAllText(
        $Path,
        (($Value | ConvertTo-Json -Depth 100) + [Environment]::NewLine),
        [Text.UTF8Encoding]::new($false)
    )
}

$ruleSet = Read-Json $ruleSetPath
$binding = Read-Json $bindingPath
$runtime = Read-Json $runtimePath
$v1Fields = Read-Json $v1FieldsPath
$coreAdapter = Get-Content -Raw -LiteralPath $coreAdapterPath

$projectionSourceId = 'candidate-static-surface-projection-review'
$reviewedRadioKeys = @(
    'frm2550qv2024:amendedReturnYesNo5',
    'frm2550qv2024:amendedReturnNo5',
    'frm2550qv2024:OptShortPrd1',
    'frm2550qv2024:OptShortPrd2'
)
$workflowKeys = @(
    'driveSelectTPExport',
    'ebirOnlineConfirmUsername',
    'ebirOnlineSecret',
    'ebirOnlineUsername',
    'frm2550qv2024:txtCurrentPage',
    'frm2550qv2024:txtMaxPage',
    'txtEmail',
    'txtEnroll',
    'txtFinalFlag'
)

$existingProjectedIds = @(
    $ruleSet.fields |
        Where-Object {
            -not @($_.source_refs | Where-Object source_id -eq $projectionSourceId).Count
        } |
        ForEach-Object field_id
)
$targets = @(
    $binding.occurrence_bindings |
        Where-Object {
            $_.source_projection.kind -eq 'raw-static-control' -and
            $existingProjectedIds -notcontains $_.key
        }
)
if ($targets.Count -ne 87 -or @($targets.key | Sort-Object -Unique).Count -ne 87) {
    throw "Expected 87 unique unprojected static controls; found $($targets.Count)."
}

$runtimeIndexById = @{}
$targetKeySet = @{}
foreach ($key in $targets.key) {
    $targetKeySet[$key] = $true
}
for ($index = 0; $index -lt $runtime.static_controls.Count; $index++) {
    $controlId = $runtime.static_controls[$index].id
    if ($controlId -and $targetKeySet.ContainsKey($controlId)) {
        if ($runtimeIndexById.ContainsKey($controlId)) {
            throw "Duplicate runtime control identity: $controlId"
        }
        $runtimeIndexById[$controlId] = $index
    }
}
$v1IndexByKey = @{}
for ($index = 0; $index -lt $v1Fields.fields.Count; $index++) {
    $fieldKey = $v1Fields.fields[$index].field_key
    if ($fieldKey -and $targetKeySet.ContainsKey($fieldKey)) {
        if ($v1IndexByKey.ContainsKey($fieldKey)) {
            throw "Duplicate v1 field identity: $fieldKey"
        }
        $v1IndexByKey[$fieldKey] = $index
    }
}
$bindingIndexByKey = @{}
for ($index = 0; $index -lt $binding.occurrence_bindings.Count; $index++) {
    $key = $binding.occurrence_bindings[$index].key
    if ($targetKeySet.ContainsKey($key)) {
        if ($bindingIndexByKey.ContainsKey($key)) {
            throw "Duplicate observed static occurrence identity: $key"
        }
        $bindingIndexByKey[$key] = $index
    }
}

$rawKeys = @()
$derivedKeys = @()
foreach ($target in $targets) {
    $key = $target.key
    if (
        -not $runtimeIndexById.ContainsKey($key) -or
        -not $v1IndexByKey.ContainsKey($key) -or
        -not $bindingIndexByKey.ContainsKey($key)
    ) {
        throw "Static projection lacks exact evidence indices: $key"
    }
    $isCoreRaw = $coreAdapter.Contains(('"' + $key + '"'))
    if ($isCoreRaw) {
        $rawKeys += $key
    } elseif ($workflowKeys -notcontains $key) {
        $derivedKeys += $key
    }
}
if (
    $rawKeys.Count -ne 34 -or
    $derivedKeys.Count -ne 44 -or
    @($targets.key | Where-Object { $workflowKeys -contains $_ }).Count -ne 9
) {
    throw "Static projection category counts changed (raw=$($rawKeys.Count), derived=$($derivedKeys.Count))."
}
foreach ($key in $reviewedRadioKeys + $workflowKeys) {
    if ($targets.key -notcontains $key) {
        throw "Reviewed static category key disappeared: $key"
    }
}
foreach ($key in $reviewedRadioKeys) {
    if ($rawKeys -notcontains $key) {
        throw "Reviewed radio control lost its exact core raw binding: $key"
    }
}

$newFields = @(
    foreach ($target in $targets) {
        $key = $target.key
        $runtimeIndex = $runtimeIndexById[$key]
        $v1Index = $v1IndexByKey[$key]
        $bindingIndex = $bindingIndexByKey[$key]
        $control = $runtime.static_controls[$runtimeIndex]
        $v1Field = $v1Fields.fields[$v1Index]
        $category = if ($rawKeys -contains $key) {
            'raw'
        } elseif ($workflowKeys -contains $key) {
            'workflow'
        } else {
            'derived'
        }
        # Only executable raw captures belong in the candidate field surface.
        # Derived/alias and workflow identities remain bound in the value-free
        # occurrence inventory until their executable behavior is reviewed.
        if ($category -ne 'raw') {
            continue
        }
        $requiredness = if ($category -eq 'workflow') {
            'hidden'
        } elseif ($category -eq 'derived') {
            'computed'
        } else {
            [string]$v1Field.required
        }
        if ($requiredness -notin @('required', 'optional', 'conditional', 'computed', 'hidden')) {
            throw "Unsupported requiredness $requiredness for $key"
        }

        $official = if ($category -eq 'raw') {
            $isBoolean = $control.control_kind -eq 'radio'
            [ordered]@{
                state = 'executable'
                normalization = @()
                coercion = if ($isBoolean) {
                    [ordered]@{
                        kind = 'boolean'
                        true_values = @('true')
                        false_values = @('false')
                        on_empty = 'error'
                        on_invalid = 'error'
                    }
                } else {
                    [ordered]@{
                        kind = 'string'
                        on_empty = 'empty-string'
                    }
                }
                review_decision = [ordered]@{
                    source_id = $projectionSourceId
                    locator = '#raw-lexical-controls'
                }
                source_refs = @(
                    [ordered]@{
                        source_id = 'v1-serialization-binding-inventory'
                        locator = "#/occurrence_bindings/$bindingIndex"
                    },
                    [ordered]@{
                        source_id = 'v1-runtime-control-inventory'
                        locator = "#/static_controls/$runtimeIndex"
                    },
                    [ordered]@{
                        source_id = $projectionSourceId
                        locator = '#raw-lexical-controls'
                    }
                )
            }
        } else {
            $locator = if ($category -eq 'workflow') {
                '#workflow-credential-and-ui-state-controls'
            } else {
                '#derived-and-alias-controls'
            }
            [ordered]@{
                state = 'documented_only'
                summary = if ($category -eq 'workflow') {
                    'Package workflow, credential, or UI state is identified but is not executable tax-form field behavior.'
                } else {
                    'Derived or alias control identity is established, but its authoritative expression, ordering, source relation, or lexical rendering is not yet executable.'
                }
                source_refs = @(
                    [ordered]@{
                        source_id = 'v1-serialization-binding-inventory'
                        locator = "#/occurrence_bindings/$bindingIndex"
                    },
                    [ordered]@{
                        source_id = 'v1-runtime-control-inventory'
                        locator = "#/static_controls/$runtimeIndex"
                    },
                    [ordered]@{
                        source_id = $projectionSourceId
                        locator = $locator
                    }
                )
            }
        }

        [ordered]@{
            field_id = $key
            value_type = if ($category -eq 'raw' -and $control.control_kind -eq 'radio') {
                'boolean'
            } else {
                'string'
            }
            control_kind = if ($category -eq 'workflow') {
                'hidden'
            } elseif ($category -eq 'derived') {
                'computed'
            } else {
                [string]$control.control_kind
            }
            requiredness = $requiredness
            group_id = $null
            calculation_id = $null
            serialized = @()
            behavior = [ordered]@{
                official = $official
                filing_safe = [ordered]@{
                    state = 'unresolved'
                    reason = 'No independent filing-safe field, derivation, workflow-state, or serialization policy has been reviewed.'
                    source_refs = @(
                        [ordered]@{
                            source_id = $projectionSourceId
                            locator = '#candidate-only-boundary'
                        }
                    )
                }
            }
            source_refs = @(
                [ordered]@{
                    source_id = 'v1-fields'
                    locator = "#/fields/$v1Index"
                },
                [ordered]@{
                    source_id = 'v1-runtime-control-inventory'
                    locator = "#/static_controls/$runtimeIndex"
                },
                [ordered]@{
                    source_id = 'v1-serialization-binding-inventory'
                    locator = "#/occurrence_bindings/$bindingIndex"
                },
                [ordered]@{
                    source_id = $projectionSourceId
                    locator = '#candidate-only-boundary'
                }
            )
        }
    }
)

$preservedFields = @(
    $ruleSet.fields |
        Where-Object {
            -not @($_.source_refs | Where-Object source_id -eq $projectionSourceId).Count
        }
)
$ruleSet.fields = @($preservedFields + $newFields)
if ($ruleSet.fields.Count -ne 94) {
    throw "Expected 94 executable candidate fields after static projection; found $($ruleSet.fields.Count)."
}
if (@($ruleSet.fields.field_id | Sort-Object -Unique).Count -ne 94) {
    throw 'Static projection introduced duplicate candidate field identities.'
}

Write-Json $ruleSetPath $ruleSet

$singletonFields = @(
    $ruleSet.fields |
        Where-Object { $null -eq $_.group_id } |
        Sort-Object field_id
)
$fixturePaths = @(
    Get-ChildItem -LiteralPath (Join-Path (Split-Path -Parent $ruleSetPath) 'fixtures') -Filter '*.json' -File |
        Sort-Object Name
)
foreach ($fixturePath in $fixturePaths) {
    $fixture = Read-Json $fixturePath.FullName
    $rawById = @{}
    foreach ($item in @($fixture.input.raw_inputs.fields)) {
        $rawById[[string]$item.field.field_id] = $item
    }
    $canonicalById = @{}
    foreach ($item in @($fixture.expected.canonical_inputs)) {
        $canonicalById[[string]$item.field.field_id] = $item
    }

    foreach ($field in $singletonFields) {
        $fieldId = [string]$field.field_id
        $isStaticProjection = @(
            $field.source_refs |
                Where-Object source_id -eq $projectionSourceId
        ).Count -gt 0
        if (-not $rawById.ContainsKey($fieldId)) {
            $text = if ($field.value_type -eq 'boolean') { 'false' } else { '' }
            $rawById[$fieldId] = [ordered]@{
                field = [ordered]@{
                    field_id = $fieldId
                    group_path = @()
                }
                value = [ordered]@{
                    state = 'text'
                    text = $text
                }
            }
        }
        if (-not $canonicalById.ContainsKey($fieldId) -or $isStaticProjection) {
            $raw = $rawById[$fieldId].value
            $canonical = if ($raw.state -eq 'absent') {
                [ordered]@{ type = 'absent' }
            } elseif ($field.value_type -eq 'boolean') {
                [ordered]@{
                    type = 'boolean'
                    value = [string]$raw.text -eq 'true'
                }
            } else {
                [ordered]@{
                    type = 'text'
                    value = [string]$raw.text
                }
            }
            $canonicalById[$fieldId] = [ordered]@{
                field = [ordered]@{
                    field_id = $fieldId
                    group_path = @()
                }
                raw = $raw
                canonical = $canonical
            }
        }
    }

    [string[]]$rawIds = @($rawById.Keys)
    [Array]::Sort($rawIds, [StringComparer]::Ordinal)
    [string[]]$canonicalIds = @($canonicalById.Keys)
    [Array]::Sort($canonicalIds, [StringComparer]::Ordinal)
    $fixture.input.raw_inputs.fields = @($rawIds | ForEach-Object { $rawById[$_] })
    $fixture.expected.canonical_inputs = @(
        $canonicalIds | ForEach-Object { $canonicalById[$_] }
    )
    Write-Json $fixturePath.FullName $fixture
}

Write-Output "Updated 2550Q v2 static projections: 34 executable raw fields, 53 identity-only documented controls, 94 total fields, and $($fixturePaths.Count) fixtures."
