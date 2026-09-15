use mik_os_x86::{build_image, disk_image, kernel_elf, try_find_qemu};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Boot the BIOS disk image under QEMU and check the serial output: the
/// kernel banner, the memory-init report, then the Phase 2.4 process demo —
/// process A prints 'A', sbrks a heap page, and touches it (demand fault ->
/// 'D'), forks, and yields; the child COW-writes 'c' and execs into a
/// program that prints 'E' and exits; the parent then reads the shared page
/// and still sees 'D' (proof the child's write stayed private), writes 'p',
/// and exits. Process B prints 'B' whenever the timer preempts, so its 'B's
/// are stripped before checking the deterministic sequence "ADcEDp".
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
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn qemu");

    // The kernel idles under the timer forever, so drain serial on a thread
    // and kill QEMU once it has had time to print.
    let mut stdout = child.stdout.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    std::thread::sleep(Duration::from_secs(10));
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
        out.contains("sched: 2 procs"),
        "scheduler did not spawn the two user processes, got: {out:?}"
    );
    // The deterministic user sequence after "sched: 2 procs" is "ADcEDp"
    // (demand fault -> 'D', COW child -> 'c', exec -> 'E', parent sees 'D',
    // parent writes 'p'); B's preempted ticks interleave 'B's anywhere, so
    // strip them before comparing.
    let tail = out.split("sched: 2 procs\n").nth(1).unwrap_or("");
    let demo: String = tail.chars().filter(|c| *c != 'B').collect();
    assert!(
        demo.starts_with("ADcEDp"),
        "demand/fork/exec sequence missing (want ADcEDp), got tail {tail:?}"
    );
}
