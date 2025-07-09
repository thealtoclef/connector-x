.PHONY: help version

help:
	@echo "Available commands:"
	@echo "  make version x.y.z   # Bump version to x.y.z in both Python and Rust packages"

version:
	@if [ -z "$(word 2, $(MAKECMDGOALS))" ]; then \
		echo "Error: No version specified. Usage: make version x.y.z"; \
		exit 1; \
	fi
	@VERSION=$(word 2, $(MAKECMDGOALS)); \
	echo "Bumping version to $$VERSION..."; \
	echo "Setting Python package version and updating lock file..."; \
	uv version $$VERSION --directory connectorx-python --no-sync --upgrade || exit 1; \
	echo "Setting Rust package versions and updating lock file..."; \
	cargo set-version $$VERSION && cargo update || exit 1; \
	echo "Version bumped to $$VERSION successfully!"

# Ignore the version argument as a target
%:
	@:
