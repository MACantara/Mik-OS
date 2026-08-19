use mik_emu::{encode, run};

#[test]
fn jmpr_jumps_to_register() {
    let load_addr = 0x400000_u64;
    let target = load_addr + 3 * 8;

    let code = vec![
        encode(0x01, 1, 0, 0, target as i64), // li x1, target
        encode(0x0F, 0, 1, 0, 0),              // jmpr x1
        encode(0x00, 0, 0, 0, 0),              // halt (should be skipped)
        encode(0x00, 0, 0, 0, 0),              // halt (target)
    ];

    let mut program: Vec<u8> = Vec::new();
    for word in code {
        program.extend_from_slice(&word.to_le_bytes());
    }

    let mut output = Vec::new();
    let exit = run(&program, &mut output).expect("emulator should run");
    assert_eq!(exit, 0);
}
