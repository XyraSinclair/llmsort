#!/bin/bash
# wait for the three teacher passes, flatten the ledger, then borrow the card for chain2.py via run.sh
cd ~/llmsort-dg
until [ "$(grep -l finished corpus2/teacher-*.log 2>/dev/null | wc -l)" -eq 3 ]; do sleep 15; done
echo "teacher done $(date -u +%FT%TZ)"
(cd corpus2 && python3 flatten.py) 2>&1 | tee corpus2/flatten.log
DG_SCRIPT=chain2.py ./run.sh
echo "chain2 exit $? $(date -u +%FT%TZ)"
