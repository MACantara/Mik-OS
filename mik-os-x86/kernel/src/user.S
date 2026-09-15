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

// prog D: seeded onto Mik-FS as file "x" — the exec-from-disk target.
// 'X', exit.
.global prog_d_start
.global prog_d_end
prog_d_start:
    mov eax, 1
    mov edi, 'X'
    int 0x80
    mov eax, 2
    int 0x80
4:  jmp 4b
prog_d_end:

// prog SH: console shell with line discipline and filesystem commands.
// Prints "mik> ", echoes bytes as typed, buffers until Enter, honors
// backspace, then splits the line into CMD [ARGS] and dispatches:
//   ls | v | q | cat NAME | run NAME | mk NAME | w NAME TEXT
// sys_read blocks, so the input loop never spins. Registers rbx/r12-r15
// survive syscalls and preemption (IrqFrame saves all 15 GPRs): rbx =
// line length, r14/r15 = command word, r12/r13 = argument tail.
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
    test rbx, rbx
    je sh_again                 // bare Enter -> fresh prompt
    // split: r14/r15 = command word, r12/r13 = args after it
    lea rsi, [rip + sh_line]
    mov rcx, rbx
    call sh_find_sp             // rdx = index of first space
    mov r14, rsi
    mov r15, rdx
    lea rsi, [rsi + rdx]
    sub rcx, rdx
    call sh_skip_sp
    mov r12, rsi
    mov r13, rcx
    // dispatch on the command word
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_ls]
    call sh_streq
    test al, al
    jnz sh_ls
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_cat]
    call sh_streq
    test al, al
    jnz sh_cat
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_run]
    call sh_streq
    test al, al
    jnz sh_run
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_mk]
    call sh_streq
    test al, al
    jnz sh_mk
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_w]
    call sh_streq
    test al, al
    jnz sh_w
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_v]
    call sh_streq
    test al, al
    jnz sh_ver
    mov rsi, r14
    mov rcx, r15
    lea rdx, [rip + sh_c_q]
    call sh_streq
    test al, al
    jnz sh_quit
    lea rsi, [rip + sh_unknown]
    call sh_print
    jmp sh_again

sh_ls:
    mov eax, 13                 // sys_ls -> kernel prints the dir
    int 0x80
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

// cat NAME: open read-mode, fread loop, close.
sh_cat:
    test r13, r13
    je sh_again                 // no argument
    mov eax, 8
    mov rdi, r12
    mov rsi, r13
    xor edx, edx                // mode 0 = read
    int 0x80
    cmp rax, -1
    je sh_nofile
    mov r12, rax                // fd (name no longer needed)
sh_cat_loop:
    mov eax, 10                 // fread(fd) -> byte or -1
    mov rdi, r12
    int 0x80
    cmp rax, -1
    je sh_cat_done
    mov edi, eax
    mov eax, 1
    int 0x80
    jmp sh_cat_loop
sh_cat_done:
    mov edi, 10
    mov eax, 1
    int 0x80                    // trailing newline
    mov eax, 9
    mov rdi, r12
    int 0x80                    // close
    jmp sh_again

// run NAME: fork, child exec_file's, parent yields so the child runs
// before the next prompt is printed.
sh_run:
    test r13, r13
    je sh_again
    mov eax, 4                  // fork
    int 0x80
    test rax, rax
    jnz sh_run_parent
    mov eax, 12                 // child: exec_file(name)
    mov rdi, r12
    mov rsi, r13
    int 0x80                    // success: never returns here
    lea rsi, [rip + sh_nofile_s]
    call sh_print
    mov eax, 2
    int 0x80
5:  jmp 5b
sh_run_parent:
    mov eax, 3                  // yield -> child execs and runs to exit
    int 0x80
    jmp sh_again

// mk NAME: create (or truncate) and close.
sh_mk:
    test r13, r13
    je sh_again
    mov eax, 8
    mov rdi, r12
    mov rsi, r13
    mov edx, 1                  // mode 1 = write (create/truncate)
    int 0x80
    cmp rax, -1
    je sh_nofile
    mov rdi, rax
    mov eax, 9
    int 0x80
    jmp sh_again

// w NAME TEXT: split args at the next space, open write-mode, fwrite
// each byte of TEXT, close.
sh_w:
    test r13, r13
    je sh_again
    mov rsi, r12
    mov rcx, r13
    call sh_find_sp             // rdx = name len
    mov r14, r12                // r14/r15 = name
    mov r15, rdx
    lea rsi, [r12 + rdx]
    sub r13, rdx
    mov rcx, r13
    call sh_skip_sp
    mov r12, rsi                // r12/r13 = text
    mov r13, rcx
    mov eax, 8
    mov rdi, r14
    mov rsi, r15
    mov edx, 1
    int 0x80
    cmp rax, -1
    je sh_nofile
    mov r14, rax                // fd
    xor r15d, r15d              // text index
sh_w_loop:
    cmp r15, r13
    jae sh_w_done
    mov eax, 11                 // fwrite(fd, byte)
    mov rdi, r14
    movzx esi, byte ptr [r12 + r15]
    int 0x80
    inc r15
    jmp sh_w_loop
sh_w_done:
    mov eax, 9
    mov rdi, r14
    int 0x80
    jmp sh_again

sh_nofile:
    lea rsi, [rip + sh_nofile_s]
    call sh_print
    jmp sh_again
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

// rdx = index of the first ' ' in [rsi, rsi+rcx), or rcx if none.
// Clobbers rdx only.
sh_find_sp:
    xor edx, edx
1:  cmp rdx, rcx
    jae 2f
    cmp byte ptr [rsi + rdx], ' '
    je 2f
    inc rdx
    jmp 1b
2:  ret

// Advance rsi/dec rcx past leading spaces. Clobbers nothing else.
sh_skip_sp:
1:  test rcx, rcx
    je 2f
    cmp byte ptr [rsi], ' '
    jne 2f
    inc rsi
    dec rcx
    jmp 1b
2:  ret

// al = 1 iff the len-delimited string (rsi,rcx) equals the NUL-terminated
// string at rdx. Clobbers rax, r8, r9.
sh_streq:
    xor r8d, r8d
1:  movzx r9d, byte ptr [rdx + r8]
    test r9d, r9d
    jz 3f
    cmp r8, rcx
    jae 4f
    cmp byte ptr [rsi + r8], r9b
    jne 4f
    inc r8
    jmp 1b
3:  xor eax, eax
    cmp r8, rcx                 // equal iff both ended together
    sete al
    ret
4:  xor eax, eax
    ret

sh_prompt:  .asciz "mik> "
sh_version: .asciz "Mik OS x86-64 shell\n"
sh_unknown: .asciz "?\n"
sh_nofile_s: .asciz "no file\n"
sh_bs_seq:  .byte 8, ' ', 8, 0
sh_c_ls:    .asciz "ls"
sh_c_cat:   .asciz "cat"
sh_c_run:   .asciz "run"
sh_c_mk:    .asciz "mk"
sh_c_w:     .asciz "w"
sh_c_v:     .asciz "v"
sh_c_q:     .asciz "q"
sh_line:    .fill 64, 1, 0
prog_sh_end:
