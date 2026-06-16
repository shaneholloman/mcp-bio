#!/usr/bin/env bash
# Ratchet: routine source tier-tests must be PURE (Tier 1-3) — no mock HTTP servers, no
# base-URL env vars, no process-env mutation. Real round-trips belong in #[ignore] Tier-4
# `live.rs` tests (verify lane), not the routine gate.
#
# Run from anywhere; checks src/sources/*/tests/. Wire into `make lint` at cutover.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! compgen -G "src/sources/*/tests" > /dev/null; then
  echo "✓ no source tier-test dirs yet (nothing to check)"
  exit 0
fi

# live.rs is the sanctioned Tier-4 home; it may use Client::new() but still must not spin a
# MockServer or mutate env. We forbid the server/env machinery everywhere under tests/.
# Ignore comment lines (//, //!, ///) so prose like "no MockServer" in doc comments does
# not trip the gate; real usages (MockServer::start(), `use wiremock`, set_env_var(...))
# live on code lines.
violations=$(grep -rnE 'MockServer|wiremock|BIOMCP_[A-Z0-9_]*BASE|set_env_var|env_lock\(' \
  src/sources/*/tests/ 2>/dev/null | grep -vE ':[0-9]+:[[:space:]]*//' || true)

if [ -n "$violations" ]; then
  echo "❌ routine source tier-tests must be pure (Tier 1-3). Found server/env machinery:"
  echo "$violations"
  echo
  echo "Fix: assert requests via RequestPlan (Tier 2), parse committed fixtures via"
  echo "decode_json (Tier 3); move any real round-trip to a #[ignore] Tier-4 test in live.rs."
  exit 1
fi

echo "✓ source tier-tests are pure (no MockServer / no BIOMCP_*_BASE / no env mutation)"
