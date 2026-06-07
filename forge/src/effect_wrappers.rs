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
//! | REQ-1 (the `os::<name>` wrapper stdlib — real `std` syscall bodies) | SHIPPED | the [`WRAPPERS`] table holds a real `std` body for each v1 target: `os::now` (`SystemTime::now()`), `os::read_byte`/`os::read_key`/`os::read_line` (`std::io::stdin().read`/`read_line`), `os::key_str` (a keystroke byte → a bounded 1-byte `TString`, the editor's host glue the surface lacks), `os::write`/`os::print` (`std::io::stdout().write_all`). Consumer: [`emit_mod_os`] (emitted into the generated crate by `build::emit_source`). Verified by `effect_link_conformance::elapsed_ok_builds_and_runs` (the linked `os::now` runs a real `clock_gettime`) + `effect_wrappers::tests::{read_key_wrapper_mirrors_read_byte_eof_sentinel,key_str_wrapper_is_bounded_one_byte_string}` (the editor's terminal-I/O wrappers) + the runnable editor `forge/tests/editor_runs.rs` (the linked `os::read_key`/`os::key_str`/`os::print` build + run with piped keystrokes). |
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
];

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
