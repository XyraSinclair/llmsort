#!/bin/bash
# pspec driver v2: patient engine wait + prune-and-retry cycles per pool.
cd ~/pspec-llmsort
export CARDINAL_SERIATE_LOGPROB_MODELS=gemma4-31b:20
log(){ echo "$(date -u +%FT%TZ) $*"; }
wait_up(){
  while :; do
    code=$(curl -s -m5 -o /dev/null -w '%{http_code}' http://127.0.0.1:8023/v1/models)
    [ "$code" = "200" ] && { log "engine up"; return; }
    sleep 15
  done
}
prune(){ python3 - "$1" <<'PY'
import json,sys
path=sys.argv[1]
try: lines=open(path).read().splitlines()
except FileNotFoundError: sys.exit()
keep=[l for l in lines if json.loads(l).get('error') is None]
open(path,'w').write('\n'.join(keep)+('\n' if keep else ''))
print(f'{path}: {len(lines)} -> {len(keep)}')
PY
}
run_pool(){ # spec out target_rows
  for cycle in 1 2 3 4 5 6 7 8; do
    n=$(wc -l < "$2" 2>/dev/null || echo 0)
    [ "$n" -ge "$3" ] && { log "$2 complete ($n)"; return; }
    wait_up
    log "cycle $cycle for $2 ($n/$3)"
    ./target/release/examples/perturbation_spectrum "$1" "$2"
    prune "$2"
  done
  log "gave up on $2 after 8 cycles"
}
run_pool /tmp/spec_anchor.json /tmp/pspec_anchor.jsonl 2544   # 2640 minus ~96 genuine refusals tolerance
run_pool /tmp/spec_lesswrong.json /tmp/pspec_lesswrong.jsonl 2544
log "all pools done"
