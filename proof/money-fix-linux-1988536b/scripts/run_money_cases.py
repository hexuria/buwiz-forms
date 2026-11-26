#!/usr/bin/env python3
import csv, json, math, os, sys, time
from pathlib import Path
import drive_money as d

OUT = Path('/home/ubuntu/forms-validation/money-box-proof')
FID = 'fp-txtATCAmt1'
RESULTS = OUT / 'money-results.jsonl'

def save_result(case, action, focused, blurred, saved, expected_focus='', expected_blur='', expected_saved=None, notes=''):
    okf = (focused == expected_focus) if expected_focus != '' else True
    okb = (blurred == expected_blur) if expected_blur != '' else True
    oks = True
    if expected_saved is not None:
        try: oks = math.isclose(float(saved), float(expected_saved), rel_tol=0, abs_tol=0.000001)
        except Exception: oks = False
    passed = okf and okb and oks
    row = {'case':case,'keys_action':action,'shown_focused':focused,'shown_after_blur':blurred,'saved_draft_value':saved,'expected_focused':expected_focus,'expected_after_blur':expected_blur,'expected_saved':expected_saved,'pass':passed,'notes':notes}
    with RESULTS.open('a') as f: f.write(json.dumps(row, ensure_ascii=False)+'\n')
    print(json.dumps(row, ensure_ascii=False), flush=True)
    return row

def settle(sec=1.5): time.sleep(sec)

def set_amount(txt):
    d.click_node(FID, wait=.25)
    d.ctrl_a()
    d.type_text(txt)
    d.tab()
    settle(1.3)

def finish_case(case, action, expected_focus, expected_blur, expected_saved, prefix):
    time.sleep(.25)
    focused = d.value(FID)
    fs = d.shot(f'{prefix}-focused')
    d.log('case-focused', case=case, focused=focused, screenshot=fs)
    d.tab()
    settle(1.8)
    blurred = d.value(FID)
    saved = d.draft_value()
    bs = d.shot(f'{prefix}-blurred')
    row = save_result(case, action, focused, blurred, saved, expected_focus, expected_blur, expected_saved)
    row['focused_screenshot'] = fs; row['blurred_screenshot'] = bs
    return row

def case_a():
    d.click_node(FID, wait=.25)
    d.type_text('15000.50')
    return finish_case('a zero click type','click, type 15000.50','15000.50','15,000.50',15000.5,'a')

def case_b():
    set_amount('1234.56')
    d.click_node(FID, wait=.25)
    d.ctrl_a()
    d.type_text('15000.50')
    return finish_case('b ctrl+a type','holding 1,234.56; click, Ctrl+A, type 15000.50','15000.50','15,000.50',15000.5,'b')

def case_c():
    set_amount('1234.56')
    d.click_node(FID, wait=.25)
    d.ctrl_a(); d.backspace()
    d.type_text('15000.50')
    return finish_case('c ctrl+a backspace type','holding 1,234.56; click, Ctrl+A, BackSpace, type 15000.50','15000.50','15,000.50',15000.5,'c')

def case_d():
    set_amount('1234.56')
    # Put keyboard focus on the preceding ATC trigger without changing it, close its popup, then Tab naturally into amount.
    d.click_node('fp-drpATC1-trigger', wait=.35)
    d.key(['Escape']); time.sleep(.25)
    d.tab(); time.sleep(.2)
    d.type_text('15000.50')
    return finish_case('d tab in type','holding 1,234.56; Tab in, type 15000.50','15000.50','15,000.50',15000.5,'d')

def zero_case(case, action, keys, expected_focus, expected_blur, expected_saved, prefix, delay=120, extra=None):
    set_amount('0')
    d.click_node(FID, wait=.25)
    d.type_text(keys, delay=delay)
    if extra: extra()
    return finish_case(case, action, expected_focus, expected_blur, expected_saved, prefix)

def paste_case():
    set_amount('0')
    d.click_node(FID, wait=.25)
    os.system("printf %s '99999999.99' | DISPLAY=:0 xclip -selection clipboard >/dev/null 2>&1")
    time.sleep(.1)
    d.key(['ctrl+v'])
    return finish_case('paste','from 0.00; paste 99999999.99 with real Ctrl+V','99999999.99','99,999,999.99',99999999.99,'paste')

def phase1():
    case_a()

def phase2():
    case_b(); case_c(); case_d()

def phase3():
    zero_case('99999.99','from 0.00; type 99999.99','99999.99','99999.99','99,999.99',99999.99,'n99999')
    zero_case('1000000.005','from 0.00; type 1000000.005','1000000.005','1000000.005','1,000,000.01',1000000.01,'round')
    def edit(): d.left(2); d.backspace()
    zero_case('left left backspace','from 0.00; type 1234567890 then Left Left BackSpace','1234567890','123456790','123,456,790.00',123456790.0,'editkeys',extra=edit)
    for i in (1,2,3):
        zero_case(f'99999999.99 run {i}',f'from 0.00; type 99999999.99 (run {i})','99999999.99','99999999.99','99,999,999.99',99999999.99,f'nines{i}')
    zero_case('burst no delay','from 0.00; type 99999999.99 with xdotool delay 0','99999999.99','99999999.99','99,999,999.99',99999999.99,'burst',delay=0)
    paste_case()

if __name__ == '__main__':
    {'1':phase1,'2':phase2,'3':phase3}[sys.argv[1]]()
