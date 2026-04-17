.PHONY: build test bench fuzz dist lock verify clean check audit

# Modern Cyrius 5.x flow. cc5 is the backing compiler; `cyrius` is the build tool.
CC := $(shell which cc5 2>/dev/null || echo $(HOME)/.cyrius/bin/cc5)

build:
	@mkdir -p build
	@cat src/main.cyr | $(CC) > build/yukti 2>build/yukti.log
	@chmod +x build/yukti
	@echo "build/yukti: $$(wc -c < build/yukti) bytes"

test:
	@mkdir -p build
	@cat tests/yukti.tcyr | $(CC) > build/yukti_test 2>build/yukti_test.log
	@chmod +x build/yukti_test
	@./build/yukti_test

bench:
	@mkdir -p build
	@cat benches/bench.bcyr | $(CC) > build/yukti_bench 2>build/yukti_bench.log
	@chmod +x build/yukti_bench
	@./build/yukti_bench

fuzz:
	@mkdir -p build
	@for f in fuzz/*.fcyr; do \
		name=$$(basename "$$f" .fcyr); \
		cat "$$f" | $(CC) > "build/$$name" 2>/dev/null; \
		chmod +x "build/$$name"; \
		./build/$$name; \
	done

dist:
	@cyrius distlib

lock:
	@cyrius deps --lock

verify:
	@cyrius deps --verify

audit:
	@cyrius audit

check: build test fuzz
	@echo "all checks passed"

clean:
	rm -rf build/
