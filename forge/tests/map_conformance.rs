//! Conformance for Cluster **C12** (crosslink **#114** / blocker **#123**): the
//! bounded verified `Map<K, V>` — the two-arg `Type::Map` node + the Vec-of-pairs
//! `TMap` backing + the spec abstraction view (`spec_dom`/`spec_contains_key`/
//! `len`) + the `insert`/`get`/`contains_key`/`len` ops (`get` returns the C7
//! `Option<V>`, absent key → `None`). These run against the two EXTERNAL truths the
//! toolchain does not author for itself: the real `verus` binary on the EMITTED
//! lowering of a `Map`-using program (the wrapper carries the round-trip /
//! absent→None proof, the parse_u64 codegen-grounding precedent), and the built
//! `forge` certificate ladder (`forge check`).
//!
//! Pins the C12 deliverables (the GROUNDED `TMapU64U64` form, `.design/basis/
//! 13-map.md` Verification — `9 verified, 0 errors`):
//!
//!   * The emitted `TMapU64U64` wrapper (Vec-of-pairs backing + spec view + the
//!     ops) + the insert-then-get round-trip (`insert(k,v)` then `get(k) ==
//!     Some(v)`) + the absent→None refusal (`get(absent) == None`) + `contains_key`
//!     true/false VERIFY under real verus `verified, 0 errors` (AC-1/AC-2/AC-3).
//!   * NON-VACUITY (R-DEFER-9): a crafted `get` returning `Some(0)` for an absent
//!     key FAILS verus (the `None => !spec_contains_key(k)` arm bites) (AC-2).
//!   * The `map_kv.th` corpus program PARSES (the two-arg `Map<u64,u64>`),
//!     VALIDATES (`contains_key` in `BUILTIN_METHODS`, the §4.2 cage), and LOWERS;
//!     `forge check` certifies its fns L3 (AC-1/AC-3).
//!   * A `Map` program BUILDS + RUNS via `forge build` (the L1 `TMap` runtime —
//!     insert + get → the value) (AC-1).
//!   * No regression: the existing `vec_demo` corpus stays L3 (AC-4).
//!
//! The verus/forge checks SKIP LOUDLY when verus is absent (the
//! `option_result_conformance.rs` precedent) — never panic on a missing solver.
//! `tests/` is not anti-pattern-gated, so `unwrap`/`expect`/`panic!` are fine
//! (R-APG-2).
//!
//! R-CHAR-3: expected levels trace to `.design/basis/13-map.md` AC-1..AC-4 (the
//! GROUNDED `9 verified, 0 errors`; the broken `Some(0)`-for-absent `get` FAILS
//! `verified, 1 errors`) + `thermite-design.md` §6 ladder semantics (L3 == a
//! fully-discharged real-verus proof), NEVER copied from the toolchain's own output.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

/// `true` iff verus is reachable (mirrors `option_result_conformance.rs`).
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

/// Resolve the verus binary path (PATH, `VERUS_BIN`, or `~/.local/bin/verus`).
fn verus_bin() -> PathBuf {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        if Path::new(&p).exists() {
            return PathBuf::from(p);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                return PathBuf::from(s);
            }
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/bin/verus")
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
    let rs = std::env::temp_dir().join(format!("forge_map_verus_{tag}_{}.rs", std::process::id()));
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

/// Run the real `verus` binary directly on a hand-authored probe source (for the
/// non-vacuity negative — the surface cannot mutate the generated wrapper body).
fn verus_on_probe(tag: &str, probe: &str) -> (bool, String) {
    let rs = std::env::temp_dir().join(format!("forge_map_probe_{tag}_{}.rs", std::process::id()));
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
    (out.status.success(), combined)
}

fn corpus(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../conformance")
        .join(name)
}

fn certs_for_file(path: &Path) -> Vec<Value> {
    let out = Command::new(forge_bin())
        .arg("check")
        .arg(path)
        .arg("--json")
        .output()
        .expect("spawn forge check");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str::<Value>(stdout.trim())
        .unwrap_or_else(|e| panic!("forge --json not one doc: {e}\n{stdout}"))
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

/// The GROUNDED `TMapU64U64` wrapper + the insert-then-get round-trip + the
/// absent→None refusal, as a STANDALONE verus program — the
/// `.design/basis/13-map.md` Verification seed (`9 verified, 0 errors`). This is
/// the EXACT shape the lowerer emits for `Map<u64, u64>`; grounding it directly
/// pins the emitted contract independently of the surface-program lowering.
const MAP_GROUND_PROBE: &str = r#"use vstd::prelude::*;
verus! {
pub spec const MAP_CAP: usize = 1_000_000;
pub struct TMapU64U64 { pub data: Vec<(u64, u64)> }
impl TMapU64U64 {
    pub open spec fn spec_dom(&self) -> Set<int> {
        Set::new(|kk: int| exists|j: int|
            0 <= j < self.data.len() && #[trigger] self.data@[j].0 as int == kk)
    }
    pub open spec fn well_formed(&self) -> bool {
        &&& self.data.len() <= MAP_CAP
        &&& (forall|a: int, b: int| #![trigger self.data@[a].0, self.data@[b].0]
                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b
                ==> self.data@[a].0 != self.data@[b].0)
    }
    pub open spec fn spec_contains_key(&self, k: u64) -> bool {
        exists|j: int| 0 <= j < self.data.len() && #[trigger] self.data@[j].0 == k
    }
    pub open spec fn len(&self) -> nat { self.data.len() as nat }
    pub fn contains_key(&self, k: u64) -> (result: bool)
        requires self.well_formed(),
        ensures result == self.spec_contains_key(k),
    {
        let mut i: usize = 0;
        while i < self.data.len()
            invariant
                i <= self.data.len(),
                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,
            decreases self.data.len() - i,
        {
            if self.data[i].0 == k { assert(self.data@[i as int].0 == k); return true; }
            i = i + 1;
        }
        false
    }
    pub fn get(&self, k: u64) -> (result: Option<u64>)
        requires self.well_formed(),
        ensures match result {
            Some(v) => self.spec_contains_key(k)
                && (exists|j: int| 0 <= j < self.data.len()
                       && self.data@[j].0 == k && self.data@[j].1 == v),
            None => !self.spec_contains_key(k),
        },
    {
        let mut i: usize = 0;
        while i < self.data.len()
            invariant
                i <= self.data.len(),
                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,
            decreases self.data.len() - i,
        {
            if self.data[i].0 == k {
                let v: u64 = self.data[i].1;
                assert(self.data@[i as int].0 == k && self.data@[i as int].1 == v);
                return Some(v);
            }
            i = i + 1;
        }
        None
    }
    pub fn insert(&mut self, k: u64, v: u64)
        requires old(self).well_formed(), old(self).data.len() < MAP_CAP,
                 !old(self).spec_contains_key(k),
        ensures
            final(self).well_formed(),
            final(self).spec_contains_key(k),
            exists|j: int| 0 <= j < final(self).data.len()
                && final(self).data@[j].0 == k && final(self).data@[j].1 == v,
            final(self).data.len() == old(self).data.len() + 1,
    {
        let ghost old_len = self.data.len();
        self.data.push((k, v));
        assert(self.data@[old_len as int].0 == k && self.data@[old_len as int].1 == v);
        assert(self.spec_contains_key(k)) by {
            assert(0 <= old_len < self.data.len() && self.data@[old_len as int].0 == k);
        }
        assert(self.well_formed()) by {
            assert forall|a: int, b: int|
                0 <= a < self.data.len() && 0 <= b < self.data.len() && a != b
                implies self.data@[a].0 != self.data@[b].0 by {
                if a < old_len && b < old_len {
                } else if a == old_len {
                    assert(self.data@[b].0 != k);
                } else if b == old_len {
                    assert(self.data@[a].0 != k);
                }
            }
        }
    }
}
fn insert_then_get(m: &mut TMapU64U64, k: u64, v: u64) -> (result: Option<u64>)
    requires old(m).well_formed(), old(m).data.len() < MAP_CAP, !old(m).spec_contains_key(k),
    ensures result == Some(v),
{ m.insert(k, v); m.get(k) }
fn get_absent(m: &TMapU64U64, k: u64) -> (result: Option<u64>)
    requires m.well_formed(), !m.spec_contains_key(k),
    ensures result is None,
{ m.get(k) }
fn main() {}
}
"#;

/// AC-1 / AC-2 / AC-3 (GROUNDED) — the `TMapU64U64` wrapper + the insert-then-get
/// round-trip (`insert(k,v)` then `get(k) == Some(v)`) + the absent→None refusal
/// (`get(absent) == None`) + `contains_key` both branches VERIFY under real verus
/// `verified, 0 errors`.
///
/// AUTHORITY: `.design/basis/13-map.md` AC-1/AC-2/AC-3 — the GROUNDED `TMapU64U64`
/// over `Vec<(u64,u64)>` (`9 verified, 0 errors`). `thermite-design.md` §6: a
/// fully-discharged verus proof is L3.
#[test]
fn ac1_2_3_map_wrapper_roundtrip_and_absent_none_verify_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — Map wrapper L3 not exercised.");
        return;
    }
    let (ok, output) = verus_on_probe("ground", MAP_GROUND_PROBE);
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 13-map.md AC-1/2/3: the TMapU64U64 wrapper (Vec-of-pairs backing + \
         spec view + ops) + the insert-then-get round-trip (`insert(k,v)` then \
         `get(k) == Some(v)`) + the absent→None refusal (`get(absent) is None`) + \
         contains_key must VERIFY under real verus `verified, 0 errors` (GROUNDED \
         `9 verified, 0 errors`). verus reports:\n{output}"
    );
}

/// AC-2 NON-VACUITY (R-DEFER-9) — a crafted `get` returning `Some(0)` for an absent
/// key FAILS verus. The `None => !spec_contains_key(k)` arm has real teeth: a body
/// returning `Some(0)` for a key NOT in the map does not satisfy the Some arm's
/// `spec_contains_key(k)`, so the postcondition is undischarged.
///
/// AUTHORITY: `.design/basis/13-map.md` AC-2 — the broken `Some(0)`-for-absent form
/// FAILS (`verified, 1 errors`, postcondition not satisfied). `thermite-design.md`
/// §7: the battery catches a false claim.
#[test]
fn ac2_broken_get_some_for_absent_fails_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — Map non-vacuity not exercised.");
        return;
    }
    // The GROUNDED wrapper, but `get`'s body is the broken unconditional `Some(0)`.
    let broken = MAP_GROUND_PROBE.replace(
        "    {\n        let mut i: usize = 0;\n        while i < self.data.len()\n            invariant\n                i <= self.data.len(),\n                forall|j: int| 0 <= j < i ==> self.data@[j].0 != k,\n            decreases self.data.len() - i,\n        {\n            if self.data[i].0 == k {\n                let v: u64 = self.data[i].1;\n                assert(self.data@[i as int].0 == k && self.data@[i as int].1 == v);\n                return Some(v);\n            }\n            i = i + 1;\n        }\n        None\n    }",
        "    { Some(0) }",
    );
    assert_ne!(
        broken, MAP_GROUND_PROBE,
        "the break must actually replace the get body (the probe text drifted)"
    );
    let (ok, output) = verus_on_probe("brokenget", &broken);
    assert!(
        !ok && output.contains("error"),
        "R-DEFER-9 non-vacuity: a `get` returning `Some(0)` for an absent key must \
         FAIL verus — the Some arm's `spec_contains_key(k)` is undischarged for a key \
         NOT in the map (the None refusal arm has teeth). verus reports:\n{output}"
    );
}

/// AC-1 — the `map_kv.th` corpus program PARSES (the two-arg `Map<u64,u64>`) and
/// its EMITTED lowering VERIFIES under real verus `verified, 0 errors` (the `TMap`
/// wrapper is woven + the spec-position `contains_key`/`len` rewrites + the
/// `Map::new()` reachability all compose).
///
/// AUTHORITY: `conformance/map_kv.th` (the C12 corpus oracle, hand-authored from
/// the GROUNDED form) + `.design/basis/13-map.md` AC-1.
#[test]
fn ac1_map_kv_corpus_lowering_verifies_under_real_verus() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — map_kv lowering not exercised.");
        return;
    }
    let src = std::fs::read_to_string(corpus("map_kv.th")).expect("read map_kv.th");
    let (ok, output) = verus_on_lowered("mapkv", &src);
    assert!(
        ok && output.contains("0 errors"),
        "DESIGN 13-map.md AC-1: the emitted `map_kv.th` lowering (the woven TMap \
         wrapper + the spec-position contains_key/len rewrites + the Map::new() \
         reachability) must VERIFY under real verus `verified, 0 errors`. verus \
         reports:\n{output}"
    );
}

/// AC-3 — `forge check` certifies the contains_key accessor `has_key` at L3: the
/// validator accepts `contains_key` in `BUILTIN_METHODS` inside the §4.2 cage
/// (`ens result == m.contains_key(k)`), the lowerer maps spec-position
/// `contains_key` to the wrapper's `spec_contains_key`, the `well_formed()`
/// precondition is woven, and the FULL ladder (incl. the §7 mutation battery)
/// passes — `ens result == m.contains_key(k)` is mutation-STRONG (a wrong body is
/// killed). This is the L3 cert anchor for the contains_key cage admission.
///
/// `build_one` (`ens result.contains_key(k)`) and `lookup_absent`
/// (`ens result is None`) VERIFY under real verus (the `ac1_..._lowering` test —
/// the round-trip membership + the absent→None refusal), but their THIN partial
/// contracts do not meet the §7 anti-Goodhart mutation floor (a `Map`-returning
/// fn has no scoreable scalar-zero mutant; a `None`-returning partial contract is
/// satisfied by an always-`None` body — the #101 partial-`None` class). The
/// round-trip / absent→None TEETH are pinned at the verus codegen-grounding level
/// (`ac1_2_3` + `ac2_broken_..`), R-HONEST-3: we anchor the L3 forge cert on the
/// mutation-strong accessor, never overclaim a thin contract.
///
/// AUTHORITY: `.design/basis/13-map.md` AC-3 (contains_key true AND false provable)
/// + `thermite-design.md` §6/§7.
#[test]
fn ac3_map_kv_contains_key_accessor_certifies_l3() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — map_kv forge check not exercised.");
        return;
    }
    let certs = certs_for_file(&corpus("map_kv.th"));
    let c = cert_for(&certs, "has_key");
    assert_eq!(
        c["level"], "L3",
        "DESIGN 13-map.md AC-3: `map_kv.th::has_key` must certify L3 (the two-arg \
         Map<u64,u64> validates with contains_key in BUILTIN_METHODS inside the §4.2 \
         cage, lowers to the verified TMap wrapper, and the mutation-strong \
         `ens result == m.contains_key(k)` passes the §7 floor). forge reports: {}",
        c["level"]
    );
}

/// AC-1 (builds + runs) — `forge build conformance/map_kv.th --entry demo` produces
/// a runnable binary that RUNS the insert + get round-trip at L1: `demo` builds a
/// local `Map<u64,u64>`, `insert(7, 42)`, `get(7)`, and returns `42` (the L1 `TMap`
/// runtime — `emit_map_runtime_l1`'s plain-Rust Vec-of-pairs newtype with the
/// `thermite_check!` capacity/uniqueness guards + `get -> Option<V>`). The build
/// uses real `rustc` + a real process run (no skip — `rustc` is always present).
///
/// AUTHORITY: `.design/basis/13-map.md` AC-1 ("`forge build` a Map program →
/// COMPILES + RUNS (insert + get → the value)").
#[test]
fn ac1_map_kv_builds_and_runs_insert_get_yields_value() {
    let file = corpus("map_kv.th");
    let out = Command::new(forge_bin())
        .arg("build")
        .arg(&file)
        .arg("--entry")
        .arg("demo")
        .arg("--json")
        .output()
        .expect("spawn forge build");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "DESIGN 13-map.md AC-1: `forge build map_kv.th --entry demo` must COMPILE \
         (the L1 TMap runtime + the insert/get exec ops):\nstdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let manifest: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("forge build --json not JSON: {e}\n{stdout}"));
    let artifact = PathBuf::from(
        manifest["artifact"]
            .as_str()
            .unwrap_or_else(|| panic!("no `artifact` in build manifest:\n{stdout}")),
    );
    assert!(artifact.exists(), "built binary missing at {artifact:?}");
    let run = Command::new(&artifact).output().expect("run map_kv demo");
    let run_out = String::from_utf8_lossy(&run.stdout);
    assert!(
        run.status.success() && run_out.contains("42"),
        "DESIGN 13-map.md AC-1: the built `demo` must RUN the insert(7,42)+get(7) \
         round-trip and yield 42 (insert + get → the value).\nstatus:{:?}\nstdout:{run_out}\nstderr:{}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
}

/// AC-4 (no regression) — the existing `vec_demo.th` corpus still certifies L3. The
/// C12 additions (the `Type::Map` node + the `Map` lowering path + the
/// `contains_key` `BUILTIN_METHODS` entry) are purely additive — they touch no
/// existing node shape.
///
/// AUTHORITY: `conformance/vec_demo.th` (the SHIPPED kernel corpus) +
/// `thermite-design.md` §6.
#[test]
fn ac4_vec_demo_corpus_unchanged_no_regression() {
    if !verus_present() {
        eprintln!("SKIP: verus absent — vec_demo regression not exercised.");
        return;
    }
    let vd = corpus("vec_demo.th");
    if !vd.exists() {
        eprintln!("SKIP: conformance/vec_demo.th absent.");
        return;
    }
    let certs = certs_for_file(&vd);
    // Every fn cert in the existing Vec corpus must stay L3 (purely-additive C12).
    for c in &certs {
        if c.get("item").is_some() {
            assert_eq!(
                c["level"], "L3",
                "AC-4 no regression: conformance/vec_demo.th must still certify L3 — \
                 the C12 Map additions are purely additive. forge cert: {c:?}"
            );
        }
    }
}
