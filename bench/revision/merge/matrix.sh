#!/bin/sh
# Run the merge kill experiment: every seed x model x arm, one run at a time.
# Usage: matrix.sh ["<seeds>"] ["<models>"]   (ARMS env overrides "html zenith")
# Defaults: seeds "1 2 3", models "glm-5.3-flash deepseek-v4.1-flash".
cd "$(dirname "$0")/../../.." || exit 1
export ZENITH_E2E_NO_SANDBOX=1
# A full-template reply is long. A low cap cuts it off and scores the arm on the cap, not the work.
export ZENITH_BENCH_MAX_TOKENS=${ZENITH_BENCH_MAX_TOKENS:-32768}
for seed in ${1:-1 2 3}; do
  for model in ${2:-glm-5.3-flash deepseek-v4.1-flash}; do
    for arm in ${ARMS:-html zenith}; do
      node bench/revision/merge/run.mjs --arm "$arm" --model "$model" --seed "$seed" ${SKILL:+--skill yes} 2>&1 | grep -E "^merge\." | tail -1
    done
  done
done
node bench/revision/merge/report.mjs
