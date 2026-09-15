// BIOS boot sector for the disk-image boot path. The BIOS loads this 512-byte
// sector at 0x7C00, checks the 0xAA55 signature, and jumps in with DL = boot
// drive. It loads the rest of the image (stage2 + flat kernel) to 0x7E00 via
// an INT 13h EDD LBA read, enables A20, enters 32-bit protected mode, and far
// jumps to stage2's pm_entry.
//
// Image format: sector 0 is this sector; sectors 1..N are stage2 immediately
// followed by the flat kernel. The builder pads the image to 64 KiB, so a
// fixed 127-sector read is always in bounds.

.section .boot16, "ax"
.code16
.global boot16_start
boot16_start:
    cli
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov sp, 0x7C00              // stack grows down into free conventional memory
    sti

    // INT 13h AH=42h (EDD LBA read): read sectors 1..127 into 0x7E00.
    // DL still holds the boot drive handed to us by the BIOS.
    mov si, offset dap          // ds:si = disk address packet
    mov ah, 0x42
    int 0x13
    jc boot_fail

    // INT 15h E820 memory map -> phys 0x5000: u32 magic 'MMAP', u32 count,
    // then 24-byte entries (base u64, length u64, type u32, acpi u32). This
    // must happen in real mode — BIOS calls vanish once CR0.PE is set. Each
    // call writes one entry at es:di and advances ebx; ebx==0 means done.
    mov dword ptr [0x5000], 0x50414D4D  // 'MMAP' magic
    mov di, 0x5008
    xor bp, bp                  // entry count (BIOS does not advance di;
                                // SeaBIOS preserves it, so a running di is ok)
    xor ebx, ebx                // continuation token
e820_loop:
    mov eax, 0xE820
    mov ecx, 24
    mov edx, 0x534D4150         // 'SMAP'
    int 0x15
    jc e820_done                // CF: unsupported, or read error
    cmp eax, 0x534D4150         // BIOS must echo SMAP in eax
    jne e820_done
    inc bp
    add di, 24
    test ebx, ebx
    jnz e820_loop
e820_done:
    mov [0x5004], bp            // u32 count at 0x5004

    // Enable A20 via the port 0x92 fast gate: set bit 1, keep bit 0 (fast
    // reset) clear.
    in al, 0x92
    or al, 0x02
    and al, 0xFE
    out 0x92, al

    // Enter protected mode: load the flat 32-bit GDT, set CR0.PE, then far
    // jump to stage2. The jump is encoded by hand (66 EA ptr16:32) because
    // the integrated assembler has no far-jmp mnemonic in 16-bit code.
    lgdt [gdt32_ptr]
    mov eax, cr0
    or al, 0x01
    mov cr0, eax
    .byte 0x66, 0xEA
    .long pm_entry
    .word 0x08                  // code32 selector

boot_fail:
    // A disk failure would otherwise hang silently; put 'E' on COM1 first.
    mov dx, 0x3F8
    mov al, 'E'
    out dx, al
    cli
1:  hlt
    jmp 1b

.align 16
dap:                            // INT 13h AH=42h disk address packet
    .byte 0x10                  // packet size
    .byte 0                     // reserved
    .word 127                   // sectors to read (image is padded to 64 KiB)
    .word 0x7E00                // destination offset
    .word 0x0000                // destination segment
    .quad 1                     // first LBA (sector 0 is this boot sector)

.align 8
gdt32:                          // flat 4 GiB code32/data32 for the mode switch
    .quad 0x0000000000000000    // null descriptor
    .quad 0x00cf9a000000ffff    // code32: base 0, limit 4 GiB, D=1, exec/read
    .quad 0x00cf92000000ffff    // data32: base 0, limit 4 GiB, read/write
gdt32_ptr:
    .word gdt32_ptr - gdt32 - 1
    .long gdt32

.org 0x1F8
.global kernel_len
kernel_len:
    .long 0                     // patched by the image builder: kernel flat size

.org 0x1FE
    .word 0xAA55                // BIOS boot signature
