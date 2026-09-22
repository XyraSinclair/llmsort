#!/bin/sh
# countries battery; replay-safe. Each line is one recipe; three at a time.
cd "$(dirname "$0")"
J="xyra-vault run $HOME/x/jev/typed-judgment -- python3 lab.py countries"
$J rate 12 & $J noul 6 & $J score9 6 & wait
$J chain 12 & $J arate 12 & $J achain 12 & wait
$J anchor 8 & $J rate 12 24 & $J rate 12 4 & wait
echo COUNTRIES DONE
