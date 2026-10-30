#!/usr/bin/env bash
set -eu
deps=/home/atlas/Github/4t145/gds/.agents/worktrees/4cecb57f-6267-4d96-8706-2d2b6717930c-w2/target/debug/deps
libs=("$deps"/libtempfile-*.rlib)
/nix/store/6l9kbzxwq9ij2lcmwl2qb1jdpqisjr8l-rustc-wrapper-1.99.0/bin/rustc --edition 2024 --test -Awarnings -L "dependency=$deps" --extern "tempfile=${libs[0]}" harness.rs -o ./check-bin
./check-bin
