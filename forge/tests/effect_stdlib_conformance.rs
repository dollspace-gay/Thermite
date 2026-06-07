//! Stage 3 effect-primitive stdlib cert oracle (`.design/basis/03-effect-stdlib.md`
//! AC-1..AC-6 / REQ-1..REQ-7; `conformance/effect-stdlib/cases.json` +
//! `conformance/effect_demo.th`). Drives the BUILT `forge` binary over the oracle
//! programs and asserts the verified-effect-primitive pattern: a `#[boundary("os::…")]`
//! effect primitive certifies L1 + boundary; a pure caller that handles BOTH `Option`
//! arms composes THROUGH the assumed `ens` to L3 + `to_boundary`; the audit manifest
//! enumerates each primitive in the TCB; the `#57` seccomp sandbox derives the
//! per-effect syscall allowlist.
//!
//! Stage 3 owns NO new validator/lower/vacuity/sandbox mechanism — outcome-coverage
//! is EMERGENT (the doc's Resolution 1): `#16` boundary L1-short-circuit + `#52`
//! verify-through + Stage-1b exhaustive-match + `#57` fx→syscall already compose.
//! This file is therefore a CONFORMANCE DEMONSTRATION over the shipped pipeline; it
//! adds no `forge`/`thermite-lower` production code.
//!
//! THE MUTATION-FLOOR OQ (resolved here): `read_doubled`'s shape-only effect
//! contract (`Some(v) => v < 512`) is intrinsically mutation-survivable (a
//! `return None` mutant satisfies it — you cannot pin a value read from the world),
//! so at the DEFAULT 60% floor it is HONESTLY gated `WeakContract` (the
//! `compose_through_weak_contract_at_default_floor` test pins this true reflection).
//! The L3 + `to_boundary` claim is therefore demonstrated at `--mutation-floor 0`
//! (the documented relaxation), exactly as a real effect-reading program would be
//! checked. The floor-0 flag MUST NOT leak to the pure corpus (`sum`/`binary_search`
//! stay at the default floor — `corpus_unaffected_at_default_floor`). No
//! mutation-exemption rule is added (it would be a soundness-adjacent new rule; the
//! orchestrator/critic weigh that, not a silent add).
//!
//! Expected values trace to the golden `conformance/effect-stdlib/cases.json` +
//! `conformance/effect_demo.th` + `.design/basis/03-effect-stdlib.md` (R-CHAR-3),
//! NEVER copied from forge's own output. The `forge check`/`audit` cases run a real
//! verus proof (the compose-through L3) so they SKIP LOUDLY if verus is absent; the
//! sandbox case RUNS a seccomp-confined binary so it SKIPS LOUDLY without a
//! `kill_process`-capable kernel (`unwrap`/`expect`/`panic!` are fine — `tests/` is
//! not anti-pattern-gated).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

fn forge_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_forge"))
}

fn conformance_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("conformance")
}

fn cases() -> Value {
    let path = conformance_dir().join("effect-stdlib").join("cases.json");
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read effect-stdlib cases.json: {e}"));
    serde_json::from_str(&src).unwrap_or_else(|e| panic!("parse effect-stdlib cases.json: {e}"))
}

/// `true` iff verus can be located — mirrors `composition_conformance.rs`. The
/// compose-through L3 is a real verus proof, so the prover must be present; the
/// boundary L1 cert resolves the verus version up-front for the proof cache.
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

/// `true` iff this host's kernel offers the `kill_process` seccomp action — mirrors
/// `sandbox_conformance.rs`. When absent the sandbox case SKIPS LOUD (OQ-3).
fn seccomp_kill_available() -> bool {
    std::fs::read_to_string("/proc/sys/kernel/seccomp/actions_avail")
        .map(|s| s.contains("kill_process"))
        .unwrap_or(false)
}

fn write_temp_program(name: &str, program: &str) -> PathBuf {
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("effect_stdlib_{name}_{pid}.th"));
    std::fs::write(&path, program).unwrap_or_else(|e| panic!("write temp .th: {e}"));
    path
}

/// Run `forge check <file> [--mutation-floor F] --json`, returning the parsed cert
/// array (empty on a non-JSON result), the exit code, and stderr.
fn run_check_json(file: &Path, mutation_floor: Option<f64>) -> (Option<i32>, Vec<Value>, String) {
    let mut cmd = Command::new(forge_bin());
    cmd.arg("check").arg(file);
    if let Some(floor) = mutation_floor {
        cmd.arg("--mutation-floor").arg(format!("{floor}"));
    }
    cmd.arg("--json");
    let out = cmd.output().unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let certs = serde_json::from_str::<Value>(stdout.trim())
        .ok()
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    (out.status.code(), certs, stderr)
}

/// Run `forge audit <file> --json`, returning (exit_code, parsed manifest, stderr).
fn run_audit_json(file: &Path) -> (Option<i32>, Option<Value>, String) {
    let out = Command::new(forge_bin())
        .arg("audit")
        .arg(file)
        .arg("--json")
        .output()
        .unwrap_or_else(|e| panic!("spawn forge: {e}"));
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let manifest = serde_json::from_str::<Value>(stdout.trim()).ok();
    (out.status.code(), manifest, stderr)
}

fn run_forge_build(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(forge_bin())
        .arg("build")
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("spawn forge build: {e}"));
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn find_cert<'a>(certs: &'a [Value], item: &str) -> &'a Value {
    certs
        .iter()
        .find(|c| c.get("item").and_then(|v| v.as_str()) == Some(item))
        .unwrap_or_else(|| panic!("no certificate for item `{item}` in {certs:?}"))
}

/// The frozen centerpiece source the oracle pins (`conformance/effect_demo.th`).
fn effect_demo() -> PathBuf {
    conformance_dir().join("effect_demo.th")
}

// ---------------------------------------------------------------------------
// AC-1a (boundary_primitive): read_small / now certify L1 + boundary + target +
// fx. The body is the foreign syscall (assumed via external_body, contract
// L1-enforced at the crossing); the closed-outcome `ens` (`match result { … }`)
// is NOT vacuity-rejected (the boundary L1-short-circuits before the value-strength
// gates) yet is not trivial (`ens true` would be rejected — see the now-trivial
// negative is covered by the shipped #16 oracle). Anchored to cases.json
// `boundary_primitive`.
// ---------------------------------------------------------------------------
#[test]
fn boundary_primitives_certify_l1_boundary_target_fx() {
    if !verus_present() {
        eprintln!(
            "SKIP boundary_primitives: verus not available — `forge check` resolves the verus \
             version up-front even for a boundary-only cert."
        );
        return;
    }
    let oracle = cases();
    let prims = oracle["boundary_primitive"]
        .as_array()
        .expect("boundary_primitive array");

    // The centerpiece source (read_small + now + read_doubled) at --mutation-floor 0.
    let (code, certs, stderr) = run_check_json(&effect_demo(), Some(0.0));
    assert_eq!(
        code,
        Some(0),
        "effect_demo.th certifies (the L1 boundaries + the L3 compose-through at floor 0); \
         stderr:\n{stderr}"
    );

    for prim in prims {
        let name = prim["name"].as_str().expect("name");
        let expect_level = prim["level"].as_str().expect("level"); // "L1"
        let expect_boundary = prim["boundary"].as_bool().expect("boundary"); // true
        let expect_target = prim["expect_target"].as_str().expect("expect_target");
        let expect_fx = prim["expect_fx_contains"]
            .as_str()
            .expect("expect_fx_contains");

        let cert = find_cert(&certs, name);
        // REQ-1/REQ-7: the effect primitive certifies L1 (foreign body, no verus run
        // on it) — never L3.
        assert_eq!(
            cert["level"],
            Value::from(expect_level),
            "`{name}` is a #[boundary] effect primitive → L1 (foreign body trusted by fiat)"
        );
        assert_ne!(
            cert["level"],
            Value::from("L3"),
            "`{name}`: no verus proof runs on a foreign syscall body"
        );
        assert_eq!(
            cert["boundary"],
            Value::from(expect_boundary),
            "`{name}` carries the boundary flag (#16)"
        );
        // REQ-6: the foreign syscall target is recorded for the TCB enumeration.
        assert_eq!(
            cert["boundary_target"],
            Value::from(expect_target),
            "`{name}` records its `os::…` foreign target"
        );
        // REQ-4: the typed §4.1 effect atom is present (read / time).
        let effects: Vec<String> = cert["effects"]
            .as_array()
            .expect("effects array")
            .iter()
            .map(|e| e.as_str().expect("effect string").to_string())
            .collect();
        assert!(
            effects.iter().any(|e| e.contains(expect_fx)),
            "`{name}` declares the typed effect `{expect_fx}`: {effects:?}"
        );
        // REQ-3: the closed-outcome `ens` is NOT vacuity-rejected (no reject cause)
        // — the boundary short-circuits before the value-strength gates.
        assert!(
            cert.get("reject").map(|r| r.is_null()).unwrap_or(true),
            "`{name}`: a closed-set `ens` (match result {{…}}) is admitted, not vacuity-rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-1b (compose_through): read_doubled handles BOTH Option arms and discharges its
// own `ens` (Some(v)=>v<512) THROUGH read_small's assumed `ens` (Some(v)=>v<256 ⇒
// v+v<512) → L3 + to_boundary via read_small (#52 verify-through + #17). Demonstrated
// at --mutation-floor 0 (the bound-style effect-caller contract is intrinsically
// mutation-survivable — the OQ). Anchored to cases.json `compose_through`.
// ---------------------------------------------------------------------------
#[test]
fn compose_through_certifies_l3_to_boundary_at_floor_zero() {
    if !verus_present() {
        eprintln!("SKIP compose_through: verus not available — the L3 weave is a real verus run.");
        return;
    }
    let oracle = cases();
    let case = oracle["compose_through"]
        .as_array()
        .expect("compose_through array")[0]
        .clone();
    let item = case["name"].as_str().expect("name"); // "read_doubled"
    let expect_level = case["level"].as_str().expect("level"); // "L3"
    let expect_via = case["via"].as_str().expect("via"); // "read_small"
    let floor = case["mutation_floor"].as_f64().expect("mutation_floor"); // 0.0

    let (code, certs, stderr) = run_check_json(&effect_demo(), Some(floor));
    assert_eq!(
        code,
        Some(0),
        "effect_demo.th certifies at --mutation-floor {floor}; stderr:\n{stderr}"
    );
    let cert = find_cert(&certs, item);
    // REQ-3/REQ-4: the caller PROVES its ens on EVERY arm through the assumed contract.
    assert_eq!(
        cert["level"],
        Value::from(expect_level),
        "`{item}` proves L3 THROUGH read_small's assumed `ens` on both arms (#52)"
    );
    // #17: scope ⊥ level — the closure crosses the boundary, so to_boundary via it.
    assert_eq!(
        cert["assurance_scope"]["kind"],
        Value::from("to_boundary"),
        "`{item}` records to_boundary (its closure reaches the effect primitive)"
    );
    assert_eq!(
        cert["assurance_scope"]["via"],
        Value::from(expect_via),
        "`{item}` records the crossing `via` read_small"
    );
}

// THE MUTATION-FLOOR OQ (resolved): at the DEFAULT floor, read_doubled's shape-only
// effect contract is HONESTLY gated `WeakContract` — a `return None` mutant satisfies
// `Some(v) => v < 512`, because you cannot pin a value READ FROM THE WORLD. This is a
// TRUE reflection (the doc's "honest grounding caveat"), not a bug: the L3 demo above
// runs at --mutation-floor 0 precisely because the bound-style effect-caller contract
// is mutation-survivable by necessity. Expected (L0 + WeakContract) from the design
// doc's caveat, not forge output (R-CHAR-3). This is why floor-0 is the honest demo
// and no mutation-exemption rule is silently added.
#[test]
fn compose_through_weak_contract_at_default_floor() {
    if !verus_present() {
        eprintln!("SKIP compose_through_weak_contract: verus not available.");
        return;
    }
    // DEFAULT floor (no --mutation-floor flag): the #12 gate runs.
    let (_code, certs, _stderr) = run_check_json(&effect_demo(), None);
    let cert = find_cert(&certs, "read_doubled");
    // At the default floor the shape-only contract does NOT reach L3 — it is gated.
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "at the DEFAULT floor a shape-only effect contract is NOT laundered to L3"
    );
    assert_eq!(
        cert["level"],
        Value::from("L0"),
        "the #12 mutation gate rejects the under-constrained contract"
    );
    assert_eq!(
        cert["reject"]["cause"],
        Value::from("WeakContract"),
        "the honest gate cause is WeakContract — the world-value contract is shape-only by \
         necessity (a return-None mutant survives `v < 512`); demonstrate L3 at floor 0"
    );
}

// ---------------------------------------------------------------------------
// AC-5 (reject/missing_arm — handled-or-loud): a caller that drops the None arm of
// read_small()'s Option is REJECTED. For a built-in Option this surfaces at verus as
// `E0004: non-exhaustive patterns: None not covered` (L0 + a failed obligation). The
// failure/EOF outcome cannot be silently dropped. Anchored to cases.json
// `reject.missing_arm`.
// ---------------------------------------------------------------------------
#[test]
fn missing_arm_is_rejected_handled_or_loud() {
    if !verus_present() {
        eprintln!("SKIP missing_arm: verus not available — the non-exhaustive reject is E0004.");
        return;
    }
    let oracle = cases();
    let case = oracle["reject"]
        .as_array()
        .expect("reject array")
        .iter()
        .find(|c| c["name"] == "missing_arm")
        .expect("missing_arm case")
        .clone();
    let program = case["program"].as_str().expect("program");
    // The oracle pins "NonExhaustive"; verus emits "non-exhaustive patterns" (E0004).
    // Match either spelling, case-insensitively — the LOUD non-exhaustive reject.
    let expect = case["expect_error_contains"]
        .as_str()
        .expect("expect_error_contains")
        .to_lowercase()
        .replace('-', "");

    let path = write_temp_program("missing_arm", program);
    let (code, certs, stderr) = run_check_json(&path, Some(0.0));
    let _ = std::fs::remove_file(&path);

    assert_ne!(code, Some(0), "a dropped Option arm does not certify");
    // Cluster C7 (`.design/basis/09-option-result.md` REQ-3, #95): `Option` is now a
    // SEEDED built-in variant set, so a `match` over it that drops the `None` arm is
    // caught LOUDLY at the VALIDATOR (`SpecError::NonExhaustiveMatch`) BEFORE lowering
    // — the compile-time tooth the oracle's `why` names ("enforced at validation").
    // Pre-C7 `Option` was inert at the validator and the missing arm fell through to
    // verus's E0004; now it is a structured spec reject (exit 2, no per-item cert).
    // Either surface is the LOUD reject the oracle pins (`expect_error_contains:
    // "NonExhaustive"`); accept whichever the toolchain emits — a validator Spec error
    // in stderr (the C7 path) OR a verus E0004 in a failed obligation.
    let validator_reject = stderr.to_lowercase().replace('-', "").contains(&expect);
    let verus_reject = certs.iter().any(|c| {
        c.get("item").and_then(|v| v.as_str()) == Some("bad")
            && c["obligations"]
                .as_array()
                .map(|os| {
                    os.iter()
                        .filter(|o| o["status"].as_str() == Some("failed"))
                        .filter_map(|o| o["diagnostic"].as_str())
                        .collect::<String>()
                        .to_lowercase()
                        .replace('-', "")
                        .contains(&expect)
                })
                .unwrap_or(false)
    });
    assert!(
        validator_reject || verus_reject,
        "the dropped-None-arm caller carries the LOUD non-exhaustive reject (oracle \
         `{expect}`) — either the C7 validator `NonExhaustiveMatch` (stderr) or a verus \
         E0004 (failed obligation). got stderr:\n{stderr}\ncerts:\n{certs:?}"
    );
}

// ---------------------------------------------------------------------------
// AC-4 (reject/overclaim_soundness — no manufactured guarantee): a caller claiming
// its Some arm is `< 100` while read_small only guarantees `< 256` does NOT discharge
// its `ens` through the boundary's assumed contract → L0 counterexample, NOT a false
// L3. The assumed `ens` is a floor the caller cannot exceed (R-DEFER-9). Anchored to
// cases.json `reject.overclaim_soundness`.
// ---------------------------------------------------------------------------
#[test]
fn overclaim_soundness_is_a_counterexample_not_a_false_l3() {
    if !verus_present() {
        eprintln!(
            "SKIP overclaim_soundness: verus not available — the soundness check is a real \
                   verus proof."
        );
        return;
    }
    let oracle = cases();
    let case = oracle["reject"]
        .as_array()
        .expect("reject array")
        .iter()
        .find(|c| c["name"] == "overclaim_soundness")
        .expect("overclaim_soundness case")
        .clone();
    let program = case["program"].as_str().expect("program");
    let expect_level = case["expect_level"].as_str().expect("expect_level"); // "L0"

    let path = write_temp_program("overclaim_soundness", program);
    let (_code, certs, _stderr) = run_check_json(&path, Some(0.0));
    let _ = std::fs::remove_file(&path);

    let cert = find_cert(&certs, "liar");
    // The decisive anti-cheat assertion: NOT laundered to a false L3.
    assert_ne!(
        cert["level"],
        Value::from("L3"),
        "`liar` claims a stronger Some bound (<100) than read_small delivers (<256) — NOT L3"
    );
    assert_eq!(
        cert["level"],
        Value::from(expect_level),
        "`liar` is the L0 counterexample (postcondition not satisfied through the contract)"
    );
    let obligations = cert["obligations"].as_array().expect("obligations array");
    assert!(
        obligations
            .iter()
            .any(|o| o["status"].as_str() == Some("failed")),
        "`liar` carries a failed-obligation witness (a real counterexample, R-DEFER-9)"
    );
    // read_small itself still certifies L1 boundary (untouched by the liar caller).
    let prim = find_cert(&certs, "read_small");
    assert_eq!(prim["level"], Value::from("L1"));
    assert_eq!(prim["boundary"], Value::from(true));
}

// ---------------------------------------------------------------------------
// AC-2 (tcb): `forge audit effect_demo.th` enumerates read_small + now as boundary
// TCB members — name + foreign target + assumed contract (req/ens/fx). The §9
// enumerable, contracted, fiat-trusted base; everything else is verified. Anchored to
// cases.json `tcb`.
// ---------------------------------------------------------------------------
#[test]
fn audit_enumerates_primitives_in_the_tcb() {
    if !verus_present() {
        eprintln!(
            "SKIP audit_enumerates_tcb: verus not available — audit runs the check pipeline."
        );
        return;
    }
    let oracle = cases();
    let case = oracle["tcb"].as_array().expect("tcb array")[0].clone();
    let expect: Vec<String> = case["expect_tcb_boundary_contains"]
        .as_array()
        .expect("expect_tcb_boundary_contains")
        .iter()
        .map(|v| v.as_str().expect("name").to_string())
        .collect();

    // `forge audit` runs the check pipeline at the DEFAULT mutation floor (it has no
    // --mutation-floor flag), so the shape-only read_doubled is WeakContract-gated →
    // the project exits NON-ZERO (the honest gate, the OQ above). The audit STILL
    // emits the full manifest + the TCB enumeration on stdout (the AC-2 deliverable is
    // the enumeration, independent of the project headline). Assert the manifest is
    // emitted + the TCB enumerates the primitives — NOT that the project passed.
    let (code, manifest, stderr) = run_audit_json(&effect_demo());
    let manifest = manifest.unwrap_or_else(|| {
        panic!("forge audit --json must emit one JSON manifest even when a fn is gated; stderr:\n{stderr}")
    });
    assert_ne!(
        code,
        Some(2),
        "exit 2 is a forge usage error (not the WeakContract project gate); stderr:\n{stderr}"
    );
    assert_eq!(manifest["manifest_version"], Value::from("v1"));

    let contracts = manifest["tcb"]["boundary_contracts"]
        .as_array()
        .expect("boundary_contracts array");
    let names: Vec<String> = contracts
        .iter()
        .map(|c| c["name"].as_str().expect("name").to_string())
        .collect();
    for want in &expect {
        assert!(
            names.contains(want),
            "the TCB enumerates `{want}` as a boundary member (R-DEFER-9): {names:?}"
        );
        let row = contracts
            .iter()
            .find(|c| c["name"].as_str() == Some(want))
            .unwrap_or_else(|| panic!("no boundary contract row `{want}`"));
        // REQ-6: the row enumerates the foreign target + the assumed contract.
        assert!(
            row["target"]
                .as_str()
                .map(|t| t.starts_with("os::"))
                .unwrap_or(false),
            "`{want}` records its `os::…` foreign target: {row:?}"
        );
        assert!(
            row.get("req").is_some(),
            "`{want}` records its assumed req: {row:?}"
        );
        assert!(
            row.get("ens").is_some(),
            "`{want}` records its assumed ens: {row:?}"
        );
        assert!(
            row.get("fx").is_some(),
            "`{want}` records its typed fx: {row:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// AC-3 (sandbox): `forge build --entry` of a program declaring `fx time` derives the
// #57 seccomp allowlist for that effect — including clock_gettime (syscall 228, per
// the design doc's fx→syscall table); a `fx pure` probe attempting a denied syscall
// is SIGSYS-killed (exit 159 = 128+31). The allowlist is fx-DERIVED and per-effect,
// WITHOUT a live os:: link (v1; OQ-4). Anchored to cases.json `sandbox`. RUNS a
// seccomp-confined binary, so SKIPS LOUD without a kill_process kernel (OQ-3).
// ---------------------------------------------------------------------------
#[test]
fn sandbox_derives_fx_time_allowlist_and_kills_off_allowlist() {
    if !seccomp_kill_available() {
        eprintln!(
            "SKIP sandbox: no /proc/sys/kernel/seccomp/actions_avail kill_process (host lacks the \
             seccomp mechanism; OQ-3)"
        );
        return;
    }
    let oracle = cases();
    let case = oracle["sandbox"].as_array().expect("sandbox array")[0].clone();
    // The oracle names the syscall (clock_gettime); the design doc's fx→syscall table
    // pins clock_gettime == 228 (the allowlist is numeric). Trace the named oracle to
    // the doc's constant (R-CHAR-3), never to forge output.
    let expect_syscall_name = case["expect_allowlist_contains_syscall"]
        .as_str()
        .expect("expect_allowlist_contains_syscall"); // "clock_gettime"
    assert_eq!(
        expect_syscall_name, "clock_gettime",
        "the oracle names the Time-effect syscall"
    );
    const CLOCK_GETTIME: i64 = 228; // .design/basis/03-effect-stdlib.md fx→syscall table

    // A fn declaring `fx time` (pure body — v1 grounds confinement via the
    // fx-declaring-body + the foreign os:: link is v1.1, OQ-4).
    let fixture = write_temp_program(
        "tf",
        "fn tf(x: u32) -> u32 req x < 100 ens result == x fx time { x }\n",
    );
    let (ok, stdout, stderr) = run_forge_build(&[
        fixture.to_str().expect("fixture path"),
        "--entry",
        "tf",
        "--sandbox",
        "--json",
    ]);
    let _ = std::fs::remove_file(&fixture);
    assert!(
        ok,
        "forge build --entry tf (fx time) --sandbox must succeed:\n{stdout}\n{stderr}"
    );
    let v: Value = serde_json::from_str(&stdout).expect("forge build --json manifest");
    assert_eq!(
        v["sandbox"]["installed"],
        Value::from(true),
        "the seccomp sandbox is on by default for --entry"
    );
    assert_eq!(
        v["sandbox"]["transitive_fx"],
        serde_json::json!(["time"]),
        "the entry's transitive fx is exactly the Time effect"
    );
    let allow: Vec<i64> = v["sandbox"]["syscall_allowlist"]
        .as_array()
        .expect("allowlist array")
        .iter()
        .map(|n| n.as_i64().expect("syscall number"))
        .collect();
    // REQ-5: the Time effect WIDENS the allowlist to include clock_gettime (228).
    assert!(
        allow.contains(&CLOCK_GETTIME),
        "AC-3: `fx time` derives an allowlist including clock_gettime ({CLOCK_GETTIME}): {allow:?}"
    );
    let artifact = PathBuf::from(
        v["artifact"]
            .as_str()
            .expect("artifact path in build manifest"),
    );

    // The fx-time binary runs CLEAN under its derived filter (the entry stays within
    // its declared effects; no off-allowlist syscall).
    {
        use std::os::unix::process::ExitStatusExt;
        let out = Command::new(&artifact)
            .output()
            .unwrap_or_else(|e| panic!("run fx-time artifact: {e}"));
        assert_eq!(
            out.status.code(),
            Some(0),
            "the fx-time binary runs clean under its derived filter; signal={:?}",
            out.status.signal()
        );
    }
    // Cleanup the copied-out artifact + its per-run dir (#53 — no leaked artifacts).
    let _ = std::fs::remove_file(&artifact);
    if let Some(parent) = artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }

    // The KILL scream: a PURE filter (fx pure, 23 syscalls) denies the openat probe →
    // SIGSYS, exit 159. This is the shipped #57 `pure_probe_killed` precedent, RUN
    // here to confirm the per-effect confinement teeth (a syscall outside the declared
    // fx is killed at the boundary).
    let sum = conformance_dir().join("sum.th");
    let (ok2, stdout2, stderr2) = run_forge_build(&[
        sum.to_str().expect("sum path"),
        "--entry",
        "sum",
        "--sandbox",
        "--sandbox-self-test",
        "--json",
    ]);
    assert!(
        ok2,
        "the pure-probe build must compile:\n{stdout2}\n{stderr2}"
    );
    let v2: Value = serde_json::from_str(&stdout2).expect("forge build --json manifest");
    let probe_artifact = PathBuf::from(
        v2["artifact"]
            .as_str()
            .expect("artifact path in build manifest"),
    );
    {
        use std::os::unix::process::ExitStatusExt;
        const SIGSYS: i32 = 31;
        let out = Command::new(&probe_artifact)
            .output()
            .unwrap_or_else(|e| panic!("run pure-probe artifact: {e}"));
        assert!(
            out.status.signal() == Some(SIGSYS) || out.status.code() == Some(159),
            "AC-3: the openat probe under the PURE filter is SIGSYS-killed (signal 31 / exit 159); \
             got code={:?} signal={:?}",
            out.status.code(),
            out.status.signal()
        );
    }
    let _ = std::fs::remove_file(&probe_artifact);
    if let Some(parent) = probe_artifact.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}

// ---------------------------------------------------------------------------
// AC-7 (corpus unaffected): the pure corpus (`sum`) stays L3 END-TO-END at the
// DEFAULT mutation floor — the floor-0 relaxation the effect caller needs MUST NOT
// leak to the pure corpus. Run `sum` with NO --mutation-floor flag and assert it
// stays L3 + end_to_end (no boundary, no WeakContract). Expected from
// `.design/basis/03-effect-stdlib.md` AC-7, not forge output (R-CHAR-3).
// ---------------------------------------------------------------------------
#[test]
fn corpus_unaffected_at_default_floor() {
    if !verus_present() {
        eprintln!("SKIP corpus_unaffected: verus not available.");
        return;
    }
    let sum = conformance_dir().join("sum.th");
    // NO mutation_floor → the DEFAULT floor (the pure corpus is NOT relaxed).
    let (code, certs, stderr) = run_check_json(&sum, None);
    assert_eq!(
        code,
        Some(0),
        "sum certifies at the default floor; stderr:\n{stderr}"
    );
    let cert = find_cert(&certs, "sum");
    assert_eq!(
        cert["level"],
        Value::from("L3"),
        "the pure corpus stays L3 at the DEFAULT floor (floor-0 did not leak)"
    );
    assert_eq!(
        cert["boundary"],
        Value::from(false),
        "the pure corpus reaches no boundary"
    );
    // END-TO-END: no crossing in the closure → scope end_to_end (or absent).
    let scope_is_e2e = match cert.get("assurance_scope") {
        None | Some(Value::Null) => true,
        Some(s) => s.get("kind").and_then(|v| v.as_str()) == Some("end_to_end"),
    };
    assert!(
        scope_is_e2e,
        "the pure corpus is END-TO-END, not to_boundary"
    );
}
