MEMORY
{
  /* Adjust these if you are using a non-standard flash/RAM layout */
  FLASH (rx) : ORIGIN = 0x42000000, LENGTH = 4M
  RAM (rwx)  : ORIGIN = 0x3FC88000, LENGTH = 320K
}

INSERT AFTER .text;
