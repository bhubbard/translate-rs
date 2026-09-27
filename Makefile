.PHONY: all build test coverage clean

all: build

build:
	cargo build --release

test:
	cargo test --all-targets

coverage:
	cargo llvm-cov --html --output-dir ./target/coverage
	@echo "Coverage HTML report generated at ./target/coverage/index.html"

coverage-open:
	cargo llvm-cov --html --open

clean:
	cargo clean
	rm -rf coverage_report target/coverage
