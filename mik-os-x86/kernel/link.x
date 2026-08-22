OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS {
    note PT_NOTE FLAGS(0);
    load PT_LOAD FLAGS(7);
}

SECTIONS {
    . = 0x100000;

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
        *(.bss .bss.*)
    } :load
}
