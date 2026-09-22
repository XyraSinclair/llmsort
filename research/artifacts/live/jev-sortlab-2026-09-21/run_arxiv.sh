#!/bin/sh
# arxiv150 battery (novelty); replay-safe. Two at a time: abstracts make the states heavy.
cd "$(dirname "$0")"
J="xyra-vault run $HOME/x/jev/typed-judgment -- python3 lab.py arxiv150"
$J rate 12 & $J noul 6 & wait
$J score9 6 & $J chain 12 & wait
$J arate 12 & $J achain 12 & wait
$J anchor 8 & $J rate 12 24 & wait
$J rate 12 4 & wait
echo ARXIV DONE
