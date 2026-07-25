param(
    [Parameter(Mandatory = $true)][string]$PackagePath,
    [int]$ResourceType = 23,
    [int]$ManifestResourceId = 129,
    [int[]]$InspectResourceIds = @()
)

$ErrorActionPreference = 'Stop'

if (-not ('EbirPackageResources' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class EbirPackageResources {
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
    static extern IntPtr LoadLibraryEx(string file, IntPtr handle, uint flags);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool FreeLibrary(IntPtr module);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern IntPtr FindResource(IntPtr module, IntPtr name, IntPtr type);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern IntPtr LoadResource(IntPtr module, IntPtr resource);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern IntPtr LockResource(IntPtr resource);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern uint SizeofResource(IntPtr module, IntPtr resource);
    delegate bool EnumNameProc(IntPtr module, IntPtr type, IntPtr name, IntPtr parameter);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool EnumResourceNames(IntPtr module, IntPtr type, EnumNameProc callback, IntPtr parameter);
    delegate bool EnumTypeProc(IntPtr module, IntPtr type, IntPtr parameter);
    [DllImport("kernel32.dll", SetLastError=true)]
    static extern bool EnumResourceTypes(IntPtr module, EnumTypeProc callback, IntPtr parameter);

    static byte[] Read(IntPtr module, int type, int name) {
        IntPtr resource = FindResource(module, (IntPtr)name, (IntPtr)type);
        if (resource == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        uint size = SizeofResource(module, resource);
        byte[] bytes = new byte[size];
        Marshal.Copy(LockResource(LoadResource(module, resource)), bytes, 0, (int)size);
        return bytes;
    }

    public static byte[] Read(string path, int type, int name) {
        IntPtr module = LoadLibraryEx(path, IntPtr.Zero, 2);
        if (module == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        try { return Read(module, type, name); }
        finally { FreeLibrary(module); }
    }

    public static int[] ListIds(string path, int type) {
        IntPtr module = LoadLibraryEx(path, IntPtr.Zero, 2);
        if (module == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        var ids = new List<int>();
        EnumNameProc callback = (m, t, name, parameter) => {
            long value = name.ToInt64();
            if (value >= 0 && value <= 65535) ids.Add((int)value);
            return true;
        };
        try {
            EnumResourceNames(module, (IntPtr)type, callback, IntPtr.Zero);
            ids.Sort();
            return ids.ToArray();
        }
        finally { FreeLibrary(module); }
    }

    public static int[] ListTypes(string path) {
        IntPtr module = LoadLibraryEx(path, IntPtr.Zero, 2);
        if (module == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
        var types = new List<int>();
        EnumTypeProc callback = (m, type, parameter) => {
            long value = type.ToInt64();
            if (value >= 0 && value <= 65535) types.Add((int)value);
            return true;
        };
        try {
            EnumResourceTypes(module, callback, IntPtr.Zero);
            types.Sort();
            return types.ToArray();
        }
        finally { FreeLibrary(module); }
    }
}
'@
}

function Get-Sha256([byte[]]$Bytes) {
    $hash = [Security.Cryptography.SHA256]::Create().ComputeHash($Bytes)
    return -join ($hash | ForEach-Object { $_.ToString('x2') })
}

function Decode-Resource([byte[]]$Bytes) {
    $decoded = [byte[]]::new($Bytes.Length)
    for ($index = 0; $index -lt $Bytes.Length; $index++) {
        $decoded[$index] = $Bytes[$index] -bxor 0xff
    }
    return $decoded
}

$package = [IO.File]::ReadAllBytes($PackagePath)
$resourceTypes = [EbirPackageResources]::ListTypes($PackagePath)
$resourceIds = [EbirPackageResources]::ListIds($PackagePath, $ResourceType)
$manifestBytes = [EbirPackageResources]::Read($PackagePath, $ResourceType, $ManifestResourceId)
$manifestText = [Text.Encoding]::Unicode.GetString($manifestBytes).TrimStart([char]0xfeff).Trim([char]0)
$manifestEntries = @($manifestText.Split('|'))
$manifestRows = for ($index = 0; $index -lt $manifestEntries.Count; $index++) {
    [ordered]@{
        index = $index
        resource_id = $ManifestResourceId + $index
        path = $manifestEntries[$index]
    }
}

$decodedMzIds = @()
foreach ($resourceId in $resourceIds) {
    if ($resourceId -eq $ManifestResourceId) { continue }
    $bytes = [EbirPackageResources]::Read($PackagePath, $ResourceType, $resourceId)
    if ($bytes.Length -ge 2 -and (($bytes[0] -bxor 0xff) -eq 0x4d) -and (($bytes[1] -bxor 0xff) -eq 0x5a)) {
        $decodedMzIds += $resourceId
    }
}

$rawMzResources = @()
$xorMzResources = @()
$resourceReadErrors = @()
foreach ($typeId in $resourceTypes) {
    try {
        $typeResourceIds = [EbirPackageResources]::ListIds($PackagePath, $typeId)
    } catch {
        $resourceReadErrors += [ordered]@{
            type = $typeId
            resource_id = $null
            error = $_.Exception.GetBaseException().Message
        }
        continue
    }
    foreach ($resourceId in $typeResourceIds) {
        try {
            $bytes = [EbirPackageResources]::Read($PackagePath, $typeId, $resourceId)
        } catch {
            $resourceReadErrors += [ordered]@{
                type = $typeId
                resource_id = $resourceId
                error = $_.Exception.GetBaseException().Message
            }
            continue
        }
        if ($bytes.Length -ge 2 -and $bytes[0] -eq 0x4d -and $bytes[1] -eq 0x5a) {
            $rawMzResources += [ordered]@{ type = $typeId; resource_id = $resourceId }
        }
        if ($bytes.Length -ge 2 -and (($bytes[0] -bxor 0xff) -eq 0x4d) -and (($bytes[1] -bxor 0xff) -eq 0x5a)) {
            $xorMzResources += [ordered]@{ type = $typeId; resource_id = $resourceId }
        }
    }
}

$inspected = foreach ($resourceId in $InspectResourceIds) {
    $raw = [EbirPackageResources]::Read($PackagePath, $ResourceType, $resourceId)
    $decoded = if ($resourceId -eq $ManifestResourceId) { $raw } else { Decode-Resource $raw }
    [ordered]@{
        resource_id = $resourceId
        manifest_path = @($manifestRows | Where-Object { $_.resource_id -eq $resourceId } | ForEach-Object { $_.path })[0]
        size = $raw.Length
        raw_sha256 = Get-Sha256 $raw
        decoded_sha256 = Get-Sha256 $decoded
        decoded_first16 = -join ($decoded[0..([Math]::Min(15, $decoded.Length - 1))] | ForEach-Object { $_.ToString('x2') })
    }
}

[ordered]@{
    package_path = $PackagePath
    package_size = $package.Length
    package_sha256 = Get-Sha256 $package
    resource_types = $resourceTypes
    resource_type = $ResourceType
    resource_count = $resourceIds.Count
    manifest_resource_id = $ManifestResourceId
    manifest_size = $manifestBytes.Length
    manifest_sha256 = Get-Sha256 $manifestBytes
    manifest_entry_count = $manifestEntries.Count
    manifest_executables = @($manifestRows | Where-Object { $_.path -match '(?i)\.exe$' })
    manifest_transport_or_checksum_helpers = @($manifestRows | Where-Object { $_.path -match '(?i)chkt|encrypt|ftp|send' })
    decoded_mz_resource_ids = $decodedMzIds
    raw_mz_resources_all_types = $rawMzResources
    xor_mz_resources_all_types = $xorMzResources
    resource_read_errors_all_types = $resourceReadErrors
    inspected_resources = @($inspected)
} | ConvertTo-Json -Depth 8
