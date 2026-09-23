#!/bin/sh
# The whole judgment grid on four cohorts spanning the knowledge ceiling; two chains in parallel.
cd "$(dirname "$0")"
J="xyra-vault run $HOME/x/jev/typed-judgment -- python3 grid.py"
( $J countries; $J films ) > grid-a.log 2>&1 &
( $J companies; $J names-aura ) > grid-b.log 2>&1 &
wait
