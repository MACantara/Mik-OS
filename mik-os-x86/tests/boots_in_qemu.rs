use mik_os_x86::{build_image, disk_image, kernel_elf, try_find_qemu};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Boot the BIOS disk image under QEMU and check the serial output: the
/// kernel banner, the memory-init report, the second-address-space demo
/// ('U' printed by the user page under its own CR3), and "EX03" from the
/// int3 exercising the IDT.
#[test]
fn bios_image_boots_and_idt_fires() {
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

    // The kernel halts forever, so drain serial on a thread and kill QEMU
    // once it has had time to print.
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
        out.contains("U <- ran under second CR3"),
        "user page did not run under its own CR3, got: {out:?}"
    );
    assert!(out.contains("EX03"), "int3 did not reach the IDT, got: {out:?}");
}
