use mik_os::kernel_pagefault;
use mik_emu::{Machine, CSR_PMODE, CSR_PTBR};

#[test]
fn mik_os_kernel_handles_page_fault() {
    let mut m = Machine::new();
    m.load_binary(&kernel_pagefault());
    let mut output = Vec::new();
    while !m.halted {
        m.step(&mut output).expect("emulator should run");
    }
    assert_eq!(m.exit_code, 0);
    // Demo '!' is printed before the fault; the handler prints "F1".
    assert_eq!(String::from_utf8(output).unwrap(), "!F1");
    assert_ne!(m.csrs[CSR_PTBR as usize], 0, "kernel should set PTBR");
    assert_eq!(m.csrs[CSR_PMODE as usize], 1, "kernel should enable paging");
}
