.PHONY: build release check test lint fmt install clean

build:
	cargo build

release:
	cargo build --release

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
