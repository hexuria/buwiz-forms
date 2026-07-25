param([string]$RulesRoot = (Split-Path -Parent $MyInvocation.MyCommand.Path))
$ErrorActionPreference = 'Stop'

function Get-ObjectProperty($value, [string]$name) {
    if ($null -eq $value) { return $null }
    return $value.PSObject.Properties[$name]
}

function Resolve-LocalSchemaReference($rootSchema, [string]$reference) {
    if (-not $reference.StartsWith('#/')) { throw "Only local JSON Schema references are supported: $reference" }
    $current = $rootSchema
    foreach ($encodedSegment in $reference.Substring(2).Split('/')) {
        $segment = $encodedSegment.Replace('~1', '/').Replace('~0', '~')
        $property = Get-ObjectProperty $current $segment
        if ($null -eq $property) { throw "Unresolved JSON Schema reference: $reference" }
        $current = $property.Value
    }
    return $current
}

function Test-JsonInstanceType($value, [string]$type) {
    switch ($type) {
        'null' { return $null -eq $value }
        'object' { return $null -ne $value -and $value -is [pscustomobject] }
        'array' { return $null -ne $value -and $value -is [System.Collections.IList] -and $value -isnot [string] }
        'string' { return $value -is [string] }
        'integer' { return $value -is [sbyte] -or $value -is [byte] -or $value -is [int16] -or $value -is [uint16] -or $value -is [int32] -or $value -is [uint32] -or $value -is [int64] -or $value -is [uint64] }
        'number' { return $value -is [ValueType] -and $value -isnot [bool] }
        'boolean' { return $value -is [bool] }
        default { throw "Unsupported JSON Schema type keyword: $type" }
    }
}

function Assert-JsonSchemaValue([Parameter(Mandatory=$false)][AllowNull()][AllowEmptyCollection()]$value, $schema, $rootSchema, [string]$instancePath) {
    $reference = Get-ObjectProperty $schema '$ref'
    if ($null -ne $reference) {
        Assert-JsonSchemaValue $value (Resolve-LocalSchemaReference $rootSchema $reference.Value) $rootSchema $instancePath
        return
    }

    $typeProperty = Get-ObjectProperty $schema 'type'
    if ($null -ne $typeProperty) {
        $allowedTypes = @($typeProperty.Value)
        $matches = $false
        foreach ($allowedType in $allowedTypes) {
            if (Test-JsonInstanceType $value $allowedType) { $matches = $true; break }
        }
        if (-not $matches) { throw "$instancePath does not match schema type: $($allowedTypes -join ', ')" }
    }

    $enumProperty = Get-ObjectProperty $schema 'enum'
    if ($null -ne $enumProperty) {
        $actual = $value | ConvertTo-Json -Compress -Depth 50
        $matches = $false
        foreach ($candidate in @($enumProperty.Value)) {
            if (($candidate | ConvertTo-Json -Compress -Depth 50) -eq $actual) { $matches = $true; break }
        }
        if (-not $matches) { throw "$instancePath is not one of the schema enum values" }
    }

    if ($value -is [string]) {
        $minLength = Get-ObjectProperty $schema 'minLength'
        if ($null -ne $minLength -and $value.Length -lt $minLength.Value) { throw "$instancePath is shorter than minLength" }
        $pattern = Get-ObjectProperty $schema 'pattern'
        if ($null -ne $pattern -and $value -notmatch $pattern.Value) { throw "$instancePath does not match schema pattern" }
        $format = Get-ObjectProperty $schema 'format'
        if ($null -ne $format -and $format.Value -eq 'date') {
            $parsed = [datetime]::MinValue
            if (-not [datetime]::TryParseExact($value, 'yyyy-MM-dd', [Globalization.CultureInfo]::InvariantCulture, [Globalization.DateTimeStyles]::None, [ref]$parsed)) {
                throw "$instancePath is not an RFC 3339 full-date"
            }
        }
    }

    if ($value -is [ValueType] -and $value -isnot [bool]) {
        $minimum = Get-ObjectProperty $schema 'minimum'
        if ($null -ne $minimum -and $value -lt $minimum.Value) { throw "$instancePath is below schema minimum" }
    }

    if ($value -is [System.Collections.IList] -and $value -isnot [string]) {
        $minItems = Get-ObjectProperty $schema 'minItems'
        if ($null -ne $minItems -and $value.Count -lt $minItems.Value) { throw "$instancePath has fewer than minItems" }
        $uniqueItems = Get-ObjectProperty $schema 'uniqueItems'
        if ($null -ne $uniqueItems -and $uniqueItems.Value) {
            $serialized = @($value | ForEach-Object { $_ | ConvertTo-Json -Compress -Depth 50 })
            if (($serialized | Sort-Object -Unique).Count -ne $serialized.Count) { throw "$instancePath violates uniqueItems" }
        }
        $items = Get-ObjectProperty $schema 'items'
        if ($null -ne $items) {
            for ($index = 0; $index -lt $value.Count; $index++) {
                Assert-JsonSchemaValue $value[$index] $items.Value $rootSchema "$instancePath/$index"
            }
        }
    }

    if ($null -ne $value -and $value -is [pscustomobject]) {
        $required = Get-ObjectProperty $schema 'required'
        if ($null -ne $required) {
            foreach ($name in @($required.Value)) {
                if ($null -eq (Get-ObjectProperty $value $name)) { throw "$instancePath is missing required property: $name" }
            }
        }
        $properties = Get-ObjectProperty $schema 'properties'
        if ($null -ne $properties) {
            foreach ($schemaProperty in $properties.Value.PSObject.Properties) {
                $instanceProperty = Get-ObjectProperty $value $schemaProperty.Name
                if ($null -ne $instanceProperty) {
                    Assert-JsonSchemaValue -value $instanceProperty.Value -schema $schemaProperty.Value -rootSchema $rootSchema -instancePath "$instancePath/$($schemaProperty.Name)"
                }
            }
        }
        $additional = Get-ObjectProperty $schema 'additionalProperties'
        if ($null -ne $additional) {
            $knownNames = @()
            if ($null -ne $properties) { $knownNames = @($properties.Value.PSObject.Properties.Name) }
            foreach ($instanceProperty in $value.PSObject.Properties) {
                if ($knownNames -contains $instanceProperty.Name) { continue }
                if ($additional.Value -is [bool] -and -not $additional.Value) {
                    throw "$instancePath has disallowed additional property: $($instanceProperty.Name)"
                }
                if ($additional.Value -is [pscustomobject]) {
                    Assert-JsonSchemaValue -value $instanceProperty.Value -schema $additional.Value -rootSchema $rootSchema -instancePath "$instancePath/$($instanceProperty.Name)"
                }
            }
        }
    }
}

$validated = @()
$v2IrRoot = (Join-Path $RulesRoot 'ir\v2').TrimEnd('\') + '\'
$v2SchemaRoot = (Join-Path $RulesRoot 'schema\v2').TrimEnd('\') + '\'
$dataFiles = Get-ChildItem -LiteralPath $RulesRoot -Recurse -File -Filter '*.json' | Where-Object {
    $_.FullName -notmatch '\\schema\\' -and
    -not $_.FullName.StartsWith($v2IrRoot, [System.StringComparison]::OrdinalIgnoreCase) -and
    -not $_.FullName.StartsWith($v2SchemaRoot, [System.StringComparison]::OrdinalIgnoreCase)
}
foreach ($file in $dataFiles) {
    $document = Get-Content -Raw -LiteralPath $file.FullName | ConvertFrom-Json
    $schemaPointer = Get-ObjectProperty $document '$schema'
    if ($null -eq $schemaPointer) { continue }
    $schemaPath = [IO.Path]::GetFullPath((Join-Path $file.DirectoryName $schemaPointer.Value))
    if (-not (Test-Path -LiteralPath $schemaPath)) { throw "Missing schema for $($file.FullName): $schemaPath" }
    $schema = Get-Content -Raw -LiteralPath $schemaPath | ConvertFrom-Json
    Assert-JsonSchemaValue $document $schema $schema '$'
    $validated += $file.FullName.Substring($RulesRoot.Length).TrimStart('\')
}
[pscustomobject]@{
    validator = 'tracked-json-schema-keyword-subset-v1'
    supported_keywords = @('$ref','type','required','properties','additionalProperties','items','enum','pattern','minimum','minLength','minItems','uniqueItems','format:date')
    schema_documents = $validated.Count
    validated = $validated
    result = 'pass'
} | ConvertTo-Json -Depth 10
