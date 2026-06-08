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
//!    `write`/`net`/`time`/`rand`/`term` widen (`term` → `ioctl`:16, #106).
//!    Deterministic (sorted, deduped).
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
//! | REQ-7 (the `term` terminal-control atom + the `ioctl` grant, #106) | SHIPPED | `TERM_SYSCALLS = &[16 /* ioctl */]` + the `"term" => TERM_SYSCALLS` arm in `syscall_allowlist`; the §4.1 `Effect::Term` atom (`thermite_syntax::ast::Effect::Term`, parsed as `fx term`) flows through `manifest::effects_of` → `transitive_fx` → the allowlist, so a `term` program's allowlist INCLUDES `ioctl`:16 and a non-`term` one EXCLUDES it (scoped to the effect). The `examples/editor/editor.th` `run` entry now declares `fx term` (its `raw_mode_on`/`raw_mode_off` boundaries) and builds+runs FULLY sandboxed (NO `--no-sandbox`). Consumer: `syscall_allowlist` (via `build::synthesize_entry_main`). Verified by `tests::term_grants_ioctl_scoped_to_the_effect` + `verus_anchor` (the term bit is non-io, `widen(8)==0`, so the proved io_allow bitset is unaffected over all 512 masks) + `editor_runs.rs` (the editor sandboxed, exit 0). The grant is `ioctl`-BROAD (OQ-5). |

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

/// The x86_64 syscall a `term` (terminal-control) effect ADDS (issue #106):
/// `ioctl` (16), the syscall the termios `tcgetattr`/`tcsetattr` boundary issues
/// for raw mode. The grant is `ioctl`-BROAD (any cmd) — classic seccomp-bpf
/// compares only `seccomp_data.nr`, not the `cmd` register, so v0.1 grants the
/// whole `ioctl` under `term` (runtime-sandbox.md REQ-7 / OQ-5). Scoped to the
/// `term` effect: a `pure`/`read`/`write`/`net` program's allowlist EXCLUDES
/// `ioctl`, so its `ioctl` is still `SIGSYS`-killed (a dedicated atom keeps a
/// plain `write` program — `print`/`write_file` — from silently acquiring `ioctl`).
const TERM_SYSCALLS: &[u32] = &[
    16, // ioctl (termios TCGETS/TCSETS — the cmd cannot be filtered, OQ-5)
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
/// `write(_)`/`net(_)`/`time`/`rand`/`term` widen (`term` → `ioctl`:16, #106). A
/// token is matched by its leading verb (`read(src)` → the `read` widening) so the
/// carried ident is irrelevant. Returns
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
            // `term` (#106) widens with `ioctl`:16 for the termios raw-mode boundary
            // (runtime-sandbox.md REQ-7). Scoped to the effect — only a `term`
            // program's allowlist gains `ioctl`.
            "term" => TERM_SYSCALLS,
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

    // REQ-7 (#106): a `term` program's allowlist INCLUDES ioctl:16; a
    // pure/read/write/net program's allowlist EXCLUDES it — the grant is SCOPED to
    // the `term` effect (a dedicated atom, not folded into `write`). Anchored to the
    // design's Table `TERM_SYSCALLS={ioctl:16}` (R-CHAR-3, the design constant).
    #[test]
    fn term_grants_ioctl_scoped_to_the_effect() {
        assert!(
            syscall_allowlist(&set(&["term"])).contains(&16),
            "fx term grants ioctl:16"
        );
        // A program WITHOUT term never gains ioctl — pure, read, write, net all deny it.
        for fx in [
            &set(&["pure"]),
            &set(&["read(src)"]),
            &set(&["write(dst)"]),
            &set(&["net(sock)"]),
        ] {
            assert!(
                !syscall_allowlist(fx).contains(&16),
                "a non-term program must NOT gain ioctl: {fx:?}"
            );
        }
        // The editor's full transitive row (read/write/alloc/diverge/term) gains ioctl.
        assert!(
            syscall_allowlist(&set(&[
                "read(input)",
                "write(output)",
                "alloc",
                "diverge",
                "term"
            ]))
            .contains(&16),
            "the editor's transitive fx (incl. term) grants ioctl"
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

// ===========================================================================
// The verus anchor (epic #60, `.design/verified/self-verification.md` REQ-8).
//
// PLACEMENT DEVIATION (Option B, orchestrator-authorized): the design doc names
// `forge/tests/sandbox_verified.rs` for this anchor, but `forge` is a binary-only
// crate (no lib target), so an external test cannot reach the internal
// `syscall_allowlist`/`BASELINE_SYSCALLS` symbols. This in-module `#[cfg(test)]`
// block reaches them directly; `thermite-verified` is a forge DEV-dependency.
// (Reported for the critic.)
//
// AC-8c — the 512-mask EXHAUSTIVE equivalence: enumerate ALL 2^9 fx-atom masks
// (WIDENED for the #106 `Term` atom, bit 8), project each to the PRODUCTION token
// set, run the PRODUCTION `syscall_allowlist`, and assert its membership over the
// FIVE sensitive user-I/O syscalls
// (openat/socket/connect/getrandom/clock_gettime) equals the VERUS-PROVED
// `thermite_verified::io_allow(mask)` bits for every mask. Expected = the proved
// bitset spec (R-CHAR-3, never forge's own output) — so the production string-keyed
// mapping computes exactly the relation Verus proved (pure-no-I/O + monotonicity +
// deny-by-default).
//
// OQ-6 (scope): verus proves SOUNDNESS over the FIVE sensitive syscalls ONLY. This
// anchor binds `syscall_allowlist`'s membership over exactly those five to the proved
// `io_allow` bits; it does NOT claim the dense `BASELINE_SYSCALLS` list is itself
// correct — that stays empirically grounded by the `sandbox_conformance` oracle. The
// soundness story is the IO-membership projection; the baseline is orthogonal to the
// modeled IO bits.
// ===========================================================================
#[cfg(test)]
mod verus_anchor {
    use super::*;
    use thermite_verified::{
        io_allow, SYS_CLOCK_GETTIME, SYS_CONNECT, SYS_GETRANDOM, SYS_OPENAT as IO_OPENAT,
        SYS_SOCKET,
    };

    /// Project a `u16` fx-atom mask to the PRODUCTION token set (the same strings the
    /// `BuildManifest.functions` rows carry). The bit positions MATCH the verus
    /// model's `u16` fx-mask: Read=0, Write=1, Net=2, Time=3, Rand=4, Alloc=5,
    /// Panic=6, Diverge=7, Term=8 (the #106 terminal-control atom). The carried
    /// ident on `read(_)`/`write(_)`/`net(_)` is irrelevant to the mapping (matched
    /// by the leading verb). WIDENED `u8`→`u16` for the 9th atom (#106).
    fn mask_to_tokens(mask: u16) -> BTreeSet<String> {
        let mut toks: BTreeSet<String> = BTreeSet::new();
        if mask & (1 << 0) != 0 {
            toks.insert("read(src)".to_string());
        }
        if mask & (1 << 1) != 0 {
            toks.insert("write(dst)".to_string());
        }
        if mask & (1 << 2) != 0 {
            toks.insert("net(sock)".to_string());
        }
        if mask & (1 << 3) != 0 {
            toks.insert("time".to_string());
        }
        if mask & (1 << 4) != 0 {
            toks.insert("rand".to_string());
        }
        if mask & (1 << 5) != 0 {
            toks.insert("alloc".to_string());
        }
        if mask & (1 << 6) != 0 {
            toks.insert("panic".to_string());
        }
        if mask & (1 << 7) != 0 {
            toks.insert("diverge".to_string());
        }
        if mask & (1 << 8) != 0 {
            toks.insert("term".to_string());
        }
        // An empty mask is `pure` (no widening atom).
        if toks.is_empty() {
            toks.insert("pure".to_string());
        }
        toks
    }

    /// The production x86_64 syscall number for each of the five sensitive syscalls,
    /// paired with its `thermite_verified::io_allow` bit (the proved bitset spec).
    /// openat=257/bit0, socket=41/bit1, connect=42/bit2, getrandom=318/bit3,
    /// clock_gettime=228/bit4. These are the syscall numbers in the design's `fx`→
    /// syscall Table (R-CHAR-3 — the design constant, not forge output).
    const SENSITIVE: &[(u32, u32)] = &[
        (257, IO_OPENAT),         // openat
        (41, SYS_SOCKET),         // socket
        (42, SYS_CONNECT),        // connect
        (318, SYS_GETRANDOM),     // getrandom
        (228, SYS_CLOCK_GETTIME), // clock_gettime
    ];

    // AC-8c (REQ-8): over ALL 256 fx-atom masks, the PRODUCTION `syscall_allowlist`'s
    // membership of the five sensitive syscalls equals the VERUS-PROVED `io_allow`
    // bits. This is the exhaustive impl==spec equivalence (mechanism (c)) binding the
    // string-keyed production mapping to the proved bitset over its FULL finite domain.
    #[test]
    fn syscall_allowlist_matches_proved_io_allow_over_all_512_masks() {
        for mask in 0u16..=511 {
            let tokens = mask_to_tokens(mask);
            let allow = syscall_allowlist(&tokens);
            let proved = io_allow(mask);
            for &(nr, bit) in SENSITIVE {
                let in_production = allow.contains(&nr);
                let in_proved = (proved & bit) != 0;
                assert_eq!(
                    in_production, in_proved,
                    "mask {mask:#010b} ({tokens:?}): syscall {nr} membership \
                     (production={in_production}) must equal the verus-proved io_allow \
                     bit {bit:#x} (proved={in_proved})"
                );
            }
        }
    }

    // AC-8c / REQ-8 PURE-NO-I/O: mask 0 (`pure`) permits NONE of the five sensitive
    // syscalls in the production allowlist — exactly the proved `io_allow(0) == 0`.
    #[test]
    fn pure_mask_permits_no_sensitive_syscall() {
        let allow = syscall_allowlist(&mask_to_tokens(0));
        assert_eq!(io_allow(0), 0, "the proved spec: pure has no I/O");
        for &(nr, _) in SENSITIVE {
            assert!(
                !allow.contains(&nr),
                "pure denies sensitive syscall {nr}: {allow:?}"
            );
        }
    }

    // AC-8c / REQ-8 MONOTONICITY (observable): adding any fx atom NEVER removes a
    // permitted sensitive syscall — a superset mask's sensitive membership is a
    // superset. Binds the proved `monotone` lemma to the production fn over a sample
    // of mask/superset pairs (the full bitset monotonicity is proved in verus).
    #[test]
    fn superset_mask_never_drops_a_sensitive_syscall() {
        for mask in 0u16..=511 {
            let base = syscall_allowlist(&mask_to_tokens(mask));
            // The full superset (all atoms) must contain every sensitive syscall the
            // sub-mask permitted (deny-by-default monotonicity, the proved lemma).
            let full = syscall_allowlist(&mask_to_tokens(0x1FF));
            for &(nr, _) in SENSITIVE {
                if base.contains(&nr) {
                    assert!(
                        full.contains(&nr),
                        "monotonicity: the full-fx allowlist must keep syscall {nr} \
                         that mask {mask:#010b} permitted"
                    );
                }
            }
        }
    }
}
