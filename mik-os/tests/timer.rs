use mik_os::kernel_timer;
use mik_emu::{Machine, CSR_PMODE, CSR_PTBR};

#[test]
fn mik_os_runs_timer_ticks() {
    let mut m = Machine::new();
    m.load_binary(&kernel_timer());
    let mut output = Vec::new();
    let mut steps = 0;
    while !m.halted {
        m.step(&mut output).expect("emulator should run");
        steps += 1;
        if steps > 50000 {
            panic!(
                "machine did not halt after 50k steps; pc={:#x}, user_mode={}",
                m.pc, m.user_mode
            );
        }
    }
    assert_eq!(m.exit_code, 0, "expected clean halt");
    assert_eq!(String::from_utf8(output).unwrap(), "!TTT");
    assert_ne!(m.csrs[CSR_PTBR as usize], 0, "kernel should set PTBR");
    assert_eq!(m.csrs[CSR_PMODE as usize], 1, "kernel should enable paging");
    assert!(!m.user_mode, "machine should halt in supervisor mode");
}
