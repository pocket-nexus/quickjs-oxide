# C1 on M2: ready-driver machine-code excerpts

Plain release binaries from `m2-repeat-builds.json`; ARM64, Rust 1.88.0, fat LTO. Addresses and branch layouts differ between builds. These are inspectable excerpts from `ready::run`, not a complete dataflow proof of every inlined producer instruction.

## baseline

Return-value transport from `execute_frame` into `ready::run`:

```asm
100179214:     	stp	x8, x9, [sp, #0xc8]
100179218:     	stp	x28, x1, [sp, #0x130]
10017921c:     	add	x0, sp, #0x140
100179220:     	mov	x1, x23
100179224:     	mov	x2, x25
100179228:     	mov	x3, x20
10017922c:     	bl	0x100447f94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E>
100179230:     	ldrb	w19, [sp, #0x140]
100179234:     	cmp	w19, #0x49
100179238:     	b.eq	0x100179250 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd8>
10017923c:     	cmp	w19, #0x46
100179240:     	ccmp	w19, #0x1e, #0x4, ne
100179244:     	b.ne	0x100179250 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0xd8>
100179248:     	ldr	x24, [sp, #0x148]
```

Direct completion region around the removed recovery call:

```asm
1001795cc:     	mov	x21, x1
1001795d0:     	tbnz	w0, #0x0, 0x10017b404 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x228c>
1001795d4:     	ldr	x0, [x21]
1001795d8:     	ldr	w8, [x0, #0x130]
1001795dc:     	cmp	w8, #0x2
1001795e0:     	b.eq	0x10017b604 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x248c>
1001795e4:     	ldr	x8, [x0, #0x140]
1001795e8:     	cbz	x8, 0x10017b604 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x248c>
1001795ec:     	str	x20, [sp, #0x110]
1001795f0:     	ldr	x8, [x0, #0x110]
1001795f4:     	ldp	w28, w27, [x8, #0xd0]
1001795f8:     	ldr	x20, [x0, #0x180]
1001795fc:     	ldr	x1, [x21, #0x28]
100179600:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
100179604:     	mov	x19, x1
100179608:     	tbnz	w0, #0x0, 0x10017b424 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x22ac>
10017960c:     	str	w28, [sp, #0xa8]
100179610:     	str	w27, [sp, #0xb0]
100179614:     	ldr	x28, [x21]
100179618:     	ldr	x8, [x28, #0x140]
10017961c:     	cbz	x8, 0x10017b690 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2518>
100179620:     	add	x1, x28, #0x140
100179624:     	ldr	x0, [sp, #0x128]
```

Remaining recovery site for an unmigrated path:

```asm
100179cec:     	str	x9, [x8, #0x8]
100179cf0:     	ldr	x8, [x25, #0x40]
100179cf4:     	add	x8, x8, #0x1
100179cf8:     	str	x8, [x25, #0x40]
100179cfc:     	ldr	x20, [sp, #0x98]
100179d00:     	ldr	x0, [x20]
100179d04:     	ldr	x1, [x20, #0x28]
100179d08:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
100179d0c:     	mov	x21, x1
100179d10:     	tbnz	w0, #0x0, 0x10017b398 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2220>
100179d14:     	str	x21, [x20, #0x30]
100179d18:     	mov	w28, #0xa               ; =10
100179d1c:     	ldp	x25, x20, [sp, #0x108]
100179d20:     	sub	w8, w28, #0xa
100179d24:     	and	w9, w8, #0xff
```

## candidate

Return-value transport from `execute_frame` into `ready::run`:

```asm
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

Direct completion region around the removed recovery call:

```asm
1001795b0:     	bl	0x100137b14 <__ZN13quickjs_oxide6engine2vm5frame10FrameStore11current_mut17h3f2d85e78871d7baE>
1001795b4:     	mov	x24, x1
1001795b8:     	tbnz	w0, #0x0, 0x10017b2ec <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2174>
1001795bc:     	ldr	x26, [x24]
1001795c0:     	ldr	w8, [x26, #0x130]
1001795c4:     	cmp	w8, #0x2
1001795c8:     	b.eq	0x10017b4a8 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2330>
1001795cc:     	ldr	x8, [x26, #0x140]
1001795d0:     	cbz	x8, 0x10017b4a8 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2330>
1001795d4:     	str	x19, [sp, #0x108]
1001795d8:     	ldr	x8, [x26, #0x110]
1001795dc:     	ldp	w21, w8, [x8, #0xd0]
1001795e0:     	str	w8, [sp, #0xb0]
1001795e4:     	ldr	x19, [x26, #0x180]
1001795e8:     	add	x1, x26, #0x140
1001795ec:     	ldr	x0, [sp, #0x120]
1001795f0:     	bl	0x100174998 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore13check_current17h13a1812719f68e40E>
1001795f4:     	cbnz	x0, 0x10017b2fc <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x2184>
1001795f8:     	and	w8, w22, #0xff
1001795fc:     	ldr	x10, [sp, #0x130]
100179600:     	ldp	x9, x1, [x10, #0x28]
100179604:     	ldr	x11, [x26, #0x180]
100179608:     	cmp	w8, #0x7
10017960c:     	b.lo	0x100179aec <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x974>
```

Remaining recovery site for an unmigrated path:

```asm
100179d30:     	ldr	x9, [sp, #0x70]
100179d34:     	ldr	x8, [x9, #0x40]
100179d38:     	add	x8, x8, #0x1
100179d3c:     	str	x8, [x9, #0x40]
100179d40:     	ldr	x19, [sp, #0x98]
100179d44:     	ldr	x0, [x19]
100179d48:     	ldr	x1, [x19, #0x28]
100179d4c:     	bl	0x100178ab4 <__ZN13quickjs_oxide6engine2vm5frame5Frame7next_pc17ha479e19618cab3d6E>
100179d50:     	mov	x24, x1
100179d54:     	tbnz	w0, #0x0, 0x10017b270 <__ZN13quickjs_oxide6engine2vm6driver5ready3run17h72f3982683eb1a08E+0x20f8>
100179d58:     	str	x24, [x19, #0x30]
100179d5c:     	mov	w26, #0xa               ; =10
100179d60:     	ldr	x24, [sp, #0x128]
100179d64:     	ldr	x19, [sp, #0x108]
```

The baseline contains two static `Frame::next_pc` calls in `ready::run`; the candidate contains one. The omitted call is on direct primitive completion. The remaining call serves an unmigrated completion. The action and result remain 16 bytes and the frame 56 bytes; equal size does not establish equal instruction cost.
