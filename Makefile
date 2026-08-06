SHELL := /bin/bash

PROJECT_ROOT := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))

.PHONY: help clean clean-build clean-generated clean-cache

help:
	@echo "Amadeus maintenance commands"
	@echo "  make clean            Remove builds, generated output, and local caches"
	@echo "  make clean-build      Remove Rust, web, and desktop build output"
	@echo "  make clean-generated  Remove logs, test output, and benchmark results"
	@echo "  make clean-cache      Remove Python and test caches"

clean: clean-build clean-generated clean-cache
	@echo "Amadeus workspace cleaned."

clean-build:
	@echo "Removing build output..."
	@cargo clean --manifest-path "$(PROJECT_ROOT)/Cargo.toml"
	@cargo clean --manifest-path "$(PROJECT_ROOT)/apps/web/src-tauri/Cargo.toml"
	@rm -rf -- \
		"$(PROJECT_ROOT)/apps/web/dist" \
		"$(PROJECT_ROOT)/apps/web/src-tauri/binaries" \
		"$(PROJECT_ROOT)/apps/web/src-tauri/gen"

clean-generated:
	@echo "Removing generated logs and results..."
	@rm -rf -- \
		"$(PROJECT_ROOT)/.amadeus/logs" \
		"$(PROJECT_ROOT)/logs" \
		"$(PROJECT_ROOT)/benchmark_runs" \
		"$(PROJECT_ROOT)/benchmarks/results" \
		"$(PROJECT_ROOT)/runtime/rag_eval/results"
	@find \
		"$(PROJECT_ROOT)/runtime/locomo/debug_logs" \
		"$(PROJECT_ROOT)/runtime/locomo/results" \
		-mindepth 1 ! -name '.gitkeep' -delete
	@find "$(PROJECT_ROOT)" -maxdepth 1 -type f \
		\( -name 'test_*.txt' -o -name '*.profraw' \) -delete

clean-cache:
	@echo "Removing local caches..."
	@find "$(PROJECT_ROOT)" \
		\( -path "$(PROJECT_ROOT)/.git" \
		-o -path "$(PROJECT_ROOT)/refs" \
		-o -path "$(PROJECT_ROOT)/node_modules" \
		-o -path "$(PROJECT_ROOT)/apps/web/node_modules" \
		-o -path "$(PROJECT_ROOT)/.opencode/node_modules" \) -prune \
		-o -type d \( -name '__pycache__' -o -name '.pytest_cache' \) \
		-exec rm -rf -- {} +
