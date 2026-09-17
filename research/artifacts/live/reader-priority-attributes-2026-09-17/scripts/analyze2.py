import json, collections, gzip, numpy as np
lens_of={"P":"lesswrong-posts","C":"lesswrong-comments","M":"manifund-proposals"}
def load(path):
    if path.endswith(".gz"): return _load(gzip.open(path,"rt"))
    return _load(open(path))
def _load(fh):
    d=collections.defaultdict(dict); fl={}
    for l in fh:
        r=json.loads(l); L=lens_of[r["list_id"][0]]; d[(L,r["criterion_name"])][r["item_id"]]=r["latent"]; fl[(L,r["criterion_name"])]=r["flip"]
    return d,fl
c1,f1=load("ledger.jsonl.gz"); c2,f2=load("pass2/ledger.jsonl")
n1=[c["name"] for c in json.load(open("cand.criteria.json"))]; n2=[c["name"] for c in json.load(open("pass2/cand2.criteria.json"))]
NB={"reading-pleasure":["humor-effectiveness","imagery-vividness","tone-craft","audience-service"],"mistake-prevention":["delta-to-informed-reader","importance-of-topic","actionability"],
    "challenge-to-priors":["steelman-strength","steelmanning","disagreement-quality","novelty-of-insight","courage-calibration"],"reader-empowerment":["actionability"],"misleading-if-trusted":["claim-calibration","calibration-display"],"source-uniqueness":["novelty-of-insight"]}
def z(v):
    v=np.asarray(v,float); s=v.std(); return (v-v.mean())/s if s>0 else v*0
def r2(B,y):
    b=np.linalg.lstsq(B.T,y,rcond=None)[0]; return 1-((y-B.T@b)**2).sum()/(y**2).sum()
rng=np.random.default_rng(0)
for L in lens_of.values():
    ex=collections.defaultdict(dict)
    for l in gzip.open(f"existing-axes/{L}.tsv.gz","rt"):
        a,e,m,s,r=l.rstrip("\n").split("\t"); ex[a][e]=float(m)
    ents=sorted(c2[(L,"reading-pleasure")]); n=len(ents)
    axes=[a for a in sorted(ex) if all(e in ex[a] for e in ents)]
    X=np.array([z([ex[a][e] for e in ents]) for a in axes]); k=X.std(1)>0; X=X[k]; axes=[a for a,kk in zip(axes,k) if kk]
    fam=collections.defaultdict(list)
    for a,row in zip(axes,X): fam[a.split("#")[0]].append(row)
    P4=np.linalg.svd(X,full_matrices=False)[2][:4]
    null=np.percentile([r2(P4,z(rng.permutation(n)+rng.normal(0,.01,n))) for _ in range(500)],95)
    C1={nm:z([c1[(L,nm)][e] for e in ents]) for nm in n1}; C2={nm:z([c2[(L,nm)][e] for e in ents]) for nm in n2}
    print(f"\n## {L} (null95 R2@4PC {null:.2f})")
    for nm in n2:
        y=C2[nm]; rep=f" repeat-r {y@C1[nm]/n:+.2f} (flip1 {f1[(L,nm)]:.2f})" if nm in C1 else ""
        near=sorted(((y@C1[o]/n,o) for o in n1 if o!=nm),key=lambda t:-abs(t[0]))[:3]
        near2=sorted(((y@C2[o]/n,o) for o in n2 if o!=nm),key=lambda t:-abs(t[0]))[:2]
        fams=" | ".join(f"{f} {z(np.mean(fam[f],0))@y/n:+.2f}" for f in NB[nm] if f in fam) or "—"
        print(f"  {nm:22s} flip {f2[(L,nm)]:.2f}{rep}  R2@4PC {r2(P4,y):.2f}  r(regret) {y@C1['regret-if-missed']/n:+.2f}  r(currency) {y@C1['discourse-currency']/n:+.2f}\n      nearest pass1: {', '.join(f'{o}{v:+.2f}' for v,o in near)}; pass2: {', '.join(f'{o}{v:+.2f}' for v,o in near2)}; families: {fams}")
    # top/bottom for posts
    if L=="lesswrong-posts":
        txt={}
        for l in open("existing-axes/lw-entities.tsv"):
            p=l.rstrip("\n").split("\t",1); txt[p[0]]=(p[1] if len(p)>1 else "")[:60]
        for nm in ["reading-pleasure","mistake-prevention","challenge-to-priors"]:
            s=sorted(c2[(L,nm)].items(),key=lambda kv:-kv[1]); print(f"  {nm} top: "+" || ".join(txt.get(e,e) for e,_ in s[:3])+"\n      bottom: "+" || ".join(txt.get(e,e) for e,_ in s[-3:]))
