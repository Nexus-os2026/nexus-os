// The ledger. Only its protocol core exists yet, and nothing outside this
// module calls it (contract P3-C11-D2A1-CONTRACT-02-R1, section 3.2). The
// attribute allows unused code only in a build that is not a test build; the
// mission that makes the core reachable from production removes or narrows it.

#[cfg_attr(not(test), allow(dead_code))]
mod protocol;

// PROVISIONAL, test builds only: a SQLite storage shell written against the
// unadopted draft P3-C11-D2A-FULL-CONTRACT-DRAFT-01. No production build
// contains it; SQLite is a dev-dependency.
#[cfg(test)]
mod provisional_shell_tests;
