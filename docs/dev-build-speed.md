# Dev build speed

Local iteration cycle matters more than CI throughput, so Jarvy tunes
`[profile.dev]` for fast rebuilds. Release builds keep full optimization.

## Currently enabled (`[profile.dev]`)

### `debug = "line-tables-only"`

Drops variable and type info from dev builds. Keeps file:line mappings for
panic backtraces and `addr2line` symbolication. Trade-off: a debugger
attached to a dev binary shows frames and locations but not local variables.
`cargo test` backtraces still print `file:line:col`. Release binaries ship
full symbols via the release profile (unchanged).

Observed on this crate: roughly +9% on a clean `cargo build`, +21% on a
rebuild. No nightly, no flags. Works on stable since Rust 1.65.

## Future candidates — nightly only

These two optimizations are real but unstable. Don't enable them in
`Cargo.toml` or `.cargo/config.toml` yet: doing so forces the build onto
nightly, which would break CI on stable. Documented here so we revisit
when they land.

### Parallel frontend (borrow + type checking on N cores)

Stage one of `cargo build` (type checking, borrow checking) runs single-core
by default. On a 4+ core machine most of the box sits idle. A nightly-only
flag spreads that work across threads.

```bash
# Per-invocation — no repo change needed.
cargo +nightly build -Zthreads=8
```

8 threads is a reasonable balance between wall time and peak memory. CI
runs mostly benefit because they spend most of their time on clean builds.

### Cranelift backend instead of LLVM

LLVM optimizes aggressively; Cranelift trades some optimization for
codegen speed. Default for dev, LLVM for release. Nightly-only.

```bash
# Step 1: install the codegen component once.
rustup component add rustc-codegen-cranelift --toolchain nightly

# Step 2: per-invocation build with the swap.
cargo +nightly build -Zcodegen-backend=cranelift
```

Caveat: Cranelift does not yet compile every construct LLVM does, especially
low-level SIMD intrinsics and inline assembly. If a build errors out, fall
back to LLVM for that crate: `cargo +nightly build` without the flag.
Jarvy itself doesn't use SIMD/intrinsics, so this is a low risk for us.

## When to revisit

- Parallel frontend: track `rustc -Zthreads` stabilization. The unstable
  book page documents the current status.
- Cranelift: track the `codegen-backend` unstable feature; once it lands
  on stable (or via a profile-keyed default) we can wire it into
  `[profile.dev]` behind a feature flag and gate CI to opt in.

Until then, the line-tables-only change alone covers the cheap win.
