#!/bin/sh
# Run build briefs. Usage: matrix.sh "<briefs>" "<seeds>" "<models>" ["<skills>"]   (ARMS env overrides "html zenith")
cd "$(dirname "$0")/../../.." || exit 1
export ZENITH_E2E_NO_SANDBOX=1 ZENITH_BENCH_ZENITH="$PWD/target/debug/zenith"
for seed in $2; do
  for brief in $1; do
    for model in $3; do
      for arm in ${ARMS:-html zenith}; do
        for skill in ${4:-no}; do
          node bench/revision/build/run.mjs --brief $brief --arm $arm --model $model --skill $skill --seed $seed 2>&1 | grep -E "^build\." | tail -1
        done
      done
    done
  done
done
