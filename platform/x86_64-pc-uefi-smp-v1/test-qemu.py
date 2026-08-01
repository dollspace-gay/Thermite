#!/usr/bin/env python3
"""Boot one Thermite UEFI image across the frozen SMP acceptance matrix."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import select
import shutil
import subprocess
import sys
import tempfile
import time


CPU_MATRIX = (1, 2, 4, 8)
SMP_PATTERN = re.compile(
    r"^THERMITE_SMP online=(?P<online>\d+) aps=(?P<aps>\d+) "
    r"unique=(?P<unique>\d+) work=(?P<work>\d+) expected=(?P<expected>\d+) "
    r"parallel_aps=(?P<parallel>\d+) stale=(?P<stale>\d+) "
    r"duplicate=(?P<duplicate>\d+) bad_ids=(?P<bad_ids>\d+)$"
)
CPU_PATTERN = re.compile(r"^THERMITE_CPUS discovered=(\d+) enabled=(\d+)$")
HANDOFF_PATTERN = re.compile(
    r"^THERMITE_HANDOFF memory_map=1 acpi_bytes=(\d+) firmware_entries=(\d+) "
    r"firmware_bytes=(\d+) framebuffer=absent command_line_bytes=(\d+) "
    r"initrd_bytes=(\d+) image_bytes=(\d+) bounds=exact$"
)
SCHED_PATTERN = re.compile(
    r"^THERMITE_SCHED tasks=(\d+) sum=(\d+) worker_cpus=(\d+) "
    r"lock_entries=(\d+) once=1 bsp_work=1$"
)
IPI_PATTERN = re.compile(r"^THERMITE_IPI epoch=1 acked=(\d+) expected=(\d+)$")
KERNEL_PATTERN = re.compile(
    r"^THERMITE_KERNEL mode=freestanding online=(\d+) failed=(\d+) "
    r"failed_apic=(\d+) firmware_calls=0$"
)
POST_SCHED_PATTERN = re.compile(
    r"^THERMITE_POST_SCHED tasks=(\d+) sum=(\d+) worker_cpus=(\d+) "
    r"ap_workers=(\d+) parallel_cpus=(\d+) lock_entries=(\d+)$"
)
POST_IPI_PATTERN = re.compile(r"^THERMITE_POST_IPI epoch=2 acked_aps=(\d+)$")
TIMER_PATTERN = re.compile(
    r"^THERMITE_TIMER source=tsc-deadline-apic per_cpu=(\d+) "
    r"tsc_ipi_fallbacks=(\d+)$"
)
TLB_PATTERN = re.compile(
    r"^THERMITE_TLB epoch=2 invalidated_cpus=(\d+) stale=0$"
)
ALLOC_PATTERN = re.compile(
    r"^THERMITE_ALLOC frames=64 heap_bytes=(\d+) allocations=(\d+) "
    r"zeroed=1 reclaimed=1 oom_rejected=(\d+) bridge=global_alloc$"
)
ATOMIC_PATTERN = re.compile(
    r"^THERMITE_ATOMIC increment_total=8386560 message_cpus=(\d+) "
    r"message_stale=(\d+) ordering=release-acquire$"
)
DEVICE_MARKER = (
    "THERMITE_DEVICE mmio_widths=8,16,32,64 pio_widths=8,16,32 "
    "barriers=4 pci=1 virtio=1 negatives=2"
)
CPU_LOCAL_PATTERN = re.compile(
    r"^THERMITE_CPU_LOCAL installed=(\d+) gs_verified=(\d+) generation=1$"
)
MODEL_PATTERN = re.compile(
    r"^THERMITE_MODEL event_action=1 atomic=1 frame=1 dma_iommu=1 scheduler=1 "
    r"registry_entries=(\d+) linked=thermite-kernel$"
)


FIRMWARE_ROOTS = (
    Path("/usr/share/edk2/ovmf"),
    Path("/usr/share/OVMF"),
    Path("/usr/share/qemu"),
)
FIRMWARE_PAIRS = (
    ("OVMF_CODE.fd", "OVMF_VARS.fd"),
    ("OVMF_CODE_4M.fd", "OVMF_VARS_4M.fd"),
)


def firmware_paths(roots: tuple[Path, ...] = FIRMWARE_ROOTS) -> tuple[Path, Path]:
    for root in roots:
        for code_name, variables_name in FIRMWARE_PAIRS:
            code = root / code_name
            variables = root / variables_name
            if code.is_file() and variables.is_file():
                return code, variables
    raise RuntimeError("a compatible OVMF code/variables firmware pair was not found")


def validate_transcript(
    lines: list[str], cpus: int, expected_online: int | None = None,
    failed_ap: int | None = None, power_action: str = "poweroff"
) -> None:
    online = cpus if expected_online is None else expected_online
    expected_boot = "THERMITE_BOOT profile=x86_64-pc-uefi-smp-v1"
    if expected_boot not in lines:
        raise RuntimeError(f"{cpus}-CPU run did not emit the profile marker")
    handoff = next(
        (HANDOFF_PATTERN.match(line) for line in lines if HANDOFF_PATTERN.match(line)), None
    )
    if handoff is None:
        raise RuntimeError(f"{cpus}-CPU normalized handoff marker is absent")
    acpi_bytes, firmware_entries, firmware_bytes, command_bytes, initrd_bytes, image_bytes = map(
        int, handoff.groups()
    )
    if (
        acpi_bytes < 20
        or firmware_entries == 0
        or firmware_bytes != firmware_entries * 24
        or command_bytes > 64 * 1024
        or initrd_bytes != 0
        or image_bytes == 0
    ):
        raise RuntimeError(f"{cpus}-CPU normalized handoff bounds are inconsistent")
    cpu_line = next((CPU_PATTERN.match(line) for line in lines if CPU_PATTERN.match(line)), None)
    if cpu_line is None or tuple(map(int, cpu_line.groups())) != (cpus, cpus):
        raise RuntimeError(f"{cpus}-CPU discovery marker is absent or inconsistent")
    smp = next((SMP_PATTERN.match(line) for line in lines if SMP_PATTERN.match(line)), None)
    if smp is None:
        raise RuntimeError(f"{cpus}-CPU run did not emit a parseable SMP marker")
    values = {name: int(value) for name, value in smp.groupdict().items()}
    required_parallel = min(1, max(0, cpus - 1))
    expected = {
        "online": cpus,
        "aps": cpus - 1,
        "unique": cpus,
        "work": cpus * 2_048,
        "expected": cpus * 2_048,
        "stale": 0,
        "duplicate": 0,
        "bad_ids": 0,
    }
    for field, wanted in expected.items():
        if values[field] != wanted:
            raise RuntimeError(
                f"{cpus}-CPU {field} mismatch: got {values[field]}, expected {wanted}"
            )
    if values["parallel"] < required_parallel:
        raise RuntimeError(
            f"{cpus}-CPU run reached only {values['parallel']} concurrent APs; "
            f"expected at least {required_parallel}"
        )
    if "THERMITE_SUCCESS gate=boot-smp-v1" not in lines:
        raise RuntimeError(f"{cpus}-CPU run did not reach its success marker")
    scheduler = next(
        (SCHED_PATTERN.match(line) for line in lines if SCHED_PATTERN.match(line)), None
    )
    required_workers = 2 if cpus >= 4 else 1
    if scheduler is None:
        raise RuntimeError(f"{cpus}-CPU firmware scheduler marker is absent")
    tasks, task_sum, workers, lock_entries = map(int, scheduler.groups())
    if (
        tasks != 4_096
        or task_sum != 4_096 * 4_095 // 2
        or workers < required_workers
        or lock_entries != cpus - 1
    ):
        raise RuntimeError(f"{cpus}-CPU firmware scheduler invariants failed: {scheduler.group(0)}")
    ipi = next((IPI_PATTERN.match(line) for line in lines if IPI_PATTERN.match(line)), None)
    if ipi is None or tuple(map(int, ipi.groups())) != (cpus, cpus):
        raise RuntimeError(f"{cpus}-CPU IPI epoch did not receive every acknowledgement")
    required_markers = (
        f"THERMITE_CLOCK monotonic=1 per_cpu={cpus}",
        "THERMITE_EXIT_BOOT_SERVICES ownership=kernel",
        "THERMITE_USER ring=3 syscall_instruction=syscall syscall=1 fault=1 resume=1",
        "THERMITE_ENTROPY source=rdrand bytes=32 health=passed",
        f"THERMITE_POWER action={power_action} terminal=1",
        "THERMITE_BOUNDARY name=kernel::clock::read@v1 symbol=tpl_clock_read "
        "contract=monotonic_with_error resolved=1",
    )
    if any(marker not in lines for marker in required_markers):
        raise RuntimeError(f"{cpus}-CPU run is missing clock, user, or power evidence")
    allocator = next(
        (ALLOC_PATTERN.match(line) for line in lines if ALLOC_PATTERN.match(line)), None
    )
    if (
        allocator is None
        or int(allocator.group(1)) != 64 * 4096
        or int(allocator.group(2)) < 3
        or int(allocator.group(3)) != 1
    ):
        raise RuntimeError(f"{cpus}-CPU run is missing allocator bridge evidence")
    atomic = next(
        (ATOMIC_PATTERN.match(line) for line in lines if ATOMIC_PATTERN.match(line)), None
    )
    if atomic is None or tuple(map(int, atomic.groups())) != (online, 0):
        raise RuntimeError(f"{cpus}-CPU release/acquire evidence is incomplete")
    if DEVICE_MARKER not in lines:
        raise RuntimeError(f"{cpus}-CPU volatile device-width evidence is incomplete")
    cpu_local = next(
        (CPU_LOCAL_PATTERN.match(line) for line in lines if CPU_LOCAL_PATTERN.match(line)), None
    )
    if cpu_local is None or tuple(map(int, cpu_local.groups())) != (online, online):
        raise RuntimeError(f"{cpus}-CPU local-storage evidence is incomplete")
    model = next(
        (MODEL_PATTERN.match(line) for line in lines if MODEL_PATTERN.match(line)), None
    )
    if model is None or int(model.group(1)) != 104:
        raise RuntimeError(f"{cpus}-CPU linked safe-kernel model evidence is incomplete")
    if not any(
        line.startswith("THERMITE_DMA device=boot-disk generation=0 ownership=cpu ")
        and line.endswith("signature=55aa stale_rejected=1")
        for line in lines
    ):
        raise RuntimeError(f"{cpus}-CPU run is missing mediated block/DMA evidence")
    if (
        "THERMITE_POST_DMA device=virtio-blk domain=identity bytes=512 "
        "generation=1 ownership=cpu signature=55aa stale_rejected=1"
        not in lines
    ):
        raise RuntimeError(f"{cpus}-CPU run is missing post-firmware virtio DMA evidence")

    kernel = next(
        (KERNEL_PATTERN.match(line) for line in lines if KERNEL_PATTERN.match(line)), None
    )
    expected_failed = 0 if failed_ap is None else 1
    expected_failed_apic = 2**32 - 1 if failed_ap is None else failed_ap
    if kernel is None or tuple(map(int, kernel.groups())) != (
        online,
        expected_failed,
        expected_failed_apic,
    ):
        raise RuntimeError(f"{cpus}-CPU run did not enter the freestanding kernel")
    if failed_ap is not None:
        failure_marker = (
            f"THERMITE_AP_FAILURE apic_id={failed_ap} state=Failed "
            f"reason=injected online={online}"
        )
        if failure_marker not in lines:
            raise RuntimeError(f"{cpus}-CPU run lacks its named AP failure transition")
    post_scheduler = next(
        (POST_SCHED_PATTERN.match(line) for line in lines if POST_SCHED_PATTERN.match(line)),
        None,
    )
    if post_scheduler is None:
        raise RuntimeError(f"{cpus}-CPU post-firmware scheduler marker is absent")
    post_tasks, post_sum, post_workers, ap_workers, parallel_cpus, post_locks = map(
        int, post_scheduler.groups()
    )
    if (
        post_tasks != 4_096
        or post_sum != 4_096 * 4_095 // 2
        or post_workers != online
        or ap_workers != online - 1
        or parallel_cpus != online
        or post_locks != online
    ):
        raise RuntimeError(
            f"{cpus}-CPU post-firmware scheduler invariants failed: "
            f"{post_scheduler.group(0)}"
        )
    post_ipi = next(
        (POST_IPI_PATTERN.match(line) for line in lines if POST_IPI_PATTERN.match(line)), None
    )
    if post_ipi is None or int(post_ipi.group(1)) != online - 1:
        raise RuntimeError(f"{cpus}-CPU post-firmware IPI epoch is incomplete")
    timer = next(
        (TIMER_PATTERN.match(line) for line in lines if TIMER_PATTERN.match(line)), None
    )
    if timer is None or int(timer.group(1)) != online:
        raise RuntimeError(f"{cpus}-CPU timer interrupt coverage is incomplete")
    tlb = next((TLB_PATTERN.match(line) for line in lines if TLB_PATTERN.match(line)), None)
    if tlb is None or int(tlb.group(1)) != online:
        raise RuntimeError(f"{cpus}-CPU TLB shootdown evidence is incomplete")
    if any(line.startswith("THERMITE_FAIL") for line in lines):
        raise RuntimeError(f"{cpus}-CPU run emitted a failure marker")


def boot_once(
    image: Path,
    cpus: int,
    output_dir: Path,
    timeout: float,
    failure_ap: int | None = None,
    power_action: str = "poweroff",
) -> None:
    code, variables = firmware_paths()
    with tempfile.TemporaryDirectory(prefix=f"thermite-qemu-{cpus}-") as scratch_name:
        variables_copy = Path(scratch_name) / "OVMF_VARS.fd"
        shutil.copyfile(variables, variables_copy)
        command = [
            "qemu-system-x86_64",
            "-machine", "q35,accel=tcg",
            "-cpu", "max,-x2apic",
            "-smp", str(cpus),
            "-m", "256M",
            "-nodefaults",
            "-no-reboot",
            "-display", "none",
            "-serial", "stdio",
            "-monitor", "none",
            "-drive", f"if=pflash,format=raw,readonly=on,file={code}",
            "-drive", f"if=pflash,format=raw,file={variables_copy}",
            "-drive", f"format=raw,if=none,id=thermite-disk,file={image}",
            "-device", "virtio-blk-pci,drive=thermite-disk,disable-modern=on",
        ]
        if failure_ap is not None:
            command.extend(
                ["-fw_cfg", f"name=opt/thermite/fail-ap,string={failure_ap}"]
            )
        if power_action != "poweroff":
            command.extend(
                ["-fw_cfg", f"name=opt/thermite/power,string={power_action}"]
            )
        process = subprocess.Popen(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            encoding="utf-8",
            errors="replace",
            env={**os.environ, "TZ": "UTC"},
        )
        assert process.stdout is not None
        thermite_lines: list[str] = []
        deadline = time.monotonic() + timeout
        success_seen = False
        terminal_observed = False
        try:
            while time.monotonic() < deadline:
                readable, _, _ = select.select([process.stdout], [], [], 0.1)
                if not readable:
                    if process.poll() is not None:
                        terminal_observed = success_seen
                        break
                    continue
                line = process.stdout.readline()
                if not line:
                    if process.poll() is not None:
                        terminal_observed = success_seen
                        break
                    continue
                clean = line.strip().replace("\r", "")
                marker = clean.find("THERMITE_")
                if marker >= 0:
                    thermite_line = clean[marker:]
                    thermite_lines.append(thermite_line)
                    scenario = (
                        "failure" if failure_ap is not None
                        else "reboot" if power_action == "reboot"
                        else "nominal"
                    )
                    print(f"cpu={cpus} scenario={scenario}: {thermite_line}")
                    if thermite_line == "THERMITE_SUCCESS gate=boot-smp-v1":
                        success_seen = True
                        deadline = min(deadline, time.monotonic() + 3.0)
        finally:
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=3)
        if not terminal_observed:
            raise RuntimeError(
                f"{cpus}-CPU {power_action} action did not terminate the virtual machine"
            )
        validate_transcript(
            thermite_lines,
            cpus,
            expected_online=cpus - 1 if failure_ap is not None else cpus,
            failed_ap=failure_ap,
            power_action=power_action,
        )
        output_dir.mkdir(parents=True, exist_ok=True)
        suffix = (
            "-failure" if failure_ap is not None
            else "-reboot" if power_action == "reboot"
            else ""
        )
        (output_dir / f"boot-{cpus}{suffix}.log").write_text(
            "\n".join(thermite_lines) + "\n", encoding="utf-8", newline="\n"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("--output-dir", type=Path, default=Path("dist/boot-evidence"))
    parser.add_argument("--timeout", type=float, default=20.0)
    parser.add_argument("--cpus", type=int, nargs="+", default=list(CPU_MATRIX))
    args = parser.parse_args()
    if not args.image.is_file():
        parser.error(f"image does not exist: {args.image}")
    if tuple(args.cpus) != CPU_MATRIX and not os.environ.get("THERMITE_ALLOW_PARTIAL_MATRIX"):
        parser.error("the release matrix must be exactly: 1 2 4 8")
    for cpus in args.cpus:
        boot_once(args.image.resolve(), cpus, args.output_dir.resolve(), args.timeout)
    if tuple(args.cpus) == CPU_MATRIX:
        boot_once(
            args.image.resolve(),
            4,
            args.output_dir.resolve(),
            args.timeout,
            failure_ap=3,
        )
        boot_once(
            args.image.resolve(),
            2,
            args.output_dir.resolve(),
            args.timeout,
            power_action="reboot",
        )
    print("THERMITE_QEMU_MATRIX_SUCCESS cpus=" + ",".join(map(str, args.cpus)))
    return 0


if __name__ == "__main__":
    sys.exit(main())
