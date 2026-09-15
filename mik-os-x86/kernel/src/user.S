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

// prog SH: console shell with line discipline. Prints "mik> ", echoes
// bytes as typed, buffers until Enter (\r or \n), honors backspace, then
// dispatches the one-letter line: 'v' version, 'q' exit, else "?".
// sys_read blocks until the input buffer has a byte, so the loop never
// spins. rbx = line length; it survives syscalls because IrqFrame
// saves/restores every GPR. This process was never forked, so its stack
// and code pages are writable — call/push and the .fill buffer are legal.
.global prog_sh_start
.global prog_sh_end
prog_sh_start:
    lea rsi, [rip + sh_prompt]
    call sh_print
    xor ebx, ebx
sh_loop:
    mov eax, 7
    int 0x80                    // sys_read -> byte in al (blocks)
    cmp al, 13                  // '\r'
    je sh_enter
    cmp al, 10                  // '\n'
    je sh_enter
    cmp al, 8                   // backspace
    je sh_bs
    cmp al, 0x7F                // DEL sent by some terminals
    je sh_bs
    cmp rbx, 63
    jae sh_loop                 // line full: drop the byte
    lea rsi, [rip + sh_line]
    mov [rsi + rbx], al
    inc rbx
    mov edi, eax                // echo as typed
    mov eax, 1
    int 0x80
    jmp sh_loop
sh_bs:
    test rbx, rbx
    je sh_loop
    dec rbx
    lea rsi, [rip + sh_bs_seq]  // erase visually: back, space, back
    call sh_print
    jmp sh_loop
sh_enter:
    mov edi, 10                 // '\n'
    mov eax, 1
    int 0x80
    cmp rbx, 1                  // only single-letter commands for now
    jne sh_maybe_empty
    lea rsi, [rip + sh_line]
    movzx eax, byte ptr [rsi]
    cmp al, 'v'
    je sh_ver
    cmp al, 'q'
    je sh_quit
    lea rsi, [rip + sh_unknown]
    call sh_print
    jmp sh_again
sh_maybe_empty:
    test rbx, rbx
    je sh_again                 // bare Enter -> fresh prompt
    lea rsi, [rip + sh_unknown]
    call sh_print
    jmp sh_again
sh_ver:
    lea rsi, [rip + sh_version]
    call sh_print
    jmp sh_again
sh_quit:
    mov eax, 2
    int 0x80
sh_dead:
    jmp sh_dead
sh_again:
    xor ebx, ebx
    lea rsi, [rip + sh_prompt]
    call sh_print
    jmp sh_loop

// rsi -> NUL-terminated string. Clobbers rax and rdi.
sh_print:
    movzx edi, byte ptr [rsi]
    test edi, edi
    je sh_ret
    mov eax, 1
    int 0x80
    inc rsi
    jmp sh_print
sh_ret:
    ret

sh_prompt:  .asciz "mik> "
sh_version: .asciz "Mik OS x86-64 shell\n"
sh_unknown: .asciz "?\n"
sh_bs_seq:  .byte 8, ' ', 8, 0
sh_line:    .fill 64, 1, 0
prog_sh_end:
