ENTRY(_start)

SECTIONS {
    . = 0x200000;

    .text :
    {
        *(.text .text.*)
    }

    . = ALIGN(4096);
    .rodata :
    {
        *(.rodata .rodata.*)
    }

    . = ALIGN(4096);
    .data :
    {
        *(.data .data.*)
        *(.got)
    }

    . = ALIGN(4096);
    .bss :
    {
        *(.bss .bss.*)
    }
}
