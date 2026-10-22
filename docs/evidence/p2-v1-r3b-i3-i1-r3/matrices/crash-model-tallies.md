# Crash-model tallies (P2-V1-R3B-I3-I1-R3)

Each state is a crash point of the procedure, after F1 (process death) or after F2 (power loss) under one ordered or per-directory schedule of the pending metadata log, pending data old or new. Each count is the number of states that took that recovery path, as the test printed it.

## r3x1: P-SUCCESSOR, the ordinary succession: 30 crash points, 400 states

| path | states |
|---|---|
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR None, repeat 23 | 100 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR None, repeat 29 | 25 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(11), repeat 23 | 38 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(2), repeat 23 | 35 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(4), repeat 23 | 31 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(5), repeat 23 | 17 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(6), repeat 23 | 13 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(7), repeat 23 | 10 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(8), repeat 23 | 43 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(9), repeat 23 | 27 |
| predecessor: R-LEFTOVER 2 step(s), R-SUCCESSOR None, repeat 23 | 9 |
| predecessor: R-LEFTOVER 2 step(s), R-SUCCESSOR None, repeat 29 | 23 |
| predecessor: R-LEFTOVER 2 step(s), R-SUCCESSOR Some(11), repeat 23 | 19 |
| successor | 10 |
| **total** | **400** |

## r3x2: The repeat after F1 at operation 6 (step 2a skipped): 24 crash points

| path | states |
|---|---|
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR None, repeat 23 | 95 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(11), repeat 23 | 38 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(2), repeat 23 | 35 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(4), repeat 23 | 31 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(5), repeat 23 | 17 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(6), repeat 23 | 13 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(7), repeat 23 | 10 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(8), repeat 23 | 43 |
| predecessor: R-LEFTOVER 1 step(s), R-SUCCESSOR Some(9), repeat 23 | 27 |
| predecessor: R-LEFTOVER 2 step(s), R-SUCCESSOR Some(11), repeat 23 | 19 |
| successor | 10 |
| **total** | **338** |

## r3x3: R-SUCCESSOR, one pool file (12 crash points) and two (13)

| path | states |
|---|---|
| 1 pool file(s): root gone, R-SUCCESSOR again Some(1) | 10 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(11) | 9 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(2) | 14 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(4) | 9 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(5) | 13 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(6) | 17 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(7) | 26 |
| 1 pool file(s): root present, R-SUCCESSOR again Some(9) | 14 |
| 2 pool file(s): root gone, R-SUCCESSOR again Some(1) | 10 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(11) | 9 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(12) | 13 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(2) | 14 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(4) | 9 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(5) | 13 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(6) | 17 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(7) | 26 |
| 2 pool file(s): root present, R-SUCCESSOR again Some(9) | 14 |
| **total** | **237** |

## r3x4: P-REVOKE: 4 crash points, 30 states; R-REVOKE's own crash points from each split

| path | states |
|---|---|
| point 0: published: P-REVOKE again | 5 |
| point 1: blocks: R-REVOKE 1 (the sync) | 5 |
| point 1: blocks: the revoked file lost (per-directory family) | 2 |
| point 1: invalid: R-REVOKE, and its crash points {\"blocks, R-REVOKE 1\": 10, \"invalid, R-REVOKE 2\": 9} | 2 |
| point 1: published: P-REVOKE again | 4 |
| point 2: blocks: R-REVOKE 1 (the sync) | 5 |
| point 2: invalid: R-REVOKE, and its crash points {\"blocks, R-REVOKE 1\": 10, \"invalid, R-REVOKE 2\": 9} | 2 |
| point 3: blocks: R-REVOKE 1 (the sync) | 5 |
| **total** | **30** |

## r3x5: P-PROV: 24 crash points, 338 states

| path | states |
|---|---|
| fresh | 10 |
| unprovisioned: P-PROV at a new root (the interrupted root absent) | 95 |
| unprovisioned: P-PROV at a new root (the interrupted root kept) | 214 |
| unprovisioned: R-REPUBLISH | 19 |
| **total** | **338** |

## r3x6: P-REQUALIFY: 6 crash points, 52 states

| path | states |
|---|---|
| revision 2 | 10 |
| unsupported: R-LEFTOVER 1 step(s), P-REQUALIFY again | 23 |
| unsupported: R-LEFTOVER 2 step(s), P-REQUALIFY again | 19 |
| **total** | **52** |
