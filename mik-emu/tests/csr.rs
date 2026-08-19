use mik_emu::{encode, run};

#[test]
fn rdcsr_and_wrcsr_round_trip() {
    // Write 0xDEAD_BEEF to PTBR (CSR 0), read it back, and print the low byte.
    let code = vec![
        // 0: li x1, 0xDEADBEEF
        encode(0x01, 1, 0, 0, 0xDEADBEEF),
        // 1: wrcsr PTBR, x1
        encode(0x12, 0, 1, 0, 0),
        // 2: rdcsr x2, PTBR
        encode(0x11, 2, 0, 0, 0),
        // 3: store8 [0x1000], x2   (print low byte = 0xEF)
        encode(0x09, 0, 0, 2, 0x1000),
        // 4: wrcsr PMODE, x0       (CSR 1 = 0; don't enable paging here)
        encode(0x12, 0, 0, 0, 1),
        // 5: rdcsr x3, PMODE
        encode(0x11, 3, 0, 0, 1),
        // 6: store8 [0x1000], x3   (print 0)
        encode(0x09, 0, 0, 3, 0x1000),
        // 7: halt
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
    // 0xEF = 239 (low byte of PTBR), then 0x00 (PMODE value)
    assert_eq!(output, [0xEF, 0x00]);
}

#[test]
fn sfence_is_accepted() {
    // SFENCE should execute without error.
    let code = vec![
        // 0: sfence
        encode(0x13, 0, 0, 0, 0),
        // 1: halt
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
}
