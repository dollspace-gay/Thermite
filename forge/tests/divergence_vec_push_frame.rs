//! DIVERGENCE (critic, #98 C6 re-audit): the emitted bounded-`Vec` `push` ensures
//! clause OMITS the element-preservation frame that the design's GROUNDED `BVec`
//! seed mandates. As a result, after a SECOND `push`, the wrapper's contract no
//! longer lets a caller prove that `get(0)` is the FIRST pushed element — the
//! prior elements are unframed.
//!
//! AUTHORITY:
//!   - `.design/basis/04-collections.md` REQ-5: "`push` lowers to `self.data.push(x)`
//!     with `... ens final(self)...data@[old_len] == x` **plus the
//!     element-preservation frame**".
//!   - `.design/basis/04-collections.md` "The verified Verus form (GROUNDED)" — the
//!     `BVec::push` seed carries the element frame
//!     `forall|j: int| 0 <= j < old_len ==> final(self).data@[j] == old(self).data@[j]`.
//!   - `.design/basis/04-collections.md` REQ-9 borrow-`get` `ens *result ==
//!     self.data@[i as int]` — only meaningful if `data@[i]` is FRAMED across a
//!     later `push`.
//!
//! TOOLCHAIN DIVERGENCE: `emit_one_vec_wrapper` (`thermite-lower/src/lower.rs`)
//! emits `push` with ens `{ well_formed; len' == len+1; data@[old_len] == x }` and
//! NO `forall|j| ... data@[j] == old data@[j]` frame (the `pop_last` it emits DOES
//! carry a kept-prefix frame — `push` is the inconsistent one).
//!
//! THE FAILURE: the emitted `TVec*` wrapper, plus a value-pinning client that
//! pushes two distinct elements and asserts `get(0)` is the first, FAILS to verify
//! under the real `verus` binary (`assertion failed` on the first-element pin),
//! because the second `push` is not framed over index 0. WITH the design's frame
//! it verifies (confirmed by the critic probe). This is a contract WEAKER than the
//! design GROUNDED form (R-DEFER-9: an obligation the design states is silently
//! dropped). Tracking: crosslink blocker.
//!
//! Un-ignore when the fixer restores the element-preservation frame on `push`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn verus_bin() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("VERUS_BIN") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    if let Ok(out) = Command::new("which").arg("verus").output() {
        if out.status.success() {
            let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(PathBuf::from(p));
            }
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let cand = PathBuf::from(home).join(".local/bin/verus");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

fn run_verus(file: &Path) -> Option<(bool, String)> {
    let bin = verus_bin()?;
    let out = Command::new(bin)
        .arg("--no-cheating")
        .arg(file)
        .current_dir(std::env::temp_dir())
        .output()
        .ok()?;
    let mut combined = String::from_utf8_lossy(&out.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&out.stderr));
    Some((out.status.success(), combined))
}

/// A `Vec<u64>` program that pushes two values, lowered by the real toolchain. The
/// emitted `TVecU64` wrapper carries whatever `push` ens the lowerer produces.
const VEC_TWO_PUSH: &str = r#"
fn two(x: u64, y: u64) -> u64
  req x < 1000000 && y < 1000000
  ens true
  fx alloc
{
    let mut v: Vec<u64> = Vec::new();
    v.push(x);
    v.push(y);
    let r = v.get(0);
    r
}
"#;

#[test]
#[ignore = "blocker #99 — push ens drops the element-preservation frame; un-ignore when fixed"]
fn divergence_push_frames_prior_elements() {
    let parsed = thermite_syntax::parse(VEC_TWO_PUSH);
    assert!(parsed.errors.is_empty(), "must parse: {:?}", parsed.errors);
    let emitted = thermite_lower::lower(&parsed.program).expect("lowers");

    // The emitted output uses the real `vstd::prelude::*` wrapper header; append a
    // value-pinning CLIENT inside the SAME verus! block that exercises the wrapper
    // contract ONLY (no body internals). The client pushes two distinct values and
    // pins that get(0) is the FIRST. This is provable iff `push`'s ens FRAMES the
    // prior elements (the design GROUNDED seed). The expected fact (get(0)==first)
    // is the design REQ-5/REQ-9 contract, not toolchain output (R-CHAR-3).
    let client = r#"
verus!{
pub fn critic_first_element_framed() {
    let mut v = TVecU64 { data: Vec::new() };
    v.push(10);
    v.push(20);
    let e0 = v.get(0);   // ens: e0 == v.data@[0]
    let e1 = v.get(1);   // ens: e1 == v.data@[1]
    assert(e0 == 10);    // get(0) is the FIRST pushed value (REQ-5 element frame)
    assert(e1 == 20);
}
}
"#;
    // Splice the client into the emitted file just before the trailing `fn main`,
    // or append a fresh verus! block (the lowerer emits a standalone verus! block
    // per item; a second verus! block in the same file is accepted by verus).
    let combined = format!("{emitted}\n{client}\n");

    let tmp = std::env::temp_dir().join("divergence_vec_push_frame.rs");
    std::fs::write(&tmp, &combined).expect("write temp");

    match run_verus(&tmp) {
        Some((ok, output)) => {
            assert!(
                ok && output.contains("verified, 0 errors") && !output.contains("error"),
                "DIVERGENCE: the emitted `push` ens lacks the element-preservation \
                 frame, so a caller cannot prove get(0) is the first element after a \
                 second push (.design/basis/04-collections.md REQ-5 GROUNDED seed: \
                 `forall|j| 0<=j<old_len ==> final(self).data@[j] == old(self).data@[j]`).\n\
                 --- verus output ---\n{output}\n--- emitted ---\n{combined}"
            );
        }
        None => eprintln!("SKIP: verus unavailable (set VERUS_BIN)."),
    }
}
