#!/bin/sh
# Run the 2x2 (arm x skill) for every model and seed. Usage: matrix.sh <task> "<seeds>" "<models>"
cd "$(dirname "$0")/../.." || exit 1
export ZENITH_E2E_NO_SANDBOX=1 ZENITH_BENCH_ZENITH="$PWD/target/debug/zenith"
for seed in $2; do
  for model in $3; do
    for arm in ${ARMS:-html zenith}; do
      for skill in no yes; do
        node bench/revision/run.mjs --task "$1" --arm $arm --model $model --skill $skill --seed $seed 2>&1 | grep -E "^[a-z-]+\.[a-z+]+\." | tail -1
      done
    done
  done
done
