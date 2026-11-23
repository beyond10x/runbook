---
format: aep.planning-md/3
id: story:conformance-capture-reads-all-output-within-a-bound
kind: story
status: draft
title: The conformance adapter captures a start value anywhere in the output, within a bound of memory
summary: 'Security pass 3 of the driver test-budget unit, P2 and P4, pre-existing: the capture reader stops taking lines after 1 MiB, and no bound holds the length of one line.'
relations:
- informed_by: story:driver-test-budget
revision: 2
---
## Outcome

The conformance adapter finds a start value the product printed wherever it stands in the product's output,
and reads that output within a stated bound of memory.

## Source

Security review pass 3 of `story:driver-test-budget` (2026-11-23), findings P2 and P4, in code older than
that unit (`driver/src/bin/sdc-conform-adapter.rs`, the capture reader):

- P2: the reader stops taking lines after 1 MiB. A value printed later is in `product.log` and is reported
  `capture_missing`.
- P4: nothing bounds the length of one line; the adapter's memory grows with it.

The unit's fix round 3 only made the answer of the first case say what the adapter read; the limit stays.

Security review pass 4 of the same unit (2026-11-23), green, left two things for this story:

- Note N2: the sentence of fix round 3 says the buffer "takes the first 1048576 bytes of a stream". That is
  approximate: a line that starts before the bound is taken whole, the count is taken after CR stripping and
  replacement of bad bytes, and a read error on the pipe gives the same sentence though no bound was reached.
- The two timeout branches of the adapter (`driver/src/bin/sdc-conform-adapter.rs:331` and `:337` at
  `dbf71410`, the "within N s" answers) still give the base's sentence for a value that was printed past the
  buffer.

## Acceptance

- A product that prints its start value after more than 1 MiB of other lines: the value is captured. One
  test, red before; the reviewer's case is kept by the coordinator for the implementor.
- A product that prints one line of 64 MiB without the value: the adapter's peak memory stays under a bound
  the page `docs/conformance.md` names, and the answer says the value did not appear. One test that reads
  the peak from the kernel's record of the adapter process.
- A value split across two writes, a NUL, a lone CR, a bad byte inside the value and a stream that ends
  without a newline behave as today (the existing cases stay green with unchanged assertions).
- `product.log` still holds every byte the product printed.
- Every answer of the adapter about a missing value is true of what the product printed: no branch (closed
  stream, the two timeout branches, a read error on the pipe) says the product did not print a value it did
  print, and a sentence that names a count of bytes names the count the reader holds. One test per branch.
- The driver test step stays within 120 s; no added test runs longer than 5 s.
