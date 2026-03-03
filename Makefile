.PHONY: test lint lint-fix coverage reset-db

run:
	cargo run -- --debug

test: lint
	cargo test

coverage:
	cargo llvm-cov --json --summary-only --output-path coverage.json && \
	cat coverage.json | jq '.data[0].files | map(select(.filename | contains("src/pact/src"))) | sort_by(.filename) | .[] | "\(.filename | split("/") | .[-1]): \(.summary.lines.percent | round)% lines, \(.summary.functions.percent | round)% functions"' -r && \
	echo "---" && \
	cat coverage.json | jq '.data[0].totals | "Total: \(.lines.percent | round)% lines, \(.functions.percent | round)% functions"' -r

lint:
	cargo fmt --check
	cargo clippy

lint-fix:
	cargo fmt
	cargo clippy --fix --allow-dirty

reset-db:
	rm -f ~/.local/share/pact/pact.db

release:
	./release.sh

install:
	cargo install --path .
