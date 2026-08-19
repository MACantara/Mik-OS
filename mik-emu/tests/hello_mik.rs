use mik_emu::{encode, run};

#[test]
fn hello_mik() {
    let load_addr = 0x400000_u64;
    let string_addr = load_addr + 7 * 8;

    let code = vec![
        encode(0x01, 1, 0, 0, string_addr as i64),
        encode(0x07, 2, 1, 0, 0),
        encode(0x0B, 0, 2, 0, 4),
        encode(0x09, 0, 0, 2, 0x1000),
        encode(0x03, 1, 1, 0, 1),
        encode(0x0D, 0, 0, 0, -4),
        encode(0x00, 0, 0, 0, 0),
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program.extend_from_slice(b"Hello, Mik!\n\0");

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");

    assert_eq!(exit, 0, "expected HALT exit code 0");
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "Hello, Mik!\n",
        "emulator output should match the test program"
    );
}
