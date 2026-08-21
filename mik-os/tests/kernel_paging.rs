use mik_os::kernel_binary;
use mik_emu::{Machine, CSR_PMODE, CSR_PTBR};

#[test]
fn mik_os_sets_up_paging() {
    let mut m = Machine::new();
    m.load_binary(&kernel_binary());
    let mut output = Vec::new();
    while !m.halted {
        m.step(&mut output).expect("emulator should run");
    }
    assert_eq!(m.exit_code, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "!?Mik OS\n");
    // Paging should be enabled and PTBR should be non-zero.
    assert_ne!(m.csrs[CSR_PTBR as usize], 0, "kernel should set PTBR");
    assert_eq!(m.csrs[CSR_PMODE as usize], 1, "kernel should enable paging");
}
