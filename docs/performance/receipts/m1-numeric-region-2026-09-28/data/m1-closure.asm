
/tmp/oxide-m1.mtJwjB/candidate-target/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100426944 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE>:
100426944: d10203ff    	sub	sp, sp, #0x80
100426948: 6d0223e9    	stp	d9, d8, [sp, #0x20]
10042694c: a90367fa    	stp	x26, x25, [sp, #0x30]
100426950: a9045ff8    	stp	x24, x23, [sp, #0x40]
100426954: a90557f6    	stp	x22, x21, [sp, #0x50]
100426958: a9064ff4    	stp	x20, x19, [sp, #0x60]
10042695c: a9077bfd    	stp	x29, x30, [sp, #0x70]
100426960: 9101c3fd    	add	x29, sp, #0x70
100426964: 79407c68    	ldrh	w8, [x3, #0x3e]
100426968: f9402049    	ldr	x9, [x2, #0x40]
10042696c: ab080129    	adds	x9, x9, x8
100426970: 540010a2    	b.hs	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426974: a9432848    	ldp	x8, x10, [x2, #0x30]
100426978: cb08014a    	sub	x10, x10, x8
10042697c: eb0a013f    	cmp	x9, x10
100426980: 54001028    	b.hi	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426984: 79407873    	ldrh	w19, [x3, #0x3c]
100426988: f940144a    	ldr	x10, [x2, #0x28]
10042698c: eb0a010d    	subs	x13, x8, x10
100426990: fa5381a0    	ccmp	x13, x19, #0x0, hi
100426994: 54000f89    	b.ls	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426998: f9400829    	ldr	x9, [x1, #0x10]
10042699c: 8b13014b    	add	x11, x10, x19
1004269a0: eb09017f    	cmp	x11, x9
1004269a4: 54001b42    	b.hs	0x100426d0c <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x3c8>
1004269a8: f940042c    	ldr	x12, [x1, #0x8]
1004269ac: d37ced6b    	lsl	x11, x11, #4
1004269b0: 386b698b    	ldrb	w11, [x12, x11]
1004269b4: 51000d6b    	sub	w11, w11, #0x3
1004269b8: 7100097f    	cmp	w11, #0x2
1004269bc: 54000e42    	b.hs	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
1004269c0: 7940606e    	ldrh	w14, [x3, #0x30]
1004269c4: 7940646b    	ldrh	w11, [x3, #0x32]
1004269c8: 710005df    	cmp	w14, #0x1
1004269cc: 540001c1    	b.ne	0x100426a04 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0xc0>
1004269d0: f9401048    	ldr	x8, [x2, #0x20]
1004269d4: eb09015f    	cmp	x10, x9
1004269d8: 540018e8    	b.hi	0x100426cf4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x3b0>
1004269dc: eb08014e    	subs	x14, x10, x8
1004269e0: 540018a3    	b.lo	0x100426cf4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x3b0>
1004269e4: eb0b01df    	cmp	x14, x11
1004269e8: 54000ce9    	b.ls	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
1004269ec: 8b081188    	add	x8, x12, x8, lsl #4
1004269f0: 8b0b1108    	add	x8, x8, x11, lsl #4
1004269f4: 3940010b    	ldrb	w11, [x8]
1004269f8: 7100297f    	cmp	w11, #0xa
1004269fc: 54000163    	b.lo	0x100426a28 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0xe4>
100426a00: 14000061    	b	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426a04: eb09011f    	cmp	x8, x9
100426a08: 540016a8    	b.hi	0x100426cdc <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x398>
100426a0c: eb0b01bf    	cmp	x13, x11
100426a10: 54000ba9    	b.ls	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426a14: 8b0a1188    	add	x8, x12, x10, lsl #4
100426a18: 8b0b1108    	add	x8, x8, x11, lsl #4
100426a1c: 3940010b    	ldrb	w11, [x8]
100426a20: 7100257f    	cmp	w11, #0x9
100426a24: 54000b08    	b.hi	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426a28: b940006b    	ldr	w11, [x3]
100426a2c: 7100096e    	subs	w14, w11, #0x2
100426a30: 5280004f    	mov	w15, #0x2               ; =2
100426a34: 1a8e31ee    	csel	w14, w15, w14, lo
100426a38: 340000ce    	cbz	w14, 0x100426a50 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x10c>
100426a3c: 710005df    	cmp	w14, #0x1
100426a40: 54000321    	b.ne	0x100426aa4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x160>
100426a44: 5280000b    	mov	w11, #0x0               ; =0
100426a48: b9400469    	ldr	w9, [x3, #0x4]
100426a4c: 1400001a    	b	0x100426ab4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x170>
100426a50: 7940086b    	ldrh	w11, [x3, #0x4]
100426a54: 79400c6e    	ldrh	w14, [x3, #0x6]
100426a58: 3600076b    	tbz	w11, #0x0, 0x100426b44 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x200>
100426a5c: 5280000b    	mov	w11, #0x0               ; =0
100426a60: f940104d    	ldr	x13, [x2, #0x20]
100426a64: eb0d014a    	subs	x10, x10, x13
100426a68: 54000909    	b.ls	0x100426b88 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x244>
100426a6c: eb0e015f    	cmp	x10, x14
100426a70: 540008c9    	b.ls	0x100426b88 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x244>
100426a74: 8b0e01aa    	add	x10, x13, x14
100426a78: eb09015f    	cmp	x10, x9
100426a7c: 54001562    	b.hs	0x100426d28 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x3e4>
100426a80: 8b0a1189    	add	x9, x12, x10, lsl #4
100426a84: 3940012a    	ldrb	w10, [x9]
100426a88: 7100295f    	cmp	w10, #0xa
100426a8c: 540007c2    	b.hs	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426a90: 7100115f    	cmp	w10, #0x4
100426a94: 540006e1    	b.ne	0x100426b70 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x22c>
100426a98: fd400520    	ldr	d0, [x9, #0x8]
100426a9c: 5280002b    	mov	w11, #0x1               ; =1
100426aa0: 14000005    	b	0x100426ab4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x170>
100426aa4: 7100097f    	cmp	w11, #0x2
100426aa8: 540006e0    	b.eq	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426aac: b9400469    	ldr	w9, [x3, #0x4]
100426ab0: fd400460    	ldr	d0, [x3, #0x8]
100426ab4: 1e620121    	scvtf	d1, w9
100426ab8: 7200017f    	tst	w11, #0x1
100426abc: 1e611c00    	fcsel	d0, d0, d1, ne
100426ac0: 1e65c001    	frintz	d1, d0
100426ac4: 1e602008    	fcmp	d0, #0.0
100426ac8: b26b6be9    	mov	x9, #0xffffffe00000     ; =281474974613504
100426acc: f2e83de9    	movk	x9, #0x41ef, lsl #48
100426ad0: 9e670122    	fmov	d2, x9
100426ad4: 1e62a400    	fccmp	d0, d2, #0x0, ge
100426ad8: 1e604420    	fccmp	d1, d0, #0x0, mi
100426adc: 54000541    	b.ne	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426ae0: aa0303f7    	mov	x23, x3
100426ae4: aa0103f9    	mov	x25, x1
100426ae8: aa0203fa    	mov	x26, x2
100426aec: aa0003f4    	mov	x20, x0
100426af0: 1e790003    	fcvtzu	w3, d0
100426af4: 910023e0    	add	x0, sp, #0x8
100426af8: aa0403e1    	mov	x1, x4
100426afc: aa0803e2    	mov	x2, x8
100426b00: 9400035a    	bl	0x100427868 <__ZN13quickjs_oxide6engine6object16ordinary_storage62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$17peek_dense_number17h2fffb5705dffd867E>
100426b04: b9400bf5    	ldr	w21, [sp, #0x8]
100426b08: 71000abf    	cmp	w21, #0x2
100426b0c: 540008a0    	b.eq	0x100426c20 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x2dc>
100426b10: b9400ff6    	ldr	w22, [sp, #0xc]
100426b14: fd400be8    	ldr	d8, [sp, #0x10]
100426b18: aa1703e8    	mov	x8, x23
100426b1c: b9401af7    	ldr	w23, [x23, #0x18]
100426b20: b9401d18    	ldr	w24, [x8, #0x1c]
100426b24: 71000ae9    	subs	w9, w23, #0x2
100426b28: 5280004a    	mov	w10, #0x2               ; =2
100426b2c: 1a893149    	csel	w9, w10, w9, lo
100426b30: 34000409    	cbz	w9, 0x100426bb0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x26c>
100426b34: 7100053f    	cmp	w9, #0x1
100426b38: 540004c1    	b.ne	0x100426bd0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x28c>
100426b3c: 52800017    	mov	w23, #0x0               ; =0
100426b40: 14000030    	b	0x100426c00 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x2bc>
100426b44: eb0e01bf    	cmp	x13, x14
100426b48: 540001e9    	b.ls	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426b4c: 8b0e014a    	add	x10, x10, x14
100426b50: eb09015f    	cmp	x10, x9
100426b54: 54000f42    	b.hs	0x100426d3c <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x3f8>
100426b58: 8b0a1189    	add	x9, x12, x10, lsl #4
100426b5c: 3940012a    	ldrb	w10, [x9]
100426b60: 7100295f    	cmp	w10, #0xa
100426b64: 54000102    	b.hs	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426b68: 7100115f    	cmp	w10, #0x4
100426b6c: 54fff960    	b.eq	0x100426a98 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x154>
100426b70: 71000d5f    	cmp	w10, #0x3
100426b74: 54000081    	b.ne	0x100426b84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x240>
100426b78: 5280000b    	mov	w11, #0x0               ; =0
100426b7c: b9400529    	ldr	w9, [x9, #0x4]
100426b80: 17ffffb3    	b	0x100426a4c <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x108>
100426b84: 5280000b    	mov	w11, #0x0               ; =0
100426b88: 3900040b    	strb	w11, [x0, #0x1]
100426b8c: 3900001f    	strb	wzr, [x0]
100426b90: a9477bfd    	ldp	x29, x30, [sp, #0x70]
100426b94: a9464ff4    	ldp	x20, x19, [sp, #0x60]
100426b98: a94557f6    	ldp	x22, x21, [sp, #0x50]
100426b9c: a9445ff8    	ldp	x24, x23, [sp, #0x40]
100426ba0: a94367fa    	ldp	x26, x25, [sp, #0x30]
100426ba4: 6d4223e9    	ldp	d9, d8, [sp, #0x20]
100426ba8: 910203ff    	add	sp, sp, #0x80
100426bac: d65f03c0    	ret
100426bb0: aa1a03e8    	mov	x8, x26
100426bb4: aa1903e1    	mov	x1, x25
100426bb8: 53107f04    	lsr	w4, w24, #16
100426bbc: 36000138    	tbz	w24, #0x0, 0x100426be0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x29c>
100426bc0: a9420d02    	ldp	x2, x3, [x8, #0x20]
100426bc4: 910023e0    	add	x0, sp, #0x8
100426bc8: 94000fb5    	bl	0x10042aa9c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots19immediate_parameter17h55ea4d49ca7521f6E>
100426bcc: 14000008    	b	0x100426bec <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x2a8>
100426bd0: f9401108    	ldr	x8, [x8, #0x20]
100426bd4: b9000ff8    	str	w24, [sp, #0xc]
100426bd8: f9000be8    	str	x8, [sp, #0x10]
100426bdc: 14000005    	b	0x100426bf0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x2ac>
100426be0: a9428d02    	ldp	x2, x3, [x8, #0x28]
100426be4: 910023e0    	add	x0, sp, #0x8
100426be8: 94000fd3    	bl	0x10042ab34 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots15immediate_local17h5297265cb89e5aeeE>
100426bec: b9400bf7    	ldr	w23, [sp, #0x8]
100426bf0: 71000aff    	cmp	w23, #0x2
100426bf4: 54000160    	b.eq	0x100426c20 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x2dc>
100426bf8: b9400ff8    	ldr	w24, [sp, #0xc]
100426bfc: fd400be9    	ldr	d9, [sp, #0x10]
100426c00: a9408b21    	ldp	x1, x2, [x25, #0x8]
100426c04: a9429343    	ldp	x3, x4, [x26, #0x28]
100426c08: 910023e0    	add	x0, sp, #0x8
100426c0c: aa1303e5    	mov	x5, x19
100426c10: 94000fef    	bl	0x10042abcc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots19admit_numeric_local17hee66c2771087e43dE>
100426c14: b9400be9    	ldr	w9, [sp, #0x8]
100426c18: 7100093f    	cmp	w9, #0x2
100426c1c: 54000081    	b.ne	0x100426c2c <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x2e8>
100426c20: 5280000b    	mov	w11, #0x0               ; =0
100426c24: aa1403e0    	mov	x0, x20
100426c28: 17ffffd8    	b	0x100426b88 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x244>
100426c2c: f9400fe8    	ldr	x8, [sp, #0x18]
100426c30: b9400fea    	ldr	w10, [sp, #0xc]
100426c34: fd400be0    	ldr	d0, [sp, #0x10]
100426c38: 1e6202c1    	scvtf	d1, w22
100426c3c: 720002bf    	tst	w21, #0x1
100426c40: 1e611d01    	fcsel	d1, d8, d1, ne
100426c44: 1e620302    	scvtf	d2, w24
100426c48: 720002ff    	tst	w23, #0x1
100426c4c: 1e621d22    	fcsel	d2, d9, d2, ne
100426c50: 1e620821    	fmul	d1, d1, d2
100426c54: 1e78002b    	fcvtzs	w11, d1
100426c58: 1e620162    	scvtf	d2, w11
100426c5c: 1e622020    	fcmp	d1, d2
100426c60: aa1403e0    	mov	x0, x20
100426c64: 54000101    	b.ne	0x100426c84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x340>
100426c68: 1e602028    	fcmp	d1, #0.0
100426c6c: 9e66002c    	fmov	x12, d1
100426c70: fa400984    	ccmp	x12, #0x0, #0x4, eq
100426c74: 54000081    	b.ne	0x100426c84 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x340>
100426c78: 36000229    	tbz	w9, #0x0, 0x100426cbc <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x378>
100426c7c: 4ea21c41    	mov.16b	v1, v2
100426c80: 14000004    	b	0x100426c90 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x34c>
100426c84: 1e620142    	scvtf	d2, w10
100426c88: 7200013f    	tst	w9, #0x1
100426c8c: 1e621c00    	fcsel	d0, d0, d2, ne
100426c90: 1e612800    	fadd	d0, d0, d1
100426c94: 1e780009    	fcvtzs	w9, d0
100426c98: 1e620121    	scvtf	d1, w9
100426c9c: 1e612000    	fcmp	d0, d1
100426ca0: 540000a1    	b.ne	0x100426cb4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x370>
100426ca4: 1e602008    	fcmp	d0, #0.0
100426ca8: 9e66000a    	fmov	x10, d0
100426cac: fa400944    	ccmp	x10, #0x0, #0x4, eq
100426cb0: 540000a0    	b.eq	0x100426cc4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x380>
100426cb4: 5280008a    	mov	w10, #0x4               ; =4
100426cb8: 14000004    	b	0x100426cc8 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x384>
100426cbc: 2b0b0149    	adds	w9, w10, w11
100426cc0: 54000306    	b.vs	0x100426d20 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x3dc>
100426cc4: 5280006a    	mov	w10, #0x3               ; =3
100426cc8: 3900010a    	strb	w10, [x8]
100426ccc: b9000509    	str	w9, [x8, #0x4]
100426cd0: 5280002b    	mov	w11, #0x1               ; =1
100426cd4: fd000500    	str	d0, [x8, #0x8]
100426cd8: 17ffffac    	b	0x100426b88 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x244>
100426cdc: b00010e3    	adrp	x3, 0x100643000 <dyld_stub_binder+0x100643000>
100426ce0: 91360063    	add	x3, x3, #0xd80
100426ce4: aa0a03e0    	mov	x0, x10
100426ce8: aa0803e1    	mov	x1, x8
100426cec: aa0903e2    	mov	x2, x9
100426cf0: 9402b080    	bl	0x1004d2ef0 <__RNvNtNtCs6sq8b9ugfBC_4core5slice5index16slice_index_fail>
100426cf4: b00010e3    	adrp	x3, 0x100643000 <dyld_stub_binder+0x100643000>
100426cf8: 91360063    	add	x3, x3, #0xd80
100426cfc: aa0803e0    	mov	x0, x8
100426d00: aa0a03e1    	mov	x1, x10
100426d04: aa0903e2    	mov	x2, x9
100426d08: 9402b07a    	bl	0x1004d2ef0 <__RNvNtNtCs6sq8b9ugfBC_4core5slice5index16slice_index_fail>
100426d0c: b00010e2    	adrp	x2, 0x100643000 <dyld_stub_binder+0x100643000>
100426d10: 91366042    	add	x2, x2, #0xd98
100426d14: aa0b03e0    	mov	x0, x11
100426d18: aa0903e1    	mov	x1, x9
100426d1c: 9402b0ae    	bl	0x1004d2fd4 <__RNvNtCs6sq8b9ugfBC_4core9panicking18panic_bounds_check>
100426d20: 1e620140    	scvtf	d0, w10
100426d24: 17ffffd6    	b	0x100426c7c <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10with_slots17headcbe44eec489dfE+0x338>
100426d28: b00010e2    	adrp	x2, 0x100643000 <dyld_stub_binder+0x100643000>
100426d2c: 91372042    	add	x2, x2, #0xdc8
100426d30: aa0a03e0    	mov	x0, x10
100426d34: aa0903e1    	mov	x1, x9
100426d38: 9402b0a7    	bl	0x1004d2fd4 <__RNvNtCs6sq8b9ugfBC_4core9panicking18panic_bounds_check>
100426d3c: b00010e2    	adrp	x2, 0x100643000 <dyld_stub_binder+0x100643000>
100426d40: 91366042    	add	x2, x2, #0xd98
100426d44: aa0a03e0    	mov	x0, x10
100426d48: aa0903e1    	mov	x1, x9
100426d4c: 9402b0a2    	bl	0x1004d2fd4 <__RNvNtCs6sq8b9ugfBC_4core9panicking18panic_bounds_check>
