#!/usr/bin/env bash
# Stale-bundle + footprint guard for the committed web-dist/ frontend bundle.
#
# Contract: the Pi embeds the committed web-dist/ via rust-embed and has no Node toolchain, so the
# bundle MUST be rebuilt and committed on the host whenever frontend/ changes. This script rebuilds
# frontend/ in place (Vite emits deterministic content-hashed files, so a fresh, matching bundle
# produces no change) and fails if that leaves web-dist/ different from what is committed. It also
# checks the gzipped JS+CSS stays within the 50 KB budget (R15.1).
#
# Exit codes: 0 = fresh and within budget (or npm unavailable — cannot verify, uses committed
# bundle as-is with a warning); 1 = stale (rebuilt bundle differs — commit it); 2 = over budget.
set -euo pipefail
cd "$(dirname "$0")/.."

BUDGET_BYTES=51200 # 50 KB

if ! command -v npm >/dev/null 2>&1; then
  echo "warning: npm not found; cannot verify web-dist/ freshness — using the committed bundle as-is" >&2
  exit 0
fi

[ -d frontend/node_modules ] || (cd frontend && npm ci --silent)
(cd frontend && npm run build --silent)

if ! git diff --quiet -- web-dist; then
  echo "error: web-dist/ was stale — it has now been rebuilt; review and commit the changes" >&2
  exit 1
fi

total=0
for f in web-dist/assets/*.js web-dist/assets/*.css; do
  [ -f "$f" ] || continue
  total=$((total + $(gzip -c "$f" | wc -c)))
done
if [ "$total" -gt "$BUDGET_BYTES" ]; then
  echo "error: bundle gzip ${total} B exceeds the ${BUDGET_BYTES} B budget (R15.1)" >&2
  exit 2
fi

echo "web-dist bundle OK: fresh and within budget (gzip ${total} B <= ${BUDGET_BYTES} B)"
