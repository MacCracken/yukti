.PHONY: check fmt clippy test test-all bench audit deny coverage doc build clean

check: fmt clippy test audit
fmt:
	cargo fmt --all -- --check
clippy:
	cargo clippy --all-targets --all-features -- -D warnings
test:
	cargo test
test-all:
	cargo test --all-features
bench:
	cargo bench --bench yantra_bench
audit:
	cargo audit
deny:
	cargo deny check
coverage:
	cargo tarpaulin --all-features --skip-clean
doc:
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
build:
	cargo build --release --all-features
clean:
	cargo clean
	rm -rf coverage/
