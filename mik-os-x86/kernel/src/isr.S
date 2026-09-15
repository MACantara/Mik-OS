// Minimal exception stubs for the x86-64 IDT. Each stub pushes its vector
// number and jumps to isr_common, which prints "EX" + the vector as two hex
// digits on COM1 and halts. This is a panic-style handler: it never returns,
// so it does not need to care which vectors carry a CPU-pushed error code.

.section .text, "ax"
.code64

.macro ISR num
.global isr_\num
isr_\num:
    push \num
    jmp isr_common
.endm

.align 8
.global isr_table
isr_table:
    .quad isr_0
    .quad isr_1
    .quad isr_2
    .quad isr_3
    .quad isr_4
    .quad isr_5
    .quad isr_6
    .quad isr_7
    .quad isr_8
    .quad isr_9
    .quad isr_10
    .quad isr_11
    .quad isr_12
    .quad isr_13
    .quad isr_14
    .quad isr_15
    .quad isr_16
    .quad isr_17
    .quad isr_18
    .quad isr_19
    .quad isr_20
    .quad isr_21
    .quad isr_22
    .quad isr_23
    .quad isr_24
    .quad isr_25
    .quad isr_26
    .quad isr_27
    .quad isr_28
    .quad isr_29
    .quad isr_30
    .quad isr_31

    ISR 0
    ISR 1
    ISR 2
    ISR 3
    ISR 4
    ISR 5
    ISR 6
    ISR 7
    ISR 8
    ISR 9
    ISR 10
    ISR 11
    ISR 12
    ISR 13
    ISR 14
    ISR 15
    ISR 16
    ISR 17
    ISR 18
    ISR 19
    ISR 20
    ISR 21
    ISR 22
    ISR 23
    ISR 24
    ISR 25
    ISR 26
    ISR 27
    ISR 28
    ISR 29
    ISR 30
    ISR 31

isr_common:
    pop rbx                     // vector number pushed by the stub
    mov dx, 0x3F8               // COM1
    mov al, 'E'
    out dx, al
    mov al, 'X'
    out dx, al
    mov eax, ebx
    shr eax, 4
    call print_nibble           // high nibble
    mov eax, ebx
    and eax, 0xF
    call print_nibble           // low nibble
    mov al, 0x0A                // '\n'
    out dx, al
    cli
1:  hlt
    jmp 1b

// al = nibble 0..15, dx = COM1. Clobbers al.
print_nibble:
    cmp al, 10
    jb 1f
    add al, 'A' - '0' - 10
1:  add al, '0'
    out dx, al
    ret

// ---------------------------------------------------------------------------
// Returning handlers: timer preemption (vector 32) and syscalls (int 0x80).
// The Rust handler receives rsp pointing at an IrqFrame (15 GPRs then the
// CPU iret frame) and returns the frame to resume in — possibly another
// process's, which is what makes this a context switch.
// ---------------------------------------------------------------------------

.macro SAVE_REGS
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
.endm

.macro RESTORE_REGS
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
.endm

.global isr_timer
isr_timer:
    SAVE_REGS
    mov rdi, rsp
    call timer_handler
    mov rsp, rax
    jmp irq_tail

.global isr_syscall
isr_syscall:
    SAVE_REGS
    mov rdi, rsp
    call syscall_handler
    mov rsp, rax
    jmp irq_tail

.global irq_tail
irq_tail:
    RESTORE_REGS
    iretq

// First entry into ring 3: rdi = a fabricated IrqFrame on a process kernel
// stack. Reuses the same pop+iretq path real interrupts take.
.global enter_user
enter_user:
    mov rsp, rdi
    jmp irq_tail

// Load the new GDT's data segments, far-return to refresh CS (0x08), and
// load the task register for the TSS at selector 0x28.
.global seg_reload
seg_reload:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    push 0x08
    lea rax, [rip + 1f]
    push rax
    retfq
1:
    mov ax, 0x28
    ltr ax
    ret
