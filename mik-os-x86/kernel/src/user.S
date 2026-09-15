// First user programs. Both are position-independent (immediates only) and
// use the int 0x80 syscall ABI: rax = number (1 write_char, 2 exit, 3
// yield), rdi = arg.

.section .rodata, "a"

// prog A: 'A', yield, 'a', yield, 'A', exit.
.global prog_a_start
.global prog_a_end
prog_a_start:
    mov eax, 1
    mov edi, 'A'
    int 0x80
    mov eax, 3
    int 0x80
    mov eax, 1
    mov edi, 'a'
    int 0x80
    mov eax, 3
    int 0x80
    mov eax, 1
    mov edi, 'A'
    int 0x80
    mov eax, 2
    int 0x80
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
