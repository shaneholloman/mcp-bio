.PHONY: build test lint check-quality-ratchet release-gate run clean spec spec-pr spec-contracts verify release-live-smoke validate-skills test-contracts install sync-python-dev

SPEC_XDIST_ARGS = -n auto --dist loadfile
SPEC_ROUTINE_PATHS = \
	spec/entity/article.md \
	spec/entity/study.md \
	spec/entity/variant.md \
	spec/surface/mcp.md \
	spec/surface/test_architecture_docs_parity_contract.py \
	spec/surface/test_biomcp_ci_path_contract.py \
	spec/surface/test_complexportal_fixture_contract.py \
	spec/surface/test_parallel_isolation_contract.py \
	spec/surface/test_search_all_cli_structure.py \
	spec/surface/test_semantic_scholar_retry_after_contract.py \
	spec/surface/test_trial_help_contract.py \
	spec/surface/test_variant_normalization_docs_contract.py
SPEC_LIVE_PATHS = \
	spec/entity/diagnostic.md \
	spec/entity/disease.md \
	spec/entity/drug.md \
	spec/entity/gene.md \
	spec/entity/pathway.md \
	spec/entity/pgx.md \
	spec/entity/phenotype.md \
	spec/entity/protein.md \
	spec/entity/trial.md \
	spec/entity/vaers.md \
	spec/surface/cli.md \
	spec/surface/discover.md

sync-python-dev:
	uv sync --extra dev --no-install-project

build:
	cargo build --release

test:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	cargo nextest run
	uv run --no-sync pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"
	uv run --no-sync mkdocs build --strict

test-contracts:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	uv run --no-sync pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"
	uv run --no-sync mkdocs build --strict

lint:
	./bin/lint
	tools/check-quality-ratchet.sh

release-gate: lint test spec

check-quality-ratchet:
	@bash tools/check-quality-ratchet.sh

run:
	cargo run --

clean:
	cargo clean

install:
	mkdir -p "$(HOME)/.local/bin"
	cargo build --release --locked
	install -m 755 target/release/biomcp "$(HOME)/.local/bin/biomcp"

spec:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	bash spec/fixtures/setup-study-spec-fixture.sh "$(CURDIR)"
	bash spec/fixtures/setup-ddinter-spec-fixture.sh "$(CURDIR)"
	. "$(CURDIR)/.cache/spec-study-env"; . "$(CURDIR)/.cache/spec-ddinter-env"; PATH="$(CURDIR)/target/release:$(PATH)" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" pytest $(SPEC_ROUTINE_PATHS) --mustmatch-lang bash --mustmatch-timeout 120 -v $(SPEC_XDIST_ARGS)'

spec-pr:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	bash spec/fixtures/setup-study-spec-fixture.sh "$(CURDIR)"
	bash spec/fixtures/setup-ddinter-spec-fixture.sh "$(CURDIR)"
	. "$(CURDIR)/.cache/spec-study-env"; . "$(CURDIR)/.cache/spec-ddinter-env"; PATH="$(CURDIR)/target/release:$(PATH)" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" pytest $(SPEC_ROUTINE_PATHS) --mustmatch-lang bash --mustmatch-timeout 180 -v $(SPEC_XDIST_ARGS)'

spec-contracts:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	bash spec/fixtures/setup-study-spec-fixture.sh "$(CURDIR)"
	. "$(CURDIR)/.cache/spec-study-env"; PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$$PWD/target/release:$$PATH" BIOMCP_BIN="$$PWD/target/release/biomcp" pytest spec/entity/article.md spec/surface/mcp.md spec/surface/test_parallel_isolation_contract.py --mustmatch-lang bash --mustmatch-timeout 180 -v'

verify:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci discover ERBB1
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci search disease melanoma --limit 3
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci search article -g BRAF --limit 3
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci variant normalize all 'NM_000248.3:c.135del'
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$$PWD/target/release:$$PATH" BIOMCP_BIN="$$PWD/target/release/biomcp" pytest $(SPEC_LIVE_PATHS) --mustmatch-lang bash --mustmatch-timeout 180 -v'

release-live-smoke:
	$(MAKE) verify

validate-skills:
	$(MAKE) sync-python-dev
	PATH="$(CURDIR)/target/release:$(PATH)" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" ./scripts/validate-skills.sh'
