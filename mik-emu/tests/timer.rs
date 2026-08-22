use mik_emu::{encode, run, CSR_TIMER};

#[test]
fn int_and_iret_round_trip() {
    let load_addr = 0x400000_u64;
    let handler = load_addr + 6 * 8;

    let code = vec![
        // 0: set the interrupt vector to the handler
        encode(0x01, 1, 0, 0, handler as i64),
        encode(0x0A, 0, 0, 1, 0x2020),
        // 2: put 'I' in x2 and issue a software interrupt
        encode(0x01, 2, 0, 0, b'I' as i64),
        encode(0x15, 0, 0, 0, 0),
        // 4: returned here after IRET
        encode(0x00, 0, 0, 0, 0),
        // 5: padding
        encode(0x00, 0, 0, 0, 0),
        // 6: handler: print the argument and return
        encode(0x09, 0, 0, 2, 0x1000),
        encode(0x16, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "I");
}

#[test]
fn timer_fires_three_times_and_halt() {
    let load_addr = 0x400000_u64;
    let handler = load_addr + 8 * 8;
    let counter_addr = 0x700010_u64;

    let code = vec![
        // 0: set the interrupt vector
        encode(0x01, 1, 0, 0, handler as i64),
        encode(0x0A, 0, 0, 1, 0x2020),
        // 2: initialize tick counter to 0
        encode(0x0A, 0, 0, 0, counter_addr as i64),
        // 3: set timer interval to 50 and start spinning
        // ponytail: the interval must be larger than the handler so the timer
        // does not fire again before the first IRET.
        encode(0x01, 1, 0, 0, 50),
        encode(0x12, 0, 1, 0, CSR_TIMER as i64),
        // 5: JMP 0 (infinite loop)
        encode(0x0D, 0, 0, 0, 0),
        // 6-7: padding
        encode(0x00, 0, 0, 0, 0),
        encode(0x00, 0, 0, 0, 0),
        // 8: handler
        // Increment tick counter
        encode(0x08, 1, 0, 0, counter_addr as i64),
        encode(0x03, 1, 1, 0, 1),
        encode(0x0A, 0, 0, 1, counter_addr as i64),
        // Print 'T'
        encode(0x01, 2, 0, 0, b'T' as i64),
        encode(0x09, 0, 0, 2, 0x1000),
        // If counter == 3, halt; otherwise IRET
        encode(0x01, 2, 0, 0, 3),
        // BEQ x1, x2, +3 -> index 15 (HALT)
        encode(0x0B, 0, 1, 2, 3),
        encode(0x16, 0, 0, 0, 0),
        // 14: padding (skipped when branch is taken)
        encode(0x00, 0, 0, 0, 0),
        // 15: HALT
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "TTT");
}
