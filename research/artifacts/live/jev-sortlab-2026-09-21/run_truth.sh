#!/bin/sh
cd "$(dirname "$0")"
J="xyra-vault run $HOME/x/jev/typed-judgment -- python3 lab.py"
for c in cities mountains rivers companies films elements; do
  $J $c rate 6 24 & $J $c noul 3 8 & $J $c anchor 3 8 & wait
done
echo TRUTH DONE
