import json, collections, gzip, numpy as np
lens_of={"P":"lesswrong-posts","C":"lesswrong-comments","M":"manifund-proposals"}
names=[c["name"] for c in json.load(open("cand.criteria.json"))]; TGT="regret-if-missed"
cand=collections.defaultdict(dict)
for l in gzip.open("ledger.jsonl.gz","rt"):
    r=json.loads(l); cand[(lens_of[r["list_id"][0]],r["criterion_name"])][r["item_id"]]=r["latent"]
def z(v):
    v=np.asarray(v,float); s=v.std(); return (v-v.mean())/s if s>0 else v*0
def r2(basis,y):
    b=np.linalg.lstsq(basis.T,y,rcond=None)[0]; return 1-((y-basis.T@b)**2).sum()/(y**2).sum()
def loo(basis,y):
    # leave-one-out predictive r for y from basis rows (k x n)
    n=len(y); pred=np.zeros(n)
    for i in range(n):
        m=np.arange(n)!=i; b=np.linalg.lstsq(basis[:,m].T,y[m],rcond=None)[0]; pred[i]=basis[:,i]@b
    return np.corrcoef(pred,y)[0,1]
rng=np.random.default_rng(0)
for L in lens_of.values():
    ex=collections.defaultdict(dict)
    for l in gzip.open(f"existing-axes/{L}.tsv.gz","rt"):
        a,e,m,s,r=l.rstrip("\n").split("\t"); ex[a][e]=float(m)
    ents=sorted(cand[(L,TGT)]); axes=[a for a in sorted(ex) if all(e in ex[a] for e in ents)]
    X=np.array([z([ex[a][e] for e in ents]) for a in axes]); keep=X.std(1)>0; X=X[keep]; axes=[a for a,k in zip(axes,keep) if k]
    C=np.array([z([cand[(L,n)][e] for e in ents]) for n in names]); n=len(ents)
    U,S,Vt=np.linalg.svd(X,full_matrices=False); var=S**2/(S**2).sum(); k80=int(np.searchsorted(np.cumsum(var),.8)+1)
    P4=Vt[:4]; Pk=Vt[:k80]
    # permutation null for max|r| and R2
    B=1000; mx=[];r4=[];rk=[]
    for _ in range(B):
        y=z(rng.permutation(n)*1.0+rng.normal(0,1e-6,n)); y=C[0][rng.permutation(n)]
        mx.append(np.abs(X@y/n).max()); r4.append(r2(P4,y)); rk.append(r2(Pk,y))
    print(f"\n## {L}: null (permuted candidate, {B} draws) max|r| median {np.median(mx):.2f} 95% {np.percentile(mx,95):.2f}; R2@4PC median {np.median(r4):.2f} 95% {np.percentile(r4,95):.2f}; R2@{k80}PC median {np.median(rk):.2f} 95% {np.percentile(rk,95):.2f}")
    ti=names.index(TGT); t=C[ti]
    base4=loo(P4,t); basek=loo(Pk,t)
    print(f"target LOO predictive r from 4 PCs {base4:.2f}, from {k80} PCs {basek:.2f}")
    print(f"{'candidate':24s} R2@4PC(vs null95)  R2@kPC(vs null95)  LOO r(target) alone  LOO 4PC+cand  LOO {k80}PC+cand")
    for i,nm in enumerate(names):
        if i==ti: continue
        a=loo(C[i:i+1],t); b=loo(np.vstack([P4,C[i]]),t); c=loo(np.vstack([Pk,C[i]]),t)
        f=lambda v,q: f"{v:.2f}{'*' if v>q else ' '}"
        print(f"{nm:24s} {f(r2(P4,C[i]),np.percentile(r4,95)):>8s}           {f(r2(Pk,C[i]),np.percentile(rk,95)):>8s}           {a:+.2f}                {b:+.2f}        {c:+.2f}")
