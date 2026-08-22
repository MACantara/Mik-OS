use mik_emu::{encode, run};

#[test]
fn trap_calls_vector_and_eret_returns() {
    let load_addr = 0x400000_u64;
    let handler = load_addr + 5 * 8;

    let code = vec![
        // 0: set the trap vector to the handler
        encode(0x01, 1, 0, 0, handler as i64),
        encode(0x0A, 0, 0, 1, 0x2000),
        // 2: put the argument in x2 and trap
        encode(0x01, 2, 0, 0, b'A' as i64),
        encode(0x0E, 0, 0, 0, 1),
        // 4: returned here after eret
        encode(0x00, 0, 0, 0, 0),
        // 5: handler: print the argument and return
        encode(0x09, 0, 0, 2, 0x1000),
        encode(0x10, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "A");
}

#[test]
fn sret_enters_user_mode_and_trap_returns() {
    // Supervisor program at 0x400000:
    //   - set trap vector
    //   - SRET to user_start
    // User program at user_start:
    //   - TRAP 1 (print 'U')
    //   - TRAP 0 (halt)
    // Handler:
    //   - print x2
    //   - ERET
    let load_addr = 0x400000_u64;
    let handler = load_addr + 6 * 8;
    let user_start = load_addr + 8 * 8;

    let code = vec![
        // 0: set the trap vector to the handler
        encode(0x01, 1, 0, 0, handler as i64),
        encode(0x0A, 0, 0, 1, 0x2000),
        // 2: set user pc and SRET
        encode(0x01, 1, 0, 0, user_start as i64),
        encode(0x14, 0, 1, 0, 0), // SRET x1
        // 4: padding
        encode(0x00, 0, 0, 0, 0),
        // 5: padding
        encode(0x00, 0, 0, 0, 0),
        // 6: handler: print the argument and return
        encode(0x09, 0, 0, 2, 0x1000),
        encode(0x10, 0, 0, 0, 0),
        // 7: user_start: TRAP 1 to print, then HALT
        encode(0x01, 2, 0, 0, b'U' as i64),
        encode(0x0E, 0, 0, 0, 1),
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "U");
}
