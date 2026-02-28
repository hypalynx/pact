.PHONY: test lint lint-fix coverage

run:
	cargo run -- --debug

test: lint
	cargo test

coverage:
	cargo llvm-cov --json --summary-only --output-path coverage.json && \
	cat coverage.json | jq '.data[0].files | map(select(.filename | contains("src/pact/src"))) | sort_by(.filename) | .[] | "\(.filename | split("/") | .[-1]): \(.summary.lines.percent | round)% lines, \(.summary.functions.percent | round)% functions"' -r

lint:
	cargo fmt --check
	cargo clippy

lint-fix:
	cargo fmt
	cargo clippy --fix --allow-dirty
