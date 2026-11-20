---
description: Find CPU, memory and allocation costs in Rust code paths, agree on fixes including breaking redesigns, and implement them.
argument-hint: "<path, crate, or area>"
---

Improve the performance of the Rust code within: $ARGUMENTS

If the scope is missing or ambiguous, ask before proceeding. Read the scoped code, its dependencies, callers, tests, and project conventions before proposing changes.

## Method

1. Map the scope: entry points, loops, data structures, allocations, threads and locks, I/O, FFI and unsafe code. Follow calls into dependencies until the cost is known.
2. Sort each path into init, load or streaming, and steady state: per frame, per event, per item or per byte. Rank paths by frequency × cost. Cold paths matter only for latency spikes and stutter.
3. Name what bounds each hot path before proposing a fix:
   - algorithmic work;
   - allocation or syscalls;
   - memory latency (pointer chasing) or memory bandwidth (streaming);
   - dependency chains (latency) or execution ports (throughput);
   - branch mispredictions;
   - front-end or instruction-cache pressure;
   - contention between threads.
4. Check every item below that applies. Back each finding with `file_path:line_number`, and never raise a guideline without evidence in the code. When a claim rests on codegen (vectorization, bounds checks, inlining, `cmov`), confirm it in the assembly of a minimal reproduction built with the root `Cargo.toml`'s `[profile.release]` and the `.cargo/config.toml` rustflags. A finding whose cost is unmeasured is a hypothesis: measure it first, or propose the measurement as the first step of its fix.
5. For a large scope, split disjoint areas across `Agent`s with this checklist. Have them return evidence-backed findings without editing, and merge them.

## Agree before editing

Before editing, use `AskUserQuestion` to agree on which findings to fix and how. Group consequential choices and recommend one fix per finding.

- Present each consequential finding concisely: impact, the hot path and what bounds it, evidence, the fix as before → after types or signatures with representative call sites, the expected effect with its reasoning, and whether it is breaking.
- Breaking changes are allowed. Recommend the design that removes a cost at its root, including new APIs, data layouts, ownership and module boundaries, over patches that keep compatibility.
- Name the trade-offs: added complexity, `unsafe`, binary size, changed float rounding, dropped CPUs.
- Offer small, local, non-breaking fixes together as one choice.
- Resolve design questions that arise during implementation before implementing them.

Impact levels:

- Critical: asymptotic blow-up, unbounded growth, latency spikes on a hot path, or UB in performance-motivated unsafe code.
- High: per-item allocation, pointer chasing, mispredicts or blocked vectorization in a hot loop, or contention.
- Medium and Low for the rest.

Costs on a Zen 2-class core, for estimating impact:

| Operation | Cost |
|---|---|
| L1 hit | about 4–5 cycles (2 ns) |
| L2 hit | about 5 ns |
| L3 hit | about 20 ns |
| RAM access | about 100 ns |
| Branch mispredict | 15–20 cycles |
| Integer division | 13–28 cycles, barely pipelined |
| Multiply | 3 cycles |
| FMA | 5 cycles latency, 2 per cycle |
| Indirect call with a varying target | about a mispredict |

- Memory moves in 64 B cache lines and 4 KiB pages.
- The L2 TLB reaches about 8 MB with 4 KiB pages.
- About 10–15 cache misses can be in flight per core.
- An SSD reads 4 KiB blocks in about 0.1 ms.

## Work and algorithms

- Remove work before speeding it up. Look for:
  - recomputation, and repeated parsing, formatting or hashing of the same value;
  - N+1 lookups, and a `get` followed by an `insert` (use `entry`);
  - full rebuilds after incremental changes;
  - full sorts where `select_nth_unstable` or a partial order suffices.
- At real sizes, constants beat asymptotics. Cache misses and mispredictions dominate, so a linear scan of a small contiguous slice often beats a tree or hash lookup. Judge at the sizes the code actually sees.
- Pick the update/query trade-off by measured operation frequencies: store raw data, keep fully precomputed results, or use a logarithmic structure in between.
- Precompute with `const` items, `const fn` tables, and `LazyLock`/`OnceLock` built once. Hoist loop-invariant work, and move invariant checks into constructors.
- Division by a runtime-invariant divisor in a hot loop should use a precomputed multiply-shift (Barrett/Lemire reduction, the `strength_reduce` crate) or a power-of-two mask. The compiler already does this for constants. Prefer unsigned types for non-negative values, because signed division and shifts need fix-ups.
- Hashing: `HashMap`'s default SipHash is slow for small keys. Use a fast hasher (FxHash, ahash) where keys are not attacker-controlled. Use a dense `Vec` indexed by id when ids are small integers, and bitsets instead of `HashSet<small int>` or `Vec<bool>`.
- Use `sort_unstable*` when stability is not needed, and `binary_search` on sorted `Vec`s over `BTreeMap`s for read-mostly data.
- Replace a hot path through a slow general algorithm with a known faster one:
  - binary GCD with `trailing_zeros` instead of `%`;
  - Montgomery form for repeated modular multiplication;
  - interleaved, blocked two-pass prefix sums;
  - block-min-then-find argmin.

## Memory layout and locality

- Count cache lines and pages touched, not elements. A strided access costs whole lines, so a 1-in-16 stride costs as much as a full pass once the data exceeds cache.
- Use contiguous storage (`Vec<T>`, slices) over `LinkedList`, `Vec<Box<T>>`, pointer-linked trees and `Rc<RefCell<…>>` graphs.
  - Each dependent pointer hop is a full-latency miss that blocks memory-level parallelism.
  - Replace pointers with typed `u32` indices into arenas. They are half the size of 64-bit pointers, so twice as many fit in cache.
- Choose between AoS and SoA:
  - AoS when an access reads most fields of one item: lookups and tree nodes.
  - SoA when a pass scans a few fields across many items: filters, columns, SIMD.
  - Split hot fields from cold ones, and move rarely used data into a side table or behind a `Box`.
- Shrink hot types; check `size_of` for types stored in bulk.
  - Rust already reorders fields to minimize padding. Order fields by hand only in `#[repr(C)]` types.
  - Box large, rare enum variants (clippy `large_enum_variant`), and use niches (`Option<NonZeroU32>`, `Option<Box<T>>`).
  - Use `u32`/`u16`/`f32` where ranges allow. `f32` halves bandwidth and doubles SIMD lanes.
  - Prefer `Box<[T]>` and `Box<str>`/`Arc<str>` to `Vec`/`String` for frozen data (2 words instead of 3, and one less indirection behind `Arc`).
  - Use arrays, `SmallVec` or `ArrayVec` for small bounded collections.
- Bit-packing and 24-bit fields trade CPU for bandwidth. They pay only when the path is memory-bound and are the last resort.
- Align deliberately with `#[repr(align(64))]` for SIMD blocks and per-thread slots. Over-aligning everything wastes cache.
- Avoid power-of-two strides and dimensions in hot 2D or strided access, such as row pitches of 2^k bytes or arrays of exactly 2^20.
  - Cache-set aliasing shrinks the effective cache: a 1024 B stride ran 10× slower than 1028 B.
  - Pad the dimension or index through a scrambling bijection.
- Random access over more than a few MB thrashes the TLB. Put large, long-lived, randomly accessed arrays on huge pages: a 2 MiB-aligned allocation plus `madvise(MADV_HUGEPAGE)` gives about 10–15% lower latency. Huge pages can hurt power-of-two-strided layouts.
- Hardware prefetchers follow only linear forward or backward streams with small strides.
  - Software prefetch (`core::arch::x86_64::_mm_prefetch`) pays only when the next address is computable several steps ahead. The classic case is Eytzinger search prefetching `16k`.
  - Branchy code prefetches through speculation, and branchless code loses that.
- Exploit memory-level parallelism. Turn one long dependent chain into several independent ones: batch lookups, interleave several traversals, or look up many keys per pass.
- Streaming passes:
  - Fuse passes, or block them to cache size, so each block goes through all stages while resident.
  - Divide-and-conquer recursion down to a cache-sized base case beats breadth-first full-array passes.
  - Read-modify-write traffic halves RAM bandwidth compared to read-only.
  - Large write-once fills and copies that are not reused soon can use non-temporal stores (more than 2× faster in RAM).
- Sorted static data queried hot:
  - A branchless binary search is 2.5–3× faster on in-cache arrays.
  - An Eytzinger layout with prefetch is about 2× faster on large arrays.
  - A 16-key-per-cache-line B-tree layout with SIMD rank is up to 8–15×.
  - A dynamic B-tree with 32-key nodes and index children is up to 18× faster than node-per-key trees.
  - Wide segment trees are 4–12× faster than Fenwick trees for dynamic prefix sums.

## Allocation, ownership and copies

- Don't allocate in steady-state loops.
  - Reuse buffers: `clear()` keeps capacity. Pass `&mut Vec<T>` or `&mut String` output buffers, or keep scratch buffers in the owning struct.
  - Size up front with `with_capacity`/`reserve`, and prefer `extend` or `collect` from exact-size iterators to repeated `push`.
- Drop intermediate `collect::<Vec<_>>()` that is only iterated again, and return `impl Iterator` where the caller streams.
- Strings:
  - `format!` and `+` allocate a new string each time. In hot paths, `write!` into a reused `String`, or `push_str` into a pre-sized one.
  - `to_string()` on an `&str` just to compare or look up is waste.
- Look for clones that only satisfy the borrow checker: `String`, `Vec`, large structs and whole configs cloned on hot paths. Fix them by:
  - borrowing;
  - `mem::take`, `mem::replace` or `Option::take` when moving out of `&mut` (for example when switching enum variants);
  - splitting structs so fields borrow independently;
  - moving ownership to where the data is consumed.
- Take the borrowed type as an argument: `&str`, `&[T]`, `&T`, `&Path`, not `&String`, `&Vec<T>` or `&Box<T>`, which add an indirection and force callers to allocate.
  - Take `String`/`Vec<T>` by value only when the callee stores them.
  - Use `Cow<'_, str>` when most calls need no change.
- A fallible function that consumes an argument should return it inside the error, like `FromUtf8Error::into_bytes`, so retries don't clone.
- Closures should capture only what they use: rebind clones of single fields in a block before `move`, not whole structs. Every `Arc::clone` is an atomic read-modify-write.
- Don't `Box<dyn Trait>` per item. Use enums, generics, one boxed strategy per collection, or on-stack `&mut dyn` for temporary dispatch.
- Heavy destructors can dominate latency:
  - `Drop` of deep trees and many small allocations on latency-critical threads;
  - hidden drop glue at scope end.
  - Recycle, arena-allocate (bump, slab), or hand large drops to a background thread.
- Many small same-lifetime allocations belong in an arena. Swapping the global allocator is a measured, crate-wide change, not a fix for one path.
- `vec![0; n]` gets zeroed pages directly and is cheap. Touching fresh pages costs page faults on first use.
- Tree transforms (folds): owned `Box` nodes let unchanged nodes be reused, borrowed input forces clones, and `Rc` shares unchanged subtrees.

## Dispatch, generics and code layout

- `dyn Trait`, `Box<dyn Fn>` and fn pointers are indirect calls: they block inlining and mispredict when targets vary.
  - In per-item loops, use generics or enum dispatch when the set of types is closed.
  - Otherwise dispatch once per batch or slice, not per element.
  - Group items by kind first so each group runs one tight loop.
- Monomorphization copies code per type, costing binary size, instruction cache and compile time.
  - Keep generic shims thin over a non-generic inner function (`fn open<P: AsRef<Path>>(p: P) { open_inner(p.as_ref()) }`).
  - Use dynamic dispatch on cold paths.
- Inlining:
  - Small non-generic functions called across crates need `#[inline]` unless LTO covers them. The release build uses thin LTO and `-Ccodegen-units=1`; tests and dev builds don't.
  - Use `#[inline(always)]` only for proven wins.
  - Mark error, slow and rare paths `#[cold]` or `#[inline(never)]` (or call `std::hint::cold_path()`), and move `panic!`/`format!` machinery out of hot functions so hot loops stay small.
- Unrolling and inlining past the instruction cache hurt. Group hot code with hot code.
- Move runtime state checks into types (typestate, generic parameters, newtypes) so the per-call `Option` or flag checks disappear. Newtypes cost nothing at runtime.
- Dense `match` compiles to jump tables. For interpreter and state-machine loops, keep state in locals and dispatch on compact tags.

## Branches

- An unpredictable data-dependent branch costs 15–20 cycles per mispredict. Predictable branches and rare checks (errors, bounds, asserts) are nearly free.
- Go branchless when predictability is below about 75%:
  - arithmetic selects, `bool as u32`, `min`/`max`/`clamp`, `std::hint::select_unpredictable`;
  - verify that the assembly got a `cmov` or a blend.
  - Keep the branch when it is well predicted or when its speculation prefetches memory.
- Make branches predictable by sorting or partitioning items by the condition or variant before the loop.
- Remove edge-case branches with sentinels and padding:
  - neutral elements that pad to the SIMD width;
  - fixed iteration counts;
  - a shared empty value instead of `Option` checks.
- Recursion costs stack traffic, so turn tail recursion and deep recursion into loops or explicit stacks. Depth-first recursion with a hybrid iterative base case keeps better temporal locality than breadth-first passes.

## Bounds checks, overflow, aliasing and floats

- Bounds checks cost code size and block vectorization. Remove them by:
  - using iterators, `zip`, `chunks_exact`, `windows` and `split_at`;
  - re-slicing every input to the same length before the loop (`let (a, b) = (&a[..n], &b[..n]);`);
  - one up-front `assert!`.
  - Use `get_unchecked` or `std::hint::assert_unchecked` only in a small module that proves the invariant, behind a safe API.
- Index with `usize` ranges. A hand-rolled `u32` counter wraps in release, so the compiler must allow for the wrap, which can defeat trip-count analysis and vectorization.
- `&mut` slices are `noalias`, which Rust gives for free where C needs `restrict`.
  - Raw pointers, `Cell`/`UnsafeCell`, and indexing one slice at two offsets lose that guarantee.
  - Pass disjoint slices (`split_at_mut`) so loops vectorize without runtime overlap checks.
- Rust has no fast-math, so float reductions (`sum`, dot products, min/max with NaN semantics) stay sequential.
  - Write explicit lane accumulators (`[f32; 8]` over `chunks_exact(8)`) and accept the changed rounding.
  - Use Kahan or pairwise summation where accuracy matters.
- Without FMA enabled at compile time, `f32::mul_add` is a slow software `fmaf` call.
- An `f32` counter stops changing at 2^24.
- Mixing magnitudes loses precision. `(x + y) * (x - y)` is both more accurate and cheaper than `x * x - y * y`.

## Instruction-level parallelism and SIMD

- Break loop-carried dependencies: use latency × throughput independent accumulators (for example about 10 FMA chains) to reach full throughput. Use multiple accumulators for integer reductions too.
- Auto-vectorization needs:
  - contiguous slices with lengths known through `chunks_exact` plus a remainder;
  - no bounds checks, calls that aren't inlined, early exits or aliasing;
  - only simple selects;
  - no loop-carried dependencies apart from integer reductions.
  - It typically gives a 5–10× speedup.
- Target features:
  - The release config sets no `-Ctarget-cpu`, so only baseline x86-64 SIMD (SSE2) is used. AVX2 roughly doubles vectorized throughput.
  - Either raise the whole build (`-Ctarget-cpu=x86-64-v3`, which drops older CPUs), or multiversion hot kernels: `#[target_feature(enable = "avx2,fma")]` functions selected once through `is_x86_feature_detected!`.
- Hand-written SIMD (`std::arch`) when the compiler fails:
  - The idiom is compare → `movemask` → `trailing_zeros`/`count_ones` for find, count, rank, filter and argmin.
  - Unroll to 2–4 vectors per iteration, and OR the masks before testing, because `movemask` and `ptest` issue once per cycle and decode width is limited.
  - Use in-register lookup tables through `pshufb` (`_mm256_shuffle_epi8`).
  - Gather is no faster than scalar loads, so lay the data out contiguously instead.
  - Keep data in vector registers across steps. Scalar↔vector moves and horizontal reductions are costly, so reduce once at the end.
  - Watch narrow accumulators for overflow and flush them into wider ones periodically.
  - Use unaligned loads that stay within one cache line: blocks that cross lines cost about 30% on light loops.
- Use the bit operations std exposes: `count_ones`, `trailing_zeros`, `leading_zeros`, `ilog2`, `rotate_left`, `is_power_of_two`, `u128` multiply for the high half. Use `memchr`/`memchr::memmem` for byte search.

## Concurrency and shared resources

- False sharing: per-thread counters, slots or queue heads in one cache line ping-pong between cores. Pad them with `crossbeam_utils::CachePadded` (128 B on x86-64) or `#[repr(align(64))]`.
- L3 and RAM bandwidth are shared: memory-bound loops scale sublinearly across threads, and one core cannot saturate RAM. Parallelize compute-bound work first, and split memory-bound work symmetrically across L3 domains.
- Locks and atomics on hot paths:
  - A guard lives to the end of its scope, so end it before slow work: I/O, allocation, `await`, callbacks.
  - Use a `RwLock` or immutable snapshots (`Arc` swap) for read-mostly data.
  - Shard or thread-localize counters.
  - Avoid `Arc::clone` per item.
- Batch channel messages and tasks instead of one per item. Never block inside async tasks: the project's I/O runtimes have two workers.

## I/O and system boundaries

- Buffer I/O with `BufReader`/`BufWriter`, read whole files once, avoid per-line or per-item syscalls, and prefer sequential to random access.
- Use `mmap` for large read-only random access.
- Data larger than cache or RAM needs block-oriented algorithms: sequential scans, k-way merges, and sort-then-join instead of random pointer jumps.
- Caches must be bounded. LRU (map plus intrusive list) stays within 2× of optimal eviction at half the memory, and a cheaper policy is fine when misses are cheap.
- FFI:
  - Borrow foreign strings as `&CStr` (zero-copy) instead of copying.
  - Reuse `CString`/`Vec<u8>` buffers for repeated calls.
  - Keep `unsafe` in minimal modules with safe, fast wrappers.
- Keep eager `format!`, `to_string` and logging arguments out of hot paths, including tracing fields evaluated even when the level is disabled.

## Build

- Check the root `Cargo.toml`'s `[profile.release]` (thin LTO, `codegen-units = 1`, `panic = "abort"`) and `.cargo/config.toml`'s rustflags for whether a target CPU is set; `[profile.bench]` adds `-Ctarget-cpu=x86-64-v3`. Many small crates optimize worse without LTO.
- PGO (`-Cprofile-generate`, `-Cprofile-use`) typically gains 10–20% on large branchy code. Propose it only together with a representative workload.
- Overflow checks and debug assertions are off in release, so don't count them as costs. Tests and dev builds show different profiles.

## Measurement

- Benchmarks:
  - Feed inputs and consume outputs through `std::hint::black_box`, warm up first, and use representative data distributions (random versus sorted changes branch prediction).
  - Measure latency (chain each call's input on the previous output) separately from throughput.
  - Avoid power-of-two sizes.
  - Time them on this machine with `rime bench` (the release-based `bench` profile), never inside `rime test` runs.
- Reduce noise: fix the CPU frequency (`cpupower frequency-set`), pin threads (`taskset`), and compare cycles, not seconds, for compute-bound code. Under a second, ±5% is noise, so repeat runs until the difference is significant.
- Tools:
  - `perf stat -e cycles,instructions,branch-misses,cache-misses,dTLB-load-misses` for IPC, mispredicts, misses and TLB;
  - `perf record -g` with flame graphs, and `perf annotate` for hot instructions (allow for skid);
  - `valgrind --tool=cachegrind --branch-sim=yes` with `cg_annotate` for per-line cache and branch simulation;
  - `valgrind --tool=dhat` for allocation counts and lifetimes;
  - `llvm-mca` for port pressure in small loops;
  - sampled in-code counters with a geometric skip for production statistics.

## Implement and verify

- Implement the agreed fixes completely, preserving behavior unless a change is approved, such as changed float rounding. Update affected callers and tests, remove obsolete code, and avoid unrelated refactors.
- Settle each fix with its agreed check before and after the change: a benchmark, a profiler counter, or the assembly. Drop a fix whose gain stays within noise unless it also simplifies the code.
- Keep benchmarks and reproductions out of the repo unless agreed.
- Run project-required formatting, lint, and tests, including affected consumers.

Finish with a brief summary of each fix and its measured effect, the verification, the findings left unfixed, and any remaining blockers.
