# Request-Plan Ratchets

BioMCP keeps source request construction deterministic before any live upstream call.
These ratchets make the routine spec gate require the language-native tests that
protect high-risk request-plan and option-help seams.

## Update Help Keeps Unsafe Checksum Override on the Option Stanza

The update command's unsafe checksum escape hatch must be proven against the
rendered option stanza, not only against prose elsewhere in long help. The
Python docs contract runs the rendered CLI help and extracts the actual option
block.

```bash
set -o pipefail
cd ../.. && uv run --no-sync pytest tests/test_update_command_docs_contract.py::test_update_help_allow_missing_checksum_option_stanza_marks_unsafe_checksum_override -v | mustmatch like "test_update_help_allow_missing_checksum_option_stanza_marks_unsafe_checksum_override"
```

## MyDisease Rejects Path and Query Separators Before Network

A disease ID is data, not a path fragment. The no-network Rust ratchet must
prove that slash, backslash, query, and fragment separators are rejected while a
valid ontology ID still plans the `/disease/{id}` request shape.

```bash
set -o pipefail
cd ../.. && cargo test --lib ticket_400_mydisease_get_rejects_path_query_separators_before_network -- --nocapture | mustmatch like "ticket_400_mydisease_get_rejects_path_query_separators_before_network"
```

## Request Commands Consume Captured Fields at Execution Boundaries

Command dispatch should not construct request structs that executors ignore.
The Rust seam tests prove discover, disease search, disease fallback, and
article dispatch consume the request fields that carry user intent into source
or backend calls.

```bash
set -o pipefail
cd ../.. && cargo test --lib ticket_400_request_command -- --nocapture | mustmatch like "ticket_400_request_command_discover_fields_drive_resolve_boundaries
ticket_400_request_command_disease_search_fields_drive_source_query_and_pagination
ticket_400_request_command_disease_fallback_fields_drive_discover_and_crosswalk_boundaries
ticket_400_request_command_article_fields_drive_execution_boundaries"
```

## PubMed and PubTator Consume Planned Auth and Cache Modes

Secret-aware article sources must use the plan's redacted auth/cache modes at
the executor boundary. These tests use synthetic keys and keyless clients so the
routine gate proves keyed behavior without requiring real credentials.

```bash
set -o pipefail
cd ../.. && cargo test --lib ticket_400_pub -- --nocapture | mustmatch like "ticket_400_pubmed_auth_and_cache_modes_are_consumed_from_request_plans
ticket_400_pubtator_auth_and_cache_modes_are_consumed_from_request_plans"
```

## Shared Retry-After Waits Stay Bounded

Shared HTTP retries should honor ordinary upstream `Retry-After` hints without
letting an extreme header park a CLI command or March worker indefinitely. The
Rust policy tests keep normal, malformed, extreme, and total-budget paths
deterministic without calling a live service.

```bash
set -o pipefail
cd ../.. && cargo test --lib ticket_403_retry -- --nocapture | mustmatch like "ticket_403_retry_after_normal_floor_is_honored
ticket_403_retry_after_malformed_values_fall_back_to_backoff
ticket_403_retry_after_extreme_values_are_capped
ticket_403_retry_send_uses_the_shared_retry_sleep_budget"
```

## Ticket 401 Surface Ratchets

The post-migration spec runner must keep routine Python surface contracts in the
routine lane, and the static ratchets around spec quality and fixture realism
must fail when weak proof shapes return. This local contract file covers the
runner, Cargo-wrapper, robust mustmatch-lint, and Figshare fixture gaps without
calling public services.

```bash
set -o pipefail
cd ../.. && uv run --no-sync pytest spec/surface/test_ticket_401_surface_ratchets.py -v | mustmatch like "test_ticket_401_quality_ratchet_rejects_printf_captured_output_mustmatch
test_ticket_401_article_figshare_fixture_uses_realistic_aacr_sibling_shapes
test_ticket_401_request_plan_ratchets_execute_named_contracts_not_list_only
test_ticket_401_routine_modes_execute_python_surface_contracts"
```
