/* qjs.order places vm::interp::run_at first. Keep its entry at 16 mod 32,
   as in main and the pre-TLS-removal binary: metadata size must not shift
   the unchanged interpreter's branch alignment. */
SECTIONS {
  .text : ALIGN(32) {
    . += 16;
    *(.text .text.*)
  }
}
INSERT BEFORE .init;
