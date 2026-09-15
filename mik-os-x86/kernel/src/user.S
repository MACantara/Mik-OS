// First user programs. Both are position-independent (immediates only) and
// use the int 0x80 syscall ABI: rax = number (1 write_char, 2 exit, 3
// yield), rdi = arg.

.section .rodata, "a"

// prog A: 'A', sbrk a heap page, store+read 'D' on it (demand fault),
// yield, 'a', yield, 'A', exit. rsi = heap base for the whole program.
.global prog_a_start
.global prog_a_end
prog_a_start:
    mov eax, 1
    mov edi, 'A'
    int 0x80
    mov eax, 6                  // sbrk(0x1000) -> rax = old brk
    mov edi, 0x1000
    int 0x80
    mov rsi, rax                // rsi = heap base (0x40002000)
    mov byte ptr [rsi], 'D'     // demand page fault -> map + retry
    mov eax, 1
    movzx edi, byte ptr [rsi]
    int 0x80                    // 'D'
    mov eax, 4
    int 0x80                    // fork -> rax = 1 parent, 0 child
    test rax, rax
    jnz parent
    // child: reads the shared heap byte ('D'), then writes 'c' — a COW
    // fault gives the child a private copy, leaving the parent's intact.
    mov byte ptr [rsi], 'c'     // COW fault -> private copy
    movzx edi, byte ptr [rsi]
    mov eax, 1
    int 0x80                    // 'c'
    mov eax, 5
    int 0x80                    // exec -> prog_c (next slice)
child_hang:
    jmp child_hang              // unreachable once exec works
parent:
    mov eax, 3
    int 0x80                    // yield -> child runs to its exit
    movzx edi, byte ptr [rsi]   // still 'D' — child's write stayed private
    mov eax, 1
    int 0x80                    // 'D'  (the COW isolation proof)
    mov byte ptr [rsi], 'p'     // parent's own COW write
    movzx edi, byte ptr [rsi]
    mov eax, 1
    int 0x80                    // 'p'
    mov eax, 2
    int 0x80                    // exit
a_hang:
    jmp a_hang                  // unreachable if exit works
prog_a_end:

// prog B: 'B' once, then spin forever — it never yields, so only the timer
// interrupt can move the CPU off it.
.global prog_b_start
.global prog_b_end
prog_b_start:
    mov eax, 1
    mov edi, 'B'
    int 0x80
2:  jmp 2b
prog_b_end:

// prog C: the exec target — 'E', exit. Loaded over the caller's user page.
.global prog_c_start
.global prog_c_end
prog_c_start:
    mov eax, 1
    mov edi, 'E'
    int 0x80
    mov eax, 2
    int 0x80
3:  jmp 3b
prog_c_end:
