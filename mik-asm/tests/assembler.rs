use mik_asm::assemble;
use mik_emu::run;

#[test]
fn assembles_hello_and_runs() {
    let src = r#"
li x1, msg

loop:
load8 x2, x1, 0
beq x2, x0, end
store8 x0, x2, 0x1000
addi x1, x1, 1
jmp loop

end:
halt

msg:
.string "Hello\n"
"#;

    let binary = assemble(src, 0x400000).expect("assembly should succeed");

    // The assembled binary must fit in RAM at 0x400000.
    assert!(!binary.is_empty());

    let mut output = Vec::new();
    let exit = run(&binary, &mut output).expect("emulator should run");

    assert_eq!(exit, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "Hello\n");
}

#[test]
fn li_with_immediate_works() {
    let src = r#"
li x2, 'H'
store8 x0, x2, 0x1000
halt
"#;

    let binary = assemble(src, 0x400000).expect("assembly should succeed");
    let mut output = Vec::new();
    let exit = run(&binary, &mut output).expect("emulator should run");

    assert_eq!(exit, 0);
    assert_eq!(String::from_utf8(output).unwrap(), "H");
}
