ENTRY(_start)
SECTIONS
{
    . = 0x400000;
    /* _start первым: Rust помечает его секцией .text.start. */
    .text : ALIGN(0x1000) { *(.text.start) *(.text .text.*) }
    .rodata : ALIGN(0x1000) { *(.rodata .rodata.*) }
    /* GOT нужен статике (Rust вызывает memset через *(%rip)): свой RW-LOAD. */
    .got : ALIGN(0x1000) { *(.got .got.*) }
    .data : ALIGN(0x1000) { *(.data .data.*) }
    .bss : ALIGN(0x1000) { *(COMMON) *(.bss .bss.*) }
    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
