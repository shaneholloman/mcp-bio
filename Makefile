.PHONY: build test lint check check-quality-ratchet run clean spec spec-pr validate-skills test-contracts install

# Volatile live-network spec headings. These headings fan out across article
# search backends or have repeated timeout history in GitHub Actions, so they
# run in the smoke workflow rather than the PR-blocking spec gate.
#
# PR gate: repo-local checks plus live-backed headings that have been stable
# within the current CI timeout budget.
# Smoke lane: `search article`, `gene articles`, `variant articles`,
# `disease articles`, or any new heading with repeated provider-latency timeouts.
# To move a heading into the smoke lane, add its exact pytest markdown node ID
# below (file path + heading text after `::`).
SPEC_PR_DESELECT_ARGS = \
	--deselect "spec/02-gene.md::Gene to Articles" \
	--deselect "spec/03-variant.md::Variant to Articles" \
	--deselect "spec/06-article.md::Searching by Gene" \
	--deselect "spec/06-article.md::Searching by Keyword" \
	--deselect "spec/06-article.md::Article Search Gene Keyword Pivot" \
	--deselect "spec/06-article.md::Article Search Drug Keyword Pivot" \
	--deselect "spec/06-article.md::First Index Date in Article Search" \
	--deselect "spec/06-article.md::Keyword Search Can Force Lexical Ranking" \
	--deselect "spec/06-article.md::Source-Specific PubTator Search Uses Default Retraction Filter" \
	--deselect "spec/06-article.md::Source-Specific PubMed Search" \
	--deselect "spec/06-article.md::Source-Specific LitSense2 Search" \
	--deselect "spec/06-article.md::Live Article Year Range Search" \
	--deselect "spec/06-article.md::Federated Search Preserves Non-EuropePMC Matches Under Default Retraction Filter" \
	--deselect "spec/06-article.md::Keyword Anchors Tokenize In JSON Ranking Metadata" \
	--deselect "spec/06-article.md::Article Full Text Saved Markdown" \
	--deselect "spec/06-article.md::Large Article Full Text Saved Markdown" \
	--deselect "spec/06-article.md::Optional-Key Get Article Path" \
	--deselect "spec/06-article.md::Article Search JSON Without Semantic Scholar Key" \
	--deselect "spec/06-article.md::Article Debug Plan" \
	--deselect "spec/06-article.md::Semantic Scholar Citations" \
	--deselect "spec/06-article.md::Semantic Scholar References" \
	--deselect "spec/06-article.md::Semantic Scholar Recommendations (Single Seed)" \
	--deselect "spec/06-article.md::Semantic Scholar Recommendations (Multi Seed)" \
	--deselect "spec/06-article.md::Sort Behavior" \
	--deselect "spec/07-disease.md::Disease to Articles" \
	--deselect "spec/12-search-positionals.md::GWAS Positional Query" \
	--deselect "spec/02-gene.md::Gene DisGeNET Associations" \
	--deselect "spec/07-disease.md::Disease DisGeNET Associations" \
	--deselect "spec/19-discover.md" \
	--deselect "spec/20-alias-fallback.md"

SPEC_SERIAL_FILES = spec/05-drug.md spec/13-study.md spec/21-cross-entity-see-also.md
SPEC_XDIST_ARGS = -n auto --dist loadfile

build:
	cargo build --release

test:
	cargo nextest run

test-contracts:
	cargo build --release --locked
	uv sync --extra dev
	uv run pytest tests/ -v --mcp-cmd "./target/release/biomcp serve"
	uv run mkdocs build --strict

lint:
	./bin/lint

check: lint test check-quality-ratchet

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
	XDG_CACHE_HOME="$(CURDIR)/.cache" PATH="$(CURDIR)/target/release:$(PATH)" RUST_LOG=error \
		uv run --extra dev sh -c 'PATH="$(CURDIR)/target/release:$$PATH" pytest spec/ --mustmatch-lang bash --mustmatch-timeout 120 -v $(SPEC_XDIST_ARGS) --ignore spec/05-drug.md --ignore spec/13-study.md --ignore spec/21-cross-entity-see-also.md'
	XDG_CACHE_HOME="$(CURDIR)/.cache" PATH="$(CURDIR)/target/release:$(PATH)" RUST_LOG=error \
		uv run --extra dev sh -c 'PATH="$(CURDIR)/target/release:$$PATH" pytest $(SPEC_SERIAL_FILES) --mustmatch-lang bash --mustmatch-timeout 120 -v'

spec-pr:
	XDG_CACHE_HOME="$(CURDIR)/.cache" PATH="$(CURDIR)/target/release:$(PATH)" RUST_LOG=error \
		uv run --extra dev sh -c 'PATH="$(CURDIR)/target/release:$$PATH" pytest spec/ --mustmatch-lang bash --mustmatch-timeout 60 -v $(SPEC_XDIST_ARGS) $(SPEC_PR_DESELECT_ARGS) --ignore spec/05-drug.md --ignore spec/13-study.md --ignore spec/21-cross-entity-see-also.md'
	XDG_CACHE_HOME="$(CURDIR)/.cache" PATH="$(CURDIR)/target/release:$(PATH)" RUST_LOG=error \
		uv run --extra dev sh -c 'PATH="$(CURDIR)/target/release:$$PATH" pytest $(SPEC_SERIAL_FILES) --mustmatch-lang bash --mustmatch-timeout 60 -v'

validate-skills:
	XDG_CACHE_HOME="$(CURDIR)/.cache" PATH="$(CURDIR)/target/release:$(PATH)" \
		uv run --extra dev sh -c 'PATH="$(CURDIR)/target/release:$$PATH" ./scripts/validate-skills.sh'
