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
    mov eax, 3
    int 0x80                    // yield
    mov eax, 1
    mov edi, 'a'
    int 0x80
    mov eax, 3
    int 0x80                    // yield
    mov eax, 1
    mov edi, 'A'
    int 0x80
    mov eax, 2
    int 0x80                    // exit
1:  jmp 1b                  // unreachable if exit works
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
