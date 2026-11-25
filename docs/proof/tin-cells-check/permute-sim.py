# Faithful port of nsh-base otp_input.rs diff() + edit() (lines 599-628), showing that ONE mid-string
# caret insertion during a fast burst produces exactly the corruption observed in the probe runs.
def diff(before, after):
    prefix = 0
    for i, (a, b) in enumerate(zip(before, after)):
        if a != b:
            break
        prefix = i + 1
    else:
        prefix = min(len(before), len(after))
    return len(before) - prefix, after[prefix:]

def edit(shown, deleted, inserted, length):
    digits = [c for c in inserted if c.isdigit()]
    if length > 0 and len(digits) >= length:
        return "".join(digits[:length])
    chars = list(shown)[: max(0, len(shown) - deleted)]
    for d in digits:
        if len(chars) >= length:
            break
        chars.append(d)
    return "".join(chars)

typed = native = "0001"          # keys 0,0,0,1 landed cleanly
native = "2" + native            # key '2' inserted at caret position 0 (stale pin yanked the caret)
deleted, inserted = diff(typed, native)
typed = edit(typed, deleted, inserted, 9)
assert typed == "20001", typed   # == the exact wrong value the 5-digit fast-burst probe produced
print("one mid-string insertion at caret 0 reproduces the observed corruption:", typed)
