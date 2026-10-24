# LLVM compiler-rt atomic runtime (unmodified source)

Version/tag: llvmorg-21.1.2
Source: https://github.com/llvm/llvm-project/tree/llvmorg-21.1.2/compiler-rt/lib/builtins
License: Apache-2.0 WITH LLVM-exception, retained in LICENSE.TXT.

Original SHA-256:
- atomic.c: 9fcf5fc6f0cb1da23f004fa9bc3af1cfcb5589f0c8d20e7b6de863591b60eb98
- assembly.h: 48a5c8e13dbae5694bbc9ab051392d58d08471ecac5e5f8cddf75f154a58cc64
- int_endianness.h: ec5175561075845b7b16a92ad5ecdfd42a284de2736dd07e4ca7011b75c37709
- LICENSE.TXT: 1a8f1058753f1ba890de984e48f0242a3a5c29a6a8f2ed9fd813f36985387e8d

Built with target C headers/compiler for i586-musl/Pentium into libatomic.a.
OpenSSL requires both atomic operations and __atomic_is_lock_free; the earlier
Zig atomics runtime lacked the latter. This full runtime supplies the standard
operations, alignment/lock-free query and locking fallback. No implementation
was stubbed, fences removed, or synchronization weakened. Main packaging must
include this license when distributing the corrected statically linked binary.
