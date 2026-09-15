use mik_os_x86::{build_image, disk_image, kernel_elf, try_find_qemu};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Boot the BIOS disk image under QEMU and check the serial output: the
/// kernel banner, the memory-init report, then the Phase 2.4 process demo —
/// process A prints 'A', sbrks a heap page, and touches it (demand fault ->
/// 'D'), forks, and yields; the child COW-writes 'c' and execs into a
/// program that prints 'E' and exits; the parent then reads the shared page
/// and still sees 'D' (proof the child's write stayed private), writes 'p',
/// and exits. Process B prints 'B' whenever the timer preempts, and the
/// serial shell prints "mik> " plus the version banner when the test writes
/// 'v' into QEMU's stdin — the Phase 3 console input path.
#[test]
fn bios_image_boots_and_process_syscalls_work() {
    let Some(qemu) = try_find_qemu() else {
        eprintln!("qemu-system-x86_64 not found; skipping boot test");
        return;
    };
    let status = Command::new("cargo")
        .args(["build", "-p", "mik-os-x86-kernel", "--target", "x86_64-unknown-none"])
        .status()
        .expect("cargo build");
    assert!(status.success(), "mik-os-x86-kernel failed to build");

    let elf = std::fs::read(kernel_elf()).expect("kernel ELF not built");
    let img = build_image(&elf).expect("build_image");
    let img_path = disk_image();
    std::fs::write(&img_path, &img).expect("write disk image");

    let mut child = Command::new(qemu)
        .arg("-drive")
        .arg(format!("format=raw,file={}", img_path.display()))
        .args(["-serial", "stdio", "-display", "none", "-no-reboot", "-no-shutdown"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn qemu");

    // The kernel idles under the timer forever, so drain serial on a thread
    // and kill QEMU once it has had time to print. -serial stdio is
    // bidirectional: writing to QEMU's stdin lands in COM1's receive
    // register, which the shell polls through sys_read.
    let mut stdout = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    // Let the demo run, then send 'v' — the shell should echo it and print
    // the version banner — then 'q' to exit the shell.
    std::thread::sleep(Duration::from_secs(3));
    let _ = stdin.write_all(b"vq");
    let _ = stdin.flush();
    std::thread::sleep(Duration::from_secs(7));
    let _ = child.kill();
    let _ = child.wait();
    let out = reader.join().unwrap_or_default();

    assert!(
        out.contains("Mik-64 -> x86-64 long mode"),
        "missing kernel banner, got: {out:?}"
    );
    assert!(
        out.contains("usable frames="),
        "E820 allocator did not report, got: {out:?}"
    );
    assert!(
        out.contains("alloc/free ok"),
        "free-list sanity check failed, got: {out:?}"
    );
    assert!(
        out.contains("sched: 3 procs"),
        "scheduler did not spawn the three user processes, got: {out:?}"
    );
    // The deterministic user sequence is "ADcEDp" (demand fault -> 'D', COW
    // child -> 'c', exec -> 'E', parent sees 'D', parent writes 'p'). B's
    // ticks and the shell's "mik> " prompt/echo interleave anywhere, so keep
    // only the marker letters before comparing.
    let tail = out.split("sched: 3 procs\n").nth(1).unwrap_or("");
    let demo: String = tail.chars().filter(|c| "ADcEp".contains(*c)).collect();
    assert!(
        demo.starts_with("ADcEDp"),
        "demand/fork/exec sequence missing (want ADcEDp), got tail {tail:?}"
    );
    // The shell answered 'v' with its version banner over COM1 input.
    assert!(
        out.contains("Mik OS x86-64 shell"),
        "shell did not answer 'v' over serial input, got: {out:?}"
    );
}
