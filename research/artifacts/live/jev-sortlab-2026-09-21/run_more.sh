#!/bin/sh
cd "$(dirname "$0")"
J="xyra-vault run ~/x/jev/typed-judgment -- python3"
$J lab.py countries arate 12 24 & $J lab.py countries anchor 8 24 & $J drift.py arxiv & wait
$J lab.py arxiv150 arate 12 24 & $J lab.py arxiv150 anchor 8 24 & $J drift.py manifund & wait
$J drift.py lw & wait
echo MORE DONE
