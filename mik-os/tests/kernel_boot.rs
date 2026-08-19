use mik_os::kernel_binary;
use mik_emu::run;

#[test]
fn mik_os_boots() {
    let mut output = Vec::new();
    let exit = run(&kernel_binary(), &mut output).expect("emulator should run");
    assert_eq!(exit, 0, "expected HALT exit code 0");
    assert_eq!(
        String::from_utf8(output).unwrap(),
        "!?Mik OS\n",
        "kernel should use the page allocator, a print syscall, and direct I/O"
    );
}
