SECTIONS
{
  .boot2 ORIGIN(FLASH_BOOT2) :
  {
    KEEP(*(.boot2));
  } > FLASH_BOOT2
} INSERT BEFORE .text;
