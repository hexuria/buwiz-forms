"""Print the SHA-256 of a plaintext encrypted by the official Encrypt.exe
(run under unicorn emulation), and whether our IAF file matches it.

Usage: python3 -I official_encrypt_sha.py <stem.plain.xml> [<stem.iaf.xml>]
"""
import hashlib, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import emu2
plain = open(sys.argv[1], 'rb').read()
name = 'IAF_RDO_Copy/x.xml'
vfs = {name.replace('/', '\\').lower(): bytearray(plain)}
code, _ = emu2.run(os.path.join(os.path.dirname(HERE), 'encexe', 'Encrypt.exe'), None, vfs=vfs, cmd=f'Encrypt.exe "{name}"')
official = bytes(next(iter(vfs.values())))
print('exit', code, 'official_sha256', hashlib.sha256(official).hexdigest())
if len(sys.argv) > 2:
    print('ours_identical', official == open(sys.argv[2], 'rb').read())
