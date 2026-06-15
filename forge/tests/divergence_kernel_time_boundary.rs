//! Re-pinned to the amended authority (orchestrator decision, #198; the #186
//! precedent): `forge build --target kernel` on the `fx time` corpus program
//! `conformance/effect_link_demo.th` (`#[boundary("os::now")] fn now`, `fx time`)
//! now emits the structured named refusal — exit nonzero, no artifact, naming
//! `time` — not a leaked raw rustc `E0433`, and not a build.
//!
//! Why (the amendment): the original critic pin (#198, against forge at 14070625)
//! observed `forge build conformance/effect_link_demo.th --target kernel` leak a raw
//! `error[E0433]: cannot find module or crate `std`` from the emitted `mod os`
//! `std::time::SystemTime::now()` wrapper inside the `#![no_std]` kernel crate. That
//! falsified the design's OQ-2 premise ("`time`/`rand` are benign for the kernel —
//! no syscall mapping"): an admitted-effect boundary carries a std-bodied wrapper.
//!
//! Amended authority (R-CHAR-3 trace — `.design/build/kernel-target.md`):
//!   - OQ-2 (RESOLVED — REJECT; amended by #198): "`time`/`rand` MOVE INTO the reject
//!     set: the v1 kernel admit set is now EXACTLY `pure`/`alloc`/`panic`/`diverge`,
//!     and `KERNEL_REJECTED_FX = [\"read\",\"write\",\"net\",\"term\",\"time\",\"rand\"]`."
//!     A kernel has no ambient clock (`clock_gettime`) or entropy (`getrandom`) any
//!     more than it has `read`/`write` (the critic's own observation).
//!   - REQ-3: a kernel refusal is "a NAMED-effect, nonzero-exit, NO-artifact
//!     structured `ForgeError`" — so `effect_link_demo.th --target kernel` returns
//!     a refusal naming `time`, exit 2, no artifact, and no raw `E0433` reaches
//!     the user (the std-bodied wrapper is refused before codegen).
//!
//! This is the regression coverage for the #198 divergence: the structured
//! `time` refusal replaces the falsified "admitted/benign" build.
//!
//! Tracking: crosslink #198 (ref #197 #164).

use std::path::PathBuf;
use std::process::Command;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

/// `.design/build/kernel-target.md` OQ-2 (amended by #198) + REQ-3: `time` is now on
/// the kernel reject list (`KERNEL_REJECTED_FX`), so the `fx time` corpus program
/// `conformance/effect_link_demo.th` must be refused with the structured named-effect
/// error (exit nonzero, no artifact, naming `time`) — not built, and not a
/// leaked raw rustc `E0433` from the emitted `std::time` `os::now` wrapper inside the
/// `#![no_std]` crate (the #198 divergence the refusal preempts before codegen).
#[test]
fn admitted_time_fx_boundary_is_refused_for_kernel() {
    let demo = corpus_dir().join("effect_link_demo.th");
    let out = Command::new(forge_bin())
        .arg("build")
        .arg(&demo)
        .arg("--target")
        .arg("kernel")
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawning `forge build` failed: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);

    // The reject pin: `time` is now in `KERNEL_REJECTED_FX`, so the build must fail
    // with the structured refusal (not exit 0, not a leaked E0433).
    assert!(
        !out.status.success(),
        "`forge build --target kernel` on the `fx time` corpus program must be REFUSED \
         (kernel-target.md OQ-2 amended by #198: time/rand are REJECTED — std-bodied \
         effect wrappers, no kernel ambient clock/entropy):\nstdout:{stdout}\nstderr:{stderr}"
    );

    // The refusal is the structured named-effect error: it names `time` and explains
    // it is a kernel-target reject (REQ-3) — the same mechanism as `read`/`write`/…
    assert!(
        stderr.contains("time"),
        "the refusal must NAME the rejected `time` effect:\n{stderr}"
    );
    assert!(
        stderr.contains("kernel"),
        "the refusal must explain it is a kernel-target reject:\n{stderr}"
    );

    // No raw rustc internal leak: the std-bodied `os::now` wrapper is refused before
    // codegen, so the user never sees `E0433` / the `std::time` path (the #198 leak).
    assert!(
        !stderr.contains("E0433") && !stderr.contains("std::time"),
        "the refusal must preempt codegen — NO leaked raw rustc `E0433`/`std::time`:\n{stderr}"
    );

    // No artifact: a refusal emits no `--json` manifest with an `artifact` path.
    assert!(
        stdout.trim().is_empty() || serde_json::from_str::<serde_json::Value>(&stdout).is_err(),
        "a refused build emits NO manifest/artifact JSON:\n{stdout}"
    );
}
