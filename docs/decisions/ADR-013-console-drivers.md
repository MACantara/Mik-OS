# ADR-013: Console Device Drivers — PCI Scan, VGA Text, PS/2 Keyboard

## Status

Accepted

## Date

2026-10-05

## Context

Milestone 3.2 makes the console real: the kernel had exactly one device
(COM1, write-only) and needed input devices plus a user-visible display.
The decisions:

1. How the kernel finds hardware: hardcoded port numbers vs. enumerable bus.
2. Which display and keyboard to drive (VGA text vs. framebuffer; PS/2 vs.
   USB HID).
3. Where each byte of user output goes once two consoles exist.

## Decision

**Enumerate PCI for real.** `pci.rs` walks all 256 buses × 32 devices × 8
functions through config ports `0xCF8`/`0xCFC` (multifunction honored via
header-type bit 7) and prints each function as
`pci bb.dd.f vend:dev class.sub`. Enumeration is the milestone's
infrastructure deliverable even though nothing claims devices yet — the
scan output already identifies the IDE controller (`0101`) and e1000 NIC
(`0200`) that M3.3/M3.4 will bind to. Full-bus scan is the simple correct
choice: 65k port reads, under a second, no bridge recursion.

**VGA text mode at `0xB8000` is the user console.** `sys_write` mirrors
every byte to both COM1 and the VGA buffer (80×25, attribute `0x07`,
`\n`/`\r`/`\b` handled, bottom-row scroll). Kernel diagnostics
(`serial::write_str`) stay serial-only: COM1 is the debug channel, VGA is
the user channel — the split every real kernel makes between printk logs
and the console. No cursor-MCR write, no colors, no ANSI — a byte stream
to a character grid is the honest minimum.

**PS/2 keyboard on IRQ1, set-1 scancodes.** `kbd.rs` reads port `0x60`,
tracks shift state, decodes the 0x00–0x3A make-code block through two
59-entry tables, and pushes ASCII into the shared input buffer — the same
buffer the UART feeds, so the shell can't tell which device typed.
`E0`-prefixed extended keys mostly fall outside the table and are ignored
(the exception — numpad Enter's `E0 1C` — harmlessly produces `'\n'`).

**Both input IRQs are edge-triggered on the master PIC**: master mask
`0xE8` unmasks IRQ0/1/4; the slave stays fully masked.

## Alternatives Considered

- **USB HID keyboard**: correct on real modern hardware but needs a USB
  host-controller driver (UHCI/XHCI) — an entire subsystem for one key.
  PS/2 exists in QEMU forever.
- **Framebuffer (GOP/Bochs VBE)**: pixels instead of characters — needs
  font data, drawing, and a real console renderer. Text mode is the
  educational minimum and already scrolls.
- **ACPI/MP-table device discovery**: proper enumeration, but AML is a
  whole interpreter; PCI config cycles answer the same "what hardware
  exists" question directly. ACPI arrives with SMP (M3.5).
- **sys_write to VGA only**: would break every serial-based test and the
  QEMU-stdio workflow; mirroring costs one function call.

## Consequences

- The VM is interactive without `-serial stdio`: keyboard → IRQ1 → buffer
  → shell; shell → `sys_write` → VGA window.
- PCI scanning proves the mechanism end-to-end and prints the map M3.3
  (block device) and M3.4 (NIC) will consume.
- Deferred, marked: claim/attach protocol (drivers currently *find*
  nothing — they will match on the scan's vendor:device), VGA hardware
  cursor, ANSI escapes, key-release events, USB, hotplug.
