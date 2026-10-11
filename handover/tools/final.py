import os, random, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import emu
from port import check_digit

def chkt(arg):
    """Port of chkt.exe exit code: 0 valid, 1 bad check digit, 2 malformed, 3 wrong arg count."""
    tin = arg.strip(' ')
    if len(tin) != 9 or not tin.isdigit():
        return 2
    expected = check_digit(tin[:8], 'units')
    actual = int(tin[8])
    if expected == actual:
        return 0
    if tin[0] == '9' and 900000004 < int(tin) < 905180009:
        return 0 if (actual + 9) % 10 == expected else 1
    return 1

exe = sys.argv[1]
rng = random.Random(11)
cases = ['000000000','999999999','222222222','900000004','900000005','905180008','905180009','905180010',
         '899999999','123456789','12345678','1234567890',' 123456789 ','12345678a','-12345678','+12345678',
         '00000000 ','']
for _ in range(int(sys.argv[2])):
    r = rng.random()
    if r < 0.4:
        n = rng.randrange(0, 1_000_000_000)
    elif r < 0.7:
        n = rng.randrange(899_990_000, 905_200_000)
    elif r < 0.85:
        n = rng.choice([900000004, 905180009]) + rng.randrange(-30, 30)
    else:
        n = rng.randrange(900_000_000, 1_000_000_000)
    b8 = f'{n:09d}'[:8]
    cases.append(b8 + str(check_digit(b8, 'units')) if rng.random() < 0.3 else f'{n:09d}')
bad = 0
for c in cases:
    got = emu.run(exe, c)[0]
    if got != chkt(c):
        bad += 1
        if bad <= 15: print('MISMATCH', repr(c), 'exe', got, 'port', chkt(c))
print(len(cases), 'cases,', bad, 'mismatches')
