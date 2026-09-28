# C2 ARM64 codegen excerpts

Rust 1.88.0 release binaries with fat LTO. Static calls and symbol spans are compiler output, not dynamic execution counts. Only selected instructions are excerpted.

## c1

### ready_run excerpt 1

```asm
100179208:     	add	x8, sp, #0x1b8
10017920c:     	orr	x8, x8, #0x1
100179210:     	stp	x8, x9, [sp, #0xc8]
100179214:     	stp	x24, x2, [sp, #0x128]
100179218:     	add	x0, sp, #0x140
10017921c:     	mov	x1, x26
100179220:     	mov	x2, x19
100179224:     	ldr	x3, [sp, #0x138]
100179228:     	bl	0x100447e5c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17h2a08996ea3bc10a8E>
10017922c:     	ldrb	w21, [sp, #0x140]
100179230:     	cmp	w21, #0x49
100179234:     	b.eq	0x10017924c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd4>
100179238:     	cmp	w21, #0x46
10017923c:     	ccmp	w21, #0x1e, #0x4, ne
100179240:     	b.ne	0x10017924c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd4>
100179244:     	ldr	x22, [sp, #0x148]
100179248:     	b	0x100179268 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xf0>
```

### ready_run excerpt 2

```asm
10017942c:     	bfi	x5, x25, #8, #1
100179430:     	add	x0, sp, #0x3d0
100179434:     	and	w6, w23, #0x1
100179438:     	ldr	x1, [sp, #0x118]
10017943c:     	mov	x2, x26
100179440:     	mov	x19, x21
100179444:     	mov	x3, x21
100179448:     	ldr	x4, [sp, #0x138]
10017944c:     	bl	0x10042ac20 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E>
100179450:     	ldr	x8, [sp, #0x3d0]
100179454:     	ldr	x9, [sp, #0x3e0]
100179458:     	cmp	x8, #0x6
10017945c:     	ldr	x11, [sp, #0x78]
100179460:     	csel	x11, x11, x9, eq
100179464:     	cmp	x8, #0x5
100179468:     	b.ne	0x10017b17c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2004>
10017946c:     	str	x11, [sp, #0x78]
```

### ready_run excerpt 3

```asm
1001798e8:     	b	0x100179218 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xa0>
1001798ec:     	lsl	x5, x28, #32
1001798f0:     	add	x0, sp, #0x3d0
1001798f4:     	and	w6, w22, #0x1
1001798f8:     	ldr	x1, [sp, #0x118]
1001798fc:     	mov	x2, x26
100179900:     	mov	x3, x19
100179904:     	ldr	x4, [sp, #0x138]
100179908:     	bl	0x10042ac20 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E>
10017990c:     	ldr	x8, [sp, #0x3d0]
100179910:     	ldr	x9, [sp, #0x3e0]
100179914:     	cmp	x8, #0x6
100179918:     	ldr	x11, [sp, #0x90]
10017991c:     	csel	x11, x11, x9, eq
100179920:     	cmp	x8, #0x5
100179924:     	b.ne	0x10017b17c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2004>
100179928:     	str	x11, [sp, #0x90]
```

### read_progress excerpt 1

```asm
10042ac84:     	ldr	w9, [x8, #0x130]
10042ac88:     	cmp	w9, #0x2
10042ac8c:     	b.eq	0x10042b6d4 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0xab4>
10042ac90:     	ldr	x8, [x8, #0x110]
10042ac94:     	ldr	d0, [x8, #0xd0]
10042ac98:     	str	d0, [sp, #0x68]
10042ac9c:     	ldr	x0, [x24]
10042aca0:     	ldr	x1, [x24, #0x28]
10042aca4:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
10042aca8:     	tbz	w0, #0x0, 0x10042acb8 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0x98>
10042acac:     	mov	w8, #0x6                ; =6
10042acb0:     	stp	x8, x1, [x19]
10042acb4:     	b	0x10042b340 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0x720>
10042acb8:     	add	x27, sp, #0x2d8
10042acbc:     	lsr	x8, x25, #32
10042acc0:     	stp	x8, x20, [sp, #0x50]
10042acc4:     	str	x1, [sp, #0x70]
```

### complete_read excerpt 1

```asm
10042c7f4:     	bl	0x100137b14 <__ZN13quickjs_oxide6engine2vm5frame10FrameStore11current_mut17h3f2d85e78871d7baE>
10042c7f8:     	mov	x8, x0
10042c7fc:     	mov	x0, x1
10042c800:     	tbnz	w8, #0x0, 0x10042c85c <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x138>
10042c804:     	ldr	x8, [x0]
10042c808:     	str	x0, [sp, #0x20]
10042c80c:     	ldr	x1, [x0, #0x28]
10042c810:     	mov	x0, x8
10042c814:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
10042c818:     	tbz	w0, #0x0, 0x10042c824 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x100>
10042c81c:     	mov	x0, x1
10042c820:     	b	0x10042c85c <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x138>
10042c824:     	str	x1, [sp, #0x18]
10042c828:     	ldr	x8, [sp, #0x20]
10042c82c:     	ldr	x19, [x8]
10042c830:     	ldr	x8, [x19, #0x140]
10042c834:     	cbz	x8, 0x10042cd38 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x614>
```

## c2

### ready_run excerpt 1

```asm
100179320:     	add	x8, sp, #0x1b8
100179324:     	orr	x8, x8, #0x1
100179328:     	stp	x8, x9, [sp, #0xc8]
10017932c:     	stp	x24, x2, [sp, #0x128]
100179330:     	add	x0, sp, #0x140
100179334:     	mov	x1, x26
100179338:     	mov	x2, x19
10017933c:     	ldr	x3, [sp, #0x138]
100179340:     	bl	0x100447f20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17h826547c213f9bec6E>
100179344:     	ldrb	w21, [sp, #0x140]
100179348:     	cmp	w21, #0x49
10017934c:     	b.eq	0x100179364 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd4>
100179350:     	cmp	w21, #0x46
100179354:     	ccmp	w21, #0x1e, #0x4, ne
100179358:     	b.ne	0x100179364 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd4>
10017935c:     	ldr	x22, [sp, #0x148]
100179360:     	b	0x100179380 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xf0>
```

### ready_run excerpt 2

```asm
100179548:     	orr	x5, x8, #0x1
10017954c:     	and	w6, w22, #0x1
100179550:     	ldr	x1, [sp, #0x118]
100179554:     	mov	x2, x26
100179558:     	mov	x19, x21
10017955c:     	mov	x3, x21
100179560:     	ldr	x4, [sp, #0x138]
100179564:     	mov	x7, x28
100179568:     	bl	0x10042ad84 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17h17ea25fc327a0ca7E>
10017956c:     	ldr	x8, [sp, #0x3d0]
100179570:     	ldr	x9, [sp, #0x3e0]
100179574:     	cmp	x8, #0x6
100179578:     	ldr	x11, [sp, #0x78]
10017957c:     	csel	x11, x11, x9, eq
100179580:     	cmp	x8, #0x5
100179584:     	b.ne	0x10017b29c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x200c>
100179588:     	str	x11, [sp, #0x78]
```

### ready_run excerpt 3

```asm
100179a08:     	ubfx	x6, x22, #32, #1
100179a0c:     	lsl	x5, x28, #32
100179a10:     	add	x0, sp, #0x3d0
100179a14:     	ldr	x1, [sp, #0x118]
100179a18:     	mov	x2, x26
100179a1c:     	mov	x3, x19
100179a20:     	ldr	x4, [sp, #0x138]
100179a24:     	mov	x7, x22
100179a28:     	bl	0x10042ad84 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17h17ea25fc327a0ca7E>
100179a2c:     	ldr	x8, [sp, #0x3d0]
100179a30:     	ldr	x9, [sp, #0x3e0]
100179a34:     	cmp	x8, #0x6
100179a38:     	ldr	x11, [sp, #0x90]
100179a3c:     	csel	x11, x11, x9, eq
100179a40:     	cmp	x8, #0x5
100179a44:     	b.ne	0x10017b29c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x200c>
100179a48:     	str	x11, [sp, #0x90]
```

### read_progress excerpt 1

```asm

/tmp/oxide-c2-candidate-plain/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010042ad84 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17h17ea25fc327a0ca7E>:
10042ad84:     	stp	x28, x27, [sp, #-0x60]!
10042ad88:     	stp	x26, x25, [sp, #0x10]
10042ad8c:     	stp	x24, x23, [sp, #0x20]
10042ad90:     	stp	x22, x21, [sp, #0x30]
10042ad94:     	stp	x20, x19, [sp, #0x40]
10042ad98:     	stp	x29, x30, [sp, #0x50]
10042ad9c:     	add	x29, sp, #0x50
10042ada0:     	sub	sp, sp, #0x380
10042ada4:     	mov	x24, x7
```

### complete_read excerpt 1

```asm

/tmp/oxide-c2-candidate-plain/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010042c890 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17h9cc13d7afd94031dE>:
10042c890:     	sub	sp, sp, #0x1f0
10042c894:     	stp	x28, x27, [sp, #0x190]
10042c898:     	stp	x26, x25, [sp, #0x1a0]
10042c89c:     	stp	x24, x23, [sp, #0x1b0]
10042c8a0:     	stp	x22, x21, [sp, #0x1c0]
10042c8a4:     	stp	x20, x19, [sp, #0x1d0]
10042c8a8:     	stp	x29, x30, [sp, #0x1e0]
10042c8ac:     	add	x29, sp, #0x1e0
10042c8b0:     	mov	x20, x4
```
