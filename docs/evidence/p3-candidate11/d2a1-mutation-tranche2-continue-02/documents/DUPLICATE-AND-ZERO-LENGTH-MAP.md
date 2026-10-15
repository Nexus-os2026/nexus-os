# Duplicate and zero-length map — P3-C11-DR01-D2A1-MUTATION-TRANCHE2-CONTINUE-02

Every staged file whose git blob equals that of a file of a previous packet (any file under `docs/evidence/p3-candidate11/` at `2e06860f4b404d1a79a0d290116239b5545b931f`), and every empty staged file. Each is a deterministic record the unchanged supervise.py writes at that exact path, bound to its own descriptor; the policy (tools/build_packet.py) enumerates the exact paths from the frozen order and the self-test numbering, with no size threshold and no wildcard, and rejects every other duplicate and every other empty file.

| Class | Path | Bytes | Literal | Descriptor | Equal previous-packet files |
|---|---|---|---|---|---|
| CGROUP_CONSTANT | `receipts/01-supervision-selftest/1/cgroup-samples.txt` | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 92 |
| SELFTEST_CONSTANT | `receipts/01-supervision-selftest/1/out.txt` | 19 | `allocated 10485760\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 11 |
| ZERO_LENGTH | `receipts/01-supervision-selftest/3/out.txt` | 0 | `(empty)` | `receipts/01-supervision-selftest/SELFTEST.txt` | 0 |
| SELFTEST_TIME_RECORD | `receipts/01-supervision-selftest/3/time-v.txt` | 784 | `Command terminated by signal 9\n\tCommand being timed: "timeout -s KILL 3 nice -n 10 /bin/sleep 30"\n\tUser time (seconds): 0.00\n\tSystem time (seconds): 0.00\n\tPercent of CPU this job got: 0%\n\tElapsed (wall clock) time (h:mm:ss or m:ss): 0:03.00\n\tAverage shared text size (kbytes): 0\n\tAverage unshared data size (kbytes): 0\n\tAverage stack size (kbytes): 0\n\tAverage total size (kbytes): 0\n\tMaximum resident set size (kbytes): 1532\n\tAverage resident set size (kbytes): 0\n\tMajor (requiring I/O) page faults: 0\n\tMinor (reclaiming a frame) page faults: 81\n\tVoluntary context switches: 2\n\tInvoluntary context switches: 0\n\tSwaps: 0\n\tFile system inputs: 0\n\tFile system outputs: 0\n\tSocket messages sent: 0\n\tSocket messages received: 0\n\tSignals delivered: 0\n\tPage size (bytes): 4096\n\tExit status: 0\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 2 |
| CGROUP_CONSTANT | `receipts/01-supervision-selftest/5/cgroup-samples.txt` | 95 | `sampler: the scope's cgroup was not found while it ran (a short run; scope-wrap.sh records it)\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 92 |
| ZERO_LENGTH | `receipts/01-supervision-selftest/5/out.txt` | 0 | `(empty)` | `receipts/01-supervision-selftest/SELFTEST.txt` | 0 |
| SELFTEST_CONSTANT | `receipts/01-supervision-selftest/6/out.txt` | 43 | `timeout: the monitored command dumped core\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 10 |
| SELFTEST_CONSTANT | `receipts/01-supervision-selftest/7/err.txt` | 10 | `to stderr\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 4 |
| SELFTEST_CONSTANT | `receipts/01-supervision-selftest/7/out.txt` | 42 | `['CARGO_NET_OFFLINE', 'LC_CTYPE', 'PATH']\n` | `receipts/01-supervision-selftest/SELFTEST.txt` | 4 |
| ZERO_LENGTH | `receipts/62-A3/03-build/build.stdout` | 0 | `(empty)` | `receipts/62-A3/03-build/descriptor.json` | 0 |
| ZERO_LENGTH | `receipts/63-A6/03-build/build.stdout` | 0 | `(empty)` | `receipts/63-A6/03-build/descriptor.json` | 0 |
| ZERO_LENGTH | `receipts/64-C8/03-build/build.stdout` | 0 | `(empty)` | `receipts/64-C8/03-build/descriptor.json` | 0 |
| ZERO_LENGTH | `receipts/65-C15/03-build/build.stdout` | 0 | `(empty)` | `receipts/65-C15/03-build/descriptor.json` | 0 |

Rows: 13. The TSV beside this file lists every equal previous-packet file by commit and path.
