.PHONY: build test bench fuzz bundle clean check

CC := $(shell which cyrius 2>/dev/null || which cc3 2>/dev/null || echo $(HOME)/Repos/cyrius/build/cc3)

build:
	@mkdir -p build
	cat src/main.cyr | $(CC) > build/yukti 2>/dev/null
	@chmod +x build/yukti
	@echo "build/yukti: $$(wc -c < build/yukti) bytes"

test:
	@mkdir -p build
	cat tests/yukti.tcyr | $(CC) > build/yukti_test 2>/dev/null
	@chmod +x build/yukti_test
	@./build/yukti_test 2>/dev/null

bench:
	@mkdir -p build
	cat benches/bench.bcyr | $(CC) > build/yukti_bench 2>/dev/null
	@chmod +x build/yukti_bench
	@./build/yukti_bench 2>/dev/null

fuzz:
	@mkdir -p build
	@for f in fuzz/*.fcyr; do \
		name=$$(basename "$$f" .fcyr); \
		cat "$$f" | $(CC) > "build/$$name" 2>/dev/null; \
		chmod +x "build/$$name"; \
		./build/$$name 2>/dev/null; \
	done

bundle:
	@./scripts/bundle.sh

check: build test fuzz
	@echo "all checks passed"

clean:
	rm -rf build/
