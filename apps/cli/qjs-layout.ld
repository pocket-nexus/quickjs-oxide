/* qjs.order places vm::interp::run_at first. Keep its entry at 16 mod 64,
   as in the pre-TLS-removal binary: metadata size must not shift the
   unchanged interpreter's branch alignment within its cache line. */
SECTIONS {
  .text : ALIGN(64) {
    . += 16;
    *(.text .text.*)
  }
}
INSERT BEFORE .init;
