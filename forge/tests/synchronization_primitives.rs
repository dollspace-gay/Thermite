//! Primitive-only acceptance for bounded waiting and synchronization mechanics.
//!
//! In-language algorithms prove at L3. The three machine-facing wait operations
//! remain explicit L1 boundaries for a consumer platform to directly refine.
//! No kernel scheduler, lock policy, protected-data policy, Rust runtime, or
//! machine body is bundled.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn forge(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "thermite_synchronization_primitives_{name}_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn checked_rows(source: &str) -> Vec<serde_json::Value> {
    let checked = forge(&["check", source, "--level", "l3", "--json"]);
    assert_success(&checked);
    serde_json::from_slice::<serde_json::Value>(&checked.stdout)
        .unwrap()
        .as_array()
        .unwrap()
        .clone()
}

fn mutation_total(rows: &[serde_json::Value]) -> (u64, u64) {
    rows.iter().fold((0, 0), |(killed, total), row| {
        let score = row["contract_quality"]["mutants_killed"].as_str().unwrap();
        let (row_killed, row_total) = score.split_once('/').unwrap();
        (
            killed + row_killed.parse::<u64>().unwrap(),
            total + row_total.parse::<u64>().unwrap(),
        )
    })
}

fn assert_false_claim_rejected(source: &str, name: &str, temp: &TempDir) {
    let path = temp.0.join(format!("{name}.th"));
    fs::write(&path, source).unwrap();
    let path_s = path.to_string_lossy().to_string();
    let rejected = forge(&["check", &path_s, "--level", "l3", "--json"]);
    assert!(
        !rejected.status.success(),
        "`{name}` unexpectedly certified"
    );
    let rows: serde_json::Value = serde_json::from_slice(&rejected.stdout).unwrap();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["item"] == name)
        .unwrap_or_else(|| panic!("missing rejection row for `{name}`"));
    assert_ne!(row["level"], "L3", "false claim certified: {row}");
}

#[test]
fn synchronization_mechanics_are_receipt_bound_and_fail_closed() {
    let wait_source = "stdlib/kernel-primitives/synchronization/wait.th";
    let once_source = "stdlib/kernel-primitives/synchronization/once.th";
    let refcount_source = "stdlib/kernel-primitives/synchronization/refcount.th";
    let seqlock_source = "stdlib/kernel-primitives/synchronization/seqlock.th";
    let ticket_source = "stdlib/kernel-primitives/synchronization/ticket_lock.th";
    let temp = TempDir::new("package");

    let wait_rows = checked_rows(wait_source);
    assert_eq!(wait_rows.len(), 16);
    for (name, target) in [
        ("cpu_pause", "thermite::cpu::pause"),
        ("wait_block", "thermite::wait::block"),
        ("cpu_halt_terminal", "thermite::cpu::halt_terminal"),
    ] {
        let row = wait_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing wait boundary `{name}`"));
        assert_eq!(row["level"], "L1");
        assert_eq!(row["boundary"], true);
        assert_eq!(row["boundary_target"], target);
    }
    assert_eq!(
        wait_rows
            .iter()
            .filter(|row| row["boundary"] == true)
            .count(),
        3
    );
    for name in [
        "wait_trace_wf",
        "bounded_wait_scan",
        "bounded_wait_change_probe",
        "bounded_wait_timeout_probe",
    ] {
        let row = wait_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing wait certificate `{name}`"));
        assert_eq!(row["level"], "L3", "`{name}` did not prove: {row}");
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&wait_rows), (21, 22));

    let halt = wait_rows
        .iter()
        .find(|row| row["item"] == "cpu_halt_terminal")
        .unwrap();
    assert!(halt["effects"]
        .as_array()
        .unwrap()
        .iter()
        .any(|effect| effect == "diverge"));

    let ticket_rows = checked_rows(ticket_source);
    assert_eq!(ticket_rows.len(), 18);
    assert!(ticket_rows.iter().all(|row| row["level"] == "L3"));
    assert!(ticket_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "ticket_lock_next_number",
        "ticket_lock_can_issue",
        "ticket_lock_issue",
        "ticket_lock_try_enter",
        "ticket_lock_release",
        "ticket_lock_fifo_probe",
        "ticket_lock_stale_probe",
        "ticket_lock_exhaustion_probe",
    ] {
        let row = ticket_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing ticket certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&ticket_rows), (41, 43));

    let once_rows = checked_rows(once_source);
    assert_eq!(once_rows.len(), 18);
    assert!(once_rows.iter().all(|row| row["level"] == "L3"));
    assert!(once_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "once_begin",
        "once_complete",
        "once_poison",
        "once_single_winner_probe",
        "once_stale_token_probe",
        "once_poison_probe",
        "once_exhaustion_probe",
    ] {
        let row = once_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing once certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&once_rows), (65, 68));

    let refcount_rows = checked_rows(refcount_source);
    assert_eq!(refcount_rows.len(), 11);
    assert!(refcount_rows.iter().all(|row| row["level"] == "L3"));
    assert!(refcount_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "refcount_acquire",
        "refcount_release",
        "refcount_lifecycle_probe",
        "refcount_exhaustion_probe",
    ] {
        let row = refcount_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing refcount certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&refcount_rows), (32, 34));

    let seqlock_rows = checked_rows(seqlock_source);
    assert_eq!(seqlock_rows.len(), 18);
    assert!(seqlock_rows.iter().all(|row| row["level"] == "L3"));
    assert!(seqlock_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "seqlock_begin_write",
        "seqlock_finish_write",
        "seqlock_read_begin",
        "seqlock_read_validate",
        "seqlock_stale_read_probe",
        "seqlock_begin_exhaustion_probe",
        "seqlock_finish_exhaustion_probe",
    ] {
        let row = seqlock_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing seqlock certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&seqlock_rows), (49, 52));

    let mut false_wait = fs::read_to_string(root().join(wait_source)).unwrap();
    false_wait.push_str(
        r#"
fn bounded_wait_false_change_claim(expected: usize) -> bool
  req true
  ens result
  fx pure
{
  let trace: WaitTrace64 = WaitTrace64 {
    observations: [expected; BOUNDED_WAIT_CAPACITY],
    len: BOUNDED_WAIT_CAPACITY,
  };
  match bounded_wait_scan(&trace, expected, 1) {
    BoundedWait64::WaitChanged64 { value: _, polls: _ } => true,
    BoundedWait64::WaitTimedOut64 { polls: _ } => false,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_wait, "bounded_wait_false_change_claim", &temp);

    let mut false_ticket = fs::read_to_string(root().join(ticket_source)).unwrap();
    false_ticket.push_str(
        r#"
fn ticket_lock_false_lifo_claim() -> bool
  req true
  ens result
  fx pure
{
  let lock: TicketLockState64 = TicketLockState64 {
    next: 2,
    serving: 0,
  };
  match ticket_lock_try_enter(&lock, 1) {
    TicketEnter64::TicketEntered64 { guard: _ } => true,
    TicketEnter64::TicketWaiting64 { ticket: _ } => false,
    TicketEnter64::TicketStale64 { ticket: _ } => false,
    TicketEnter64::TicketUnknown64 { ticket: _ } => false,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_ticket, "ticket_lock_false_lifo_claim", &temp);

    let mut false_once = fs::read_to_string(root().join(once_source)).unwrap();
    false_once.push_str(
        r#"
fn once_false_second_winner_claim() -> bool
  req true
  ens result
  fx pure
{
  let running: OnceState64 = OnceState64 {
    phase: 1,
    generation: 1,
  };
  match once_begin(running) {
    OnceBegin64::OnceStarted64 { state: _, token: _ } => true,
    OnceBegin64::OnceBusy64 { state: _ } => false,
    OnceBegin64::OnceAlreadyComplete64 { state: _ } => false,
    OnceBegin64::OnceAlreadyPoisoned64 { state: _ } => false,
    OnceBegin64::OnceGenerationExhausted64 { state: _ } => false,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_once, "once_false_second_winner_claim", &temp);

    let mut false_refcount = fs::read_to_string(root().join(refcount_source)).unwrap();
    false_refcount.push_str(
        r#"
fn refcount_false_resurrection_claim() -> bool
  req true
  ens result
  fx pure
{
  let retired: RefCountState64 = RefCountState64 {
    count: 0,
    retired: true,
  };
  match refcount_acquire(retired) {
    RefCountAcquire64::RefCountAcquired64 { state: _ } => true,
    RefCountAcquire64::RefCountRetired64 { state: _ } => false,
    RefCountAcquire64::RefCountExhausted64 { state: _ } => false,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_refcount, "refcount_false_resurrection_claim", &temp);

    let mut false_seqlock = fs::read_to_string(root().join(seqlock_source)).unwrap();
    false_seqlock.push_str(
        r#"
fn seqlock_false_stale_read_claim() -> bool
  req true
  ens result
  fx pure
{
  let advanced: SeqLockState64 = seqlock_from_even(2);
  seqlock_read_validate(&advanced, 0)
}
"#,
    );
    assert_false_claim_rejected(&false_seqlock, "seqlock_false_stale_read_claim", &temp);

    let bundle = temp.0.join("synchronization.verified");
    let bundle_s = bundle.to_string_lossy().to_string();
    assert_success(&forge(&[
        "build",
        "stdlib/kernel-primitives/synchronization.thpkg.json",
        "--level",
        "l3",
        "--export",
        "ticket_lock_can_issue",
        "--target",
        "kernel",
        "--out",
        &bundle_s,
        "--json",
    ]));
    assert_success(&forge(&["verify-build", &bundle_s, "--replay", "--json"]));

    for relative in [
        "evidence/thermite-package/manifest.json",
        "evidence/thermite-package/source-map.json",
        "evidence/thermite-package/source/synchronization/once.th",
        "evidence/thermite-package/source/synchronization/refcount.th",
        "evidence/thermite-package/source/synchronization/seqlock.th",
        "evidence/thermite-package/source/synchronization/ticket_lock.th",
        "evidence/thermite-package/source/synchronization/wait.th",
    ] {
        assert!(bundle.join(relative).is_file(), "missing `{relative}`");
    }

    let plan: serde_json::Value =
        serde_json::from_slice(&fs::read(bundle.join("evidence/artifact-plan.v1")).unwrap())
            .unwrap();
    assert_eq!(
        plan["package"]["name"],
        "thermite_synchronization_primitives"
    );
    assert_eq!(plan["exports"][0]["thermite_name"], "ticket_lock_can_issue");

    let tv: serde_json::Value = serde_json::from_slice(
        &fs::read(bundle.join("evidence/translation-validation.json")).unwrap(),
    )
    .unwrap();
    assert!(
        tv["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|row| row["verdict"] == "faithful"),
        "strict synchronization surface contains non-faithful TV rows: {tv}"
    );

    let bound_wait = bundle.join("evidence/thermite-package/source/synchronization/wait.th");
    let mut tampered = fs::read_to_string(&bound_wait).unwrap();
    tampered.push_str("\n// receipt tamper\n");
    fs::write(&bound_wait, tampered).unwrap();
    let tamper_rejected = forge(&["verify-build", &bundle_s, "--json"]);
    assert!(
        !tamper_rejected.status.success(),
        "tampered waiting source unexpectedly validated"
    );
}
