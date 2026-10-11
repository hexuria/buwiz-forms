"""Candidate port of chkt.exe, checked against the emulated binary."""
import os, random, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import emu


def digital_root(n):
    while n >= 10:
        n = sum(int(c) for c in str(n))
    return n


def check_digit(d8, carry):
    total, weight, offset = 0, 256, 8
    for c in d8:
        t = int(c) + offset
        if t >= 10:
            t = t % 10 if carry == 'units' else t // 10
        total += digital_root(t * weight)
        offset -= 1
        weight //= 2
    u = total % 10
    return 0 if u == 0 else 10 - u


def status(tin, carry='units'):
    return 0 if check_digit(tin[:8], carry) == int(tin[8]) else 1


if __name__ == '__main__':
    exe = sys.argv[1]
    n = int(sys.argv[2])
    rng = random.Random(7)
    mism = {'units': 0, 'tens': 0}
    for _ in range(n):
        base8 = f'{rng.randrange(0, 90_000_000):08d}'  # avoid the 9xx special range
        # Half the samples use the "right" check digit for the units rule.
        tin = base8 + (str(check_digit(base8, 'units')) if rng.random() < 0.5 else str(rng.randrange(10)))
        code, _ = emu.run(exe, tin)
        for k in mism:
            if status(tin, k) != code:
                mism[k] += 1
    print(mism)
