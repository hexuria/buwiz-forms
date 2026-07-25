param(
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))
)

$ErrorActionPreference = 'Stop'

$ruleSetPath = Join-Path $RepoRoot 'rules\ir\v2\2550q-v2024-p7.9.6.0\rule-set.json'
$bindingPath = Join-Path $RepoRoot 'rules\forms\2550q-v2024\fixtures\serialization-binding-inventory-v796.json'
$v1FieldsPath = Join-Path $RepoRoot 'rules\forms\2550q-v2024\fields.json'

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
$v1Fields = Read-Json $v1FieldsPath

$groups = @(
    [ordered]@{
        group_id = 'schedule-1-capital-good-row'
        members = @(
            'txtDatePurchase1', 'txtSourceCode1', 'txtDescription1',
            'txtAmountPurchase1', 'txtInputTax1', 'txtEstimatedLife1',
            'txtRecognizedLife1', 'txtAllowedInputTax1', 'txtBalanceInputTax1'
        )
    },
    [ordered]@{
        group_id = 'schedule-3-creditable-vat-row'
        members = @(
            'txtDateCovered3', 'txtDateCovered3To',
            'txtNameWithHoldingAgent3', 'txtIncomePayment3',
            'txtTotalTaxWithHeld3'
        )
    },
    [ordered]@{
        group_id = 'schedule-4-advance-vat-row'
        members = @(
            'txtDate4', 'txtDate4To', 'txtNameOfMiller4',
            'txtNameOfTaxpayer4', 'txtOfficialReceiptNumber4',
            'txtAmountPaid4'
        )
    },
    [ordered]@{
        group_id = 'item-19-additional-row'
        members = @(
            'frm2550qv2024:totalTaxPayableNo19Description',
            'frm2550qv2024:totalTaxPayableNo19Amount'
        )
    },
    [ordered]@{
        group_id = 'item-42-additional-row'
        members = @(
            'frm2550qv2024:totalTaxPayableNo42Description',
            'frm2550qv2024:totalTaxPayableNo42Amount'
        )
    },
    [ordered]@{
        group_id = 'item-47-additional-row'
        members = @(
            'frm2550qv2024:totalTaxPayableNo47Description',
            'frm2550qv2024:totalTaxPayableNo47Amount'
        )
    },
    [ordered]@{
        group_id = 'item-56-additional-row'
        members = @(
            'frm2550qv2024:totalTaxPayableNo56Description',
            'frm2550qv2024:totalTaxPayableNo56Amount'
        )
    }
)

$bindingGroups = @($binding.dynamic_groups | ForEach-Object { $_.group_id })
$expectedGroups = @($groups | ForEach-Object { $_.group_id })
if (($bindingGroups -join "`n") -ne ($expectedGroups -join "`n")) {
    throw 'Binding inventory group identities differ from the reviewed adapter identities.'
}

$memberIds = @($groups | ForEach-Object { $_.members })
if ($memberIds.Count -ne 28 -or @($memberIds | Sort-Object -Unique).Count -ne 28) {
    throw 'Expected 28 unique repeated member identities.'
}

$v1FieldIndexByKey = @{}
for ($fieldIndex = 0; $fieldIndex -lt $v1Fields.fields.Count; $fieldIndex++) {
    $fieldKey = $v1Fields.fields[$fieldIndex].field_key
    if ($fieldKey) {
        $v1FieldIndexByKey[$fieldKey] = $fieldIndex
    }
}
foreach ($memberId in $memberIds) {
    $descriptorKey = "$memberId{N>=0}"
    if (-not $v1FieldIndexByKey.ContainsKey($descriptorKey)) {
        throw "Missing v1 dynamic-family descriptor: $descriptorKey"
    }
}

$groupObjects = @(
    foreach ($group in $groups) {
        [ordered]@{
            group_id = $group.group_id
            min_occurs = 0
            max_occurs = $null
            instance_identity = 'assigned-stable-id'
            members = $group.members
            source_refs = @(
                [ordered]@{
                    source_id = 'v1-serialization-binding-inventory'
                    locator = "#/dynamic_groups/$([array]::IndexOf($groups, $group))"
                },
                [ordered]@{
                    source_id = 'candidate-group-projection-review'
                    locator = '#identity-decision'
                }
            )
        }
    }
)

$existingNonGroupFields = @($ruleSet.fields | Where-Object {
    $null -eq $_.group_id -or $memberIds -notcontains $_.field_id
})
$groupFields = @(
    foreach ($group in $groups) {
        foreach ($memberId in $group.members) {
            $descriptorKey = "$memberId{N>=0}"
            $v1FieldIndex = $v1FieldIndexByKey[$descriptorKey]
            [ordered]@{
                field_id = $memberId
                value_type = 'string'
                control_kind = 'text'
                requiredness = 'conditional'
                group_id = $group.group_id
                calculation_id = $null
                serialized = @()
                behavior = [ordered]@{
                    official = [ordered]@{
                        state = 'executable'
                        normalization = @()
                        coercion = [ordered]@{
                            kind = 'string'
                            on_empty = 'empty-string'
                        }
                        review_decision = [ordered]@{
                            source_id = 'candidate-group-projection-review'
                            locator = '#raw-string-behavior-decision'
                        }
                        source_refs = @(
                            [ordered]@{
                                source_id = 'v1-serialization-binding-inventory'
                                locator = '#/dynamic_groups'
                            },
                            [ordered]@{
                                source_id = 'candidate-group-projection-review'
                                locator = '#raw-string-behavior-decision'
                            }
                        )
                    }
                    filing_safe = [ordered]@{
                        state = 'unresolved'
                        reason = 'No filing-safe repeated-row lexical, calculation, validation, or serialization policy has been independently reviewed.'
                        source_refs = @(
                            [ordered]@{
                                source_id = 'candidate-group-projection-review'
                                locator = '#candidate-only-decision'
                            }
                        )
                    }
                }
                source_refs = @(
                    [ordered]@{
                        source_id = 'v1-fields'
                        locator = "#/fields/$v1FieldIndex"
                    },
                    [ordered]@{
                        source_id = 'v1-serialization-binding-inventory'
                        locator = '#/dynamic_groups'
                    },
                    [ordered]@{
                        source_id = 'candidate-group-projection-review'
                        locator = '#raw-string-behavior-decision'
                    }
                )
            }
        }
    }
)

$ruleSet.field_groups = $groupObjects
$ruleSet.fields = @($existingNonGroupFields + $groupFields)

if ($ruleSet.field_groups.Count -ne 7 -or $ruleSet.fields.Count -ne 60) {
    throw "Expected 7 groups and 60 total candidate fields; found $($ruleSet.field_groups.Count) groups and $($ruleSet.fields.Count) fields."
}

Write-Json $ruleSetPath $ruleSet
Write-Output "Updated 2550Q v2 group projections: 7 groups, 28 members, 60 total fields."
