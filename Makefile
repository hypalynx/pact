.PHONY: test

test: fmt

fmt:
	cargo fmt --check

fmt-fix:
	cargo fmt
