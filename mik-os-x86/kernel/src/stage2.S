// Stage2 loader for the disk-image boot path. The boot sector loads this blob
// at 0x7E00 and far jumps to pm_entry with CS = a flat 32-bit code segment.
// It copies the flat kernel image (which follows stage2 in the loaded blob)
// to its link address 0x400000, zeroes the kernel .bss, and jumps to the
// kernel's own _start — delivering exactly the handoff state PVH provides:
// 32-bit protected mode, paging off, flat segments.

.section .stage2, "ax"
.code32
.global pm_entry
pm_entry:
    mov ax, 0x10                // data32 selector from the boot sector's GDT
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov esp, 0x90000            // scratch stack in conventional memory

    // Copy the flat kernel to 0x400000. Source = stage2_end (the byte after
    // this blob), length = kernel_len patched into the boot sector header.
    mov esi, offset stage2_end
    mov edi, 0x400000
    mov ecx, [0x7DF8]           // kernel_len at boot sector offset 0x1F8
    rep movsb

    // Zero the kernel .bss (page tables and stack live there).
    mov edi, offset __bss_start
    mov ecx, offset __bss_end
    sub ecx, edi
    xor eax, eax
    rep stosb

    mov eax, offset _start      // kernel 32-bit entry VMA
    jmp eax
.global stage2_end
stage2_end:
