#!/usr/bin/env python3
"""Compare bir-core's official_tin_check_code with the official chkt.exe.

Run from the repository root:
python scripts/dev/tin_check_digit_chkt_compare.py --chkt C:/path/chkt.exe --work C:/path/work
chkt.exe is not committed; supply the hash-checked official package copy.
"""
import argparse, concurrent.futures, datetime, hashlib, json, random, re, subprocess, sys, time
from pathlib import Path

SEED = 20261126
RANDOM_TOTAL = 200_000
RANGE_START = 900_000_000
RANGE_END = 905_200_000
PREFIX_COUNT = 2_000
FIXED = [
    "000000000","111111111","222222222","333333333","444444444","555555555","666666666","777777777","888888888","999999999",
    "123456788","123456789","111111114","010558054",
]
DUMMIES = {"000000000", "222222222", "999999999"}
TEST_FILE_TINS = [
    "010558054","123456788","123456789","111111114","111111115","900000004","900000005","900000006",
    "905180008","905180009","905180000","905180010","901234560","901234569","999999995","899999999",
    "159880005","428804832","674465222","686956816","868654684","137756848","299687453","806689686",
    "834141843","747000872","564539832","768488845","735260086","980512832","833838458","824757551",
    "527104586","806407983","830080574","629771961","473622146","257627256","900013294","903220207",
    "900423236","900580588","901505975","904282195",
]

def sha256(path):
    h=hashlib.sha256()
    with open(path,'rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''): h.update(b)
    return h.hexdigest()

def random_roots():
    rng=random.Random(SEED)
    for first in range(10):
        for _ in range(RANDOM_TOTAL//10):
            yield f"{first}{rng.randrange(100_000_000):08d}"

def prefix_roots():
    rng=random.Random(SEED+1)
    for _ in range(PREFIX_COUNT):
        prefix=f"{rng.randrange(100_000_000):08d}"
        for last in range(10): yield prefix+str(last)

def generate_inputs(path):
    counts={}
    with open(path,'w',encoding='ascii',newline='\n') as f:
        for name,items in [
            ('fixed',FIXED),
            ('random_200000',random_roots()),
            ('range_900000000_905200000',(f"{n:09d}" for n in range(RANGE_START,RANGE_END+1))),
            ('prefix_ladders',prefix_roots()),
        ]:
            start=f.tell(); n=0
            for tin in items: f.write(tin+'\n'); n+=1
            counts[name]=n
    return counts

def chkt_one(exe,tin):
    raw=subprocess.run([exe,tin],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode
    return raw, (1 if tin in DUMMIES else raw)

def main():
    global RANDOM_TOTAL
    ap=argparse.ArgumentParser()
    ap.add_argument('--chkt',required=True)
    ap.add_argument('--work',required=True)
    ap.add_argument('--workers',type=int,default=24)
    ap.add_argument('--random-count',type=int,default=RANDOM_TOTAL,help='lower only if chkt.exe is too slow; recorded in the result')
    args=ap.parse_args()
    RANDOM_TOTAL=args.random_count
    root=Path.cwd(); work=Path(args.work); work.mkdir(parents=True,exist_ok=True)
    exe=str(Path(args.chkt))
    digest=sha256(exe)
    inputs=work/'tin-inputs.txt'; ours=work/'tin-ours.tsv'; valid_range=work/'valid-range.tmp'
    t0=time.time()
    print('generating inputs',flush=True)
    counts=generate_inputs(inputs)
    print('building rust helper',flush=True)
    subprocess.run(['cargo','build','-p','bir-core','--example','tin_check_digit_stdin'],cwd=root,check=True)
    helper=root/'target'/'debug'/'examples'/('tin_check_digit_stdin.exe' if sys.platform=='win32' else 'tin_check_digit_stdin')
    print('evaluating rust port',flush=True)
    with open(inputs,'rb') as fi, open(ours,'wb') as fo:
        subprocess.run([str(helper)],stdin=fi,stdout=fo,check=True)
    # Fixed-seed recorded random sample: 500 roots for each first digit.
    sample_targets=set(TEST_FILE_TINS)
    rng=random.Random(SEED)
    for first in range(10):
        for i in range(RANDOM_TOTAL//10):
            tin=f"{first}{rng.randrange(100_000_000):08d}"
            if i < 500:
                sample_targets.add(tin)
    sample_answers={}
    total=0; different=0; differences=[]; valid_count=0; bycat={}
    bounds=[]; pos=0
    for k in ['fixed','random_200000','range_900000000_905200000','prefix_ladders']:
        bounds.append((pos,pos+counts[k],k)); pos+=counts[k]
    def cat(ix):
        for a,b,n in bounds:
            if a<=ix<b: return n
        return 'unknown'
    print('running chkt.exe comparisons',flush=True)
    with open(inputs,'r',encoding='ascii') as fi, open(ours,'r',encoding='ascii') as fo, open(valid_range,'w',encoding='ascii',newline='\n') as vf:
      with concurrent.futures.ThreadPoolExecutor(args.workers) as pool:
        pending=[]
        def flush():
            nonlocal total,different,valid_count
            for ix,tin,our,raw,official in pending:
                c=cat(ix); bycat[c]=bycat.get(c,0)+1; total+=1
                if tin in sample_targets: sample_answers[tin]=official
                if c=='range_900000000_905200000' and official==0:
                    vf.write(tin+'\n'); valid_count+=1
                if our!=official:
                    different+=1
                    if len(differences)<50: differences.append({'tin':tin,'ours':our,'chkt_getTinChkCode':official,'raw_chkt_exit':raw,'category':c})
            pending.clear()
        chunk=[]
        for ix,(line,oline) in enumerate(zip(fi,fo)):
            tin=line.strip(); our=int(oline.rstrip('\n').rsplit('\t',1)[1]); chunk.append((ix,tin,our))
            if len(chunk)>=5000:
                for (ix2,tin2,our2),(raw,official) in zip(chunk,pool.map(lambda z: chkt_one(exe,z[1]),chunk)):
                    pending.append((ix2,tin2,our2,raw,official))
                flush(); chunk.clear()
                if total%100000<5000: print('compared',total,flush=True)
        if chunk:
            for (ix2,tin2,our2),(raw,official) in zip(chunk,pool.map(lambda z: chkt_one(exe,z[1]),chunk)):
                pending.append((ix2,tin2,our2,raw,official))
            flush()
    # Write the recorded fixed-sample table.
    rows={tin:sample_answers[tin] for tin in sorted(sample_targets) if tin in sample_answers}
    with open(valid_range,encoding='ascii') as vf:
        for line in vf: rows[line.strip()]=0
    date=datetime.datetime.now(datetime.timezone.utc).date().isoformat()
    table=root/'crates'/'bir-core'/'tests'/'data'/'tin-check-digit-chkt.tsv'
    table.parent.mkdir(parents=True,exist_ok=True)
    with open(table,'w',encoding='ascii',newline='\n') as f:
        f.write(f"# chkt.exe sha256: {digest}\n")
        f.write("# source: hexuria/buwiz-validation app/chkt.exe, official eBIRForms offline package manifest 7.9.6.2.1\n")
        f.write(f"# recorded: {date}; full comparison roots: {total}\n")
        f.write("# columns: 9-digit TIN root<TAB>getTinChkCode result (chkt.exe exit code, with the three dummy overrides)\n")
        for tin in sorted(rows): f.write(f"{tin}\t{rows[tin]}\n")
    result={
      'chkt_sha256':digest,
      'chkt_source':'hexuria/buwiz-validation app/chkt.exe, official eBIRForms offline package manifest 7.9.6.2.1',
      'date_utc':date,'total_compared':total,'different':different,'differences_first50':differences,
      'counts_requested':counts,'counts_compared_by_category':bycat,'valid_in_900000000_905200000':valid_count,
      'recorded_rows':len(rows),'random_seed':SEED,'random_count':RANDOM_TOTAL,'workers':args.workers,
      'seconds':round(time.time()-t0,3),
      'ranges':'200000 fixed-seed random roots stratified over first digits; every root 900000000-905200000 inclusive; 2000 fixed-seed 8-digit prefixes x all last digits; repeated-digit and named fixed roots',
    }
    out=root/'crates'/'bir-core'/'tests'/'data'/'tin-check-digit-chkt-results.json'
    out.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'total':total,'different':different,'rows':len(rows),'valid_range':valid_count,'result':str(out),'table':str(table)},indent=2))

if __name__=='__main__': main()


