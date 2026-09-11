$ErrorActionPreference = 'Stop'
$path = 'C:\eBIRForms\ebfSFTP.exe'
$bytes = [System.IO.File]::ReadAllBytes($path)
$asm = [System.Reflection.Assembly]::Load($bytes)   # Load, not run
$mod = $asm.GetModules()[0]

# Build opcode map (name -> OpCode) for both 1-byte and 2-byte (0xFE prefix) opcodes
$opsByValue = @{}
foreach ($f in [System.Reflection.Emit.OpCodes].GetFields([System.Reflection.BindingFlags]'Public,Static')) {
    $op = $f.GetValue($null)
    $opsByValue[([int]$op.Value -band 0xFFFF)] = $op
}

function Dump-IL($mi) {
    $body = $mi.GetMethodBody()
    if ($null -eq $body) { Write-Output "    <no body>"; return }
    $il = $body.GetILAsByteArray()
    $i = 0
    while ($i -lt $il.Length) {
        $offset = $i
        $b = $il[$i]; $i++
        if ($b -eq 0xFE) { $val = 0xFE00 -bor $il[$i]; $i++ } else { $val = $b }
        $op = $opsByValue[[int]$val]
        if ($null -eq $op) { Write-Output ("    IL_{0:X4}: .byte 0x{1:X2}" -f $offset,$b); continue }
        $name = $op.Name
        $operand = ''
        switch ($op.OperandType.ToString()) {
            'InlineNone'      { }
            'ShortInlineI'    { $operand = "$([sbyte]$il[$i])"; $i+=1 }
            'ShortInlineVar'  { $operand = "$($il[$i])"; $i+=1 }
            'ShortInlineBrTarget' { $t=[sbyte]$il[$i]; $i+=1; $operand="IL_{0:X4}" -f ($i+$t) }
            'InlineI'         { $operand = "$([BitConverter]::ToInt32($il,$i))"; $i+=4 }
            'InlineVar'       { $operand = "$([BitConverter]::ToUInt16($il,$i))"; $i+=2 }
            'InlineBrTarget'  { $t=[BitConverter]::ToInt32($il,$i); $i+=4; $operand="IL_{0:X4}" -f ($i+$t) }
            'InlineI8'        { $operand = "$([BitConverter]::ToInt64($il,$i))"; $i+=8 }
            'ShortInlineR'    { $operand = "$([BitConverter]::ToSingle($il,$i))"; $i+=4 }
            'InlineR'         { $operand = "$([BitConverter]::ToDouble($il,$i))"; $i+=8 }
            'InlineString'    { $tok=[BitConverter]::ToInt32($il,$i); $i+=4; try { $operand='"'+$mod.ResolveString($tok)+'"' } catch { $operand="str(0x{0:X8})" -f $tok } }
            'InlineMethod'    { $tok=[BitConverter]::ToInt32($il,$i); $i+=4; try { $m=$mod.ResolveMethod($tok); $operand=("{0}::{1}" -f $m.DeclaringType.FullName,$m.Name) } catch { $operand="meth(0x{0:X8})" -f $tok } }
            'InlineField'     { $tok=[BitConverter]::ToInt32($il,$i); $i+=4; try { $fld=$mod.ResolveField($tok); $operand=("{0}::{1}" -f $fld.DeclaringType.FullName,$fld.Name) } catch { $operand="fld(0x{0:X8})" -f $tok } }
            'InlineType'      { $tok=[BitConverter]::ToInt32($il,$i); $i+=4; try { $t=$mod.ResolveType($tok); $operand=$t.FullName } catch { $operand="type(0x{0:X8})" -f $tok } }
            'InlineTok'       { $tok=[BitConverter]::ToInt32($il,$i); $i+=4; $operand="tok(0x{0:X8})" -f $tok }
            'InlineSig'       { $tok=[BitConverter]::ToInt32($il,$i); $i+=4; $operand="sig(0x{0:X8})" -f $tok }
            'InlineSwitch'    { $n=[BitConverter]::ToInt32($il,$i); $i+=4; $i+=4*$n; $operand="switch[$n]" }
            default           { $operand = "?"+$op.OperandType }
        }
        Write-Output ("    IL_{0:X4}: {1,-12} {2}" -f $offset,$name,$operand)
    }
}

foreach ($t in $asm.GetTypes()) {
    Write-Output ("TYPE {0}  (base={1})" -f $t.FullName, $t.BaseType)
    foreach ($fld in $t.GetFields([System.Reflection.BindingFlags]'Public,NonPublic,Static,Instance')) {
        Write-Output ("  FIELD {0} {1}" -f $fld.FieldType.Name, $fld.Name)
    }
    $methods = @($t.GetConstructors([System.Reflection.BindingFlags]'Public,NonPublic,Static,Instance')) + @($t.GetMethods([System.Reflection.BindingFlags]'Public,NonPublic,Static,Instance,DeclaredOnly'))
    foreach ($mi in $methods) {
        $ps = ($mi.GetParameters() | ForEach-Object { "$($_.ParameterType.Name) $($_.Name)" }) -join ', '
        Write-Output ("  METHOD {0}({1})" -f $mi.Name, $ps)
        try { Dump-IL $mi } catch { Write-Output "    <il error: $_>" }
    }
    Write-Output ""
}
