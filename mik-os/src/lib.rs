//! Mik-64 kernel source for the MVP.
//!
//! Because Mik-64 is a custom ISA, the kernel is hand-assembled here using
//! `mik_emu::encode`. This crate produces the flat binary that the emulator
//! loads. The real Rust Mik OS for x86-64 will come later.

use mik_emu::encode;

/// Return the flat Mik-64 kernel binary for the MVP.
pub fn kernel_binary() -> Vec<u8> {
    let load_addr = 0x400000_u64;
    let string_addr = load_addr + 33 * 8;
    let syscall_handler = load_addr + 26 * 8;
    let after_alloc = load_addr + 6 * 8;
    let after_free = load_addr + 12 * 8;

    // Hand-assembled Mik-64 kernel:
    //
    //     ; initialize the bump-allocator's next_page pointer
    //     li   x1, 0x701000
    //     store64 [0x700000], x1
    //
    //     ; set the trap vector to the syscall handler
    //     li   x1, syscall_handler
    //     store64 [0x2000], x1
    //
    //     ; allocate a page, write '!' to it, and print it
    //     li   x10, after_alloc
    //     jmp  alloc_page
    // after_alloc:
    //     li   x3, '!'
    //     store8 [x2], x3
    //     load8 x3, [x2]
    //     store8 [0x1000], x3
    //
    //     ; free_page is a no-op for the bump allocator
    //     li   x10, after_free
    //     jmp  free_page
    // after_free:
    //     ; use a syscall to print '?' (syscall 1 = print char in x2)
    //     li   x2, '?'
    //     trap 1
    //
    //     ; print "Mik OS\n" directly
    //     li   x1, string
    // loop:
    //     load8 x2, [x1]
    //     beq  x2, x0, done
    //     store8 [0x1000], x2
    //     addi x1, x1, 1
    //     jmp  loop
    // done:
    //     ; syscall 0 = halt
    //     trap 0
    //
    // alloc_page:
    //     load64 x2, [0x700000]
    //     addi x3, x2, 0x1000
    //     store64 [0x700000], x3
    //     jmpr x10
    //
    // free_page:
    //     jmpr x10
    //
    // syscall_handler:
    //     beq  x10, x0, sys_halt
    //     li   x3, 1
    //     beq  x10, x3, sys_print_char
    //     eret
    // sys_print_char:
    //     store8 [0x1000], x2
    //     eret
    // sys_halt:
    //     halt
    //
    // string:
    //     .asciz "Mik OS\n"
    let code = vec![
        // 0: initialize next_page = 0x701000
        encode(0x01, 1, 0, 0, 0x701000),
        // 1: store it at 0x700000
        encode(0x0A, 0, 0, 1, 0x700000),
        // 2: set the trap vector to the syscall handler
        encode(0x01, 1, 0, 0, syscall_handler as i64),
        // 3: store it at 0x2000
        encode(0x0A, 0, 0, 1, 0x2000),
        // 4: set up return address and call alloc_page
        encode(0x01, 10, 0, 0, after_alloc as i64),
        // 5: jmp alloc_page
        encode(0x0D, 0, 0, 0, 16),
        // 6: after_alloc: write '!' to the page and print it
        encode(0x01, 3, 0, 0, b'!' as i64),
        encode(0x09, 0, 2, 3, 0),
        encode(0x07, 3, 2, 0, 0),
        encode(0x09, 0, 0, 3, 0x1000),
        // 10: set up return and call free_page
        encode(0x01, 10, 0, 0, after_free as i64),
        // 11: jmp free_page
        encode(0x0D, 0, 0, 0, 14),
        // 12: after_free: syscall to print '?'
        encode(0x01, 2, 0, 0, b'?' as i64),
        // 13: trap 1
        encode(0x0E, 0, 0, 0, 1),
        // 14: print "Mik OS\n" from the embedded string
        encode(0x01, 1, 0, 0, string_addr as i64),
        // 15: loop: load8 x2, [x1]
        encode(0x07, 2, 1, 0, 0),
        // 16: beq x2, x0, done
        encode(0x0B, 0, 2, 0, 4),
        // 17: store8 [0x1000], x2
        encode(0x09, 0, 0, 2, 0x1000),
        // 18: addi x1, x1, 1
        encode(0x03, 1, 1, 0, 1),
        // 19: jmp loop
        encode(0x0D, 0, 0, 0, -4),
        // 20: done: trap 0 (halt)
        encode(0x0E, 0, 0, 0, 0),
        // 21: alloc_page: load64 x2, [0x700000]
        encode(0x08, 2, 0, 0, 0x700000),
        // 22: addi x3, x2, 0x1000
        encode(0x03, 3, 2, 0, 0x1000),
        // 23: store64 [0x700000], x3
        encode(0x0A, 0, 0, 3, 0x700000),
        // 24: jmpr x10
        encode(0x0F, 0, 10, 0, 0),
        // 25: free_page: jmpr x10
        encode(0x0F, 0, 10, 0, 0),
        // 26: syscall_handler: beq x10, x0, sys_halt
        encode(0x0B, 0, 10, 0, 6),
        // 27: li x3, 1
        encode(0x01, 3, 0, 0, 1),
        // 28: beq x10, x3, sys_print_char
        encode(0x0B, 0, 10, 3, 2),
        // 29: eret (unknown syscall)
        encode(0x10, 0, 0, 0, 0),
        // 30: sys_print_char: store8 [0x1000], x2
        encode(0x09, 0, 0, 2, 0x1000),
        // 31: eret
        encode(0x10, 0, 0, 0, 0),
        // 32: sys_halt: halt
        encode(0x00, 0, 0, 0, 0),
    ];

    assert_eq!(code.len(), 33, "code length must match string address calculation");

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program.extend_from_slice(b"Mik OS\n\0");
    program
}
