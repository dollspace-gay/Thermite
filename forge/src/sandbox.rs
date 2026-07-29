//! `forge/src/sandbox.rs` — the runtime effect sandbox (issue #57): a seccomp-bpf
//! syscall-allowlist filter, derived from a `forge build --entry`'s transitive
//! `fx` row, installed (before the entry runs) into the generated `main`. A syscall
//! outside the declared effects makes the kernel kill the process with `SIGSYS`.
//! This discharges the `thermite-design.md` §4.1 promise that the `fx` row is a
//! runtime contract, not only a compile-time one.
//!
//! Governing design: `.design/forge/runtime-sandbox.md`. Oracle:
//! `conformance/sandbox/cases.json`.
//!
//! ## The three seams this module composes (it owns no new walker / effect vocab)
//!
//! 1. transitive `fx` ([`transitive_fx`]): the union of `manifest::effects_of`
//!    over `{entry} ∪ closure::reachable_in_file_fns(program, entry)` — the same
//!    #17 cycle-safe, source-order reachability `check::item_subprogram` consumes,
//!    restricted to the entry's intra-file closure. A `#[boundary]`/`#[slag]` fn in
//!    the closure contributes its declared `fx`, and is confined to that.
//! 2. `fx` → syscall allowlist ([`syscall_allowlist_for_arch`]): each `fx` token
//!    maps to a fixed set of target-architecture syscall numbers ([the table](#the-fx--syscall-table));
//!    `pure` is the minimal baseline (run + print + the panic/abort path), `read`/
//!    `write`/`net`/`time`/`rand`/`term` widen (`term` → `ioctl`, #106).
//!    Deterministic (sorted, deduped). [`syscall_allowlist`] remains the x86_64
//!    compatibility wrapper used by the existing verification anchor.
//! 3. the BPF prelude ([`emit_sandbox_prelude`]): the Rust source that, as the
//!    first statements of the generated `main`, builds a classic `sock_filter[]`
//!    program (cfg-selected arch guard for x86_64 or aarch64 → load `nr` → a
//!    `BPF_JEQ` per allowed syscall → `SECCOMP_RET_ALLOW`, default
//!    `SECCOMP_RET_KILL_PROCESS`) and installs it via `prctl(PR_SET_NO_NEW_PRIVS)`
//!    + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`.
//!
//! ## The fx → syscall table
//!
//! The baseline (always, incl. `pure`/`alloc`) is the set a trivial `std` Rust
//! binary needs to start up, `println!`, run the L1 `thermite_check!` panic/abort
//! path, and exit — empirically grounded (`.design/forge/runtime-sandbox.md`
//! Verification). It excludes `openat`/`socket`/`getrandom`/`clock_gettime`
//! so a `pure` filter denies file I/O, network, rand, and time. `read`/`write`/`net`/
//! `time`/`rand` add their syscalls; `alloc`/`panic`/`diverge` add nothing beyond the
//! baseline (`panic` unwinds + writes to stderr via the baseline `write`+`exit_group`).
//!
//! ## No `libc` crate dependency (self-contained)
//!
//! The generated binary is a `std` program → it already links libc, so the prelude
//! declares `extern "C" { fn prctl(...); fn syscall(...); }` resolved against that
//! libc — no `libc` crate is added to `forge/Cargo.toml`. The `unsafe` lives in the
//! emitted source (the generated binary), not in `forge/src/`.
//!
//! ## Determinism (R-CODE-5)
//!
//! The same transitive `fx` yields the same sorted-deduped allowlist and a
//! byte-identical prelude.
//! [`syscall_allowlist`] sorts + dedups; [`emit_sandbox_prelude`] iterates the
//! sorted vector. No wall-clock / unordered iteration.
//!
//! ## REQ status
//!
//! <!-- generated:reqs view=forge-sandbox-status -->
//! Source: `.design/reqs/registry.toml`
//!
//! | ID | Status | Owner | Title | Follow-up |
//! |---|---|---|---|---|
//! | REQ-FORGE-SANDBOX-DEFAULT-ON | shipped | `forge/src/sandbox.rs` | Sandbox enabled by default for entry builds |  |
//! | REQ-FORGE-SANDBOX-MANIFEST | shipped | `forge/src/sandbox.rs` | Reproducible prelude and manifest record |  |
//! | REQ-FORGE-SANDBOX-PRELUDE | shipped | `forge/src/sandbox.rs` | Seccomp prelude installation |  |
//! | REQ-FORGE-SANDBOX-PROBE | shipped | `forge/src/sandbox.rs` | Sandbox self-test probe enforcement |  |
//! | REQ-FORGE-SANDBOX-SYSCALL-MAP | shipped | `forge/src/sandbox.rs` | Effect row to syscall allowlist mapping |  |
//! | REQ-FORGE-SANDBOX-TERM-IOCTL | shipped | `forge/src/sandbox.rs` | Term effect grants scoped ioctl |  |
//! | REQ-FORGE-SANDBOX-TRANSITIVE-FX | shipped | `forge/src/sandbox.rs` | Transitive effect derivation for sandbox |  |
//! <!-- /generated:reqs -->

use std::collections::BTreeSet;

use thermite_syntax::Program;

use crate::closure::reachable_in_file_fns;
use crate::manifest::effects_of;

/// The x86_64 syscall numbers a trivial `std` Rust binary needs to start up,
/// `println!`, run the always-active L1 `thermite_check!` panic/abort path, and
/// exit (the `pure`/`alloc` baseline, `.design/forge/runtime-sandbox.md` Table).
/// Excludes `openat`/`socket`/`getrandom`/`clock_gettime` so a pure filter denies
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
    15,  // rt_sigreturn  (the panic/abort unwind path — a violation panics, not killed)
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

/// The x86_64 syscalls a `read(_)` effect adds (file-open + stat + seek; `read`/
/// `close` are already in the baseline). `.design/forge/runtime-sandbox.md` Table.
const READ_SYSCALLS: &[u32] = &[
    8,   // lseek
    257, // openat
    262, // newfstatat
    332, // statx
];

/// The x86_64 syscalls a `write(_)` effect adds (`write` already baseline). Table.
const WRITE_SYSCALLS: &[u32] = &[
    74,  // fsync
    257, // openat
    262, // newfstatat
];

/// The x86_64 syscalls a `net(_)` effect adds (socket lifecycle). Table.
const NET_SYSCALLS: &[u32] = &[
    41, // socket
    42, // connect
    44, // sendto
    45, // recvfrom
    54, // setsockopt
    55, // getsockopt
];

/// The x86_64 syscalls a `time` effect adds. Table.
const TIME_SYSCALLS: &[u32] = &[
    228, // clock_gettime
    230, // clock_nanosleep
];

/// The x86_64 syscall a `rand` effect adds. Table.
const RAND_SYSCALLS: &[u32] = &[
    318, // getrandom
];

/// The x86_64 syscall a `term` (terminal-control) effect adds (issue #106):
/// `ioctl` (16), the syscall the termios `tcgetattr`/`tcsetattr` boundary issues
/// for raw mode. The grant is `ioctl`-broad (any cmd): classic seccomp-bpf
/// compares only `seccomp_data.nr`, not the `cmd` register, so v0.1 grants the
/// whole `ioctl` under `term` (runtime-sandbox.md REQ-7 / OQ-5). Scoped to the
/// `term` effect: a `pure`/`read`/`write`/`net` program's allowlist excludes
/// `ioctl`, so its `ioctl` is still `SIGSYS`-killed (a dedicated atom keeps a
/// plain `write` program — `print`/`write_file` — from acquiring `ioctl`).
const TERM_SYSCALLS: &[u32] = &[
    16, // ioctl (termios TCGETS/TCSETS — the cmd cannot be filtered, OQ-5)
];

/// The aarch64 syscall numbers use the Linux `asm-generic/unistd.h` numbering. The
/// table mirrors the x86_64 effect-surface shape, but the runtime baseline still
/// needs live arm64-Linux conformance before it should be treated as empirically
/// maxed. In particular, arm64 has no `arch_prctl` or `poll` syscall; the table uses
/// the syscalls available on native aarch64 Linux.
const BASELINE_AARCH64_SYSCALLS: &[u32] = &[
    57,  // close
    63,  // read
    64,  // write
    93,  // exit
    94,  // exit_group
    96,  // set_tid_address
    98,  // futex
    99,  // set_robust_list
    123, // sched_getaffinity
    132, // sigaltstack
    134, // rt_sigaction
    135, // rt_sigprocmask
    139, // rt_sigreturn
    178, // gettid
    214, // brk
    215, // munmap
    222, // mmap
    226, // mprotect
    233, // madvise
    261, // prlimit64
    293, // rseq
];

const READ_AARCH64_SYSCALLS: &[u32] = &[
    56,  // openat
    62,  // lseek
    79,  // newfstatat
    291, // statx
];

const WRITE_AARCH64_SYSCALLS: &[u32] = &[
    56, // openat
    79, // newfstatat
    82, // fsync
];

const NET_AARCH64_SYSCALLS: &[u32] = &[
    198, // socket
    203, // connect
    206, // sendto
    207, // recvfrom
    208, // setsockopt
    209, // getsockopt
];

const TIME_AARCH64_SYSCALLS: &[u32] = &[
    113, // clock_gettime
    115, // clock_nanosleep
];

const RAND_AARCH64_SYSCALLS: &[u32] = &[
    278, // getrandom
];

const TERM_AARCH64_SYSCALLS: &[u32] = &[
    29, // ioctl
];

/// The x86_64 `openat` syscall number — the [`emit_probe`] self-test attempts it
/// (`--sandbox-self-test`); denied under a `pure` filter (kill), allowed under
/// `read`. Mirrors the `READ_SYSCALLS` `openat`:257 entry.
const SYS_OPENAT_X86_64: u32 = 257;
/// The aarch64 `openat` syscall number (`asm-generic/unistd.h`).
const SYS_OPENAT_AARCH64: u32 = 56;
/// Compatibility alias for x86_64-only tests and the Verus anchor.
const SYS_OPENAT: u32 = SYS_OPENAT_X86_64;

/// Seccomp architectures the generated runner can install natively.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeccompArch {
    X86_64,
    Aarch64,
}

impl SeccompArch {
    pub fn host() -> Option<Self> {
        match std::env::consts::ARCH {
            "x86_64" => Some(Self::X86_64),
            "aarch64" => Some(Self::Aarch64),
            _ => None,
        }
    }
}

/// Whether a `forge build --entry` produces a sandboxed runner (REQ-4). On by
/// default for `--entry` (the §4.1 default is enforcement, not opt-in); `--no-sandbox`
/// opts out (a debugging / no-seccomp-platform escape hatch).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxMode {
    /// Inject the seccomp prelude as the first statements of `main` (the default).
    On,
    /// No prelude (the `--no-sandbox` escape hatch).
    Off,
}

/// The transitive `fx` token set for `entry` in `program` (REQ-2): the union of
/// `effects_of(&f.contract.fx)` over `{entry} ∪
/// closure::reachable_in_file_fns(program, entry)`. Reuses the same #17 cycle-safe,
/// source-order reachability walker `check::item_subprogram` consumes, rather than
/// a duplicate. A `#[boundary]`/`#[slag]` fn reached in the closure contributes its
/// declared `fx` (confined to that). Returns a sorted `BTreeSet` of the same
/// `["pure"]` / `read(x)` tokens the `BuildManifest.functions` rows carry
/// (deterministic, R-CODE-5).
pub fn transitive_fx(program: &Program, entry: &str) -> BTreeSet<String> {
    // The closure: every in-file `fn` the entry transitively reaches, plus the
    // entry itself (reachable_in_file_fns excludes `start`).
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
/// baseline unioned with every widening token's added syscalls ([the table](#the-fx--syscall-table)).
/// `pure`/`alloc`/`panic`/`diverge` add nothing beyond the baseline; `read(_)`/
/// `write(_)`/`net(_)`/`time`/`rand`/`term` widen (`term` → `ioctl`:16, #106). A
/// token is matched by its leading verb (`read(src)` → the `read` widening) so the
/// carried ident is irrelevant. Returns
/// the syscall numbers sorted and deduped — the same transitive `fx` yields the
/// byte-identical allowlist (deterministic, R-CODE-5).
fn baseline_syscalls(arch: SeccompArch) -> &'static [u32] {
    match arch {
        SeccompArch::X86_64 => BASELINE_SYSCALLS,
        SeccompArch::Aarch64 => BASELINE_AARCH64_SYSCALLS,
    }
}

fn widening_syscalls(arch: SeccompArch, verb: &str) -> &'static [u32] {
    match (arch, verb) {
        (SeccompArch::X86_64, "read") => READ_SYSCALLS,
        (SeccompArch::X86_64, "write") => WRITE_SYSCALLS,
        (SeccompArch::X86_64, "net") => NET_SYSCALLS,
        (SeccompArch::X86_64, "time") => TIME_SYSCALLS,
        (SeccompArch::X86_64, "rand") => RAND_SYSCALLS,
        (SeccompArch::X86_64, "term") => TERM_SYSCALLS,
        (SeccompArch::Aarch64, "read") => READ_AARCH64_SYSCALLS,
        (SeccompArch::Aarch64, "write") => WRITE_AARCH64_SYSCALLS,
        (SeccompArch::Aarch64, "net") => NET_AARCH64_SYSCALLS,
        (SeccompArch::Aarch64, "time") => TIME_AARCH64_SYSCALLS,
        (SeccompArch::Aarch64, "rand") => RAND_AARCH64_SYSCALLS,
        (SeccompArch::Aarch64, "term") => TERM_AARCH64_SYSCALLS,
        // "pure" / "alloc" / "panic" / "diverge" / any unknown → baseline-only.
        _ => &[],
    }
}

/// Map a transitive `fx` token set to the native syscall allowlist for `arch`
/// (REQ-3): the baseline unioned with every widening token's added syscalls.
/// Returns sorted and deduped numbers.
pub fn syscall_allowlist_for_arch(transitive_fx: &BTreeSet<String>, arch: SeccompArch) -> Vec<u32> {
    let mut set: BTreeSet<u32> = baseline_syscalls(arch).iter().copied().collect();
    for tok in transitive_fx {
        // The leading verb (before any `(`) selects the widening set; `pure`/
        // `alloc`/`panic`/`diverge` are baseline-only (no widening).
        let verb = tok.split('(').next().unwrap_or(tok);
        let widen = widening_syscalls(arch, verb);
        for &nr in widen {
            set.insert(nr);
        }
    }
    set.into_iter().collect()
}

/// Map a transitive `fx` token set to the x86_64 syscall allowlist (REQ-3). This
/// compatibility wrapper preserves the historical API used by the Verus anchor.
pub fn syscall_allowlist(transitive_fx: &BTreeSet<String>) -> Vec<u32> {
    syscall_allowlist_for_arch(transitive_fx, SeccompArch::X86_64)
}

/// Map a transitive `fx` token set to the host architecture's manifest allowlist.
/// Unsupported hosts fall back to the historical x86_64 table; sandboxed generated
/// runners still reject unsupported compile targets in [`emit_sandbox_prelude`].
pub fn syscall_allowlist_for_host(transitive_fx: &BTreeSet<String>) -> Vec<u32> {
    match SeccompArch::host().unwrap_or(SeccompArch::X86_64) {
        SeccompArch::X86_64 => syscall_allowlist(transitive_fx),
        SeccompArch::Aarch64 => syscall_allowlist_for_arch(transitive_fx, SeccompArch::Aarch64),
    }
}

fn emit_filter_lines(arch_label: &str, audit_arch_const: &str, allowlist: &[u32]) -> String {
    // The classic-BPF program. Each accepted-syscall comparison is a single
    // `BPF_JMP|BPF_JEQ|BPF_K` instruction: if `nr == <num>` jump to the ALLOW
    // return (jt), else fall through (jf=0) to the next comparison. After the last
    // comparison the program falls through to the KILL return. The header loads the
    // arch then the syscall number; a non-native arch is killed (REQ-1, OQ-3).
    //
    // `seccomp_data` layout (offsets): nr @ 0, arch @ 4.
    let mut filter_lines = String::new();

    filter_lines.push_str(&format!(
        "        // load seccomp_data.arch (offset 4); kill if not {arch_label}\n\
         \x20       BpfStmt(BPF_LD | BPF_W | BPF_ABS, 4),\n\
         \x20       BpfJump(BPF_JMP | BPF_JEQ | BPF_K, {audit_arch_const}, 1, 0),\n\
         \x20       BpfStmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),\n\
         \x20       // load seccomp_data.nr (offset 0)\n\
         \x20       BpfStmt(BPF_LD | BPF_W | BPF_ABS, 0),\n",
    ));

    // One JEQ per allowed syscall (jt=0 → execute the following ALLOW return).
    for &nr in allowlist {
        filter_lines.push_str(&format!(
            "        BpfJump(BPF_JMP | BPF_JEQ | BPF_K, {nr}, 0, 1),\n\
             \x20       BpfStmt(BPF_RET | BPF_K, SECCOMP_RET_ALLOW),\n"
        ));
    }

    // Default action: kill the whole process (a syscall off the allowlist → SIGSYS).
    filter_lines.push_str("        BpfStmt(BPF_RET | BPF_K, SECCOMP_RET_KILL_PROCESS),\n");
    filter_lines
}

/// Emit the Rust source of the seccomp-bpf filter-install prelude for `transitive_fx`
/// (REQ-1/REQ-3/REQ-5): a self-contained block that builds a cfg-selected classic
/// `sock_filter[]` program (x86_64 or aarch64 arch-guard → load `nr` → a `BPF_JEQ`
/// per allowed syscall → `SECCOMP_RET_ALLOW`, default
/// `SECCOMP_RET_KILL_PROCESS`) and installs it via
/// `prctl(PR_SET_NO_NEW_PRIVS)` + `prctl(PR_SET_SECCOMP, SECCOMP_MODE_FILTER)`. The
/// `prctl` is declared `extern "C"` (resolved against the std binary's already-linked
/// libc — no libc crate dependency). Injected as the first statements of the
/// generated `main` so the entry runs under the filter.
///
/// Byte-deterministic: each per-arch allowlist is sorted by
/// [`syscall_allowlist_for_arch`], so the same transitive `fx` yields the
/// byte-identical prelude (REQ-5, R-CODE-5).
pub fn emit_sandbox_prelude(transitive_fx: &BTreeSet<String>) -> String {
    let x86_allowlist = syscall_allowlist_for_arch(transitive_fx, SeccompArch::X86_64);
    let aarch64_allowlist = syscall_allowlist_for_arch(transitive_fx, SeccompArch::Aarch64);
    let x86_filter_lines = emit_filter_lines("x86_64", "AUDIT_ARCH_X86_64", &x86_allowlist);
    let aarch64_filter_lines =
        emit_filter_lines("aarch64", "AUDIT_ARCH_AARCH64", &aarch64_allowlist);
    format!(
        r##"
// ---- thermite #57 runtime effect sandbox (seccomp-bpf, fx-derived) ----------
// Installed as the first statements of `main`, before the entry call, so the entry
// (and any boundary/slag body it reaches) runs UNDER the filter. A syscall off the
// fx-derived allowlist -> SECCOMP_RET_KILL_PROCESS -> SIGSYS -> process killed.
// Raw `extern "C"` prctl resolved against the std binary's linked libc (no libc
// crate). Deterministic: the allowlist below is the sorted fx->syscall projection.
{{
    // classic-BPF opcodes / seccomp constants (Linux).
    const BPF_LD: u16 = 0x00;
    const BPF_W: u16 = 0x00;
    const BPF_ABS: u16 = 0x20;
    const BPF_JMP: u16 = 0x05;
    const BPF_JEQ: u16 = 0x10;
    const BPF_K: u16 = 0x00;
    const BPF_RET: u16 = 0x06;
    const AUDIT_ARCH_X86_64: u32 = 0xC000_003E;
    const AUDIT_ARCH_AARCH64: u32 = 0xC000_00B7;
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

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    compile_error!("thermite #57 sandbox supports only x86_64 and aarch64 Linux runners");

    #[cfg(target_arch = "x86_64")]
    static FILTER: &[SockFilter] = &[
{x86_filter_lines}    ];

    #[cfg(target_arch = "aarch64")]
    static FILTER: &[SockFilter] = &[
{aarch64_filter_lines}    ];

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

/// Emit the Rust source of the `--sandbox-self-test` probe (REQ-6): a raw
/// `syscall(SYS_openat, ...)` injected after the filter install and before the entry
/// call, so the kill/allow is observable. Under a `pure` filter `openat` is
/// non-allowlisted → `SIGSYS` (the process dies, exit 159); under a `read(_)` filter
/// `openat` is allowlisted → the probe returns and the entry runs normally (exit 0).
/// This is the v0.1 demonstrability device (pure Thermite never attempts a denied
/// syscall itself); a production runner has no probe.
pub fn emit_probe() -> String {
    format!(
        r##"
// ---- thermite #57 sandbox self-test probe (--sandbox-self-test only) --------
// A raw openat after the filter install: under a pure filter it is non-allowlisted
// -> SIGSYS -> the process is killed before the entry call (exit 159); under a
// read(_) filter openat is allowlisted -> the probe returns and the entry runs.
{{
    #[cfg(target_arch = "x86_64")]
    const SYS_OPENAT: i64 = {SYS_OPENAT};
    #[cfg(target_arch = "aarch64")]
    const SYS_OPENAT: i64 = {SYS_OPENAT_AARCH64};
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    compile_error!("thermite #57 sandbox probe supports only x86_64 and aarch64 Linux runners");

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

    // REQ-3: the pure baseline excludes openat (257) / socket (41) / getrandom
    // (318) / clock_gettime (228), so a pure filter denies file I/O, net, rand,
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

    // REQ-3: read(_) widens the allowlist to include openat (257) — the fx-derived
    // split the pure/read oracle cases assert.
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
    // family, R-DEFER-8); alloc/panic/diverge stay baseline-only.
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

    // Issue #27: the aarch64 table is parallel to the x86_64 table but uses native
    // Linux arm64 syscall numbers (`openat` 56, `ioctl` 29, `clock_gettime` 113).
    #[test]
    fn aarch64_allowlist_uses_native_syscall_numbers() {
        let read = syscall_allowlist_for_arch(&set(&["read(src)"]), SeccompArch::Aarch64);
        assert!(
            read.contains(&56),
            "read(_) allowlists aarch64 openat: {read:?}"
        );
        assert!(
            !read.contains(&257),
            "aarch64 read table must not leak x86_64 openat: {read:?}"
        );

        let time = syscall_allowlist_for_arch(&set(&["time"]), SeccompArch::Aarch64);
        assert!(time.contains(&113), "time allowlists aarch64 clock_gettime");
        assert!(
            !time.contains(&228),
            "aarch64 time table excludes x86_64 clock_gettime"
        );

        let term = syscall_allowlist_for_arch(&set(&["term"]), SeccompArch::Aarch64);
        assert!(term.contains(&29), "term allowlists aarch64 ioctl");
        assert!(
            !term.contains(&16),
            "aarch64 term table excludes x86_64 ioctl"
        );
    }

    // REQ-7 (#106): a `term` program's allowlist includes ioctl:16; a
    // pure/read/write/net program's allowlist excludes it — the grant is scoped to
    // the `term` effect (a dedicated atom, not folded into `write`). Anchored to the
    // design's Table `TERM_SYSCALLS={ioctl:16}` (R-CHAR-3, the design constant).
    #[test]
    fn term_grants_ioctl_scoped_to_the_effect() {
        assert!(
            syscall_allowlist(&set(&["term"])).contains(&16),
            "fx term grants ioctl:16"
        );
        // A program without term never gains ioctl — pure, read, write, net all deny it.
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

    // REQ-3 / R-CODE-5: the allowlist is sorted and deduped (read's openat:257 is not
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

    // REQ-2: a `read(src)` entry's transitive fx carries read, so the allowlist
    // widens. Anchored to the oracle's `rf` fixture shape.
    #[test]
    fn transitive_fx_carries_read() {
        let prog = parse("fn rf(x: u32) -> u32 req x < 100 ens result == x fx read(src) { x }");
        let fx = transitive_fx(&prog, "rf");
        assert!(fx.contains("read(src)"), "rf declares read(src): {fx:?}");
        assert!(syscall_allowlist(&fx).contains(&257), "→ openat widened");
    }

    // REQ-2: a caller's transitive fx unions a callee's declared row (the §4.1
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
    // byte-deterministic over the same transitive fx (R-CODE-5).
    #[test]
    fn prelude_installs_and_is_deterministic() {
        let pure_fx = set(&["pure"]);
        let a = emit_sandbox_prelude(&pure_fx);
        let b = emit_sandbox_prelude(&pure_fx);
        assert_eq!(a, b, "REQ-5: same transitive fx → byte-identical prelude");
        assert!(
            a.contains("PR_SET_SECCOMP") && a.contains("PR_SET_NO_NEW_PRIVS"),
            "REQ-1: the prelude installs the seccomp filter"
        );
        assert!(
            a.contains("SECCOMP_RET_KILL_PROCESS"),
            "REQ-1: the default action is kill-process"
        );
        assert!(
            a.contains("AUDIT_ARCH_X86_64") && a.contains("AUDIT_ARCH_AARCH64"),
            "issue #27: the prelude carries both supported arch guards"
        );
        // the pure prelude must not JEQ openat (257/56); a read prelude must for
        // each cfg-selected architecture.
        assert!(
            !a.contains("BPF_JEQ | BPF_K, 257"),
            "pure prelude has no openat comparison"
        );
        assert!(
            !a.contains("BPF_JEQ | BPF_K, 56"),
            "pure prelude has no aarch64 openat comparison"
        );
        let read = emit_sandbox_prelude(&set(&["read(s)"]));
        assert!(
            read.contains("BPF_JEQ | BPF_K, 257"),
            "read prelude has an x86_64 openat comparison"
        );
        assert!(
            read.contains("BPF_JEQ | BPF_K, 56"),
            "read prelude has an aarch64 openat comparison"
        );
    }

    // REQ-6: the probe is a raw openat syscall (the demonstrability device).
    #[test]
    fn probe_is_a_raw_openat() {
        let p = emit_probe();
        assert!(p.contains("SYS_OPENAT") && p.contains("syscall"));
        assert!(p.contains(&SYS_OPENAT.to_string()));
        assert!(p.contains(&SYS_OPENAT_AARCH64.to_string()));
    }
}

// ===========================================================================
// The verus anchor (epic #60, `.design/verified/self-verification.md` REQ-8).
//
// Placement deviation (Option B, orchestrator-authorized): the design doc names
// `forge/tests/sandbox_verified.rs` for this anchor, but `forge` is a binary-only
// crate (no lib target), so an external test cannot reach the internal
// `syscall_allowlist`/`BASELINE_SYSCALLS` symbols. This in-module `#[cfg(test)]`
// block reaches them directly; `thermite-verified` is a forge dev-dependency.
//
// AC-8c — the 512-mask exhaustive equivalence: enumerate all 2^9 fx-atom masks
// (widened for the #106 `Term` atom, bit 8), project each to the production token
// set, run the production `syscall_allowlist`, and assert its membership over the
// five sensitive user-I/O syscalls
// (openat/socket/connect/getrandom/clock_gettime) equals the verus-proved
// `thermite_verified::io_allow(mask)` bits for every mask. Expected = the proved
// bitset spec (R-CHAR-3, never forge's own output), so the production string-keyed
// mapping computes the relation Verus proved (pure-no-I/O + monotonicity +
// deny-by-default).
//
// OQ-6 (scope): verus proves soundness over the five sensitive syscalls only. This
// anchor binds `syscall_allowlist`'s membership over those five to the proved
// `io_allow` bits; it does not claim the dense `BASELINE_SYSCALLS` list is itself
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

    /// Project a `u16` fx-atom mask to the production token set (the same strings the
    /// `BuildManifest.functions` rows carry). The bit positions match the verus
    /// model's `u16` fx-mask: Read=0, Write=1, Net=2, Time=3, Rand=4, Alloc=5,
    /// Panic=6, Diverge=7, Term=8 (the #106 terminal-control atom). The carried
    /// ident on `read(_)`/`write(_)`/`net(_)` is irrelevant to the mapping (matched
    /// by the leading verb). Widened `u8`→`u16` for the 9th atom (#106).
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

    // AC-8c (REQ-8): over all 256 fx-atom masks, the production `syscall_allowlist`'s
    // membership of the five sensitive syscalls equals the verus-proved `io_allow`
    // bits. This is the exhaustive impl==spec equivalence (mechanism (c)) binding the
    // string-keyed production mapping to the proved bitset over its full finite domain.
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

    // AC-8c / REQ-8 pure-no-I/O: mask 0 (`pure`) permits none of the five sensitive
    // syscalls in the production allowlist — the proved `io_allow(0) == 0`.
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

    // AC-8c / REQ-8 monotonicity (observable): adding any fx atom never removes a
    // permitted sensitive syscall — a superset mask's sensitive membership is a
    // superset. Binds the proved `monotone` lemma to the production fn over a sample
    // of mask/superset pairs (the full bitset monotonicity is proved in verus).
    #[test]
    fn superset_mask_never_drops_a_sensitive_syscall() {
        for mask in 0u16..=511 {
            let base = syscall_allowlist(&mask_to_tokens(mask));
            // The full superset (all atoms) contains every sensitive syscall the
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
