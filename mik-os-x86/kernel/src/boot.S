.section .note.Xen, "a"
.align 4
    .long 4                  # namesz
    .long 4                  # descsz
    .long 18                 # type = XEN_ELFNOTE_PHYS32_ENTRY
    .ascii "Xen\0"
    .align 4
    .long _start             # 32-bit physical entry point

.section .text, "ax"
.code32
.global _start
_start:
    cli
    lea esp, [stack_top]

    # identity map the first 2 MiB with a huge page
    lea eax, [pdpt]
    or eax, 0x03
    mov [pml4], eax

    lea eax, [pd]
    or eax, 0x03
    mov [pdpt], eax

    mov eax, 0x83
    mov [pd], eax

    # enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    # load PML4
    lea eax, [pml4]
    mov cr3, eax

    # enable long mode (EFER.LME)
    mov ecx, 0xC0000080
    rdmsr
    or eax, 1 << 8
    wrmsr

    # enable paging
    mov eax, cr0
    or eax, 1 << 31
    mov cr0, eax

    # load the 64-bit GDT
    lgdt [gdt64_ptr]

    # far return into long mode, setting CS to 0x08
    push 0x08
    lea eax, [long_mode_start]
    push eax
    retf

.code64
long_mode_start:
    mov ax, 0x10
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax

    call kmain

    cli
hang:
    hlt
    jmp hang

.section .rodata
.align 8
gdt64:
    .quad 0x0000000000000000
    .set gdt64_code, . - gdt64
    .quad 0x00af9a000000ffff
    .set gdt64_data, . - gdt64
    .quad 0x00af92000000ffff
gdt64_ptr:
    .word gdt64_ptr - gdt64 - 1
    .long gdt64

.section .bss
.align 4096
pml4:
    .space 4096
pdpt:
    .space 4096
pd:
    .space 4096
stack_bottom:
    .space 16384
stack_top:
