#!/bin/sh
# The CI gate (fmt, clippy -D warnings, tests, doctests, docs) for one commit,
# so main is never red. Rust never compiles on a Mac: there it runs on colo2.
# Usage: ops/gate.sh [rev]   (default HEAD). Wired as .githooks/pre-push.
set -eu
rev=$(git rev-parse "${1:-HEAD}")
gate='quiet() { out=$("$@" 2>&1) || { printf "%s\n" "$out"; return 1; }; }
  cargo fmt --all -- --check &&
  cargo clippy --workspace --all-targets --all-features --locked -q -- -D warnings &&
  quiet cargo test --workspace --all-targets --all-features --locked -q &&
  quiet cargo test --workspace --doc --locked -q &&
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked -q'
if [ "$(uname)" != Darwin ]; then
    dir=$(mktemp -d); trap 'git worktree remove --force "$dir"' EXIT
    git worktree add -q --detach "$dir" "$rev"
    cd "$dir" && sh -c "$gate"
    exit
fi
host=${LLMSORT_GATE_HOST:-colo2}
ssh "$host" 'mkdir -p ~/build && cd ~/build && { [ -d llmsort-gate.git ] || git init -q --bare llmsort-gate.git; }'
git push -q -f --no-verify "$host:build/llmsort-gate.git" "$rev:refs/heads/gate"
ssh "$host" "set -e; cd ~/build
  [ -d llmsort-gate ] || git clone -q llmsort-gate.git llmsort-gate
  cd llmsort-gate && git fetch -q origin gate && git checkout -q -f --detach $rev && git clean -fdq
  export PATH=\$HOME/.cargo/bin:\$PATH CARGO_TARGET_DIR=\$HOME/build/llmsort-gate-target
  nice sh -c '$gate'"
echo "gate: $rev green"
