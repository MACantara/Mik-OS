use mik_os::kernel_os;
use mik_emu::{Machine, CSR_PMODE, CSR_PTBR};

/// Boot the full Phase-1 kernel: two processes, timer preemption, fork, exec,
/// demand paging, and exit. Expected per-process output order is fixed but the
/// interleaving under the timer is not, so assertions are order-tolerant.
#[test]
fn mik_os_runs_two_processes() {
    let mut m = Machine::new();
    m.load_binary(&kernel_os());
    let mut output = Vec::new();
    let mut steps = 0;
    while !m.halted {
        m.step(&mut output).expect("emulator should run");
        steps += 1;
        if steps > 200_000 {
            panic!(
                "machine did not halt after 200k steps; pc={:#x}, user_mode={}",
                m.pc, m.user_mode
            );
        }
    }
    assert_eq!(m.exit_code, 0, "expected clean halt");

    let out = String::from_utf8(output).unwrap();
    // '!' is the boot demo-page write emitted by build_common; ignore it.
    let out = out.strip_prefix('!').unwrap_or(&out);
    // I (init) -> P (parent) / C (child) -> Q D (prog1, after exec+demand) -> E.
    assert_eq!(out.len(), 6, "expected exactly 6 output chars, got {:?}", out);
    assert!(out.starts_with('I'), "init prints first: {:?}", out);
    for c in ['P', 'C', 'Q', 'D', 'E'] {
        assert!(out.contains(c), "missing {} in {:?}", c, out);
    }
    let pos = |c: char| out.find(c).unwrap();
    assert!(pos('P') < pos('E'), "parent prints P before E: {:?}", out);
    assert!(
        pos('C') < pos('Q') && pos('Q') < pos('D'),
        "child prints C, then execs prog1 which prints Q then D: {:?}",
        out
    );

    assert_ne!(m.csrs[CSR_PTBR as usize], 0, "kernel should set PTBR");
    assert_eq!(m.csrs[CSR_PMODE as usize], 1, "kernel should enable paging");
}
