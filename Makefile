.PHONY: build release release-build check test lint fmt install clean

build:
	cargo build

release-build:
	cargo build --release

release:
	@VER=$$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/'); \
	MAJOR=$$(echo $$VER | cut -d. -f1); \
	MINOR=$$(echo $$VER | cut -d. -f2); \
	PATCH=$$(echo $$VER | cut -d. -f3); \
	LAST_TYPE=$$(git log --merges -1 --format='%s' | sed -n 's/^Merge.*from.*\/\([a-z]*\).*/\1/p'); \
	if [ -z "$$LAST_TYPE" ]; then \
		LAST_TYPE=$$(git log -1 --format='%s' | sed -n 's/^\([a-z]*\).*/\1/p'); \
	fi; \
	case "$$LAST_TYPE" in \
		feat) MINOR=$$((MINOR + 1)); PATCH=0 ;; \
		*)    PATCH=$$((PATCH + 1)) ;; \
	esac; \
	NEXT="$$MAJOR.$$MINOR.$$PATCH"; \
	TAG="v$$NEXT"; \
	echo "$$VER -> $$NEXT ($$LAST_TYPE)"; \
	cargo set-version "$$NEXT"; \
	git add Cargo.toml Cargo.lock; \
	git commit -m "chore(release): $$TAG"; \
	git tag -a "$$TAG" -m "Release $$TAG"; \
	git push origin main --follow-tags; \
	cargo publish --allow-dirty

check:
	cargo check

test:
	cargo test

lint:
	cargo clippy
	cargo fmt -- --check

fmt:
	cargo fmt

install:
	cargo install --path .

clean:
	cargo clean
