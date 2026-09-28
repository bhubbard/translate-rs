.PHONY: all build test coverage clean

all: build

build:
	cargo build --release

test:
	cargo test --all-targets

test-accuracy:
	cargo test --test accuracy_test

test-perf:
	cargo test --test performance_test

bench-perf:
	cargo run --release --example bench_translation

bench-accuracy:
	python3 benches/accuracy_bench.py

coverage:
	cargo llvm-cov --html --output-dir ./coverage_report
	@echo "Coverage HTML report generated at ./coverage_report/html/index.html"

coverage-open:
	cargo llvm-cov --html --open

clean:
	cargo clean
	rm -rf coverage_report target/coverage
