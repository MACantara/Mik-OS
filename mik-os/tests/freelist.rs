use mik_os::kernel_freelist;
use mik_emu::Machine;

const SENTINEL: u64 = 0xCAFEBABE;
const DEMO_PAGE: u64 = 0x701000;
const NEXT_PAGE: u64 = 0x700000;
const FREE_HEAD: u64 = 0x700008;

#[test]
fn mik_os_reuses_freed_page() {
    let mut m = Machine::new();
    m.load_binary(&kernel_freelist());
    let mut output = Vec::new();
    while !m.halted {
        m.step(&mut output).expect("emulator should run");
    }
    assert_eq!(m.exit_code, 0);

    let next_page = u64::from_le_bytes(m.mem[NEXT_PAGE as usize..][..8].try_into().unwrap());
    let free_head = u64::from_le_bytes(m.mem[FREE_HEAD as usize..][..8].try_into().unwrap());
    let reused_value = u64::from_le_bytes(m.mem[DEMO_PAGE as usize..][..8].try_into().unwrap());

    assert_eq!(next_page, 0x709000, "bump counter should not advance after reuse");
    assert_eq!(free_head, 0, "free list should be empty after pop");
    assert_eq!(reused_value, SENTINEL, "freed page should be reused and written");
}
