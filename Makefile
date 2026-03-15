APP_DIR := app

.PHONY: frontend-build nexus-os clean-nexus-os

frontend-build:
	npm --prefix $(APP_DIR) run build

nexus-os: frontend-build
	cargo build --release -p nexus-protocols --bin nexus-os
	cp target/release/nexus-os ./nexus-os

clean-nexus-os:
	rm -f ./nexus-os
