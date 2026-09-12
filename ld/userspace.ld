ENTRY(_start)
SECTIONS
{
    . = 0x400000;
    .text : ALIGN(0x1000) { *(.text.start) *(.text .text.*) }
    .rodata : ALIGN(0x1000) { *(.rodata .rodata.*) }
    .got : ALIGN(0x1000) { *(.got .got.*) }
    .data : ALIGN(0x1000) { *(.data .data.*) }
    .bss : ALIGN(0x1000) { *(COMMON) *(.bss .bss.*) }
    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) }
}
