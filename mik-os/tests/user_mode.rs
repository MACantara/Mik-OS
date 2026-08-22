use mik_os::kernel_user_mode;
use mik_emu::{Machine, CSR_PMODE, CSR_PTBR};

#[test]
fn mik_os_runs_user_mode_program() {
    let mut m = Machine::new();
    m.load_binary(&kernel_user_mode());
    let mut output = Vec::new();
    while !m.halted {
        m.step(&mut output).expect("emulator should run");
    }
    assert_eq!(m.exit_code, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "!U");
    assert_ne!(m.csrs[CSR_PTBR as usize], 0, "kernel should set PTBR");
    assert_eq!(m.csrs[CSR_PMODE as usize], 1, "kernel should enable paging");
    assert!(!m.user_mode, "machine should halt in supervisor mode");
}
