# ADR-008: Legacy PIC + PIT for the Scheduler Tick

## Status

Accepted

## Date

2026-10-02

## Context

Milestone 2.3 needs a periodic interrupt to drive preemption: something that
fires on a cadence, reaches the IDT, and lets the scheduler reclaim the CPU
from a running process.

The x86 platform offers two generations of timer/interrupt plumbing:

- **8259 PIC pair + 8253/8254 PIT** — the 1980s chipset: two cascaded PICs
  deliver IRQ0–15; PIT channel 0 is wired to IRQ0. Still present (or
  faithfully emulated) on every PC and under QEMU.
- **LAPIC + APIC timer** — the modern per-CPU path: more precise, per-core,
  calibrated against a reference clock. Requires parsing ACPI/MP tables or
  CPUID enumeration and disables the PIC first.

The PIC also needs remapping: by default IRQ0–7 land on vectors 8–15, which
collide with CPU exceptions (double fault is vector 8!). The classic remap
moves them to vectors 32–47.

## Decision

Use the legacy **PIC remapped to vectors 32–47 + PIT channel 0 at ~100 Hz**
for the tick. Remap both PICs, mask every line except IRQ0, program the PIT
divisor for 100 Hz, and acknowledge each interrupt with EOI to the master
PIC *before* scheduling (so the next tick can arrive even if we switch away).

One subtlety the implementation absorbs: **the BIOS leaves the PIT running**.
Its ~18.2 Hz tick keeps generating IRQ0 while `IF=0` during kernel init; the
edge sits latched in the PIC's IRR and delivers the instant the first
`iretq` raises `IF` — preempting the first user instruction. `sched::start`
drains it: `sti` briefly while the scheduler is still marked inactive (the
tick handler EOI's and drops the frame), then `cli`, then arm scheduling.
This ordering — arm the PIT last, drain once, then enter ring 3 — makes the
first dispatch deterministic.

## Alternatives Considered

- **LAPIC + APIC timer**: the production answer, but adds CPU detection, MSR
  programming, and ACPI parsing to a milestone whose goal is *preemption*,
  not timer quality. A natural upgrade once the kernel needs per-core timing
  or SMP (M2.6+).
- **HPET**: higher resolution, also table-driven; same argument as APIC —
  deferred.
- **Cooperative-only scheduling** (no timer): `yield` alone can't demonstrate
  preemption — a spinning process would never be descheduled (process B
  exists precisely to prove the timer reclaims the CPU).

## Consequences

- Timer IRQ = vector 32 = `isr_timer` → `timer_handler` → `schedule()`.
- IRQ1–15 stay masked; no other device interrupts exist yet.
- `pic::eoi()` is the only PIC interaction in the hot path; remap/init live
  in `pic.rs` (~40 lines of port writes).
- The latched-IRQ drain is a real-hardware gotcha worth remembering: whatever
  ran before you (BIOS) may have left interrupts pending that fire on your
  first `sti`/`iretq`.
- Limitations, marked in code: fixed 100 Hz, no calibration, no IRQ1+
  handling (keyboard etc. would need unmasking + handlers).
