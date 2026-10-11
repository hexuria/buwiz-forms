import struct, sys
import os; sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'pylib'))
import capstone

def load(path):
    d = open(path, 'rb').read()
    pe = struct.unpack_from('<I', d, 0x3c)[0]
    nsec = struct.unpack_from('<H', d, pe + 6)[0]
    optsz = struct.unpack_from('<H', d, pe + 20)[0]
    opt = pe + 24
    entry, = struct.unpack_from('<I', d, opt + 16)
    base, = struct.unpack_from('<I', d, opt + 28)
    secs = []
    for i in range(nsec):
        o = opt + optsz + i * 40
        name = d[o:o + 8].rstrip(b'\0').decode()
        vsize, va, rsize, raw = struct.unpack_from('<IIII', d, o + 8)
        secs.append((name, base + va, vsize, d[raw:raw + rsize]))
    return d, base, base + entry, secs

def mem(secs, addr, n):
    for name, va, vs, data in secs:
        if va <= addr < va + max(vs, len(data)):
            off = addr - va
            return (data[off:off + n]).ljust(n, b'\0')
    return None

def dis(secs, addr, count=60, stop_ret=True):
    md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_32)
    code = mem(secs, addr, count * 15)
    out = []
    for ins in md.disasm(code, addr):
        out.append(f'{ins.address:08x}: {ins.mnemonic} {ins.op_str}')
        count -= 1
        if count <= 0 or (stop_ret and ins.mnemonic == 'ret'):
            break
    return '\n'.join(out)

if __name__ == '__main__':
    d, base, entry, secs = load(sys.argv[1])
    if len(sys.argv) == 2:
        print(hex(base), hex(entry))
        for s in secs:
            print(s[0], hex(s[1]), hex(s[2]), len(s[3]))
    else:
        print(dis(secs, int(sys.argv[2], 16), int(sys.argv[3]) if len(sys.argv) > 3 else 60))
