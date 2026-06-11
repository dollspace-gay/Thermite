//! Divergence pin (#201, audits #200 / commit a0d8ea64): `scripts/audit.sh`
//! check [2] (full-corpus translation-validation) SWALLOWS a Divergent finding.
//!
//! THE DIVERGENCE (false assurance in the audit script — the worst-case
//! R-HONEST-3 / R-DEFER-9 failure mode):
//!
//!   `forge tv` / `forge exec-tv` / `forge body-tv` exit NONZERO on any
//!   Divergent obligation — `forge/src/cli.rs::run_tv` (and `run_body_tv` /
//!   `run_exec_tv`, the SAME convention) documents and implements:
//!     "Any DIVERGENT clause (corpus OR generated) is a real lowering-fidelity
//!      finding → verification-failure exit" → `ExitCode::from(EXIT_VERIFICATION_FAILURE)`.
//!
//!   But `scripts/audit.sh` check [2]'s per-subcommand loop does:
//!     out="$("$FORGE" "$sub" "$f" 2>&1)"; src=$?
//!     if [ "$src" -ne 0 ]; then prog_admit=0; continue; fi
//!   — i.e. it treats EVERY nonzero exit as "forge refused to admit" and skips
//!   parsing the report. A Divergent run (exit 1, report header "… 1 DIVERGENT …")
//!   is therefore never counted: the program prints a green
//!   "ok <name> 0 divergent", TOT_DIV stays 0, and check [2] prints
//!   "PASS ZERO divergent across the whole corpus". The gate that exists to
//!   catch exactly one condition CANNOT fire on that condition.
//!
//! AUTHORITY (R-CHAR-3 — the expected value is NOT taken from the script):
//!   - `scripts/audit.sh` own stated contract (header + check [2] note):
//!     "PASS iff ZERO Divergent across the corpus" — so one Divergent ⇒ FAIL ⇒ RC=1.
//!   - `forge/src/cli.rs::run_tv` — the Divergent ⇒ nonzero-exit convention the
//!     script must consume (the fake forge below reproduces it exactly, including
//!     `forge/src/contract_tv.rs::render_report`'s header line shape).
//!   - goal.md R-HONEST-3 (no silent masking of an infidelity); thermite-design.md
//!     §1 (trust relocation: the audit is what the skeptic relies on).
//!
//! METHOD: extract check [2] VERBATIM from the live `scripts/audit.sh` (between
//! its `# CHECK 2` / `# CHECK 3` banners), run it against a fake `forge` that
//! reports one corpus program as `1 DIVERGENT` with the REAL exit convention
//! (exit 1), and assert the authority's expectation: the check must FAIL (RC=1)
//! and must NOT print the green zero-divergent PASS. This test FAILS against
//! a0d8ea64 (the swallow) and passes once the generator consumes the
//! verification-failure exit as a finding instead of `continue`-ing past it.
//!
//! Tracking: crosslink #201 (audits #200).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .expect("repo root")
}

/// The fake forge: reproduces `forge/src/cli.rs::run_tv`'s EXACT observable
/// contract for a divergent corpus file — `render_report`'s header line
/// (`contract_tv.rs`) plus the verification-failure exit (EXIT_VERIFICATION_FAILURE
/// = 1) — for `tv conformance/binary_search.th`; clean (exit 0) everywhere else.
const FAKE_FORGE: &str = r#"#!/usr/bin/env bash
sub="$1"; f="$2"
if [ "$sub" = "tv" ] && [[ "$f" == *binary_search.th ]]; then
  echo "contract-TV (corpus) $f: 6 clause(s) checked, 5 faithful, 1 DIVERGENT, 0 skipped, 0 unverifiable"
  echo "  binary_search ens#1 — DIVERGENT: counterexample at mid=0"
  exit 1
fi
echo "contract-TV (corpus) $f: 4 clause(s) checked, 4 faithful, 0 DIVERGENT, 0 skipped, 0 unverifiable"
exit 0
"#;

#[test]
fn divergence_audit_check2_swallows_divergent_exit() {
    let root = repo_root();
    let script = fs::read_to_string(root.join("scripts/audit.sh")).expect("read scripts/audit.sh");

    // Extract check [2] verbatim between its section banners.
    let start = script.find("# CHECK 2 ").expect(
        "scripts/audit.sh: '# CHECK 2' banner not found (script restructured? re-anchor this pin)",
    );
    let end = script.find("# CHECK 3 ").expect(
        "scripts/audit.sh: '# CHECK 3' banner not found (script restructured? re-anchor this pin)",
    );
    assert!(start < end, "CHECK 2 banner must precede CHECK 3");
    let check2 = &script[start..end];
    assert!(
        check2.contains("DIVERGENT"),
        "extracted check [2] section lost its divergent gate — re-anchor this pin"
    );

    let scratch = std::env::temp_dir().join(format!("th_audit_pin_{}", std::process::id()));
    fs::create_dir_all(&scratch).expect("scratch dir");
    let fake_forge = scratch.join("fakeforge");
    fs::write(&fake_forge, FAKE_FORGE).expect("write fake forge");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&fake_forge, fs::Permissions::from_mode(0o755)).expect("chmod");
    }

    // The harness: the script's own stubs (color/printers), a present VERUS (the
    // gate under test is NOT the skip path), the fake forge, then check [2] verbatim.
    let harness = format!(
        "#!/usr/bin/env bash\n\
         set -uo pipefail\n\
         cd {root}\n\
         B=; G=; R=; Y=; Z=;\n\
         bold() {{ echo \"$1\"; }}\n\
         pass() {{ echo \"  PASS $1\"; }}\n\
         fail() {{ echo \"  FAIL $1\"; }}\n\
         skip() {{ echo \"  SKIP $1\"; }}\n\
         note() {{ echo \"       $1\"; }}\n\
         VERUS=/usr/bin/true\n\
         FORGE={fake}\n\
         RC=0\n\
         SKIPPED_GUARANTEES=()\n\
         {check2}\n\
         echo \"FINAL_RC=$RC\"\n",
        root = root.display(),
        fake = fake_forge.display(),
        check2 = check2,
    );
    let harness_path = scratch.join("check2_harness.sh");
    fs::write(&harness_path, harness).expect("write harness");

    let out = Command::new("bash")
        .arg(&harness_path)
        .output()
        .expect("run check [2] harness");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();

    fs::remove_dir_all(&scratch).ok();

    // AUTHORITY EXPECTATION 1 — "PASS iff ZERO Divergent": one Divergent obligation
    // (delivered with forge's real verification-failure exit) must trip the gate.
    assert!(
        stdout.contains("FINAL_RC=1"),
        "scripts/audit.sh check [2] swallowed a DIVERGENT finding: forge tv reported \
         '1 DIVERGENT' and exited with EXIT_VERIFICATION_FAILURE (the cli.rs::run_tv \
         convention), but the corpus gate did not fail (RC stayed 0 — the nonzero \
         exit was treated as 'not admitted' and the report was never parsed).\n\
         --- harness output ---\n{stdout}"
    );

    // AUTHORITY EXPECTATION 2 — no green zero-divergent PASS may be printed for a
    // corpus containing a Divergent program (R-HONEST-3: never mask an infidelity).
    assert!(
        !stdout.contains("ZERO divergent across the whole corpus"),
        "scripts/audit.sh check [2] printed the green zero-divergent PASS over a \
         corpus containing a DIVERGENT program (false assurance).\n\
         --- harness output ---\n{stdout}"
    );
}
