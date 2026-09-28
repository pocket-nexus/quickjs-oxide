# C1 V8-base ARM64 codegen excerpts

Rust 1.88.0 release binaries with fat LTO. Static calls and symbol spans are compiler output, not dynamic execution counts. Only selected instructions are excerpted.

## baseline

### ready_run excerpt 1

```asm
10017920c:     	add	x8, sp, #0x1b8
100179210:     	orr	x8, x8, #0x1
100179214:     	stp	x8, x9, [sp, #0xc8]
100179218:     	stp	x28, x1, [sp, #0x130]
10017921c:     	add	x0, sp, #0x140
100179220:     	mov	x1, x23
100179224:     	mov	x2, x25
100179228:     	mov	x3, x20
10017922c:     	bl	0x10044b0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E>
100179230:     	ldrb	w19, [sp, #0x140]
100179234:     	cmp	w19, #0x49
100179238:     	b.eq	0x100179250 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd8>
10017923c:     	cmp	w19, #0x46
100179240:     	ccmp	w19, #0x1e, #0x4, ne
100179244:     	b.ne	0x100179250 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd8>
100179248:     	ldr	x24, [sp, #0x148]
10017924c:     	b	0x10017926c <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xf4>
```

### ready_run excerpt 2

```asm
100179440:     	ldr	x8, [sp, #0x120]
100179444:     	bfi	x5, x8, #8, #1
100179448:     	add	x0, sp, #0x3d0
10017944c:     	and	w6, w22, #0x1
100179450:     	mov	x1, x21
100179454:     	mov	x2, x23
100179458:     	mov	x3, x25
10017945c:     	mov	x4, x20
100179460:     	bl	0x10042deb8 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E>
100179464:     	ldr	x8, [sp, #0x3d0]
100179468:     	ldr	x9, [sp, #0x3e0]
10017946c:     	cmp	x8, #0x6
100179470:     	ldr	x12, [sp, #0x70]
100179474:     	csel	x12, x12, x9, eq
100179478:     	cmp	x8, #0x5
10017947c:     	b.ne	0x10017b2a0 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2128>
100179480:     	str	x12, [sp, #0x70]
```

### ready_run excerpt 3

```asm
1001797e0:     	b	0x10017b784 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x260c>
1001797e4:     	lsl	x5, x26, #32
1001797e8:     	add	x0, sp, #0x3d0
1001797ec:     	and	w6, w24, #0x1
1001797f0:     	mov	x1, x21
1001797f4:     	mov	x2, x23
1001797f8:     	mov	x3, x25
1001797fc:     	mov	x4, x20
100179800:     	bl	0x10042deb8 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E>
100179804:     	ldr	x8, [sp, #0x3d0]
100179808:     	ldr	x9, [sp, #0x3e0]
10017980c:     	cmp	x8, #0x6
100179810:     	ldr	x12, [sp, #0x90]
100179814:     	csel	x12, x12, x9, eq
100179818:     	cmp	x8, #0x5
10017981c:     	b.ne	0x10017b2a0 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2128>
100179820:     	str	x12, [sp, #0x90]
```

### read_progress excerpt 1

```asm
10042df1c:     	ldr	w9, [x8, #0x130]
10042df20:     	cmp	w9, #0x2
10042df24:     	b.eq	0x10042e96c <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0xab4>
10042df28:     	ldr	x8, [x8, #0x110]
10042df2c:     	ldr	d0, [x8, #0xd0]
10042df30:     	str	d0, [sp, #0x68]
10042df34:     	ldr	x0, [x24]
10042df38:     	ldr	x1, [x24, #0x28]
10042df3c:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
10042df40:     	tbz	w0, #0x0, 0x10042df50 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0x98>
10042df44:     	mov	w8, #0x6                ; =6
10042df48:     	stp	x8, x1, [x19]
10042df4c:     	b	0x10042e5d8 <__ZN13quickjs_oxide6engine2vm15property_driver13read_progress17had9f68c916f6de86E+0x720>
10042df50:     	add	x27, sp, #0x2d8
10042df54:     	lsr	x8, x25, #32
10042df58:     	stp	x8, x20, [sp, #0x50]
10042df5c:     	str	x1, [sp, #0x70]
```

### complete_read excerpt 1

```asm
10042fa8c:     	bl	0x100137b14 <__ZN13quickjs_oxide6engine2vm5frame10FrameStore11current_mut17h3f2d85e78871d7baE>
10042fa90:     	mov	x8, x0
10042fa94:     	mov	x0, x1
10042fa98:     	tbnz	w8, #0x0, 0x10042faf4 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x138>
10042fa9c:     	ldr	x8, [x0]
10042faa0:     	str	x0, [sp, #0x20]
10042faa4:     	ldr	x1, [x0, #0x28]
10042faa8:     	mov	x0, x8
10042faac:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
10042fab0:     	tbz	w0, #0x0, 0x10042fabc <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x100>
10042fab4:     	mov	x0, x1
10042fab8:     	b	0x10042faf4 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x138>
10042fabc:     	str	x1, [sp, #0x18]
10042fac0:     	ldr	x8, [sp, #0x20]
10042fac4:     	ldr	x19, [x8]
10042fac8:     	ldr	x8, [x19, #0x140]
10042facc:     	cbz	x8, 0x10042ffd0 <__ZN13quickjs_oxide6engine2vm15property_driver13complete_read17hb0ef43f329d8e4b6E+0x614>
```

## C1

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
