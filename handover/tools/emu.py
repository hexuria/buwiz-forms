"""Run chkt.exe (FPC i386 Win32) under unicorn with stubbed kernel32 imports.

Usage: python3 -I tools/emu.py chkt.exe TIN [TIN...]  -> prints "TIN exitcode"
"""
import os, struct, sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'pylib'))
from unicorn import Uc, UC_ARCH_X86, UC_MODE_32, UC_HOOK_CODE, UcError
from unicorn.x86_const import *

# stdcall argument counts for every import the binary uses.
ARGS = {
    'CloseHandle': 1, 'CreateEventA': 4, 'CreateFileA': 7, 'CreateThread': 6,
    'DeleteCriticalSection': 1, 'EnterCriticalSection': 1, 'EnumResourceLanguagesA': 5,
    'EnumResourceNamesA': 4, 'EnumResourceTypesA': 3, 'ExitProcess': 1, 'ExitThread': 1,
    'FindResourceA': 3, 'FindResourceExA': 4, 'FreeResource': 1, 'GetCommandLineA': 0,
    'GetConsoleMode': 2, 'GetConsoleOutputCP': 0, 'GetCurrentProcess': 0,
    'GetCurrentProcessId': 0, 'GetCurrentThreadId': 0, 'GetLastError': 0,
    'GetModuleFileNameA': 3, 'GetModuleHandleA': 1, 'GetProcessHeap': 0,
    'GetStartupInfoA': 1, 'GetStdHandle': 1, 'GetThreadPriority': 1, 'HeapAlloc': 3,
    'HeapFree': 3, 'InitializeCriticalSection': 1, 'LeaveCriticalSection': 1,
    'LoadResource': 2, 'LocalAlloc': 2, 'LocalFree': 1, 'LockResource': 1,
    'MultiByteToWideChar': 6, 'ReadFile': 5, 'ReadProcessMemory': 5, 'ResetEvent': 1,
    'ResumeThread': 1, 'SetEvent': 1, 'SetFilePointer': 4, 'SetLastError': 1,
    'SetThreadPriority': 2, 'SetUnhandledExceptionFilter': 1, 'SizeofResource': 2,
    'Sleep': 1, 'SuspendThread': 1, 'TerminateThread': 2, 'TlsAlloc': 0,
    'TlsGetValue': 1, 'TlsSetValue': 2, 'TryEnterCriticalSection': 1,
    'WaitForSingleObject': 2, 'WideCharToMultiByte': 8, 'WriteFile': 5,
    'SysFreeString': 1, 'SysReAllocStringLen': 3, 'CharLowerBuffW': 2,
    'CharUpperBuffW': 2, 'MessageBoxA': 4,
}

STUB_BASE, HEAP_BASE, HEAP_SIZE = 0x70000000, 0x10000000, 0x01000000
STACK_TOP, DATA = 0x00300000, 0x60000000
TEB = 0x7ffd0000


class Exit(Exception):
    pass


def run(path, tin):
    raw = open(path, 'rb').read()
    pe = struct.unpack_from('<I', raw, 0x3c)[0]
    nsec, optsz = struct.unpack_from('<H', raw, pe + 6)[0], struct.unpack_from('<H', raw, pe + 20)[0]
    opt = pe + 24
    entry, = struct.unpack_from('<I', raw, opt + 16)
    base, = struct.unpack_from('<I', raw, opt + 28)
    image_size, = struct.unpack_from('<I', raw, opt + 56)
    imp_rva, = struct.unpack_from('<I', raw, opt + 96 + 8)

    mu = Uc(UC_ARCH_X86, UC_MODE_32)
    mu.mem_map(base, (image_size + 0xfff) & ~0xfff)
    mu.mem_write(base, raw[:0x400])
    for i in range(nsec):
        o = opt + optsz + i * 40
        vsize, va, rsize, rawoff = struct.unpack_from('<IIII', raw, o + 8)
        mu.mem_write(base + va, raw[rawoff:rawoff + rsize])

    # Resolve imports to stub addresses.
    stubs = {}
    d = base + imp_rva
    while True:
        ilt, _, _, name_rva, iat = struct.unpack('<5I', mu.mem_read(d, 20))
        if name_rva == 0:
            break
        k = 0
        while True:
            thunk, = struct.unpack('<I', mu.mem_read(base + (ilt or iat) + k * 4, 4))
            if thunk == 0:
                break
            nm = bytes(mu.mem_read(base + thunk + 2, 64)).split(b'\0')[0].decode()
            addr = STUB_BASE + len(stubs) * 16
            stubs[addr] = nm
            mu.mem_write(base + iat + k * 4, struct.pack('<I', addr))
            k += 1
        d += 20
    mu.mem_map(STUB_BASE, 0x10000)
    mu.mem_write(STUB_BASE, b'\xc3' * 0x10000)
    mu.mem_map(STACK_TOP - 0x100000, 0x100000)
    mu.mem_map(HEAP_BASE, HEAP_SIZE)
    mu.mem_map(DATA, 0x10000)
    mu.mem_map(TEB, 0x10000)
    mu.mem_write(TEB + 0x18, struct.pack('<I', TEB))
    mu.mem_write(TEB + 0x04, struct.pack('<I', STACK_TOP))
    mu.mem_write(TEB + 0x08, struct.pack('<I', STACK_TOP - 0x100000))
    mu.mem_write(TEB, struct.pack('<I', 0xffffffff))
    # Flat GDT plus an FS descriptor whose base is the fake TEB.
    GDT = 0x7fff0000
    mu.mem_map(GDT, 0x1000)

    def desc(base_, limit, access):
        return struct.pack('<HHBBBB', limit & 0xffff, base_ & 0xffff, (base_ >> 16) & 0xff,
                           access, 0xc0 | ((limit >> 16) & 0xf), (base_ >> 24) & 0xff)

    mu.mem_write(GDT, b'\0' * 8 + desc(0, 0xfffff, 0x9a) + desc(0, 0xfffff, 0x92)
                 + desc(TEB, 0xfffff, 0x92) + desc(0, 0xfffff, 0x92))
    mu.reg_write(UC_X86_REG_GDTR, (0, GDT, 5 * 8 - 1, 0))
    mu.reg_write(UC_X86_REG_SS, 4 << 3)
    mu.reg_write(UC_X86_REG_DS, 2 << 3)
    mu.reg_write(UC_X86_REG_ES, 2 << 3)
    mu.reg_write(UC_X86_REG_FS, 3 << 3)

    cmdline = f'chkt.exe "{tin}"'.encode() + b'\0'
    mu.mem_write(DATA, cmdline)
    modname = b'C:\\eBIRForms\\chkt.exe\0'
    state = {'heap': HEAP_BASE, 'tls': {}, 'tlsn': 0, 'out': b'', 'last': 0}

    def arg(i):
        esp = mu.reg_read(UC_X86_REG_ESP)
        return struct.unpack('<I', mu.mem_read(esp + 4 + 4 * i, 4))[0]

    def api(nm):
        if nm == 'ExitProcess':
            raise Exit(arg(0))
        if nm == 'GetCommandLineA':
            return DATA
        if nm == 'GetModuleFileNameA':
            buf, n = arg(1), arg(2)
            mu.mem_write(buf, modname[:n])
            return len(modname) - 1
        if nm in ('GetModuleHandleA',):
            return base
        if nm == 'GetStdHandle':
            return {0xfffffff6: 1, 0xfffffff5: 2, 0xfffffff4: 3}.get(arg(0), 0)
        if nm == 'GetConsoleMode':
            return 0
        if nm == 'GetProcessHeap':
            return 0x1234
        if nm in ('HeapAlloc', 'LocalAlloc'):
            size = arg(2) if nm == 'HeapAlloc' else arg(1)
            p = (state['heap'] + 15) & ~15
            state['heap'] = p + size + 16
            mu.mem_write(p, b'\0' * size)
            return p
        if nm in ('HeapFree',):
            return 1
        if nm == 'LocalFree':
            return 0
        if nm == 'TlsAlloc':
            state['tlsn'] += 1
            return state['tlsn']
        if nm == 'TlsGetValue':
            return state['tls'].get(arg(0), 0)
        if nm == 'TlsSetValue':
            state['tls'][arg(0)] = arg(1)
            return 1
        if nm == 'WriteFile':
            buf, n, written = arg(1), arg(2), arg(3)
            state['out'] += bytes(mu.mem_read(buf, n))
            if written:
                mu.mem_write(written, struct.pack('<I', n))
            return 1
        if nm == 'GetLastError':
            return state['last']
        if nm == 'SetLastError':
            state['last'] = arg(0)
            return 0
        if nm in ('GetCurrentThreadId', 'GetCurrentProcessId'):
            return 4
        if nm == 'GetCurrentProcess':
            return 0xffffffff
        if nm in ('CreateEventA',):
            return 0x99
        if nm in ('EnumResourceTypesA', 'EnumResourceNamesA', 'EnumResourceLanguagesA',
                  'FindResourceA', 'FindResourceExA', 'GetConsoleOutputCP'):
            return 0
        if nm == 'GetStartupInfoA':
            mu.mem_write(arg(0), b'\0' * 68)
            return 0
        if nm == 'MessageBoxA':
            raise RuntimeError('MessageBox')
        return 1

    def hook(uc, address, size, _):
        nm = stubs.get(address)
        if nm is None:
            return
        ret = api(nm)
        esp = uc.reg_read(UC_X86_REG_ESP)
        ra, = struct.unpack('<I', uc.mem_read(esp, 4))
        uc.reg_write(UC_X86_REG_EAX, ret & 0xffffffff)
        uc.reg_write(UC_X86_REG_ESP, esp + 4 + 4 * ARGS[nm])
        uc.reg_write(UC_X86_REG_EIP, ra)

    mu.hook_add(UC_HOOK_CODE, hook, begin=STUB_BASE, end=STUB_BASE + 0x10000)
    esp = STACK_TOP - 0x1000
    mu.mem_write(esp, struct.pack('<I', STUB_BASE + 0xfff0))
    mu.reg_write(UC_X86_REG_ESP, esp)
    try:
        mu.emu_start(base + entry, STUB_BASE + 0xfff0, count=50_000_000)
    except Exit as e:
        return e.args[0], state['out']
    except UcError as e:
        return f'error {e} at {mu.reg_read(UC_X86_REG_EIP):#x}', state['out']
    return 'returned', state['out']


if __name__ == '__main__':
    for t in sys.argv[2:]:
        code, out = run(sys.argv[1], t)
        print(t, code, out)
