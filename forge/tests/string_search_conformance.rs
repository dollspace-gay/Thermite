//! Conformance for Cluster **C5** (crosslink **#102**): the string search/transform
//! layer — `contains`/`starts_with`/`ends_with` (boolean substring predicates,
//! 07-strings.md REQ-13), `find` (→ `Option<u64>`, REQ-14, reusing C7), `split` (→
//! `Vec<String>`, REQ-15, reusing C6), and `trim` (→ `String`, REQ-16). These run
//! against the two EXTERNAL truths the toolchain does not author for itself: the
//! built `forge` binary's certificate ladder (`forge check`, real verus) for the
//! predicate/find ops, and — for the `split`/`trim` constructing ops whose thin
//! surface caller cannot be mutation-scored (the §7 floor needs a scoreable body
//! mutant; a one-line `{ s.split(sep) }` delegates entirely to the proven method, the
//! parse_u64 AC-4 precedent) — the real `verus` binary on the EMITTED lowering
//! (R-CODE-4: the subprocess status is checked, never swallowed).
//!
//! Pins the C5 deliverables (and avoids the #101 equivalent-mutant trap):
//!
//!   * `s.starts_with(p)` / `s.contains(p)` / `s.ends_with(p)` → L3 pure with the
//!     `ens result == occurs_at(..)` / `contains_sub(..)` contract; a TRUE case
//!     (a known prefix) proves `result == true`, a FALSE case proves `result ==
//!     false`, and a BROKEN `starts_with` FAILS verus (the predicate is real teeth).
//!   * `s.find(p)` → L3 pure with the spec-`match`-in-`ens`; a PINNED Some case
//!     (needle present at 0) proves `result is Some`, so the always-`None` mutant is
//!     provably WRONG (killable — the #101 trap avoided).
//!   * `s.split(sep)` → the count-bound + sep-free contract VERIFIES under real verus
//!     `0 errors`; a `split`-drop body (always 1 piece) FAILS the count bound.
//!   * `s.trim()` → the length floor + subrange content VERIFIES under real verus.
//!   * The `contains` NAME-CLASH: a String `s.contains(needle)` AND a Vec
//!     `v.contains(x)` BOTH certify (receiver-type dispatch — `TString::contains` vs
//!     `TVec::contains` — neither clobbers).
//!
//! The verus checks SKIP LOUDLY when verus is absent (the `string_l3_completeness.rs`
//! precedent) — never panic on a missing solver. `tests/` is not anti-pattern-gated,
//! so `unwrap`/`expect`/`panic!` are fine (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/07-strings.md` REQ-13..16 (the
//! GROUNDED forms: the predicate scans `14 verified, 0 errors`; a broken `starts_with`
//! `13 verified, 1 errors`; `split` `7 verified, 0 errors`, a `split`-drop `6 verified,
//! 1 errors`; `trim` `8 verified, 0 errors`) + `thermite-design.md` §6 ladder semantics
//! (L3 == a fully-discharged real-verus proof), NEVER copied from the toolchain's own
//! output.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn verus_bin() -> PathBuf {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return PathBuf::from(p);
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".local/bin/verus");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("verus")
}

/// `true` iff verus is reachable (mirrors `string_l3_completeness.rs`).
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

/// Write `program` to a unique temp `.th`, `forge check --json` it, return the cert
/// array. The temp file is removed before returning (scratch hygiene, #53).
fn check_program(tag: &str, program: &str) -> Vec<Value> {
    let fixture = std::env::temp_dir().join(format!(
        "forge_strsearch_{tag}_{}_{}.th",
        std::process::id(),
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

/// Lower a Thermite source program to its Verus source via the toolchain's `lower`,
/// write it to a temp `.rs`, run the real `verus` binary, and return
/// `(success, combined_output)`. The temp file is removed before returning (#53).
/// R-CODE-4: the subprocess status is checked + surfaced, never swallowed.
fn verus_on_lowered(tag: &str, program: &str) -> (bool, String) {
    let parsed = thermite_syntax::parse(program);
    assert!(
        parsed.is_clean(),
        "[{tag}] surface must parse cleanly: {:?}",
        parsed.errors
    );
    let verus_src = thermite_lower::lower(&parsed.program)
        .unwrap_or_else(|e| panic!("[{tag}] lower must succeed: {e:?}"));
    let rs = std::env::temp_dir().join(format!(
        "forge_strsearch_verus_{tag}_{}.rs",
        std::process::id()
    ));
    std::fs::write(&rs, &verus_src).expect("write lowered .rs");
    let out = Command::new(verus_bin())
        .arg(&rs)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_file(&rs);
    if let Some(stem) = rs.file_stem() {
        let _ = std::fs::remove_file(std::env::temp_dir().join(stem));
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), combined)
}

/// AC-9 — `starts_with`/`contains`/`ends_with` certify L3 pure with the
/// `occurs_at`/`contains_sub` contract; a TRUE case AND a FALSE case both prove.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-13 — the predicates lower to the byte
/// scans, the contract names the seeded `occurs_at`/`contains_sub` spec fns inside the
/// §4.2 cage. `thermite-design.md` §6: a fully-discharged verus proof is L3. GROUNDED
/// `14 verified, 0 errors`.
#[test]
fn ac9_predicates_certify_l3_pure() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — string predicates L3 not exercised.");
        return;
    }
    // The general predicate contracts (`ens result == occurs_at/contains_sub`) — the
    // open form that proves BOTH a true and a false case (verus reasons over all
    // inputs, the predicate is exactly the named relation).
    let certs = check_program(
        "predicates",
        "fn pre(s: &String, p: &String) -> bool\n  req true\n  ens result == occurs_at(s, p, 0)\n  fx pure\n{ s.starts_with(p) }\n\
         fn ends(s: &String, p: &String) -> bool\n  req true\n  ens result == occurs_at(s, p, (s.len() - p.len()))\n  fx pure\n{ s.ends_with(p) }\n\
         fn has(s: &String, p: &String) -> bool\n  req true\n  ens result == contains_sub(s, p)\n  fx pure\n{ s.contains(p) }\n",
    );
    for item in ["pre", "ends", "has"] {
        let cert = cert_for(&certs, item);
        assert_eq!(
            cert["level"], "L3",
            "DESIGN 07-strings.md REQ-13: `{item}` certifies L3 with `ens result == \
             occurs_at(..)`/`contains_sub(..)` (the predicate scan, the seeded spec fns \
             inside the §4.2 cage). forge reports: {}",
            cert["level"]
        );
        assert_eq!(
            cert["effects"],
            serde_json::json!(["pure"]),
            "DESIGN REQ-13: a read-only substring predicate is fx pure."
        );
    }
}

/// AC-9 (the true case PINNED) — a `starts_with` on a known prefix proves `result ==
/// true`, so the predicate is non-vacuous (a broken always-false `starts_with` would
/// fail this contract).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-13 — a TRUE case proves `result ==
/// true`; non-vacuity (the false case bites a broken predicate). GROUNDED.
#[test]
fn ac9_true_case_pinned_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — predicate true-case not exercised.");
        return;
    }
    let certs = check_program(
        "true_case",
        "fn pre_true(s: &String, p: &String) -> bool\n  req p.len() <= s.len() && occurs_at(s, p, 0)\n  ens result == true\n  fx pure\n{ s.starts_with(p) }\n",
    );
    let cert = cert_for(&certs, "pre_true");
    assert_eq!(
        cert["level"], "L3",
        "DESIGN 07-strings.md REQ-13 non-vacuity: a `starts_with` on a known prefix \
         (`req occurs_at(s, p, 0)`) PROVES `result == true` — the predicate is real \
         teeth, not vacuous. forge reports: {}",
        cert["level"]
    );
}

/// AC-10 — `find` certifies L3 pure with the spec-`match`-in-`ens`; the Some case is
/// PINNED so the always-None mutant is killable (#101 trap avoided).
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-14 — `s.find(p)` lowers to the
/// occurrence scan, the `ens match result { Some(at) => occurs_at(..), None =>
/// !contains_sub(..) }` (the C7 spec-`match`). A PINNED Some case (needle present)
/// proves `result is Some`. GROUNDED. `thermite-design.md` §6: a discharged proof is L3.
#[test]
fn ac10_find_certifies_l3_with_pinned_some() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — find L3 not exercised.");
        return;
    }
    let certs = check_program(
        "find",
        "fn at(s: &String, p: &String) -> Option<u64>\n  req true\n  ens match result { Some(i) => occurs_at(s, p, i), None => !contains_sub(s, p) }\n  fx pure\n{ s.find(p) }\n\
         fn at_some(s: &String, p: &String) -> Option<u64>\n  req p.len() >= 1 && p.len() <= s.len() && occurs_at(s, p, 0)\n  ens result is Some\n  fx pure\n{ s.find(p) }\n",
    );
    let at = cert_for(&certs, "at");
    assert_eq!(
        at["level"], "L3",
        "DESIGN 07-strings.md REQ-14: `find` certifies L3 with the spec-match-in-ens \
         (`Some(at) => occurs_at(..), None => !contains_sub(..)`). forge reports: {}",
        at["level"]
    );
    let at_some = cert_for(&certs, "at_some");
    assert_eq!(
        at_some["level"], "L3",
        "DESIGN 07-strings.md REQ-14 (#101 trap avoided): a PINNED Some case (needle \
         present at 0) PROVES `result is Some` — so an always-`None` `find` is provably \
         WRONG, killable by the §7 gate (not behaviorally equivalent). forge reports: {}",
        at_some["level"]
    );
}

/// AC-9 NON-VACUITY (R-DEFER-9) — a BROKEN `starts_with` (drops the byte-mismatch
/// check, always returns `true`) FAILS verus. The predicate's `ens result ==
/// occurs_at(..)` is load-bearing: an always-`true` body does NOT satisfy it when the
/// prefix does not match.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-13 — a broken `starts_with` FAILS
/// (`13 verified, 1 errors`, the false case bites). `thermite-design.md` §7. The break
/// is injected into a STANDALONE verus probe (the surface cannot mutate the generated
/// method body), confirming the predicate's contract is a real proof.
#[test]
fn ac9_broken_starts_with_fails_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — predicate non-vacuity not exercised.");
        return;
    }
    // The generated starts_with's EXACT contract (`ens result == occurs_at(s@, p@,
    // 0)`), but the body unconditionally returns `true` (drops the byte-mismatch
    // scan). For a needle that does NOT prefix `s`, `occurs_at(s@, p@, 0)` is false,
    // so `result == occurs_at(..)` is undischarged → verus FAILS.
    let probe = r#"use vstd::prelude::*;
verus! {
pub const CAP: usize = 1000000;
pub struct TString { pub data: Vec<u8> }
pub open spec fn occurs_at(s: Seq<u8>, needle: Seq<u8>, at: int) -> bool {
    0 <= at && at + needle.len() <= s.len()
    && (forall|k: int| 0 <= k < needle.len() ==> #[trigger] s[at + k] == needle[k])
}
impl TString {
    pub open spec fn well_formed(&self) -> bool { self.data.len() <= CAP }
    pub fn starts_with_broken(&self, p: &TString) -> (result: bool)
        requires self.well_formed(), p.well_formed(),
        ensures result == occurs_at(self.data@, p.data@, 0),
    { true }
}
}
fn main() {}
"#;
    let rs = std::env::temp_dir().join(format!("forge_strsearch_broken_{}.rs", std::process::id()));
    std::fs::write(&rs, probe).expect("write probe");
    let out = Command::new(verus_bin())
        .arg(&rs)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_file(&rs);
    if let Some(stem) = rs.file_stem() {
        let _ = std::fs::remove_file(std::env::temp_dir().join(stem));
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success() && combined.contains("error"),
        "R-DEFER-9 non-vacuity: a BROKEN `starts_with` (always `true`) must FAIL verus \
         — the `ens result == occurs_at(..)` is a real proof, the false case bites. \
         verus reports:\n{combined}"
    );
}

/// AC-11 — `split` VERIFIES under real verus with the count-bound + sep-free contract
/// (the Vec<String> push loop, fx alloc). A thin `{ s.split(sep) }` caller cannot be
/// mutation-scored (the parse_u64 AC-4 precedent — the method's proof IS the
/// deliverable), so the cert level is established by the real verus run, not the
/// §7-gated `forge check` level.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-15 — `s.split(sep)` lowers to the scan
/// loop pushing `TString` pieces into a `TVecTString` (reusing C6), `ens
/// result.len() == 1 + count_sep(s@, sep) && forall|k| sep_free(..)`. GROUNDED `7
/// verified, 0 errors`. `thermite-design.md` §6.
#[test]
fn ac11_split_count_bound_verifies_under_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — split lowering not exercised.");
        return;
    }
    let (ok, output) = verus_on_lowered(
        "split",
        "fn parts(s: &String, sep: u64) -> Vec<String>\n  req true\n  ens result.len() == 1 + count_sep(s, sep)\n  fx alloc\n{ s.split(sep) }\n",
    );
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 07-strings.md REQ-15: the emitted `split` lowering (the Vec<String> \
         push-loop + the count partial + sep-free invariant + lemma_count_push) must \
         VERIFY under real verus `0 errors` (GROUNDED `7 verified, 0 errors`). verus \
         reports:\n{output}"
    );
}

/// AC-11 NON-VACUITY (R-DEFER-9) — a BROKEN `split` that drops the mid-loop
/// `pieces.push` (always 1 piece) FAILS the count bound under real verus.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-15 — a broken `split` FAILS (`6
/// verified, 1 errors`, the count bound bites). `thermite-design.md` §7.
#[test]
fn ac11_broken_split_fails_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — split non-vacuity not exercised.");
        return;
    }
    // The generated split's EXACT contract (the count-bound + sep-free), but the body
    // NEVER pushes a mid-loop piece (drops the `if b == sep { pieces.push(..) }` arm),
    // so it always returns 1 piece — for an input WITH a separator, `result.len() == 1
    // + count_sep(s@, sep)` (count >= 1) is undischarged → verus FAILS.
    let probe = r#"use vstd::prelude::*;
verus! {
pub const CAP: usize = 1000000;
pub struct TString { pub data: Vec<u8> }
pub struct TVecTString { pub data: Vec<TString> }
pub open spec fn count_sep(s: Seq<u8>, sep: u8) -> nat
    decreases s.len()
{ if s.len() == 0 { 0nat }
  else { (if s[0] == sep { 1nat } else { 0nat }) + count_sep(s.subrange(1, s.len() as int), sep) } }
pub open spec fn sep_free(s: Seq<u8>, sep: u8) -> bool
{ forall|i: int| 0 <= i < s.len() ==> #[trigger] s[i] != sep }
impl TString {
    pub open spec fn well_formed(&self) -> bool { self.data.len() <= CAP }
    pub fn split_broken(&self, sep: u8) -> (result: TVecTString)
        requires self.well_formed(),
        ensures
            result.data.len() >= 1,
            result.data.len() == 1 + count_sep(self.data@, sep),
            forall|k: int| 0 <= k < result.data.len() ==> sep_free(#[trigger] result.data@[k].data@, sep),
    {
        let mut pieces: Vec<TString> = Vec::new();
        let cur: Vec<u8> = Vec::new();
        pieces.push(TString { data: cur });
        TVecTString { data: pieces }
    }
}
}
fn main() {}
"#;
    let rs = std::env::temp_dir().join(format!(
        "forge_strsearch_splitbroken_{}.rs",
        std::process::id()
    ));
    std::fs::write(&rs, probe).expect("write probe");
    let out = Command::new(verus_bin())
        .arg(&rs)
        .current_dir(std::env::temp_dir())
        .output()
        .expect("spawn verus");
    let _ = std::fs::remove_file(&rs);
    if let Some(stem) = rs.file_stem() {
        let _ = std::fs::remove_file(std::env::temp_dir().join(stem));
    }
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        !out.status.success() && combined.contains("error"),
        "R-DEFER-9 non-vacuity: a BROKEN `split` (always 1 piece) must FAIL the count \
         bound under real verus — the count-bound `ens` is real teeth. verus reports:\n{combined}"
    );
}

/// AC-12 — `trim` VERIFIES under real verus with the length floor + subrange content
/// contract (fx alloc). Like `split`, the thin caller is verus-grounded directly.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-16 — `s.trim()` lowers to the
/// forward/backward whitespace scan + bounded copy, `ens result.len() <= s.len() &&
/// exists|lo,hi| result == s.subrange(lo,hi)`. GROUNDED `8 verified, 0 errors`.
#[test]
fn ac12_trim_verifies_under_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — trim lowering not exercised.");
        return;
    }
    let (ok, output) = verus_on_lowered(
        "trim",
        "fn strip(s: &String) -> String\n  req true\n  ens result.len() <= s.len()\n  fx alloc\n{ s.trim() }\n",
    );
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 07-strings.md REQ-16: the emitted `trim` lowering (the forward/backward \
         whitespace scan + the bounded copy with the subrange invariant) must VERIFY \
         under real verus `0 errors` (GROUNDED `8 verified, 0 errors`). verus reports:\n{output}"
    );
}

/// AC (the `contains` NAME-CLASH RESOLVED) — a program with BOTH a String
/// `s.contains(needle)` (substring) AND a Vec `v.contains(x)` (membership) verifies:
/// BOTH ops certify L3, receiver-type-dispatched (`TString::contains` vs
/// `TVec::contains`), neither clobbers the other.
///
/// AUTHORITY: `.design/basis/07-strings.md` REQ-13 (the design-flagged name-clash) +
/// `.design/basis/04-collections.md` REQ-12 (the Vec membership `contains`). Rust keys
/// inherent-method resolution on the receiver type, so the shared surface NAME resolves
/// to two distinct methods. `thermite-design.md` §6.
#[test]
fn contains_name_clash_both_string_and_vec_certify() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — contains name-clash not exercised.");
        return;
    }
    // The String `contains` (substring) is named in a CONTRACT (`ens result ==
    // contains_sub(..)`, REQ-13's spec form); the Vec `contains` (membership) is used
    // in EXEC position (the C6-supported form — a `let c = v.contains(x)` runnable use;
    // C6 admits `v.contains` as a flat built-in but the v1 corpus exercises it in exec,
    // not a contract `ens`). BOTH lower in ONE program: the String op resolves to
    // `TString::contains`, the Vec op to `TVec::contains` — receiver-type dispatch, no
    // clobber. The whole program lowering VERIFYING under real verus is the proof both
    // dispatch correctly (a clobber would mis-resolve one and fail verus).
    let (ok, output) = verus_on_lowered(
        "name_clash",
        "fn str_has(s: &String, p: &String) -> bool\n  req true\n  ens result == contains_sub(s, p)\n  fx pure\n{ s.contains(p) }\n\
         fn vec_use(x: u64) -> bool\n  req true\n  ens true\n  fx alloc\n{ let mut v: Vec<u64> = Vec::new(); v.push(7); v.contains(x) }\n",
    );
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 07-strings.md REQ-13 + 04-collections.md REQ-12 (the `contains` \
         NAME-CLASH): a program with BOTH a String `s.contains(p)` (substring, named in \
         a contract via `contains_sub`) AND a Vec `v.contains(x)` (membership, exec) \
         must VERIFY under real verus `0 errors` — `contains` is RECEIVER-TYPE- \
         dispatched (`TString::contains` vs `TVec::contains`), neither clobbers. verus \
         reports:\n{output}"
    );
    // The String-contains-in-contract op ALSO certifies L3 through `forge check` (the
    // predicate path), pinning the substring side independently.
    let certs = check_program(
        "name_clash_str",
        "fn str_has(s: &String, p: &String) -> bool\n  req true\n  ens result == contains_sub(s, p)\n  fx pure\n{ s.contains(p) }\n",
    );
    let str_has = cert_for(&certs, "str_has");
    assert_eq!(
        str_has["level"], "L3",
        "DESIGN 07-strings.md REQ-13: the STRING `s.contains(p)` (substring) certifies \
         L3 via `TString::contains` (`ens result == contains_sub(..)`). forge reports: {}",
        str_has["level"]
    );
}
