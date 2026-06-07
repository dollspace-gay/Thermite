//! `forge/src/effect_wrappers.rs` — the runnable effect LINK (Basis Stage 8, issue
//! **#81**): the canonical `os::<name>` → real-`std`-syscall-wrapper SOURCE table,
//! and [`emit_mod_os`], which assembles a self-contained `mod os { … }` block
//! carrying EXACTLY the wrappers a built program's `#[boundary("os::<name>")]`
//! targets name.
//!
//! Governing design: `.design/basis/08-runnable-effect-link.md`. Oracle:
//! `conformance/effect-link/cases.json`.
//!
//! ## Why this module exists (the GROUNDED gap)
//!
//! `thermite_lower::lower_l1` lowers a `#[boundary("os::now")]` fn to an L1 wrapper
//! whose crossing is `let result = os::now();` (`thermite-lower/src/l1.rs`
//! `lower_boundary_fn_l1`). With no `os` module in the generated crate, raw `rustc`
//! fails `error[E0433]: cannot find module or crate \`os\``. Stage 8 supplies the
//! missing module: [`emit_mod_os`] prepends a `mod os { pub fn now() -> u64 { … } }`
//! (the real `std` syscall body) so `os::now()` RESOLVES + rustc LINKS it + the
//! binary RUNS + does real I/O (`08-runnable-effect-link.md` REQ-1/REQ-2).
//!
//! ## The decision (OQ-1/OQ-2, resolved emit-`mod os`, inline table)
//!
//! Per `08-runnable-effect-link.md` OQ-1, the link EMITS a self-contained `mod os`
//! into the crate (option (a)) rather than linking a `thermite-stdlib` crate
//! dependency (option (b)) — keeping the single-source raw-`rustc` build hermetic
//! (no `cargo`/dependency resolution, `build.md` OQ-2). Per OQ-2, the target→source
//! table lives INLINE here in `forge/src/` (the orchestrator's settled packaging) —
//! the canonical, reviewed wrapper SOURCE. The module emits ONLY the wrappers the
//! program names (minimal TCB, REQ-2/REQ-6) and is byte-deterministic (the table is
//! a fixed `const`, the emission is sorted; R-CODE-5).
//!
//! ## The wrappers' syscalls are #57-seccomp-CONFINED (REQ-4)
//!
//! The emitted wrappers run UNDER the SHIPPED #57 seccomp filter
//! `synthesize_entry_main` installs FIRST in the generated `main`: `os::now`'s
//! `clock_gettime` is in the `time`-widened allowlist, but an out-of-`fx` syscall is
//! `SIGSYS`-killed. The link does NOT widen the trust boundary — it makes the
//! manifest-enumerated boundary RUNNABLE under the same confinement.
//!
//! ## REQ status (`.design/basis/08-runnable-effect-link.md`)
//!
//! | REQ | Status | Evidence |
//! |---|---|---|
//! | REQ-1 (the `os::<name>` wrapper stdlib — real `std` syscall bodies) | SHIPPED | the [`WRAPPERS`] table holds a real `std` body for each v1 target: `os::now` (`SystemTime::now()`), `os::read_byte`/`os::read_key`/`os::read_line` (`std::io::stdin().read`/`read_line`), `os::key_str` (a keystroke byte → a bounded 1-byte `TString`, the editor's host glue the surface lacks), `os::write`/`os::print` (`std::io::stdout().write_all`), the editor's terminal-control + render boundaries `os::raw_mode_on`/`os::raw_mode_off`/`os::read_key_raw`/`os::write_frame` (#90), and the editor's file-LOAD/SAVE boundaries `os::read_file` (`std::fs::read` of the fixed `THERMITE_EDITOR_FILE`/`/tmp` path → empty `TString` on error) / `os::write_file` (`std::fs::write`, 0/1 status, #125). Consumer: [`emit_mod_os`] (emitted into the generated crate by `build::emit_source`). Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (the linked `os::now` runs a real `clock_gettime`) + `effect_wrappers::tests::{read_key_wrapper_mirrors_read_byte_eof_sentinel,key_str_wrapper_is_bounded_one_byte_string,read_file_wrapper_is_total_empty_on_error,write_file_wrapper_is_total_status_arm}` (the editor's terminal-I/O + file-I/O wrappers) + the runnable editor `forge/tests/editor_runs.rs` (the linked `os::read_key_raw`/`os::write_frame`/`os::read_file`/`os::write_file` build + run with piped multi-line keystrokes + a save). |
//! | REQ-2 (`forge build` LINKS via emit-`mod os` keyed off boundary targets) | SHIPPED | [`emit_mod_os`] assembles a `mod os { … }` carrying EXACTLY the wrappers in the given target set (sorted, deterministic); `build::reachable_boundary_targets` keys it off the program's `#[boundary]` fns; `build::emit_source` prepends it. Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (rustc exit 0, no `E0433`) + `effect_wrappers::tests::emits_only_named_wrappers`. |
//! | REQ-3 (a verified program COMPILES + RUNS + does real I/O) | SHIPPED | the linked `os::now` wrapper does a real `clock_gettime` → `elapsed_ok()` prints a live Unix timestamp. Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (run exit 0, output is a u64 < 4_000_000_000). |

use std::collections::BTreeSet;

use crate::cli::ForgeError;

/// One entry in the canonical `os::<name>` → wrapper-source table: the bare wrapper
/// name (the segment after `os::`) and the EXACT `pub fn` Rust source the link emits
/// into the generated crate's `mod os` (REQ-1). The body is the real `std` syscall
/// wrapper — trusted-by-fiat, #57-confined.
struct Wrapper {
    /// The bare name after `os::` (e.g. `"now"` for the target `"os::now"`).
    name: &'static str,
    /// The full `pub fn <name>(…) -> … { <real std body> }` source, emitted verbatim
    /// inside `mod os { … }`. Signed to MATCH the boundary's lowered signature
    /// (`thermite-lower/src/l1.rs` `lower_boundary_fn_l1`: the wrapper forwards its
    /// params to `os::<name>(args)`).
    source: &'static str,
}

/// The canonical v1 `os::<name>` wrapper SOURCE table (REQ-1) — the real `std`
/// syscall bodies for the v1 Read / Write / Time families
/// (`08-runnable-effect-link.md`, the wrapper-set table). Trusted-by-fiat (you
/// cannot prove the kernel), #57-seccomp-CONFINED, #15-manifest-ENUMERATED. A
/// honest I/O wrapper handles its error arm (returns the EOF sentinel / a status
/// code) rather than `unwrap`-panicking (the design's "honest error handling").
///
/// `Net`/`Rand` follow the IDENTICAL shape and are v1.1 (one row each). `Alloc`
/// needs no `os::` wrapper (the Rust allocator under the baseline allowlist).
const WRAPPERS: &[Wrapper] = &[
    // os::now (Time) — the minimal primitive: no input, no failure arm. The real
    // `clock_gettime` (#57 syscall 228). `unwrap_or(0)` keeps the wrapper total
    // (a pre-1970 clock cannot occur on a live host; 0 is the honest floor).
    Wrapper {
        name: "now",
        source: "    pub fn now() -> u64 {\n        \
                 std::time::SystemTime::now()\n            \
                 .duration_since(std::time::UNIX_EPOCH)\n            \
                 .map(|d| d.as_secs())\n            \
                 .unwrap_or(0)\n    }\n",
    },
    // os::read_byte (Read) — the closed-outcome-set primitive: one byte from stdin,
    // or the EOF sentinel 256 (the design's `read_demo` shape, `ens result <= 256`).
    // A read error is the honest EOF arm (256), never a panic.
    Wrapper {
        name: "read_byte",
        source: "    pub fn read_byte() -> u64 {\n        \
                 use std::io::Read;\n        \
                 let mut buf = [0u8; 1];\n        \
                 match std::io::stdin().read(&mut buf) {\n            \
                 Ok(1) => buf[0] as u64,\n            \
                 _ => 256,\n        }\n    }\n",
    },
    // os::read_key (Read) — the editor's keystroke primitive: one raw byte from
    // stdin, or the EOF sentinel 256 (the editor's `read_key`, `ens result <= 256`).
    // IDENTICAL closed-outcome shape to `read_byte` (the keystroke IS a raw byte);
    // a separate name keeps the editor's terminal-input boundary legible in the
    // manifest. A read error is the honest EOF arm (256), never a panic.
    Wrapper {
        name: "read_key",
        source: "    pub fn read_key() -> u64 {\n        \
                 use std::io::Read;\n        \
                 let mut buf = [0u8; 1];\n        \
                 match std::io::stdin().read(&mut buf) {\n            \
                 Ok(1) => buf[0] as u64,\n            \
                 _ => 256,\n        }\n    }\n",
    },
    // os::key_str (Alloc) — the editor's host glue the surface lacks: a keystroke
    // byte → a one-byte Stage-7 `String` to insert (`ens result.len() <= 1`). A
    // representable byte (`k <= 255`) yields a 1-byte `TString`; a control/EOF key
    // (`k >= 256`, or any non-byte value) yields the EMPTY string (the bounded,
    // honest arm — `result.len() <= 1` holds in every case). Total, no panic.
    Wrapper {
        name: "key_str",
        source: "    pub fn key_str(k: u64) -> super::TString {\n        \
                 if k <= 255 {\n            \
                 super::TString { data: vec![k as u8] }\n        \
                 } else {\n            \
                 super::TString { data: Vec::new() }\n        }\n    }\n",
    },
    // os::read_line (Read) — a line from stdin as a Stage-7 `String` (the lowered
    // `TString` newtype, `pub data: Vec<u8>`). A read error yields an empty line
    // (the honest arm), never a panic.
    Wrapper {
        name: "read_line",
        source: "    pub fn read_line() -> super::TString {\n        \
                 let mut s = String::new();\n        \
                 let _ = std::io::stdin().read_line(&mut s);\n        \
                 super::TString { data: s.into_bytes() }\n    }\n",
    },
    // os::write (Write) — hand the bytes of a Stage-7 `String` to stdout
    // (`write_all`, #57 syscall `write`:1). Returns a status `u64` (0 = ok, 1 =
    // I/O error) — the honest closed status arm, never a panic.
    Wrapper {
        name: "write",
        source: "    pub fn write(s: super::TString) -> u64 {\n        \
                 use std::io::Write;\n        \
                 match std::io::stdout().write_all(&s.data) {\n            \
                 Ok(()) => 0,\n            \
                 Err(_) => 1,\n        }\n    }\n",
    },
    // os::print (Write) — write a Stage-7 `String` to stdout and flush. Returns a
    // status `u64` (0 = ok, 1 = I/O error), the honest closed status arm.
    Wrapper {
        name: "print",
        source: "    pub fn print(s: super::TString) -> u64 {\n        \
                 use std::io::Write;\n        \
                 let mut out = std::io::stdout();\n        \
                 match out.write_all(&s.data).and_then(|()| out.flush()) {\n            \
                 Ok(()) => 0,\n            \
                 Err(_) => 1,\n        }\n    }\n",
    },
    // os::raw_mode_on (the editor's terminal-control boundary, #90) — put the
    // terminal into RAW mode (clear ICANON + ECHO so each keystroke reaches the
    // editor live, no line buffering, no echo; VMIN=1/VTIME=0 for a blocking
    // 1-byte read) via extern-C `tcgetattr`/`tcsetattr`. libc is resolved against
    // the std binary's already-linked libc (NO libc crate dependency — the SAME
    // `extern "C"`-against-std path the #57 seccomp prelude's `prctl` uses), so the
    // generated single-file crate stays self-contained. The ORIGINAL termios is
    // saved into a process-global `OnceLock` on first entry so `raw_mode_off` can
    // restore it. GRACEFUL on a non-TTY stdin (piped): `tcgetattr` returns nonzero
    // (ENOTTY), the wrapper returns 1 and leaves the terminal untouched — NO crash,
    // NO panic (the honest status arm). Trusted-by-fiat (you cannot prove the
    // kernel), #57-seccomp-confined to its declared `fx`.
    Wrapper {
        name: "raw_mode_on",
        source: TERMIOS_RAW_MODE_SOURCE,
    },
    // os::raw_mode_off (#90) — restore the terminal's ORIGINAL mode (the saved
    // termios). MUST run on the editor's exit path so the terminal is never left in
    // raw mode. A no-op (returns 0) when raw mode was never entered (no saved
    // termios — the non-TTY/piped case) or on a `tcsetattr` error (returns 1), never
    // a panic. Shares the `__THERMITE_ORIG_TERMIOS` OnceLock + the extern-C decls
    // with `raw_mode_on` (emitted once in `TERMIOS_RAW_MODE_SOURCE`); this entry is
    // EMPTY so the pair is emitted exactly once even when both targets are named.
    Wrapper {
        name: "raw_mode_off",
        source: "",
    },
    // os::read_key_raw (the editor's keystroke boundary, #90) — read one keystroke
    // from stdin, returning the raw bytes PACKED into a u64 for the verified
    // `decode`: byte b0 in bits 0..9, b1 in bits 9..18, b2 in bits 18..27 (each
    // field 0..256, or the 256 EOF sentinel). A plain key reads 1 byte (b1=b2=0); an
    // ESC (0x1b) reads the 2-byte arrow tail. The closed outcome set is honest — the
    // world produces bytes or EOF, never more; a read error/EOF is the 256 sentinel
    // arm, never a panic. Trusted, #57-confined to the `read` syscall set.
    Wrapper {
        name: "read_key_raw",
        source: "    pub fn read_key_raw() -> u64 {\n        \
                 use std::io::Read;\n        \
                 let mut buf = [0u8; 1];\n        \
                 let b0: u64 = match std::io::stdin().read(&mut buf) {\n            \
                 Ok(1) => buf[0] as u64,\n            \
                 _ => 256,\n        };\n        \
                 if b0 != 27 {\n            \
                 return b0;\n        }\n        \
                 let b1: u64 = match std::io::stdin().read(&mut buf) {\n            \
                 Ok(1) => buf[0] as u64,\n            \
                 _ => 256,\n        };\n        \
                 let b2: u64 = match std::io::stdin().read(&mut buf) {\n            \
                 Ok(1) => buf[0] as u64,\n            \
                 _ => 256,\n        };\n        \
                 b0 + (b1 << 9) + (b2 << 18)\n    }\n",
    },
    // os::write_frame (the editor's render boundary, #90) — write the rendered frame
    // String's bytes to stdout and flush. Returns a status u64 (0 = ok, 1 = I/O
    // error), the honest closed status arm, never a panic. Trusted, #57-confined to
    // the `write` syscall set. (Identical shape to `print`; a distinct name keeps the
    // editor's render boundary legible in the #15 manifest.)
    Wrapper {
        name: "write_frame",
        source: "    pub fn write_frame(s: super::TString) -> u64 {\n        \
                 use std::io::Write;\n        \
                 let mut out = std::io::stdout();\n        \
                 match out.write_all(&s.data).and_then(|()| out.flush()) {\n            \
                 Ok(()) => 0,\n            \
                 Err(_) => 1,\n        }\n    }\n",
    },
    // os::read_file (the editor's file-LOAD boundary, #125) — read the editor's fixed
    // demo file (THERMITE_EDITOR_FILE) into a Stage-7 `String` (the lowered `TString`
    // newtype, `pub data: Vec<u8>`). The v0.1 `forge build --entry run` synthesizes no
    // path arg, so the load source is a FIXED path: the `THERMITE_EDITOR_FILE` env var
    // if set, else `/tmp/thermite_editor.txt`. A missing file / read error yields the
    // EMPTY string (the honest arm — a fresh buffer), NEVER a panic. The byte content
    // is taken verbatim (`\n` bytes are preserved — the multi-line buffer is one
    // String). Trusted-by-fiat, #57-confined to the `read`/`open` syscall set.
    Wrapper {
        name: "read_file",
        source: "    pub fn read_file() -> super::TString {\n        \
                 let path = std::env::var(\"THERMITE_EDITOR_FILE\")\n            \
                 .unwrap_or_else(|_| \"/tmp/thermite_editor.txt\".to_string());\n        \
                 match std::fs::read(&path) {\n            \
                 Ok(bytes) => super::TString { data: bytes },\n            \
                 Err(_) => super::TString { data: Vec::new() },\n        }\n    }\n",
    },
    // os::write_file (the editor's file-SAVE boundary, #125 — Ctrl-S) — write the
    // buffer `String`'s bytes to the editor's fixed demo file (THERMITE_EDITOR_FILE if
    // set, else `/tmp/thermite_editor.txt`). Returns a status u64 (0 = ok, 1 = I/O
    // error), the honest closed status arm, NEVER a panic. The bytes (incl. the `\n`
    // line breaks) are written verbatim — the multi-line buffer round-trips through
    // read_file. Trusted-by-fiat, #57-confined to the `open`/`write` syscall set.
    Wrapper {
        name: "write_file",
        source: "    pub fn write_file(s: super::TString) -> u64 {\n        \
                 let path = std::env::var(\"THERMITE_EDITOR_FILE\")\n            \
                 .unwrap_or_else(|_| \"/tmp/thermite_editor.txt\".to_string());\n        \
                 match std::fs::write(&path, &s.data) {\n            \
                 Ok(()) => 0,\n            \
                 Err(_) => 1,\n        }\n    }\n",
    },
];

/// The extern-C termios raw-mode wrapper pair (`os::raw_mode_on` + `os::raw_mode_off`,
/// #90), emitted VERBATIM into `mod os` when EITHER target is named (the
/// `raw_mode_off` table row is empty — this source carries both so the shared
/// `extern "C"` decls + the `__THERMITE_ORIG_TERMIOS` OnceLock are emitted exactly
/// once, never duplicated). The `Termios` struct + `tcgetattr`/`tcsetattr` are
/// declared `extern "C"` and resolve against the std binary's already-linked libc
/// (NO libc crate dependency — the SAME hermetic single-file path the #57 seccomp
/// prelude's `prctl`/`syscall` use). Trusted-by-fiat, #57-seccomp-confined.
///
/// Honest error handling (R-CODE-2): a non-TTY stdin (the piped/no-TTY case) makes
/// `tcgetattr` return nonzero (ENOTTY); the wrapper returns 1 and leaves the terminal
/// untouched — NO crash, NO panic. The `unsafe` blocks are documented leaf FFI
/// primitives (the only way to call the libc termios syscalls), each with a
/// `// SAFETY:` note; the structs are POD the kernel fills.
const TERMIOS_RAW_MODE_SOURCE: &str = r#"    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Termios {
        c_iflag: u32,
        c_oflag: u32,
        c_cflag: u32,
        c_lflag: u32,
        c_line: u8,
        c_cc: [u8; 32],
        c_ispeed: u32,
        c_ospeed: u32,
    }
    extern "C" {
        fn tcgetattr(fd: i32, termios_p: *mut Termios) -> i32;
        fn tcsetattr(fd: i32, optional_actions: i32, termios_p: *const Termios) -> i32;
    }
    // The original terminal mode, saved on the first `raw_mode_on` so `raw_mode_off`
    // can restore it. A process-global OnceLock (no Mutex/RefCell escape hatch).
    static __THERMITE_ORIG_TERMIOS: std::sync::OnceLock<Termios> = std::sync::OnceLock::new();
    pub fn raw_mode_on() -> u64 {
        const STDIN_FD: i32 = 0;
        const ICANON: u32 = 0x0000_0002;
        const ECHO: u32 = 0x0000_0008;
        const TCSANOW: i32 = 0;
        const VMIN: usize = 6;
        const VTIME: usize = 5;
        let mut t = Termios {
            c_iflag: 0, c_oflag: 0, c_cflag: 0, c_lflag: 0,
            c_line: 0, c_cc: [0u8; 32], c_ispeed: 0, c_ospeed: 0,
        };
        // SAFETY: leaf FFI primitive — `tcgetattr` fills `*mut Termios` (a #[repr(C)]
        // POD matching the libc layout) for the valid stdin fd; the only way to read
        // the terminal mode. A non-TTY fd returns nonzero (handled below, no UB).
        let got = unsafe { tcgetattr(STDIN_FD, &mut t as *mut Termios) };
        if got != 0 {
            // Not a TTY (piped stdin) or error: leave the terminal untouched (the
            // graceful no-crash arm) — read_key_raw still reads the piped bytes.
            return 1;
        }
        let _ = __THERMITE_ORIG_TERMIOS.set(t);
        let mut raw = t;
        raw.c_lflag &= !(ICANON | ECHO);
        raw.c_cc[VMIN] = 1;
        raw.c_cc[VTIME] = 0;
        // SAFETY: leaf FFI primitive — `tcsetattr` reads `*const Termios` (the same
        // #[repr(C)] POD) and applies it to the valid stdin fd; the only way to set
        // raw mode. Returns nonzero on error (handled, no UB).
        let set = unsafe { tcsetattr(STDIN_FD, TCSANOW, &raw as *const Termios) };
        if set != 0 { 1 } else { 0 }
    }
    pub fn raw_mode_off() -> u64 {
        const STDIN_FD: i32 = 0;
        const TCSANOW: i32 = 0;
        match __THERMITE_ORIG_TERMIOS.get() {
            // Restore the saved original mode. If raw mode was never entered (no
            // saved termios — the non-TTY/piped case), this is a clean no-op.
            Some(orig) => {
                // SAFETY: leaf FFI primitive — `tcsetattr` reads `*const Termios` (the
                // saved #[repr(C)] POD) and restores it on the valid stdin fd. Returns
                // nonzero on error (handled, no UB).
                let r = unsafe { tcsetattr(STDIN_FD, TCSANOW, orig as *const Termios) };
                if r != 0 { 1 } else { 0 }
            }
            None => 0,
        }
    }
"#;

/// The `os::` prefix every v1 effect-primitive target carries
/// (`08-runnable-effect-link.md`, the `os::<name>` convention). A target that does
/// not start with `os::` is not a v1 syscall wrapper (a future `ext::`/crates.io
/// boundary; out of Stage-8 scope) and is reported as unsupported by
/// [`emit_mod_os`] rather than silently linked.
const OS_PREFIX: &str = "os::";

/// Look up the wrapper SOURCE for a bare wrapper name (the segment after `os::`).
/// Returns `None` for an unknown name (the caller maps it to a structured error).
fn wrapper_for(name: &str) -> Option<&'static Wrapper> {
    WRAPPERS.iter().find(|w| w.name == name)
}

/// Assemble the self-contained `mod os { … }` block carrying EXACTLY the wrappers
/// the given boundary `targets` name (REQ-1/REQ-2). Each target is an `os::<name>`
/// string (the `BoundaryAttr.target` the lowered crossing `os::<name>(args)` calls).
///
/// - An EMPTY target set (a program with no reachable `os::` boundary) yields an
///   EMPTY string — no `mod os` is emitted (the pure corpus is byte-unaffected,
///   `08-runnable-effect-link.md` AC-7).
/// - The wrappers are emitted in SORTED order (`targets` is a `BTreeSet`,
///   re-sorted by name) so the emission is byte-deterministic (R-CODE-5, REQ-2).
/// - A target that is not an `os::<known>` wrapper is a structured `ForgeError`
///   (R-CODE-2: a `#[boundary("net::connect")]` v1.1 target or a typo'd `os::noo`
///   names no v1 wrapper — the build refuses rather than emit an unresolved call).
///
/// The block is `#[allow(dead_code)]` (a program may declare a boundary fn it does
/// not call from the entry; its wrapper is still emitted to keep the crate
/// self-contained, but rustc would warn it unused — the wrapper is the live TCB
/// surface, not dead in intent).
pub fn emit_mod_os(targets: &BTreeSet<String>) -> Result<String, ForgeError> {
    if targets.is_empty() {
        return Ok(String::new());
    }

    // Resolve each target to its wrapper SOURCE, collecting by bare name so a
    // duplicate target (the same `os::now` declared twice) emits one wrapper. A
    // `BTreeMap` keeps the emission sorted + deterministic (R-CODE-5).
    let mut bodies: std::collections::BTreeMap<&'static str, &'static str> =
        std::collections::BTreeMap::new();
    for target in targets {
        let name = target.strip_prefix(OS_PREFIX).ok_or_else(|| {
            ForgeError::Usage(format!(
                "the boundary target `{target}` is not an `os::<name>` syscall wrapper; \
                 `forge build`'s runnable link (Stage 8) supports only the v1 `os::` \
                 effect primitives ({})",
                supported_names()
            ))
        })?;
        let wrapper = wrapper_for(name).ok_or_else(|| {
            ForgeError::Usage(format!(
                "no `forge build` runnable wrapper for the boundary target `{target}`; \
                 the v1 effect-link wrappers are: {}",
                supported_names()
            ))
        })?;
        bodies.insert(wrapper.name, wrapper.source);
    }

    let mut out = String::new();
    out.push_str("#[allow(dead_code)]\nmod os {\n");
    for source in bodies.values() {
        out.push_str(source);
    }
    out.push_str("}\n");
    Ok(out)
}

/// The comma-joined list of supported v1 wrapper names (for the unsupported-target
/// error message). Deterministic (the table order).
fn supported_names() -> String {
    WRAPPERS
        .iter()
        .map(|w| format!("os::{}", w.name))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets(items: &[&str]) -> BTreeSet<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn empty_targets_emit_no_module() {
        // AC-7: a program with no `os::` boundary emits no `mod os` (the pure corpus
        // is byte-unaffected).
        assert_eq!(emit_mod_os(&targets(&[])).unwrap(), "");
    }

    #[test]
    fn emits_only_named_wrappers() {
        // REQ-2 (minimal TCB): a program touching only `os::now` links ONLY `now`.
        let out = emit_mod_os(&targets(&["os::now"])).unwrap();
        assert!(out.contains("mod os {"), "the block is emitted: {out}");
        assert!(out.contains("pub fn now()"), "now is linked: {out}");
        assert!(
            !out.contains("pub fn read_byte"),
            "only the named wrapper is linked (no read_byte): {out}"
        );
        assert!(
            !out.contains("pub fn print"),
            "only the named wrapper is linked (no print): {out}"
        );
    }

    #[test]
    fn now_wrapper_is_the_grounded_clock_gettime_body() {
        // REQ-1: the `os::now` body is the GROUNDED `SystemTime::now()` form
        // (`08-runnable-effect-link.md` Grounding (2)). Anchored to the design's
        // pinned wrapper source, not toolchain output (R-CHAR-3).
        let out = emit_mod_os(&targets(&["os::now"])).unwrap();
        assert!(out.contains("SystemTime::now()"), "{out}");
        assert!(
            out.contains("duration_since(std::time::UNIX_EPOCH)"),
            "{out}"
        );
        assert!(out.contains("as_secs()"), "{out}");
        assert!(
            out.contains("unwrap_or(0)"),
            "the wrapper is total (honest floor), no panic: {out}"
        );
    }

    #[test]
    fn read_byte_wrapper_uses_eof_sentinel_256() {
        // REQ-1: `os::read_byte`'s honest closed-outcome set is byte-or-256-EOF
        // (the design's `read_demo` shape `ens result <= 256`).
        let out = emit_mod_os(&targets(&["os::read_byte"])).unwrap();
        assert!(out.contains("std::io::stdin().read(&mut buf)"), "{out}");
        assert!(
            out.contains("_ => 256"),
            "the EOF/error arm is the 256 sentinel, not a panic: {out}"
        );
    }

    fn emit_or_fail(items: &[&str]) -> String {
        let result = emit_mod_os(&targets(items));
        assert!(result.is_ok(), "emit_mod_os({items:?}) should succeed");
        result.unwrap_or_default()
    }

    #[test]
    fn read_key_wrapper_mirrors_read_byte_eof_sentinel() {
        // REQ-1 (the editor's terminal-input boundary): `os::read_key` is the
        // keystroke primitive — one raw byte from stdin or the 256 EOF sentinel
        // (the editor's `read_key`, `ens result <= 256`), the honest closed
        // outcome set, never a panic. Anchored to the design's pinned wrapper
        // shape (R-CHAR-3), not toolchain output.
        let out = emit_or_fail(&["os::read_key"]);
        assert!(out.contains("pub fn read_key() -> u64"), "{out}");
        assert!(out.contains("std::io::stdin().read(&mut buf)"), "{out}");
        assert!(
            out.contains("_ => 256"),
            "the EOF/error arm is the 256 sentinel, not a panic: {out}"
        );
    }

    #[test]
    fn key_str_wrapper_is_bounded_one_byte_string() {
        // REQ-1 (the editor's host glue): `os::key_str` maps a keystroke byte to a
        // bounded one-byte `String` — a representable byte (`k <= 255`) is a 1-byte
        // `TString`, a control/EOF key the EMPTY string (`ens result.len() <= 1`
        // holds in every case). Total, no panic. Anchored to the design's pinned
        // wrapper shape (R-CHAR-3).
        let out = emit_or_fail(&["os::key_str"]);
        assert!(
            out.contains("pub fn key_str(k: u64) -> super::TString"),
            "{out}"
        );
        assert!(
            out.contains("if k <= 255"),
            "the representable-byte guard keeps result.len() <= 1: {out}"
        );
        assert!(
            out.contains("vec![k as u8]"),
            "a representable byte yields a 1-byte TString: {out}"
        );
        assert!(
            out.contains("data: Vec::new()"),
            "a control/EOF key yields the empty string (the bounded arm): {out}"
        );
    }

    #[test]
    fn read_file_wrapper_is_total_empty_on_error() {
        // REQ-1 (the editor's file-LOAD boundary, #125): `os::read_file` reads the
        // fixed demo path into a `TString`; a missing file / read error is the EMPTY
        // string (the honest arm — a fresh buffer), NEVER a panic. Anchored to the
        // design's pinned wrapper shape (R-CHAR-3).
        let out = emit_or_fail(&["os::read_file"]);
        assert!(
            out.contains("pub fn read_file() -> super::TString"),
            "{out}"
        );
        assert!(
            out.contains("std::fs::read(&path)"),
            "the load reads the fixed path's bytes: {out}"
        );
        assert!(
            out.contains("Err(_) => super::TString { data: Vec::new() }"),
            "a read error yields the EMPTY string (fresh buffer), not a panic: {out}"
        );
        assert!(
            out.contains("THERMITE_EDITOR_FILE"),
            "the load source is the fixed env/`/tmp` path (v0.1 synthesizes no arg): {out}"
        );
    }

    #[test]
    fn write_file_wrapper_is_total_status_arm() {
        // REQ-1 (the editor's file-SAVE boundary, #125 — Ctrl-S): `os::write_file`
        // writes the buffer bytes (incl. `\n`) to the fixed demo path, returning a
        // status u64 (0 = ok, 1 = I/O error) — the honest closed arm, NEVER a panic.
        // Anchored to the design's pinned wrapper shape (R-CHAR-3).
        let out = emit_or_fail(&["os::write_file"]);
        assert!(
            out.contains("pub fn write_file(s: super::TString) -> u64"),
            "{out}"
        );
        assert!(
            out.contains("std::fs::write(&path, &s.data)"),
            "the save writes the buffer bytes verbatim (the `\\n` line breaks): {out}"
        );
        assert!(
            out.contains("Ok(()) => 0") && out.contains("Err(_) => 1"),
            "the status arm is 0=ok / 1=error, not a panic: {out}"
        );
    }

    #[test]
    fn emission_is_sorted_deterministic() {
        // REQ-2 (R-CODE-5): the same target set emits byte-identical output, sorted
        // by name regardless of insertion order.
        let a = emit_mod_os(&targets(&["os::print", "os::now", "os::read_byte"])).unwrap();
        let b = emit_mod_os(&targets(&["os::now", "os::read_byte", "os::print"])).unwrap();
        assert_eq!(a, b, "the emission is order-independent (deterministic)");
        let now_at = a.find("pub fn now()").unwrap();
        let print_at = a.find("pub fn print(").unwrap();
        let read_at = a.find("pub fn read_byte()").unwrap();
        assert!(
            now_at < print_at && print_at < read_at,
            "sorted by name: {a}"
        );
    }

    #[test]
    fn unknown_os_target_is_a_structured_error() {
        // R-CODE-2: an `os::<typo>` names no v1 wrapper → a Usage error, never an
        // emitted unresolved call.
        let err = emit_mod_os(&targets(&["os::nonesuch"])).unwrap_err();
        match err {
            ForgeError::Usage(msg) => {
                assert!(msg.contains("os::nonesuch"), "names the bad target: {msg}");
                assert!(msg.contains("os::now"), "lists the supported set: {msg}");
            }
            other => panic!("expected a Usage error, got {other:?}"),
        }
    }

    #[test]
    fn non_os_target_is_a_structured_error() {
        // R-CODE-2: a non-`os::` boundary target (a v1.1 `net::`/crates.io target) is
        // out of Stage-8 scope → a Usage error, never silently linked.
        let err = emit_mod_os(&targets(&["ext::frob"])).unwrap_err();
        match err {
            ForgeError::Usage(msg) => assert!(msg.contains("ext::frob"), "{msg}"),
            other => panic!("expected a Usage error, got {other:?}"),
        }
    }
}
