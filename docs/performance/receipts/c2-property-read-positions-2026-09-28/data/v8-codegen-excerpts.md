# C2 V8-base ARM64 codegen excerpts

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
100179228:     	bl	0x10044afbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17h2a08996ea3bc10a8E>
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
10017944c:     	bl	0x10042dd80 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E>
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
100179908:     	bl	0x10042dd80 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E>
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
10042dde4:     	ldr	w9, [x8, #0x130]
10042dde8:     	cmp	w9, #0x2
10042ddec:     	b.eq	0x10042e834 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0xab4>
10042ddf0:     	ldr	x8, [x8, #0x110]
10042ddf4:     	ldr	d0, [x8, #0xd0]
10042ddf8:     	str	d0, [sp, #0x68]
10042ddfc:     	ldr	x0, [x24]
10042de00:     	ldr	x1, [x24, #0x28]
10042de04:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
10042de08:     	tbz	w0, #0x0, 0x10042de18 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0x98>
10042de0c:     	mov	w8, #0x6                ; =6
10042de10:     	stp	x8, x1, [x19]
10042de14:     	b	0x10042e4a0 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0x720>
10042de18:     	add	x27, sp, #0x2d8
10042de1c:     	lsr	x8, x25, #32
10042de20:     	stp	x8, x20, [sp, #0x50]
10042de24:     	str	x1, [sp, #0x70]
```

### complete_read excerpt 1

```asm
10042f954:     	bl	0x100137b14 <__ZN13quickjs_oxide6engine2vm5frame10FrameStore11current_mut17h3f2d85e78871d7baE>
10042f958:     	mov	x8, x0
10042f95c:     	mov	x0, x1
10042f960:     	tbnz	w8, #0x0, 0x10042f9bc <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x138>
10042f964:     	ldr	x8, [x0]
10042f968:     	str	x0, [sp, #0x20]
10042f96c:     	ldr	x1, [x0, #0x28]
10042f970:     	mov	x0, x8
10042f974:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
10042f978:     	tbz	w0, #0x0, 0x10042f984 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x100>
10042f97c:     	mov	x0, x1
10042f980:     	b	0x10042f9bc <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x138>
10042f984:     	str	x1, [sp, #0x18]
10042f988:     	ldr	x8, [sp, #0x20]
10042f98c:     	ldr	x19, [x8]
10042f990:     	ldr	x8, [x19, #0x140]
10042f994:     	cbz	x8, 0x10042fe98 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x614>
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
100179340:     	bl	0x10044b080 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17h826547c213f9bec6E>
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
100179568:     	bl	0x10042dee4 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17h17ea25fc327a0ca7E>
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
100179a28:     	bl	0x10042dee4 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17h17ea25fc327a0ca7E>
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

/tmp/oxide-c2-v8-candidate-plain/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010042dee4 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17h17ea25fc327a0ca7E>:
10042dee4:     	stp	x28, x27, [sp, #-0x60]!
10042dee8:     	stp	x26, x25, [sp, #0x10]
10042deec:     	stp	x24, x23, [sp, #0x20]
10042def0:     	stp	x22, x21, [sp, #0x30]
10042def4:     	stp	x20, x19, [sp, #0x40]
10042def8:     	stp	x29, x30, [sp, #0x50]
10042defc:     	add	x29, sp, #0x50
10042df00:     	sub	sp, sp, #0x380
10042df04:     	mov	x24, x7
```

### complete_read excerpt 1

```asm

/tmp/oxide-c2-v8-candidate-plain/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010042f9f0 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17h9cc13d7afd94031dE>:
10042f9f0:     	sub	sp, sp, #0x1f0
10042f9f4:     	stp	x28, x27, [sp, #0x190]
10042f9f8:     	stp	x26, x25, [sp, #0x1a0]
10042f9fc:     	stp	x24, x23, [sp, #0x1b0]
10042fa00:     	stp	x22, x21, [sp, #0x1c0]
10042fa04:     	stp	x20, x19, [sp, #0x1d0]
10042fa08:     	stp	x29, x30, [sp, #0x1e0]
10042fa0c:     	add	x29, sp, #0x1e0
10042fa10:     	mov	x20, x4
```
