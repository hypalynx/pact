.PHONY: test

run:
	cargo run -- --debug

test: fmt

lint:
	cargo fmt --check
	cargo clippy

lint-fix:
	cargo fmt
	cargo clippy --fix
