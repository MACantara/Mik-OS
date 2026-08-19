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
