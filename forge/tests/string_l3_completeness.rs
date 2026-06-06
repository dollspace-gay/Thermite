//! Regression tests for the two String-lowering completeness gaps that blocked the
//! verified text-editor core (`Buf { text: String, cursor }` + insert/delete via
//! slice+concat) — crosslink **#86**, both L3 / `forge check` lowering gaps.
//!
//! Stage 7 Strings shipped (#79), but two reachable lowering paths were never
//! exercised by `conformance/string_demo.th`:
//!
//!   GAP 1 — `slice`'s exec-position arg coercion. The `TString` wrapper's index
//!   accessor `slice(lo: usize, hi: usize)` takes `usize`, but a Thermite surface
//!   index is commonly a `u64` (`s.slice(0, k)` with `k: u64`). Verus does NO
//!   implicit `u64 -> usize` narrowing, so the un-coerced arg produced
//!   `error[E0308]: expected usize, found u64` -> L0. The fix coerces a non-literal
//!   index arg of BOTH string index intrinsics (`byte_at`/`slice`) with `as usize`
//!   (`thermite-lower::lower` `lower_expr` MethodCall exec arm + `is_usize_cast`).
//!
//!   GAP 2 — the `TString` wrapper def woven into the per-item sub-program when a
//!   `String`/`Type::String` is REACHABLE as a struct/enum FIELD type (not just a
//!   fn param/return). `struct Buf { text: String, .. }`'s field lowered to `pub
//!   text: TString` but the per-item sub-program did not EMIT the wrapper def
//!   (`error[E0425]: cannot find type TString`) -> L0. The fix extends
//!   `thermite-lower::lower::program_uses_string` to scan struct/enum field types
//!   and fn-local `let` annotations (the whole String-reachability class), and
//!   rewrites a `String` FIELD receiver's `.len()`/`.byte_at(i)` to the wrapper SPEC
//!   fns in spec position (the fn-signature `Ctx::string_fields` + the struct-`inv`
//!   `lower_inv_expr` MethodCall arm for `inv cursor <= text.len()`).
//!
//! These run the BUILT `forge` binary end-to-end (real verus). If verus is absent
//! they SKIP LOUDLY (never panic on a missing solver), matching
//! `divergence_strings.rs`.
//!
//! R-CHAR-3: expected levels trace to `.design/basis/07-strings.md` REQ-4 (the
//! bounded `slice` `ens result.len() == hi - lo`, the `concat` length identity, the
//! no-OOB `byte_at`, the `well_formed` capacity invariant) and `thermite-design.md`
//! §6 ladder semantics (L3 == a fully-discharged real-verus proof; L0 == an
//! undischarged obligation), NEVER copied from forge's own output. The negative
//! (insufficient-bound slice -> NOT laundered to L3) pins non-vacuity (R-DEFER-9).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `divergence_strings.rs`).
fn verus_present() -> bool {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return true;
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty() {
            return true;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if PathBuf::from(home).join(".local/bin/verus").exists() {
            return true;
        }
    }
    false
}

/// Write `program` to a temp `.th`, `forge check --json` it, return the cert array.
/// The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_str_l3_{tag}_{}_{}.th",
        std::process::id(),
        // a per-call discriminator so concurrent tests never collide on the path
        // (DETERMINISTIC within a test — the tag — but unique across tests).
        tag.len()
    ));
    std::fs::write(&fixture, program).expect("write fixture");
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(&fixture)
        .arg("--json")
        .output()
        .expect("spawn forge");
    let _ = std::fs::remove_file(&fixture);
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| {
            panic!(
                "forge --json stdout must be one JSON doc: {e}\nstdout:\n{stdout}\nstderr:\n{}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
        .as_array()
        .expect("array of certs")
        .clone()
}

fn cert_for<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no cert for `{item}` in {certs:?}"))
}

/// GAP 1 — a `String` `slice(lo, hi)` whose `hi` is a `u64` parameter certifies
/// L3: the exec arg lowering coerces `k as usize` for the `usize` accessor (was
/// `error[E0308]: expected usize, found u64` -> L0).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-4 — the bounded `slice` lowers to
/// `req self.well_formed() && lo <= hi && hi <= len, ens result.len() == hi - lo`.
/// The `req s.len() <= 1_000_000` establishes `s.well_formed()` (the CAP bound, the
/// SAME headroom `join`'s `req` establishes for `concat`'s `well_formed`); `k <=
/// s.len()` discharges `hi <= len`. `thermite-design.md` §6: a fully-discharged
/// verus proof is L3. The `fx alloc` is the constructing slice copy (REQ-4).
#[test]
fn gap1_slice_u64_arg_coerces_and_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — slice u64 coercion not exercised.");
        return;
    }
    let certs = check_program(
        "slice",
        "fn pre(s: String, k: u64) -> String\n  req s.len() <= 1_000_000 && k <= s.len()\n  ens result.len() == k\n  fx alloc\n{ s.slice(0, k) }\n",
    );
    let pre = cert_for(&certs, "pre");
    assert_eq!(
        pre["level"], "L3",
        "DESIGN 07-strings.md REQ-4: slice(0, k) with k: u64 must coerce `k as usize` \
         for the usize accessor and certify L3 (ens result.len() == k by slice's \
         `result.len() == hi - lo`). A missing coercion was `error[E0308]: expected \
         usize, found u64` -> L0. forge reports: {}",
        pre["level"]
    );
    assert_eq!(
        pre["effects"],
        serde_json::json!(["alloc"]),
        "DESIGN REQ-4: a constructing slice copy carries fx alloc."
    );
}

/// GAP 1 (the editor op the gap blocked) — a bounded MID-STRING INSERT via
/// slice+concat certifies L3. `s.slice(0, p).concat(ins).concat(s.slice(p,
/// s.len()))`: the `s.len()` arg is a non-literal `u64`, so it MUST coerce `as
/// usize` for the second `slice` (the GAP-1 fix applied to the realistic editor
/// path, not just the single triggering site).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-4 — `slice`'s `ens result.len() ==
/// hi - lo` + `concat`'s `ens result.len() == a.len() + b.len()` compose to
/// `result.len() == p + ins.len() + (s.len() - p) == s.len() + ins.len()`. The `req
/// s.len() + ins.len() <= 1_000_000` keeps every intermediate `concat` under CAP.
#[test]
fn gap1_mid_string_insert_via_slice_concat_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — insert-via-slice+concat not exercised.");
        return;
    }
    let certs = check_program(
        "insert",
        "fn insert(s: String, ins: String, p: u64) -> String\n  req s.len() + ins.len() <= 1_000_000 && p <= s.len()\n  ens result.len() == s.len() + ins.len()\n  fx alloc\n{ s.slice(0, p).concat(ins).concat(s.slice(p, s.len())) }\n",
    );
    let insert = cert_for(&certs, "insert");
    assert_eq!(
        insert["level"], "L3",
        "DESIGN REQ-4: the bounded mid-string insert (slice+concat+concat) certifies \
         L3 — slice's length identity + concat's length identity compose to \
         result.len() == s.len() + ins.len(); the `s.len()` (u64) slice arg coerces \
         `as usize`. forge reports: {}",
        insert["level"]
    );
}

/// GAP 2 — a `struct Buf { text: String, cursor: u64 }` with a String-FIELD
/// type-invariant (`inv cursor <= text.len()`) and a constructing `fn mk(t: String)
/// -> Buf` both certify L3: the `TString` wrapper def is woven into the per-item
/// sub-program because `String` is REACHABLE as a struct field type (was
/// `error[E0425]: cannot find type TString` -> L0), and the inv's `text.len()`
/// rewrites to the wrapper spec fn `self.text.spec_len()`.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-2 (a `String` struct field; the
/// `TString` wrapper keyed on the node kind) + REQ-4 (the `well_formed` capacity
/// invariant, the `spec_len` spec accessor a contract names) + `.design/basis/01-
/// adts.md` REQ-8 (the struct type-invariant `well_formed` predicate). The `mk` ens
/// `result.text.len() == t.len()` reads the String field's spec length.
#[test]
fn gap2_buf_struct_with_string_field_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — Buf String-field weave not exercised.");
        return;
    }
    let certs = check_program(
        "buf",
        "struct Buf { text: String, cursor: u64 }\n  inv cursor <= text.len()\n\nfn mk(t: String) -> Buf\n  req t.len() <= 1_000_000\n  ens result.text.len() == t.len()\n  fx alloc\n{ Buf { text: t, cursor: 0 } }\n",
    );
    let buf = cert_for(&certs, "Buf");
    assert_eq!(
        buf["level"], "L3",
        "DESIGN REQ-2/REQ-4 + 01-adts REQ-8: the Buf struct (String field + inv \
         cursor <= text.len()) certifies L3 once the TString wrapper is woven into \
         the per-item sub-program (String reachable as a field type) and \
         text.len() rewrites to text.spec_len(). Was `cannot find type TString` -> \
         L0. forge reports: {}",
        buf["level"]
    );
    let mk = cert_for(&certs, "mk");
    assert_eq!(
        mk["level"], "L3",
        "DESIGN REQ-2/REQ-4: the constructing `mk(t: String) -> Buf` certifies L3 — \
         the woven TString wrapper + the result.text.len() == t.len() field-length \
         identity. forge reports: {}",
        mk["level"]
    );
    assert_eq!(
        mk["effects"],
        serde_json::json!(["alloc"]),
        "DESIGN REQ-4: constructing a Buf owning a moved String carries fx alloc."
    );
}

/// GAP 2 (the second reachable form) — a `fn` READING `b.text.len()` from a `&Buf`
/// parameter certifies L3: the String-FIELD receiver `b.text`'s `.len()` rewrites
/// to `b.text.spec_len()` in the `ens` contract (the field analog of the bare
/// `String`-value rewrite), and the wrapper is woven because `Buf`'s field reaches
/// `String`.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-4 — a contract over a `String`
/// names the wrapper SPEC fn (`spec_len`), the exec `len` cannot be named in a
/// contract; `.design/basis/01-adts.md` REQ-8 — `b.well_formed()` is woven for the
/// invariant-bearing `Buf` param.
#[test]
fn gap2_fn_reading_string_field_len_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — String-field read not exercised.");
        return;
    }
    let certs = check_program(
        "readbuf",
        "struct Buf { text: String, cursor: u64 }\n  inv cursor <= text.len()\n\nfn buf_len(b: &Buf) -> u64\n  req b.text.len() <= 1_000_000\n  ens result == b.text.len()\n  fx pure\n{ b.text.len() }\n",
    );
    let buf_len = cert_for(&certs, "buf_len");
    assert_eq!(
        buf_len["level"], "L3",
        "DESIGN REQ-4: reading b.text.len() from a &Buf param certifies L3 — the \
         String-field receiver's .len() rewrites to .spec_len() in the contract. \
         forge reports: {}",
        buf_len["level"]
    );
    assert_eq!(
        buf_len["effects"],
        serde_json::json!(["pure"]),
        "DESIGN REQ-4: a read-only String-field len() is pure."
    );
}

/// NON-VACUITY (R-DEFER-9) — the GAP-1 coercion does NOT launder an unsound slice:
/// `slice`'s `req self.well_formed() && lo <= hi && hi <= len` is load-bearing. A
/// contract that does NOT establish `s.well_formed()` (no CAP bound on `s.len()`)
/// leaves `slice`'s `self.well_formed()` precondition undischarged -> verus FAILS
/// -> L0. The `as usize` coercion fixes the TYPE mismatch only; it never weakens
/// the bound (the same way `byte_at`'s `i < len` stays load-bearing).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-4 (slice requires
/// `self.well_formed()`) + AC-4 / R-DEFER-9 (a missing bound is caught, not
/// laundered). `thermite-design.md` §7 (the battery catches vacuity).
#[test]
fn gap1_slice_precondition_is_load_bearing() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — slice non-vacuity not exercised.");
        return;
    }
    // No `req s.len() <= 1_000_000` — so `s.well_formed()` (data.len() <= CAP) is
    // NOT establishable -> slice's `self.well_formed()` precondition undischarged.
    let certs = check_program(
        "slice_unbounded",
        "fn bad(s: String, k: u64) -> String\n  req k <= s.len()\n  ens result.len() == k\n  fx alloc\n{ s.slice(0, k) }\n",
    );
    let bad = cert_for(&certs, "bad");
    assert_eq!(
        bad["level"], "L0",
        "R-DEFER-9 non-vacuity: without a CAP bound `s.well_formed()` is unprovable, \
         so slice's `self.well_formed()` precondition is undischarged -> L0 (the \
         coercion fixes the TYPE, never the bound). forge reports: {}",
        bad["level"]
    );
}
