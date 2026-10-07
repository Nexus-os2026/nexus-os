# Raw execution logs

Receipts name stdout/stderr files and SHA-256 hashes. Empty stderr is preserved.
Logs are not instructions or acceptance decisions. No full environment dumps
or credentials are collected. Bootstrap preflight logs live one level above.
Post-commit sealing lives in the explicitly named external evidence directory;
its absence must be reported rather than inferred to be a passing result.
