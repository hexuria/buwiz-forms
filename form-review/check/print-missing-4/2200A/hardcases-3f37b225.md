# 2200A hard cases at 3f37b225 (prints: p5-3f37b225-hardcases-A.pdf, p5-3f37b225-hardcases-B-negative.pdf)

The 5-slot Drawee Bank/Agency column: the sheet's own heading is "Drawee Bank/Agency" and the official
program's field (frm2200Av2020:cashBank per runtime-control-inventory-v796.json) is free text, maxlength null —
a bank/agency NAME, not a bank code. 5 slots cannot hold any real bank name; every real payment row's bank
is therefore blank on paper at 3f37b225 (e.g. "BPI" fits; "BANKX" (5) fits; anything longer prints nothing).

Per case (return A = rows with fit/blank matrix; return B = negative amount):
- bank 5 chars BANKX: prints whole. 6 chars BANKXX: blank. 20 chars: blank.
- number 6 digits: prints. 7 digits: blank. 12 digits: blank.
- amount 11 peso digits: prints whole with centavos. 12 digits: blank entirely.
- 0.05: prints 0 pesos / 05 centavos, correct.
- 1234.567 (three nonzero centavo digits): blank entirely.
- -500.00: amount blank; bank, number, date of the same row still print.
- a row part left blank is never truncated and never shows a wrong value; the minus sign is never dropped
  because nothing of the amount prints at all.

"Held but not printed" ranking: a held amount that prints nothing is a factual omission on the filer's copy
but never a wrong statement; main prints nothing at all for the same rows, so every blank case here is
strictly better than main for the 12-digit amount (f8dc14c5 printed a wrong value; main prints nothing),
and equal-to-main-but-observable for the rest (blank either way; the head at least prints what fits in the
other columns of the same row).
