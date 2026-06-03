.PHONY: build test lint check check-quality-ratchet release-gate run clean spec spec-pr spec-contracts release-live-smoke validate-skills test-contracts install sync-python-dev

SPEC_XDIST_ARGS = -n auto --dist loadfile

sync-python-dev:
	uv sync --extra dev --no-install-project

build:
	cargo build --release

test:
	cargo nextest run

test-contracts:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	uv run --no-sync pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"
	uv run --no-sync mkdocs build --strict

lint:
	./bin/lint

check: lint test test-contracts check-quality-ratchet

release-gate: check spec-contracts

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
# Keep live/upstream-heavy canaries out of the main xdist partition.
	cargo build --release --locked
	$(MAKE) sync-python-dev
	bash spec/fixtures/setup-study-spec-fixture.sh "$(CURDIR)"
	bash spec/fixtures/setup-ddinter-spec-fixture.sh "$(CURDIR)"
	. "$(CURDIR)/.cache/spec-study-env"; . "$(CURDIR)/.cache/spec-ddinter-env"; PATH="$(CURDIR)/target/release:$(PATH)" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" pytest spec/entity/ spec/surface/ --mustmatch-lang bash --mustmatch-timeout 120 -v $(SPEC_XDIST_ARGS) --deselect spec/entity/protein.md --deselect spec/entity/disease.md --deselect spec/surface/discover.md --deselect spec/entity/pathway.md'
	. "$(CURDIR)/.cache/spec-study-env"; . "$(CURDIR)/.cache/spec-ddinter-env"; PATH="$(CURDIR)/target/release:$(PATH)" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" pytest spec/entity/protein.md spec/entity/disease.md spec/surface/discover.md --mustmatch-lang bash --mustmatch-timeout 120 -v'

spec-pr:
# Keep live/upstream-heavy canaries out of the main xdist partition.
	cargo build --release --locked
	$(MAKE) sync-python-dev
	bash spec/fixtures/setup-study-spec-fixture.sh "$(CURDIR)"
	bash spec/fixtures/setup-ddinter-spec-fixture.sh "$(CURDIR)"
	. "$(CURDIR)/.cache/spec-study-env"; . "$(CURDIR)/.cache/spec-ddinter-env"; PATH="$(CURDIR)/target/release:$(PATH)" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" pytest spec/entity/ spec/surface/ --mustmatch-lang bash --mustmatch-timeout 180 -v $(SPEC_XDIST_ARGS) --deselect spec/entity/protein.md --deselect spec/entity/disease.md --deselect spec/surface/discover.md --deselect spec/entity/pathway.md'
	. "$(CURDIR)/.cache/spec-study-env"; . "$(CURDIR)/.cache/spec-ddinter-env"; PATH="$(CURDIR)/target/release:$(PATH)" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" BIOMCP_BIN="$(CURDIR)/target/release/biomcp" pytest spec/entity/protein.md spec/entity/disease.md spec/surface/discover.md --mustmatch-lang bash --mustmatch-timeout 180 -v'

spec-contracts:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$$PWD/target/release:$$PATH" BIOMCP_BIN="$$PWD/target/release/biomcp" pytest spec/surface/cli.md spec/surface/test_parallel_isolation_contract.py --mustmatch-lang bash --mustmatch-timeout 180 -v'

release-live-smoke:
	cargo build --release --locked
	$(MAKE) sync-python-dev
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci discover ERBB1
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci search disease melanoma --limit 3
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci search article -g BRAF --limit 3
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" tools/biomcp-ci variant normalize all 'NM_000248.3:c.135del'
	PATH="$${PWD}/target/release:$$PATH" BIOMCP_BIN="$${PWD}/target/release/biomcp" \
		uv run --no-sync sh -c 'PATH="$$PWD/target/release:$$PATH" BIOMCP_BIN="$$PWD/target/release/biomcp" pytest spec/entity/pathway.md --mustmatch-lang bash --mustmatch-timeout 180 -v'

validate-skills:
	$(MAKE) sync-python-dev
	PATH="$(CURDIR)/target/release:$(PATH)" \
		uv run --no-sync sh -c 'PATH="$(CURDIR)/target/release:$$PATH" ./scripts/validate-skills.sh'
