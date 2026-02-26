.PHONY: test

run:
	cargo run -- --debug

test: fmt

fmt:
	cargo fmt --check

fmt-fix:
	cargo fmt
