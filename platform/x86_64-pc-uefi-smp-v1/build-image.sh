#!/usr/bin/env bash
set -euo pipefail

profile_root=$(cd "$(dirname "$0")" && pwd)
workspace_root=$(cd "$profile_root/../.." && pwd)
runtime_manifest="$profile_root/runtime/Cargo.toml"
output_path=${1:-"$workspace_root/dist/thermite-kernel.img"}
source_epoch=${SOURCE_DATE_EPOCH:-1704067200}
policy_rlib=${THERMITE_VERIFIED_POLICY_RLIB:?THERMITE_VERIFIED_POLICY_RLIB is required}
policy_deps=${THERMITE_VERIFIED_POLICY_DEPS:?THERMITE_VERIFIED_POLICY_DEPS is required}

if [[ ! -f "$policy_rlib" || ! -d "$policy_deps" ]]; then
    echo "verified Thermite policy artifact or dependency closure is absent" >&2
    exit 2
fi
policy_root=$(cd "$(dirname "$policy_rlib")/.." && pwd)

case "$source_epoch" in
    ''|*[!0-9]*)
        echo "SOURCE_DATE_EPOCH must be a non-negative integer" >&2
        exit 2
        ;;
esac
if (( source_epoch < 315532800 )); then
    echo "SOURCE_DATE_EPOCH must be representable by FAT (1980-01-01 or later)" >&2
    exit 2
fi

for tool in cargo mkfs.fat mmd mcopy sha256sum llvm-objdump llvm-nm llvm-pdbutil file; do
    command -v "$tool" >/dev/null || {
        echo "required image tool is missing: $tool" >&2
        exit 2
    }
done

scratch=$(mktemp -d "${TMPDIR:-/tmp}/thermite-kernel-image.XXXXXX")
cleanup() {
    rm -rf -- "$scratch"
}
trap cleanup EXIT

export SOURCE_DATE_EPOCH="$source_epoch"
export TZ=UTC
encoded_flags=$'-C\x1fpanic=abort\x1f-C\x1fno-redzone=yes\x1f-C\x1ftarget-feature=+sse2\x1f--extern\x1f'
encoded_flags+="thermite_kernel_policy=$policy_rlib"
encoded_flags+=$'\x1f-L\x1f'
encoded_flags+="dependency=$policy_deps"
encoded_flags+=$'\x1f'
encoded_flags+="--remap-path-prefix=$workspace_root=."
encoded_flags+=$'\x1f'
encoded_flags+="--remap-path-prefix=$policy_root=thermite-policy"
encoded_flags+=$'\x1f-C\x1flink-arg=/include:tpl_atomic_fetch'
encoded_flags+=$'\x1f-C\x1flink-arg=/include:tpl_atomic_compare_exchange'
encoded_flags+=$'\x1f-C\x1flink-arg=/include:tpl_atomic_load'
encoded_flags+=$'\x1f-C\x1flink-arg=/include:tpl_atomic_store'
export CARGO_ENCODED_RUSTFLAGS="$encoded_flags"
cargo build --manifest-path "$runtime_manifest" --target x86_64-unknown-uefi --release --locked

runtime_target="$profile_root/runtime/target/x86_64-unknown-uefi/release"
efi_source="$runtime_target/BOOTX64.efi"
pdb_source="$runtime_target/BOOTX64.pdb"
efi_stage="$scratch/BOOTX64.EFI"
pdb_stage="$scratch/thermite-kernel.pdb"
sections_stage="$scratch/thermite-kernel.sections"
symbols_stage="$scratch/thermite-kernel.symbols"
image_stage="$scratch/thermite-kernel.img"
cp "$efi_source" "$efi_stage"
cp "$pdb_source" "$pdb_stage"
touch -d "@$source_epoch" "$efi_stage"
touch -d "@$source_epoch" "$pdb_stage"

efi_kind=$(file -b "$efi_stage")
# libmagic formats this description differently across distributions (for
# example, `for EFI (application)` versus `(EFI application)`). Validate the
# stable semantic fields instead of one host-specific sentence.
if [[ "$efi_kind" != *"PE32+"* ||
      "$efi_kind" != *"EFI"* ||
      "$efi_kind" != *"application"* ||
      "$efi_kind" != *"x86-64"* ]]; then
    echo "UEFI closure has the wrong executable kind: $efi_kind" >&2
    exit 1
fi
section_table=$(llvm-objdump -h "$efi_stage")
printf '%s\n' "$section_table" \
    | sed "s|$efi_stage|thermite-kernel.efi|" \
    >"$sections_stage"
for section in .text .rdata .reloc; do
    grep -F " $section " <<<"$section_table" >/dev/null || {
        echo "UEFI closure is missing required section $section" >&2
        exit 1
    }
done
if grep -F " .idata " <<<"$section_table" >/dev/null; then
    echo "UEFI closure imports hosted symbols through .idata" >&2
    exit 1
fi
undefined_symbols=$(llvm-nm -u "$efi_stage" 2>/dev/null || true)
if [[ -n "$undefined_symbols" ]]; then
    echo "UEFI closure has undefined symbols:" >&2
    echo "$undefined_symbols" >&2
    exit 1
fi

llvm-pdbutil dump -publics "$pdb_stage" >"$symbols_stage"
for symbol in \
    efi_main thermite_ap_trampoline_start thermite_ap_trampoline_end \
    thermite_ap_rust_entry thermite_ipi_handler thermite_timer_handler \
    thermite_page_fault_handler thermite_syscall_entry thermite_enter_user \
    thermite_policy_execute tpl_atomic_compare_exchange tpl_atomic_fetch \
    tpl_atomic_load tpl_atomic_store \
    tpl_clock_read memcpy memmove memset; do
    grep -F "\`$symbol\`" "$symbols_stage" >/dev/null || {
        echo "debug symbol closure is missing required symbol $symbol" >&2
        exit 1
    }
done

truncate -s 64M "$image_stage"
mkfs.fat --invariant -F 32 -i 5448524d -n THERMITE "$image_stage" >/dev/null
mmd -i "$image_stage" ::/EFI ::/EFI/BOOT
mcopy -i "$image_stage" "$efi_stage" ::/EFI/BOOT/BOOTX64.EFI

output_dir=$(dirname "$output_path")
mkdir -p "$output_dir"
output_base=$(basename "$output_path")
published_efi="$output_dir/${output_base%.img}.efi"
published_pdb="$output_dir/${output_base%.img}.pdb"
published_sections="$output_dir/${output_base%.img}.sections"
published_symbols="$output_dir/${output_base%.img}.symbols"
published_receipt="$output_dir/${output_base%.img}.receipt"

image_sha=$(sha256sum "$image_stage" | cut -d' ' -f1)
efi_sha=$(sha256sum "$efi_stage" | cut -d' ' -f1)
pdb_sha=$(sha256sum "$pdb_stage" | cut -d' ' -f1)
sections_sha=$(sha256sum "$sections_stage" | cut -d' ' -f1)
symbols_sha=$(sha256sum "$symbols_stage" | cut -d' ' -f1)
source_closure_sha=$(
    cd "$workspace_root"
    while IFS= read -r relative; do
        sha256sum "$relative"
    done <"platform/x86_64-pc-uefi-smp-v1/source-allowlist.txt" \
        | sha256sum \
        | cut -d' ' -f1
)

receipt_stage="$scratch/receipt"
{
    printf 'schema=ThermitePlatformConformanceReceiptV2\n'
    printf 'profile=x86_64-pc-uefi-smp-v1\n'
    printf 'artifact_class=platform_conformance_demonstration\n'
    printf 'migration_complete=false\n'
    printf 'assurance_scope=platform_conformance_to_boundary\n'
    printf 'source_date_epoch=%s\n' "$source_epoch"
    printf 'canonical_source_allowlist=platform/x86_64-pc-uefi-smp-v1/source-allowlist.txt\n'
    printf 'source_closure_sha256=%s\n' "$source_closure_sha"
    printf 'uefi_sha256=%s\n' "$efi_sha"
    printf 'debug_symbols_sha256=%s\n' "$pdb_sha"
    printf 'section_table_sha256=%s\n' "$sections_sha"
    printf 'symbol_table_sha256=%s\n' "$symbols_sha"
    printf 'image_sha256=%s\n' "$image_sha"
    printf 'image_size=67108864\n'
    printf 'boot_path=EFI/BOOT/BOOTX64.EFI\n'
    printf 'trusted_boundary=firmware,compiler,linker,target-platform-layer\n'
} >"$receipt_stage"

mv "$image_stage" "$output_path"
mv "$efi_stage" "$published_efi"
mv "$pdb_stage" "$published_pdb"
mv "$sections_stage" "$published_sections"
mv "$symbols_stage" "$published_symbols"
mv "$receipt_stage" "$published_receipt"
printf '%s\n' "$output_path"
