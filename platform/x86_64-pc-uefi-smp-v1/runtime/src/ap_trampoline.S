.section .trampoline,"ax"
.balign 16
.code16
.global thermite_ap_trampoline_start
.global thermite_ap_trampoline_end

/*
 * The profile reserves physical page 0x80000 for this position-independent
 * startup stub.  Absolute far targets are therefore fixed profile data, not
 * linker addresses.  The BSP writes CR3, the Rust AP entry, and one stack top
 * per xAPIC ID into the parameter area beginning at 0x80c00.
 */
thermite_ap_trampoline_start:
    cli
    cld
    movw %cs, %ax
    movw %ax, %ds
    movw %ax, %es
    lgdtw thermite_ap_gdt_pointer - thermite_ap_trampoline_start
    movl %cr0, %eax
    orl $1, %eax
    movl %eax, %cr0
    ljmpl $0x08, $(0x80000 + thermite_ap_protected - thermite_ap_trampoline_start)

.code32
thermite_ap_protected:
    movw $0x10, %ax
    movw %ax, %ds
    movw %ax, %es
    movw %ax, %ss
    movl $0x80bf0, %esp

    movl 0x80c00, %eax
    movl %eax, %cr3
    movl %cr4, %eax
    orl $(1 << 5), %eax
    movl %eax, %cr4

    movl $0xc0000080, %ecx
    rdmsr
    orl $((1 << 8) | (1 << 11)), %eax
    wrmsr

    movl %cr0, %eax
    orl $(1 << 31), %eax
    movl %eax, %cr0
    ljmpl $0x18, $(0x80000 + thermite_ap_long - thermite_ap_trampoline_start)

.code64
thermite_ap_long:
    movl $1, %eax
    cpuid
    shrl $24, %ebx
    andl $63, %ebx
    movq 0x80c10(,%rbx,8), %rsp
    testq %rsp, %rsp
    jz thermite_ap_park
    andq $-16, %rsp
    subq $32, %rsp
    xorq %rbp, %rbp
    movl %ebx, %ecx
    movq 0x80c08, %rax
    callq *%rax

thermite_ap_park:
    cli
1:
    hlt
    jmp 1b

.balign 8
thermite_ap_gdt:
    .quad 0x0000000000000000
    .quad 0x00cf9a000000ffff
    .quad 0x00cf92000000ffff
    .quad 0x00af9a000000ffff
thermite_ap_gdt_end:
thermite_ap_gdt_pointer:
    .word thermite_ap_gdt_end - thermite_ap_gdt - 1
    .long 0x80000 + thermite_ap_gdt - thermite_ap_trampoline_start

thermite_ap_trampoline_end:
