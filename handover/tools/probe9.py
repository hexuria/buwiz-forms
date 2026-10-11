import os, random, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import emu, port
exe = sys.argv[1]
rng = random.Random(3)
bases = ['90000000', '90000001', '90518000', '90518001', '90517999'] + [f'{rng.randrange(90000000, 90518001):08d}' for _ in range(60)]
for b in bases:
    ok = [d for d in range(10) if emu.run(exe, b + str(d))[0] == 0]
    print(b, 'accepted', ok, 'normal', port.check_digit(b, 'units'))
