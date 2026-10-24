# event-listener 5.4.2 compatibility patch

Source: https://crates.io/crates/event-listener/5.4.2
Repository: https://github.com/smol-rs/event-listener
Registry checksum: 5a23add41df1562121a9393cb065eab5146a1242410f23a644851e90cfd669d2
Original per-file SHA-256 values: EVENT-LISTENER-SOURCES.json (do not rewrite).
Upstream MIT and Apache-2.0 licenses and package metadata are retained.

## Local change

Only `event-listener/src/notify.rs`, `full_fence()`:
32-bit x86 Linux musl uses `lock or dword ptr [temporary], 0` instead of upstream's
`lock not`. All other targets keep upstream behavior. OR changes flags, so that
branch deliberately omits the `preserves_flags` assembly option. Memory effects
and the LOCK/full-barrier operation remain; this is not a compiler-only fence,
removed synchronization, or weakened ordering. The temporary remains UnsafeCell.

Actual iSH evidence: the user runs Alpine-minirootfs3.23.3-x86. `tela --help`
worked; `ish-smoke` trapped at0x591463 with bytes F0 F7 10. Disassembly identified
this exact LOCK NOT inside async-lock mutex notification. Inspected iSH1.3.2 and
current sources support LOCK OR; their historical MFENCE decoding did not emit
a host barrier. We therefore retain an explicit locked operation rather than
switch to an accepted-but-potentially-no-op MFENCE on that emulator.

This is a narrow compatibility fix, not a proof of iSH's entire memory model.
Verify emitted code, notification/contended handoff behavior, and the actual
device. No TLS or application cryptographic parameters are changed by this patch.
The root Cargo.toml patch selects this source. Public release review must include
these files/licenses and describe the compatibility change; nothing is published
or submitted upstream automatically.
