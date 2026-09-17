import json, collections, gzip, numpy as np
lens_of={"P":"lesswrong-posts","C":"lesswrong-comments","M":"manifund-proposals"}
names=[c["name"] for c in json.load(open("cand.criteria.json"))]
NB={"front-loading":["skimmability","information-density","explanatory-compression"],
 "portable-payload":["explanatory-compression","generality","delta-to-informed-reader","actionability"],
 "read-now-premium":["timelessness","time-sensitivity-truth","urgency","timing-relevance"],
 "source-uniqueness":["novelty-of-insight","delta-to-informed-reader","canonical-pointing"],
 "canonicality":["canonical-pointing","novelty-of-insight","rereadability"],
 "breadth-of-consequence":["generality","interdisciplinary-reach","importance-of-topic"],
 "reader-empowerment":["actionability","delta-to-informed-reader"],
 "misleading-if-trusted":["claim-calibration","calibration-display","urgency-honesty"],
 "compounding-value":["prerequisite-mapping","generality","rereadability"],
 "effort-to-value":["skimmability","idea-density","gift-density","audience-service"],
 "discourse-currency":["timing-relevance","thread-advancement","urgency","importance-of-topic"],
 "regret-if-missed":["importance-of-topic","delta-to-informed-reader","rereadability","timelessness"]}
cand=collections.defaultdict(dict)
for l in gzip.open("ledger.jsonl.gz","rt"):
    r=json.loads(l); cand[(lens_of[r["list_id"][0]],r["criterion_name"])][r["item_id"]]=r["latent"]
def z(v):
    v=np.asarray(v,float); s=v.std(); return (v-v.mean())/s if s>0 else v*0
for L in lens_of.values():
    ex=collections.defaultdict(dict)
    for l in gzip.open(f"existing-axes/{L}.tsv.gz","rt"):
        a,e,m,s,r=l.rstrip("\n").split("\t"); ex[a][e]=float(m)
    ents=sorted(cand[(L,"regret-if-missed")]); n=len(ents)
    fams=collections.defaultdict(list)
    for a in ex:
        if all(e in ex[a] for e in ents): fams[a.split("#")[0]].append(z([ex[a][e] for e in ents]))
    print(f"## {L}")
    for nm in names:
        c=z([cand[(L,nm)][e] for e in ents]); parts=[]
        for f in NB[nm]:
            if f not in fams: parts.append(f"{f} —"); continue
            comp=z(np.mean(fams[f],0)); rs=[c@v/n for v in fams[f]]
            parts.append(f"{f} comp{comp@c/n:+.2f} max{max(rs,key=abs):+.2f}")
        print(f"  {nm:24s} " + " | ".join(parts))
