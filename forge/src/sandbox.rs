//! `forge/src/sandbox.rs` — the runtime effect sandbox (issue #57): a seccomp-bpf
//! syscall-allowlist filter, DERIVED from a `forge build --entry`'s transitive
//! `fx` row, installed (BEFORE the entry runs) into the generated `main`. A syscall
//! outside the declared effects makes the kernel kill the process with `SIGSYS`.
//! This discharges the `thermite-design.md` §4.1 promise that the `fx` row is a
//! RUNTIME contract, not only a compile-time one.
//!
//! Governing design: `.design/forge/runtime-sandbox.md`. Oracle:
//! `conformance/sandbox/cases.json`.
//!
//! ## The three seams this module COMPOSES (it owns no new walker / effect vocab)
//!
//! 1. **transitive `fx`** ([`transitive_fx`]): the union of `manifest::effects_of`
//!    over `{entry} ∪ closure::reachable_in_file_fns(program, entry)` — the SAME
//!    #17 cycle-safe, source-order reachability `check::item_subprogram` consumes,
//!    restricted to the entry's intra-file closure. A `#[boundary]`/`#[slag]` fn in
//!    the closure contributes its DECLARED `fx` (it is confined to exactly that).
//! 2. **`fx` → syscall allowlist** ([`syscall_allowlist`]): each `fx` token maps to
//!    a fixed set of x86_64 syscall numbers ([the table](#the-fx--syscall-table));
//!    `pure` is the minimal baseline (run + print + the panic/abort path), `read`/
//!    `write`/`net`/`time`/`rand` widen. Deterministic (sorted, deduped).
//! 3. **the BPF prelude** ([`emit_sandbox_prelude`]): the Rust SOURCE that, as the
//!    first statements of the generated `main`, builds a classic `sock_filter[]`
//!    program (arch-guard for x86_64 → load `nr` → a `BPF_JEQ` per allowed syscall →
//!    `SECCOMP_RET_ALLOW`, default `SECCOMP_RET_KILL_PROCESS`) and installs it via
//!    `prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`.
//!
//! ## The fx → syscall table
//!
//! The baseline (always, incl. `pure`/`alloc`) is the set a trivial `std` Rust
//! binary needs to start up, `println!`, run the L1 `thermite_check!` PANIC/abort
//! path, and exit — empirically grounded (`.design/forge/runtime-sandbox.md`
//! Verification). It pointedly EXCLUDES `openat`/`socket`/`getrandom`/`clock_gettime`
//! so a `pure` filter denies file I/O, network, rand, and time. `read`/`write`/`net`/
//! `time`/`rand` ADD their syscalls; `alloc`/`panic`/`diverge` add nothing beyond the
//! baseline (`panic` unwinds + writes to stderr via the baseline `write`+`exit_group`).
//!
//! ## No `libc` crate dependency (self-contained)
//!
//! The generated binary is a `std` program → it already links libc, so the prelude
//! declares `extern "C" { fn prctl(...); fn syscall(...); }` resolved against THAT
//! libc — no `libc` crate is added to `forge/Cargo.toml`. The `unsafe` lives in the
//! EMITTED source (the generated binary), never in `forge/src/`.
//!
//! ## Determinism (R-CODE-5)
//!
//! Same transitive `fx` → same sorted-deduped allowlist → byte-identical prelude.
//! [`syscall_allowlist`] sorts + dedups; [`emit_sandbox_prelude`] iterates the
//! sorted vector. No wall-clock / unordered iteration.
//!
//! ## REQ status
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (seccomp prelude install via raw libc `prctl`) | SHIPPED | `pub fn emit_sandbox_prelude` emits a `sock_filter[]` program (arch-guard + per-syscall `BPF_JEQ` → `SECCOMP_RET_ALLOW`, default `SECCOMP_RET_KILL_PROCESS`) installed via `prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`, raw `extern "C"` (no libc crate). Consumer: `build::synthesize_entry_main`. Verified by `sandbox_conformance::pure_runs_clean` / `probe_killed`. |
//! | REQ-2 (transitive-`fx` derivation via `closure.rs`) | SHIPPED | `pub fn transitive_fx` unions `manifest::effects_of` over `{entry} ∪ closure::reachable_in_file_fns(program, entry)`. Consumer: `build::synthesize_entry_main` + `build::build_file` (the `BuildManifest::sandbox` record). Verified by `sandbox_conformance::probe_allowed_when_fx_widens` (the `read` fx widens the allowlist). |
//! | REQ-3 (`fx` → syscall allowlist mapping) | SHIPPED | `pub fn syscall_allowlist` maps each token to the pinned x86_64 set ([the table](#the-fx--syscall-table)); `pure` baseline excludes `openat`, `read(_)` adds it. Consumer: `emit_sandbox_prelude` (via `build::synthesize_entry_main`). Verified by `sandbox_conformance::probe_killed` (no openat) vs `probe_allowed_when_fx_widens` (openat) + unit `pure_baseline_excludes_io_syscalls`. |
//! | REQ-4 (sandbox-on-by-default for `--entry`, `--no-sandbox` opt-out) | SHIPPED | `build::synthesize_entry_main` injects the prelude FIRST when `SandboxMode::On` (`build::SandboxConfig::default` = on); `--no-sandbox` → `SandboxMode::Off` (no prelude); a library build emits no `main` at all. Consumer: `cli::run_build` (the `--sandbox`/`--no-sandbox` flags). Verified by `sandbox_conformance::no_sandbox_omits_prelude` + `cli::tests::parses_build_sandbox_flags`. |
//! | REQ-5 (reproducible prelude + manifest record) | SHIPPED | `emit_sandbox_prelude` is byte-deterministic (sorted allowlist); the `build::BuildManifest::sandbox` (`SandboxRecord`) field records the installed allowlist. Verified by unit `prelude_installs_and_is_deterministic` + `sandbox_conformance::pure_runs_clean` (the recorded allowlist excludes openat). |
//! | REQ-6 (demonstrable enforcement — probe + clean pure run) | SHIPPED | `pub fn emit_probe` injects (under `--sandbox-self-test`, AFTER the filter) a raw `syscall(SYS_openat, ...)`. Consumer: `build::synthesize_entry_main`. Verified by `sandbox_conformance::probe_killed` (exit 159) vs `probe_allowed_when_fx_widens` (exit 0). |

use std::collections::BTreeSet;

use thermite_syntax::Program;

use crate::closure::reachable_in_file_fns;
use crate::manifest::effects_of;

/// The x86_64 syscall numbers a trivial `std` Rust binary needs to start up,
/// `println!`, run the always-active L1 `thermite_check!` PANIC/abort path, and
/// exit (the `pure`/`alloc` baseline, `.design/forge/runtime-sandbox.md` Table).
/// EXCLUDES `openat`/`socket`/`getrandom`/`clock_gettime` so a pure filter denies
/// file I/O, network, rand, and time. Sorted ascending (deterministic).
const BASELINE_SYSCALLS: &[u32] = &[
    0,   // read
    1,   // write
    3,   // close
    7,   // poll
    9,   // mmap
    10,  // mprotect
    11,  // munmap
    12,  // brk
    13,  // rt_sigaction
    14,  // rt_sigprocmask
    15,  // rt_sigreturn  (the panic/abort unwind path — a violation PANICS, not killed)
    28,  // madvise
    60,  // exit
    131, // sigaltstack
    158, // arch_prctl
    186, // gettid
    202, // futex
    204, // sched_getaffinity
    218, // set_tid_address
    231, // exit_group   (the panic/abort exit path)
    273, // set_robust_list
    302, // prlimit64
    334, // rseq
];

/// The x86_64 syscalls a `read(_)` effect ADDS (file-open + stat + seek; `read`/
/// `close` are already in the baseline). `.design/forge/runtime-sandbox.md` Table.
const READ_SYSCALLS: &[u32] = &[
    8,   // lseek
    257, // openat
    262, // newfstatat
    332, // statx
];

/// The x86_64 syscalls a `write(_)` effect ADDS (`write` already baseline). Table.
const WRITE_SYSCALLS: &[u32] = &[
    74,  // fsync
    257, // openat
    262, // newfstatat
];

/// The x86_64 syscalls a `net(_)` effect ADDS (socket lifecycle). Table.
const NET_SYSCALLS: &[u32] = &[
    41, // socket
    42, // connect
    44, // sendto
    45, // recvfrom
    54, // setsockopt
    55, // getsockopt
];

/// The x86_64 syscalls a `time` effect ADDS. Table.
const TIME_SYSCALLS: &[u32] = &[
    228, // clock_gettime
    230, // clock_nanosleep
];

/// The x86_64 syscall a `rand` effect ADDS. Table.
const RAND_SYSCALLS: &[u32] = &[
    318, // getrandom
];

/// The x86_64 `openat` syscall number — the [`emit_probe`] self-test attempts it
/// (`--sandbox-self-test`); denied under a `pure` filter (kill), allowed under
/// `read`. Mirrors the `READ_SYSCALLS` `openat`:257 entry.
const SYS_OPENAT: u32 = 257;

/// Whether a `forge build --entry` produces a sandboxed runner (REQ-4). ON BY
/// DEFAULT for `--entry` (the §4.1 default is enforcement, not opt-in); `--no-sandbox`
/// opts out (a debugging / no-seccomp-platform escape hatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// Inject the seccomp prelude as the first statements of `main` (the default).
    On,
    /// No prelude (the `--no-sandbox` escape hatch).
    Off,
}

/// The transitive `fx` token set for `entry` in `program` (REQ-2): the UNION of
/// `effects_of(&f.contract.fx)` over `{entry} ∪
/// closure::reachable_in_file_fns(program, entry)`. Reuses the SAME #17 cycle-safe,
/// source-order reachability walker `check::item_subprogram` consumes — never a
/// duplicate. A `#[boundary]`/`#[slag]` fn reached in the closure contributes its
/// DECLARED `fx` (confined to exactly that). Returns a sorted `BTreeSet` of the same
/// `["pure"]` / `read(x)` tokens the `BuildManifest.functions` rows carry
/// (deterministic, R-CODE-5).
pub fn transitive_fx(program: &Program, entry: &str) -> BTreeSet<String> {
    // The closure: every in-file `fn` the entry transitively reaches, plus the
    // entry itself (reachable_in_file_fns EXCLUDES `start`).
    let mut names = reachable_in_file_fns(program, entry);
    names.insert(entry.to_string());

    let mut tokens: BTreeSet<String> = BTreeSet::new();
    for item in &program.items {
        if let thermite_syntax::Item::Fn(f) = item {
            if names.contains(&f.name) {
                for tok in effects_of(&f.contract.fx) {
                    tokens.insert(tok);
                }
            }
        }
    }
    tokens
}

/// Map a transitive `fx` token set to the x86_64 syscall allowlist (REQ-3): the
/// baseline UNION every widening token's added syscalls ([the table](#the-fx--syscall-table)).
/// `pure`/`alloc`/`panic`/`diverge` add nothing beyond the baseline; `read(_)`/
/// `write(_)`/`net(_)`/`time`/`rand` widen. A token is matched by its leading verb
/// (`read(src)` → the `read` widening) so the carried ident is irrelevant. Returns
/// the syscall numbers SORTED + DEDUPED — the same transitive `fx` yields the
/// byte-identical allowlist (deterministic, R-CODE-5).
pub fn syscall_allowlist(transitive_fx: &BTreeSet<String>) -> Vec<u32> {
    let mut set: BTreeSet<u32> = BASELINE_SYSCALLS.iter().copied().collect();
    for tok in transitive_fx {
        // The leading verb (before any `(`) selects the widening set; `pure`/
        // `alloc`/`panic`/`diverge` are baseline-only (no widening).
        let verb = tok.split('(').next().unwrap_or(tok);
        let widen: &[u32] = match verb {
            "read" => READ_SYSCALLS,
            "write" => WRITE_SYSCALLS,
            "net" => NET_SYSCALLS,
            "time" => TIME_SYSCALLS,
            "rand" => RAND_SYSCALLS,
            // "pure" / "alloc" / "panic" / "diverge" / any unknown → baseline-only.
            _ => &[],
        };
        for &nr in widen {
            set.insert(nr);
        }
    }
    set.into_iter().collect()
}

/// Emit the Rust SOURCE of the seccomp-bpf filter-install prelude for `allowlist`
/// (REQ-1/REQ-3/REQ-5): a self-contained block that builds a classic `sock_filter[]`
/// program (x86_64 arch-guard → load `nr` → a `BPF_JEQ` per allowed syscall →
/// `SECCOMP_RET_ALLOW`, default `SECCOMP_RET_KILL_PROCESS`) and installs it via
/// `prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`. The
/// `prctl` is declared `extern "C"` (resolved against the std binary's already-linked
/// libc — NO libc crate dependency). Injected as the FIRST statements of the
/// generated `main` so the entry runs UNDER the filter.
///
/// Byte-deterministic: `allowlist` is iterated in order (the caller passes the
/// sorted [`syscall_allowlist`] output), so the same transitive `fx` yields the
/// byte-identical prelude (REQ-5, R-CODE-5).
pub fn emit_sandbox_prelude(allowlist: &[u32]) -> String {
    // The classic-BPF program. Each accepted-syscall comparison is a single
    // `BPF_JMP|BPF_JEQ|BPF_K` instruction: if `nr == <num>` jump to the ALLOW
    // return (jt), else fall through (jf=0) to the next comparison. After the last
    // comparison the program falls through to the KILL return. The header loads the
    // arch then the syscall number; a non-x86_64 arch is killed (REQ-1, OQ-3).
    //
    // `seccomp_data` layout (offsets): nr @ 0, arch @ 4.
    let mut filter_lines = String::new();

    // Header: load arch, kill if not x86_64; load nr.
    filter_lines.push_str(
        "        // load seccomp_data.arch (offset 4); kill if not x86_64\n\
         \x20       BpfStmt(BPF_LD | BPF_W | BPF_ABS, 4),\n\
         \x20       BpfJump(BPF_JMP | BPF_JEQ | BPF_K, AUDIT_ARCH_X86_64, 1, 0),\n\
         \x20       BpfStmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),\n\
         \x20       // load seccomp_data.nr (offset 0)\n\
         \x20       BpfStmt(BPF_LD | BPF_W | BPF_ABS, 0),\n",
    );

    // One JEQ per allowed syscall (jt=1 → jump over the fall-through to ALLOW).
    for &nr in allowlist {
        filter_lines.push_str(&format!(
            "        BpfJump(BPF_JMP | BPF_JEQ | BPF_K, {nr}, 0, 1),\n\
             \x20       BpfStmt(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),\n"
        ));
    }

    // Default action: kill the whole process (a syscall off the allowlist → SIGSYS).
    filter_lines.push_str("        BpfStmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),\n");

    format!(
        r##"
// ---- thermite #57 runtime effect sandbox (seccomp-bpf, fx-derived) ----------
// Installed as the FIRST statements of `main`, BEFORE the entry call, so the entry
// (and any boundary/slag body it reaches) runs UNDER the filter. A syscall off the
// fx-derived allowlist -> SECCOMP_RET_KILL_PROCESS -> SIGSYS -> process killed.
// Raw `extern "C"` prctl resolved against the std binary's linked libc (no libc
// crate). Deterministic: the allowlist below is the sorted fx->syscall projection.
{{
    // classic-BPF opcodes / seccomp constants (x86_64 Linux).
    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    const AUDIT_ARCH_X86_64: u32 = 0xC000_003E;
    const SECCOMP_RET_ALLOW: u32 = 0x7fff_0000;
    const SECCOMP_RET_KILL_PROCESS: u32 = 0x8000_0000;
    const PR_SET_NO_NEW_PRIVS: i32 = 38;
    const PR_SET_SECCOMP: i32 = 22;
    const SECCOMP_MODE_FILTER: u64 = 2;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SockFilter {{ code: u16, jt: u8, jf: u8, k: u32 }}
    #[repr(C)]
    struct SockFprog {{ len: u16, filter: *const SockFilter }}

    #[allow(non_snake_case)]
    const fn BpfStmt(code: u16, k: u32) -> SockFilter {{ SockFilter {{ code, jt: 0, jf: 0, k }} }}
    #[allow(non_snake_case)]
    const fn BpfJump(code: u16, k: u32, jt: u8, jf: u8) -> SockFilter {{ SockFilter {{ code, jt, jf, k }} }}

    extern "C" {{
        fn prctl(option: i32, arg2: u64, arg3: u64, arg4: u64, arg5: u64) -> i32;
    }}

    static FILTER: &[SockFilter] = &[
{filter_lines}    ];

    // SAFETY: `prctl` is the documented Linux seccomp-install primitive; FILTER is a
    // valid, static, correctly-sized classic-BPF program and FILTER.len() fits u16
    // (the allowlist is small). PR_SET_NO_NEW_PRIVS must precede PR_SET_SECCOMP for
    // an unprivileged install. A non-zero return aborts (the sandbox must not be
    // silently skipped).
    unsafe {{
        if prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {{
            eprintln!("thermite #57 sandbox: PR_SET_NO_NEW_PRIVS failed");
            std::process::abort();
        }}
        let prog = SockFprog {{ len: FILTER.len() as u16, filter: FILTER.as_ptr() }};
        if prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER, (&prog as *const SockFprog) as u64, 0, 0) != 0 {{
            eprintln!("thermite #57 sandbox: PR_SET_SECCOMP failed");
            std::process::abort();
        }}
    }}
}}
"##
    )
}

/// Emit the Rust SOURCE of the `--sandbox-self-test` probe (REQ-6): a raw
/// `syscall(SYS_openat, ...)` injected AFTER the filter install and BEFORE the entry
/// call, so the kill/allow is observable. Under a `pure` filter `openat` is
/// non-allowlisted → `SIGSYS` (the process dies, exit 159); under a `read(_)` filter
/// `openat` is allowlisted → the probe returns and the entry runs normally (exit 0).
/// This is the v0.1 demonstrability device (pure Thermite never attempts a denied
/// syscall itself); a production runner has NO probe.
pub fn emit_probe() -> String {
    format!(
        r##"
// ---- thermite #57 sandbox self-test probe (--sandbox-self-test ONLY) --------
// A raw openat AFTER the filter install: under a pure filter it is non-allowlisted
// -> SIGSYS -> the process is killed BEFORE the entry call (exit 159); under a
// read(_) filter openat is allowlisted -> the probe returns and the entry runs.
{{
    const SYS_OPENAT: i64 = {SYS_OPENAT};
    const AT_FDCWD: i64 = -100;
    extern "C" {{
        fn syscall(num: i64, ...) -> i64;
    }}
    // SAFETY: a single direct openat syscall on a benign path; the seccomp filter is
    // already installed, so a pure filter kills the process here (the demonstration).
    unsafe {{
        let _ = syscall(SYS_OPENAT, AT_FDCWD, b"/dev/null\0".as_ptr(), 0i64);
    }}
}}
"##
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(src: &str) -> Program {
        let parsed = thermite_syntax::parse(src);
        assert!(parsed.is_clean(), "fixture must parse: {:?}", parsed.errors);
        parsed.program
    }

    fn set(tokens: &[&str]) -> BTreeSet<String> {
        tokens.iter().map(|s| s.to_string()).collect()
    }

    // REQ-3: the pure baseline EXCLUDES openat (257) / socket (41) / getrandom
    // (318) / clock_gettime (228) — so a pure filter denies file I/O, net, rand,
    // time. Anchored to the design's Table (the grounded baseline), not toolchain
    // self-output (R-CHAR-3).
    #[test]
    fn pure_baseline_excludes_io_syscalls() {
        let allow = syscall_allowlist(&set(&["pure"]));
        assert!(!allow.contains(&257), "pure denies openat: {allow:?}");
        assert!(!allow.contains(&41), "pure denies socket: {allow:?}");
        assert!(!allow.contains(&318), "pure denies getrandom: {allow:?}");
        assert!(
            !allow.contains(&228),
            "pure denies clock_gettime: {allow:?}"
        );
        // but allows the baseline run + print + panic/abort path.
        assert!(allow.contains(&1), "write (print/panic) allowed");
        assert!(allow.contains(&231), "exit_group (panic/exit) allowed");
        assert!(allow.contains(&15), "rt_sigreturn (panic unwind) allowed");
    }

    // REQ-3: read(_) WIDENS the allowlist to include openat (257) — the fx-derived
    // split the PURE/READ oracle cases assert.
    #[test]
    fn read_fx_widens_to_openat() {
        let allow = syscall_allowlist(&set(&["read(src)"]));
        assert!(
            allow.contains(&257),
            "read(_) allowlists openat (257): {allow:?}"
        );
        // the carried ident is irrelevant — read(anything) widens.
        assert!(syscall_allowlist(&set(&["read(foo)"])).contains(&257));
    }

    // REQ-3: net/time/rand each widen to their pinned syscalls (the whole token
    // family, R-DEFER-8), and alloc/panic/diverge stay baseline-only.
    #[test]
    fn widening_tokens_cover_the_family() {
        assert!(syscall_allowlist(&set(&["net(s)"])).contains(&41)); // socket
        assert!(syscall_allowlist(&set(&["time"])).contains(&228)); // clock_gettime
        assert!(syscall_allowlist(&set(&["rand"])).contains(&318)); // getrandom
        assert!(syscall_allowlist(&set(&["write(o)"])).contains(&257)); // openat
                                                                        // alloc/panic/diverge add nothing beyond the baseline.
        assert_eq!(
            syscall_allowlist(&set(&["alloc", "panic", "diverge"])),
            syscall_allowlist(&set(&["pure"]))
        );
    }

    // REQ-3 / R-CODE-5: the allowlist is sorted + deduped (read's openat:257 is not
    // duplicated by write's openat:257) — deterministic.
    #[test]
    fn allowlist_is_sorted_and_deduped() {
        let allow = syscall_allowlist(&set(&["read(a)", "write(b)"]));
        let mut sorted = allow.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(allow, sorted, "allowlist is sorted + deduped");
    }

    // REQ-2: the transitive fx unions the entry's row with its closure's. A sum-shape
    // pure entry calling only a spec fn is pure. Anchored to the corpus `sum` shape.
    #[test]
    fn transitive_fx_of_pure_entry_is_pure() {
        let prog = parse(
            "spec fn spec_id(x: u32) -> u32 dec 0 { x }\n\
             fn sum(xs: &[u32]) -> u64 req xs.len() <= 10 ens result == 0 fx pure { 0 }",
        );
        let fx = transitive_fx(&prog, "sum");
        assert_eq!(fx, set(&["pure"]), "a pure entry's transitive fx is pure");
    }

    // REQ-2: a `read(src)` entry's transitive fx carries read — so the allowlist
    // widens. Anchored to the oracle's `rf` fixture shape.
    #[test]
    fn transitive_fx_carries_read() {
        let prog = parse("fn rf(x: u32) -> u32 req x < 100 ens result == x fx read(src) { x }");
        let fx = transitive_fx(&prog, "rf");
        assert!(fx.contains("read(src)"), "rf declares read(src): {fx:?}");
        assert!(syscall_allowlist(&fx).contains(&257), "→ openat widened");
    }

    // REQ-2: a caller's transitive fx UNIONS a callee's declared row (the §4.1
    // subsumption: the entry's effective row is the union over its closure).
    #[test]
    fn transitive_fx_unions_callee_row() {
        let prog = parse(
            "fn helper(x: u32) -> u32 req x < 100 ens result == x fx read(src) { x }\n\
             fn caller(x: u32) -> u32 req x < 100 ens result == x fx read(src) { helper(x) }",
        );
        let fx = transitive_fx(&prog, "caller");
        assert!(
            fx.contains("read(src)"),
            "caller's transitive fx includes the closure's read: {fx:?}"
        );
    }

    // REQ-1/REQ-5: the prelude installs the filter (PR_SET_SECCOMP) and is
    // byte-deterministic over the same allowlist (R-CODE-5).
    #[test]
    fn prelude_installs_and_is_deterministic() {
        let allow = syscall_allowlist(&set(&["pure"]));
        let a = emit_sandbox_prelude(&allow);
        let b = emit_sandbox_prelude(&allow);
        assert_eq!(a, b, "REQ-5: same allowlist → byte-identical prelude");
        assert!(
            a.contains("PR_SET_SECCOMP") && a.contains("PR_SET_NO_NEW_PRIVS"),
            "REQ-1: the prelude installs the seccomp filter"
        );
        assert!(
            a.contains("SECCOMP_RET_KILL_PROCESS"),
            "REQ-1: the default action is kill-process"
        );
        // the pure prelude must NOT JEQ openat (257); a read prelude must.
        assert!(
            !a.contains("BPF_JEQ | BPF_K, 257"),
            "pure prelude has no openat comparison"
        );
        let read = emit_sandbox_prelude(&syscall_allowlist(&set(&["read(s)"])));
        assert!(
            read.contains("BPF_JEQ | BPF_K, 257"),
            "read prelude has an openat comparison"
        );
    }

    // REQ-6: the probe is a raw openat syscall (the demonstrability device).
    #[test]
    fn probe_is_a_raw_openat() {
        let p = emit_probe();
        assert!(p.contains("SYS_OPENAT") && p.contains("syscall"));
        assert!(p.contains(&SYS_OPENAT.to_string()));
    }
}
