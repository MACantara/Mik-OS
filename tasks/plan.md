# Implementation Plan: Free-List Physical Page Allocator

## Overview

Add a singly-linked free list to the Mik-64 kernel's physical page allocator. The free list sits alongside the existing bump allocator so freed pages are reused before new pages are claimed. This is the smallest step toward the memory-management milestone in `ROADMAP.md`.

## Architecture Decisions

- **Free list head at `0x700008`:** the bump counter stays at `0x700000`, and the new `free_list_head` pointer uses the next 8 bytes. The machine zeroes all memory, so the head starts empty.
- **First 8 bytes of a freed page hold the next pointer:** this is the classic embedded free-list pattern and needs no extra metadata.
- **Keep `alloc_page` and `free_page` as `JMPR` subroutines:** they are already used by the kernel and tests, so the calling convention is preserved.
- **One kernel binary and test file:** add `kernel_freelist()` and `mik-os/tests/freelist.rs`. No other kernel binaries change unless they choose to call `free_page`.

## Task List

### Task 1: Implement `free_page` and `alloc_page` free-list logic

**Description:** Change `alloc_page` so it pops `free_list_head` if it is non-zero. If the head is zero, fall back to bumping `next_page`. Change `free_page` from a no-op to a real subroutine that pushes a page onto the free list.

**Acceptance criteria:**
- [x] `alloc_page` returns the head page when the free list is not empty.
- [x] `alloc_page` bumps `next_page` when the free list is empty.
- [x] `free_page` stores the previous head in the first 8 bytes of the freed page and stores the freed page as the new head.
- [x] Existing `kernel_binary()` and `kernel_pagefault()` still boot and pass their tests.

**Verification:**
- `cargo test` passes.
- `run.ps1` prints the expected boot message.

**Dependencies:** None.

**Files likely touched:**
- `mik-os/src/lib.rs`

**Estimated scope:** Small.

### Task 2: Add `kernel_freelist()` and integration test

**Description:** Create a new kernel binary that uses `build_common`, frees the demo page after paging is enabled, allocates again, and writes a sentinel to the reused page. Add a test that loads the binary, runs to halt, and verifies the page was reused.

**Acceptance criteria:**
- [x] `kernel_freelist()` runs to halt without page faults.
- [x] The second `alloc_page` returns the same physical address as the freed demo page.
- [x] `next_page` at `0x700000` does not change after the second allocation.
- [x] `free_list_head` at `0x700008` is zero after the second allocation.

**Verification:**
- `cargo test -p mik-os --test freelist` passes.

**Dependencies:** Task 1.

**Files likely touched:**
- `mik-os/src/lib.rs`
- `mik-os/tests/freelist.rs`

**Estimated scope:** Small.

### Checkpoint: Free-List Allocator Complete

- [x] `cargo test` passes.
- [x] `run.ps1` still works.
- [x] The allocator can allocate, free, and re-allocate a physical page.
- [x] Code is committed and merged.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Free-list pointer stored in a page that is later mapped and written | High | Only free pages hold list pointers; once re-allocated the page is user/kernel data. |
| Freed page tables or MMIO pages end up on the free list | Medium | `free_page` is a low-level primitive; callers must only pass general RAM pages. Add inline comments. |
| Hand-assembly makes the free-list logic verbose | Low | Keep the subroutines under a dozen instructions and add comments mapping each instruction to its semantics. |

## Open Questions

- Should `free_page` accept a zero page or silently return? (For now, assume the caller only frees a valid allocated page.)
