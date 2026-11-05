import json,glob,statistics as st,sys
OUT=sys.argv[1] if len(sys.argv)>1 else "."
tags=["fib","loopsum","collatz","primes","gcdsum","tak","triples","digitsum","isqrt","inversions","inv400"]
rows=[]
for t in tags:
    per={}
    for r in (1,2,3):
        d=json.load(open(f"{OUT}/{t}_r{r}.json"))
        for x in d["results"]:
            per.setdefault(x["command"],[]).append(x)
    def agg(k):
        times=[tt for x in per[k] for tt in x["times"]]
        meds=[x["median"]*1000 for x in per[k]]
        return st.median(times)*1000,min(times)*1000,max(times)*1000,meds
    a=agg("sley-2.1.7");b=agg("sley-2.1.8");p=agg("cpython-3.13")
    def vm(v):
        L=[json.loads(l) for l in open(f"{OUT}/vm{v}_{t}.jsonl") if l.strip()]
        return st.median([x["vm_micros"] for x in L]),L[0]["instructions"],L[0]["fuel"],L[0]["result"]
    va,vb=vm(217),vm(218)
    rows.append((t,a,b,p,va,vb))
    print(f"| {t} | {a[0]:.1f} [{a[1]:.1f}-{a[2]:.1f}] | {b[0]:.1f} [{b[1]:.1f}-{b[2]:.1f}] | {a[0]/b[0]:.2f}x | {p[0]:.1f} [{p[1]:.1f}-{p[2]:.1f}] | {b[0]/p[0]:.2f}x | {va[1]:,} → {vb[1]:,} | {va[0]/1000:.1f} → {vb[0]/1000:.1f} | round medians 2.1.8 {', '.join(f'{m:.1f}' for m in b[3])} | res {va[3]}=={vb[3]} |")
