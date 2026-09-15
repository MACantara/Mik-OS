OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS {
    note PT_NOTE FLAGS(0);
    boot PT_LOAD FLAGS(7);
    load PT_LOAD FLAGS(7);
}

SECTIONS {
    /* The BIOS loads sector 0 at 0x7C00 and the stage2+kernel blob at 0x7E00.
       These VMAs must match the physical load addresses because the boot code
       uses absolute operands before paging exists. They get their own phdr so
       the kernel's `load` segment stays compact at 0x400000. */
    .boot16 0x7C00 : {
        KEEP(*(.boot16))
    } :boot

    .stage2 0x7E00 : {
        KEEP(*(.stage2))
    } :boot

    . = 0x400000;

    .note : {
        *(.note.*)
    } :note

    .text : {
        *(.text .text.*)
    } :load

    .rodata : {
        *(.rodata .rodata.*)
    } :load

    .data : {
        *(.data .data.*)
    } :load

    .bss : ALIGN(4096) {
        __bss_start = .;
        *(.bss .bss.*)
        __bss_end = .;
    } :load
}
