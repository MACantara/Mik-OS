use mik_os_x86::{build_image, disk_image, kernel_elf, try_find_qemu};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Boot the BIOS disk image under QEMU and drive the shell over COM1.
/// Sends each line byte-by-byte (paced: QEMU's Windows stdio chardev can
/// hold a burst host-side), waits, kills QEMU, returns the serial output.
fn boot_and_type(qemu: &Path, img: &Path, lines: &[&str]) -> String {
    let mut child: Child = Command::new(qemu)
        .arg("-drive")
        .arg(format!("format=raw,file={}", img.display()))
        .args(["-serial", "stdio", "-display", "none", "-no-reboot", "-no-shutdown"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn qemu");

    let mut stdout = child.stdout.take().unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    std::thread::sleep(Duration::from_secs(3)); // let the demo finish
    for line in lines {
        for &b in line.as_bytes() {
            let _ = stdin.write_all(&[b]);
            let _ = stdin.flush();
            std::thread::sleep(Duration::from_millis(250));
        }
        std::thread::sleep(Duration::from_millis(800)); // let the command run
    }
    std::thread::sleep(Duration::from_secs(2));
    let _ = child.kill();
    let _ = child.wait();
    reader.join().unwrap_or_default()
}

/// M3.3 end to end: the kernel banner and E820 allocator, the PCI scan,
/// the ATA probe, Mik-FS mount, the M2.4 process demo (ADcEDp), and the
/// shell exercising the filesystem — `cat` a seeded file, `run` a program
/// exec'd from disk, `w`/`cat` round-trip a new file — then a SECOND boot
/// on the same image proves the write survived a restart.
#[test]
fn bios_image_boots_and_filesystem_persists() {
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

    // Boot 1: read the seeded file, exec "x" from disk, write+read a file.
    let out = boot_and_type(
        &qemu,
        &img_path,
        &["cat hello.txt\n", "run x\n", "w n FSOK\n", "cat n\n"],
    );

    assert!(
        out.contains("Mik-64 -> x86-64 long mode"),
        "missing kernel banner, got: {out:?}"
    );
    assert!(
        out.contains("usable frames="),
        "E820 allocator did not report, got: {out:?}"
    );
    assert!(out.contains("alloc/free ok"), "free-list sanity failed, got: {out:?}");
    assert!(out.contains("ata ok"), "ATA probe failed, got: {out:?}");
    assert!(out.contains("fs: "), "Mik-FS did not mount, got: {out:?}");
    assert!(
        out.contains("sched: 3 procs"),
        "scheduler did not spawn the three user processes, got: {out:?}"
    );
    // Deterministic user sequence "ADcEDp" (demand fault, COW child, exec,
    // isolated parent read, parent write) — B's ticks and shell output
    // interleave anywhere, so keep only the marker letters.
    let tail = out.split("sched: 3 procs\n").nth(1).unwrap_or("");
    let demo: String = tail.chars().filter(|c| "ADcEp".contains(*c)).collect();
    assert!(
        demo.starts_with("ADcEDp"),
        "demand/fork/exec sequence missing (want ADcEDp), got tail {tail:?}"
    );
    assert!(
        out.contains("Hello from Mik-FS"),
        "cat hello.txt did not print the seeded file, got: {out:?}"
    );
    // `run x` execs the file's bytes: prog_d prints 'X'. Uppercase X is
    // not a hex digit and appears nowhere else in the output.
    assert!(out.contains('X'), "run x did not exec from disk, got: {out:?}");
    assert!(
        out.contains("FSOK"),
        "w/cat round-trip failed, got: {out:?}"
    );

    // Boot 2 on the same image file: `cat n` must still print FSOK — the
    // write reached the host image, not just the guest's cache.
    let out2 = boot_and_type(&qemu, &img_path, &["cat n\n"]);
    assert!(
        out2.contains("FSOK"),
        "file written in boot 1 did not survive restart, got: {out2:?}"
    );
}
