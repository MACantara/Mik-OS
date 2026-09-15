// Tiny first user program: print 'U' on COM1 and return. It is position-
// independent — only immediate operands and ret — so it runs at whatever
// virtual address the kernel maps it to. The kernel copies these bytes into
// a freshly allocated frame and maps that frame with PTE_U.

.section .rodata, "a"
.global user_prog_start
.global user_prog_end
user_prog_start:
    mov al, 'U'
    mov dx, 0x3F8
    out dx, al
    ret
user_prog_end:
