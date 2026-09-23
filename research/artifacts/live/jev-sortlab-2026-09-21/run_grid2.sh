#!/bin/sh
# Closing the state-size axis: the cheap forms at k = 24 / 48, choice at k = 4.
cd "$(dirname "$0")"
J="xyra-vault run $HOME/x/jev/typed-judgment -- python3 grid.py"
R="rate-L3-k24 rate-L5-k24 rate-L10-k48 tophalf-k24 topbot-k4 tri-k24"
( $J countries $R; $J films $R ) > grid-d.log 2>&1 &
( $J companies $R; $J names-aura $R ) > grid-e.log 2>&1 &
wait
