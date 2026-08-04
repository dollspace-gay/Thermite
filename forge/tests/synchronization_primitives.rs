//! Primitive-only acceptance for bounded waiting, synchronization, queue, and deque mechanics.
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
    let barrier_source = "stdlib/kernel-primitives/synchronization/barrier.th";
    let epoch_ack_source = "stdlib/kernel-primitives/synchronization/epoch_ack.th";
    let mpsc_source = "stdlib/kernel-primitives/synchronization/mpsc_queue.th";
    let once_source = "stdlib/kernel-primitives/synchronization/once.th";
    let refcount_source = "stdlib/kernel-primitives/synchronization/refcount.th";
    let seqlock_source = "stdlib/kernel-primitives/synchronization/seqlock.th";
    let ticket_source = "stdlib/kernel-primitives/synchronization/ticket_lock.th";
    let work_deque_source = "stdlib/kernel-primitives/synchronization/work_deque.th";
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
    for row in &wait_rows {
        if row["boundary"] == true {
            continue;
        }
        assert_eq!(
            row["level"], "L3",
            "an in-language waiting primitive fell below L3: {row}"
        );
    }
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

    let barrier_rows = checked_rows(barrier_source);
    assert_eq!(barrier_rows.len(), 18);
    assert!(barrier_rows.iter().all(|row| row["level"] == "L3"));
    assert!(barrier_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "barrier_membership_mask_valid",
        "barrier_wf",
        "barrier_register",
        "barrier_unregister",
        "barrier_arrive",
        "barrier_stale_generation_probe",
        "barrier_membership_freeze_probe",
        "barrier_arrival_classify",
        "barrier_classification_probe",
        "barrier_generation_next",
        "barrier_generation_exhaustion_probe",
    ] {
        let row = barrier_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing barrier certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&barrier_rows), (141, 149));

    let epoch_ack_rows = checked_rows(epoch_ack_source);
    assert_eq!(epoch_ack_rows.len(), 49);
    assert!(epoch_ack_rows.iter().all(|row| row["level"] == "L3"));
    assert!(epoch_ack_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "epoch_ack_wf",
        "epoch_ack_register",
        "epoch_ack_unregister",
        "epoch_ack_begin",
        "epoch_ack_record",
        "epoch_ack_withdraw",
        "epoch_ack_close",
        "epoch_ack_record_preserves_other_probe",
        "epoch_ack_duplicate_probe",
        "epoch_ack_stale_epoch_probe",
        "epoch_ack_future_epoch_probe",
        "epoch_ack_register_frozen_probe",
        "epoch_ack_begin_snapshot_probe",
        "epoch_ack_epoch_exhaustion_probe",
        "epoch_ack_generation_exhaustion_probe",
        "epoch_ack_close_complete_probe",
    ] {
        let row = epoch_ack_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing epoch/ack certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&epoch_ack_rows), (96, 100));

    let mpsc_rows = checked_rows(mpsc_source);
    assert_eq!(mpsc_rows.len(), 32);
    assert!(mpsc_rows.iter().all(|row| row["level"] == "L3"));
    assert!(mpsc_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "mpsc_queue_wf",
        "mpsc_queue_slot",
        "mpsc_queue_empty",
        "mpsc_queue_reserve",
        "mpsc_queue_publish",
        "mpsc_queue_pop",
        "mpsc_queue_fifo_probe",
        "mpsc_queue_duplicate_publish_probe",
        "mpsc_queue_stale_publish_probe",
        "mpsc_queue_slot_conflict_probe",
    ] {
        let row = mpsc_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing MPSC queue certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&mpsc_rows), (100, 119));

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

    let work_deque_rows = checked_rows(work_deque_source);
    assert_eq!(work_deque_rows.len(), 45);
    assert!(work_deque_rows.iter().all(|row| row["level"] == "L3"));
    assert!(work_deque_rows.iter().all(|row| row["boundary"] == false));
    for name in [
        "work_deque_wf",
        "work_deque_slot",
        "work_deque_push",
        "work_deque_owner_begin_pop",
        "work_deque_owner_commit_pop",
        "work_deque_owner_cancel",
        "work_deque_steal_begin",
        "work_deque_steal_commit",
        "work_deque_owner_lifo_probe",
        "work_deque_thief_fifo_probe",
        "work_deque_last_race_thief_wins_probe",
        "work_deque_last_race_owner_wins_probe",
        "work_deque_split_ends_probe",
        "work_deque_cancel_probe",
        "work_deque_exhaustion_probe",
        "work_deque_owner_generation_exhaustion_probe",
    ] {
        let row = work_deque_rows
            .iter()
            .find(|row| row["item"] == name)
            .unwrap_or_else(|| panic!("missing work-deque certificate `{name}`"));
        assert_ne!(row["contract_quality"]["mutants_killed"], "0/0");
    }
    assert_eq!(mutation_total(&work_deque_rows), (211, 247));

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

    let mut false_barrier = fs::read_to_string(root().join(barrier_source)).unwrap();
    false_barrier.push_str(
        r#"
fn barrier_false_duplicate_advances_claim() -> bool
  req true
  ens result
  fx pure
{
  barrier_arrival_classify(true, true, true, true, true) == 4
}
"#,
    );
    assert_false_claim_rejected(
        &false_barrier,
        "barrier_false_duplicate_advances_claim",
        &temp,
    );

    let mut false_epoch_ack = fs::read_to_string(root().join(epoch_ack_source)).unwrap();
    false_epoch_ack.push_str(
        r#"
fn epoch_ack_false_other_cleared_claim() -> bool
  req true
  ens result
  fx pure
{
  let state: EpochAckState64 = EpochAckState64 {
    active: 3,
    pending: 3,
    generations: [0; EPOCH_ACK_CAPACITY],
    epoch: 1,
    open: true,
    capacity: EPOCH_ACK_CAPACITY,
  };
  let participant: EpochAckParticipant64 = EpochAckParticipant64 {
    slot: 0,
    generation: 0,
  };
  let round: EpochAckRound64 = EpochAckRound64 { epoch: 1 };
  match epoch_ack_record(state, participant, round) {
    EpochAckRecord64::EpochAckRecorded64 {
      state: next,
      participant: returned,
      round: returned_round,
    } => !epoch_ack_is_pending(&next, 1),
    _ => false,
  }
}
"#,
    );
    assert_false_claim_rejected(
        &false_epoch_ack,
        "epoch_ack_false_other_cleared_claim",
        &temp,
    );

    let mut false_mpsc = fs::read_to_string(root().join(mpsc_source)).unwrap();
    false_mpsc.push_str(
        r#"
fn mpsc_queue_false_bypass_claim(value: u64) -> bool
  req true
  ens result
  fx pure
{
  let mut ready: [bool; MPSC_QUEUE_CAPACITY] = [false; MPSC_QUEUE_CAPACITY];
  let mut tickets: [usize; MPSC_QUEUE_CAPACITY] = [0; MPSC_QUEUE_CAPACITY];
  let mut values: [u64; MPSC_QUEUE_CAPACITY] = [0; MPSC_QUEUE_CAPACITY];
  ready[1] = true;
  tickets[1] = 1;
  values[1] = value;
  let queue: MpscQueue64 = MpscQueue64 {
    reserved: 2,
    consumed: 0,
    ready: ready,
    tickets: tickets,
    values: values,
  };
  match mpsc_queue_pop(queue) {
    MpscPop64::MpscPopped64 { queue: _, ticket, value: got } =>
      ticket == 1 && got == value,
    MpscPop64::MpscEmpty64 { queue: _ } => false,
    MpscPop64::MpscPending64 { queue: _, ticket: _ } => false,
    MpscPop64::MpscSlotConflict64 { queue: _, ticket: _, observed_ticket: _ } => false,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_mpsc, "mpsc_queue_false_bypass_claim", &temp);

    let mut false_work_deque = fs::read_to_string(root().join(work_deque_source)).unwrap();
    false_work_deque.push_str(
        r#"
fn work_deque_false_both_win_claim(value: u64) -> bool
  req true
  ens result
  fx pure
{
  let mut ready: [bool; WORK_DEQUE_CAPACITY] = [false; WORK_DEQUE_CAPACITY];
  let mut values: [u64; WORK_DEQUE_CAPACITY] = [0; WORK_DEQUE_CAPACITY];
  ready[0] = true;
  values[0] = value;
  let pending: WorkDeque64 = WorkDeque64 {
    top: 0,
    bottom: 0,
    owner_generation: 1,
    owner_pending: true,
    ready: ready,
    tickets: [0; WORK_DEQUE_CAPACITY],
    values: values,
  };
  let owner: WorkDequeOwnerToken64 = WorkDequeOwnerToken64 {
    generation: 1,
    observed_top: 0,
    observed_bottom: 1,
    ticket: 0,
    slot: 0,
    value: value,
  };
  let thief: WorkDequeStealToken64 = WorkDequeStealToken64 {
    observed_top: 0,
    observed_bottom: 1,
    ticket: 0,
    slot: 0,
    value: value,
  };
  match work_deque_steal_commit(pending, thief) {
    WorkDequeStealCommit64::WorkDequeStolen64 { deque: thief_won, ticket: _, value: _, last: _ } =>
      match work_deque_owner_commit_pop(thief_won, owner) {
        WorkDequeOwnerCommit64::WorkDequeOwnerPopped64 { deque: _, ticket: _, value: _, last: _ } => true,
        WorkDequeOwnerCommit64::WorkDequeOwnerLost64 { deque: _, token: _ } => false,
        WorkDequeOwnerCommit64::WorkDequeOwnerCommitStale64 { deque: _, token: _ } => false,
        WorkDequeOwnerCommit64::WorkDequeOwnerCommitSlotVacant64 { deque: _, token: _ } => false,
        WorkDequeOwnerCommit64::WorkDequeOwnerCommitSlotConflict64 {
          deque: _,
          token: _,
          observed_ticket: _,
          observed_value: _,
        } => false,
      },
    WorkDequeStealCommit64::WorkDequeStealRetry64 { deque: _, token: _ } => false,
    WorkDequeStealCommit64::WorkDequeStealTokenMalformed64 { deque: _, token: _ } => false,
    WorkDequeStealCommit64::WorkDequeStealCommitSlotVacant64 { deque: _, token: _ } => false,
    WorkDequeStealCommit64::WorkDequeStealCommitSlotConflict64 {
      deque: _,
      token: _,
      observed_ticket: _,
      observed_value: _,
    } => false,
  }
}
"#,
    );
    assert_false_claim_rejected(&false_work_deque, "work_deque_false_both_win_claim", &temp);

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
        "evidence/thermite-package/source/synchronization/barrier.th",
        "evidence/thermite-package/source/synchronization/epoch_ack.th",
        "evidence/thermite-package/source/synchronization/mpsc_queue.th",
        "evidence/thermite-package/source/synchronization/once.th",
        "evidence/thermite-package/source/synchronization/refcount.th",
        "evidence/thermite-package/source/synchronization/seqlock.th",
        "evidence/thermite-package/source/synchronization/ticket_lock.th",
        "evidence/thermite-package/source/synchronization/wait.th",
        "evidence/thermite-package/source/synchronization/work_deque.th",
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
