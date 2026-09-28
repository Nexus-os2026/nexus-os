APP_DIR := app

.PHONY: frontend-build nexus-os clean-nexus-os

frontend-build:
	npm --prefix $(APP_DIR) run build

# WITHDRAWN during Phase Zero (P0-FINAL-GATE-CLOSURE, dossier item J3): the
# protocols `nexus-os` binary is withdrawn. This target builds and copies
# nothing; it fails.
nexus-os:
	@echo "make nexus-os: withdrawn during Phase Zero; nothing is built" >&2; exit 1

clean-nexus-os:
	rm -f ./nexus-os
