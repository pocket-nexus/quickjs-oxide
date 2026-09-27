
/private/tmp/oxide-m2-candidate-plain/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

0000000100447f94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E>:
100447f94: 6db82beb    	stp	d11, d10, [sp, #-0x80]!
100447f98: 6d0123e9    	stp	d9, d8, [sp, #0x10]
100447f9c: a9026ffc    	stp	x28, x27, [sp, #0x20]
100447fa0: a90367fa    	stp	x26, x25, [sp, #0x30]
100447fa4: a9045ff8    	stp	x24, x23, [sp, #0x40]
100447fa8: a90557f6    	stp	x22, x21, [sp, #0x50]
100447fac: a9064ff4    	stp	x20, x19, [sp, #0x60]
100447fb0: a9077bfd    	stp	x29, x30, [sp, #0x70]
100447fb4: 9101c3fd    	add	x29, sp, #0x70
100447fb8: d10f43ff    	sub	sp, sp, #0x3d0
100447fbc: aa0103f7    	mov	x23, x1
100447fc0: aa0003f5    	mov	x21, x0
100447fc4: a9508420    	ldp	x0, x1, [x1, #0x108]
100447fc8: 97f3bed3    	bl	0x100137b14 <__ZN13quickjs_oxide6engine2vm5frame10FrameStore11current_mut17h3f2d85e78871d7baE>
100447fcc: 36000060    	tbz	w0, #0x0, 0x100447fd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44>
100447fd0: f90006a1    	str	x1, [x21, #0x8]
100447fd4: 14000017    	b	0x100448030 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x9c>
100447fd8: f90097e1    	str	x1, [sp, #0x128]
100447fdc: f940003a    	ldr	x26, [x1]
100447fe0: b9413348    	ldr	w8, [x26, #0x130]
100447fe4: 7100091f    	cmp	w8, #0x2
100447fe8: 5402e7c0    	b.eq	0x10044dce0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d4c>
100447fec: b9400348    	ldr	w8, [x26]
100447ff0: 3602e788    	tbz	w8, #0x0, 0x10044dce0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d4c>
100447ff4: aa1a03f8    	mov	x24, x26
100447ff8: f8408f08    	ldr	x8, [x24, #0x8]!
100447ffc: b402e9e8    	cbz	x8, 0x10044dd38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5da4>
100448000: f940a348    	ldr	x8, [x26, #0x140]
100448004: b402ea68    	cbz	x8, 0x10044dd50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5dbc>
100448008: 91050354    	add	x20, x26, #0x140
10044800c: 910082f6    	add	x22, x23, #0x20
100448010: aa1603e0    	mov	x0, x22
100448014: aa1403e1    	mov	x1, x20
100448018: 97f4b260    	bl	0x100174998 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore13check_current17h13a1812719f68e40E>
10044801c: f100001f    	cmp	x0, #0x0
100448020: 9a9f02d1    	csel	x17, x22, xzr, eq
100448024: 9a800290    	csel	x16, x20, x0, eq
100448028: b40001c0    	cbz	x0, 0x100448060 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcc>
10044802c: f90006a0    	str	x0, [x21, #0x8]
100448030: 52800928    	mov	w8, #0x49               ; =73
100448034: 390002a8    	strb	w8, [x21]
100448038: 910f43ff    	add	sp, sp, #0x3d0
10044803c: a9477bfd    	ldp	x29, x30, [sp, #0x70]
100448040: a9464ff4    	ldp	x20, x19, [sp, #0x60]
100448044: a94557f6    	ldp	x22, x21, [sp, #0x50]
100448048: a9445ff8    	ldp	x24, x23, [sp, #0x40]
10044804c: a94367fa    	ldp	x26, x25, [sp, #0x30]
100448050: a9426ffc    	ldp	x28, x27, [sp, #0x20]
100448054: 6d4123e9    	ldp	d9, d8, [sp, #0x10]
100448058: 6cc82beb    	ldp	d11, d10, [sp], #0x80
10044805c: d65f03c0    	ret
100448060: f94097e0    	ldr	x0, [sp, #0x128]
100448064: a9c2a013    	ldp	x19, x8, [x0, #0x28]!
100448068: f90093e8    	str	x8, [sp, #0x120]
10044806c: d360fd08    	lsr	x8, x8, #32
100448070: b502d968    	cbnz	x8, 0x10044db9c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c08>
100448074: 91044348    	add	x8, x26, #0x110
100448078: f9008be8    	str	x8, [sp, #0x110]
10044807c: f9400108    	ldr	x8, [x8]
100448080: f9403509    	ldr	x9, [x8, #0x68]
100448084: b402d8c9    	cbz	x9, 0x10044db9c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c08>
100448088: f940310a    	ldr	x10, [x8, #0x60]
10044808c: 9100414a    	add	x10, x10, #0x10
100448090: f100053f    	cmp	x9, #0x1
100448094: 54000081    	b.ne	0x1004480a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x110>
100448098: d280000b    	mov	x11, #0x0               ; =0
10044809c: f94093ef    	ldr	x15, [sp, #0x120]
1004480a0: 1400000b    	b	0x1004480cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x138>
1004480a4: d280000b    	mov	x11, #0x0               ; =0
1004480a8: f94093ef    	ldr	x15, [sp, #0x120]
1004480ac: d341fd2c    	lsr	x12, x9, #1
1004480b0: 8b0b018d    	add	x13, x12, x11
1004480b4: b86d794e    	ldr	w14, [x10, x13, lsl #2]
1004480b8: 6b0f01df    	cmp	w14, w15
1004480bc: 9a8d816b    	csel	x11, x11, x13, hi
1004480c0: cb0c0129    	sub	x9, x9, x12
1004480c4: f100053f    	cmp	x9, #0x1
1004480c8: 54ffff28    	b.hi	0x1004480ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x118>
1004480cc: b86b7949    	ldr	w9, [x10, x11, lsl #2]
1004480d0: 6b0f013f    	cmp	w9, w15
1004480d4: 5402d641    	b.ne	0x10044db9c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c08>
1004480d8: f9006fe0    	str	x0, [sp, #0xd8]
1004480dc: f9402d1c    	ldr	x28, [x8, #0x58]
1004480e0: f94093ea    	ldr	x10, [sp, #0x120]
1004480e4: eb0a039f    	cmp	x28, x10
1004480e8: 5402da49    	b.ls	0x10044dc30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c9c>
1004480ec: f90087f1    	str	x17, [sp, #0x108]
1004480f0: 9100c209    	add	x9, x16, #0x30
1004480f4: a90f63e9    	stp	x9, x24, [sp, #0xf0]
1004480f8: 9100820b    	add	x11, x16, #0x20
1004480fc: f9008ff0    	str	x16, [sp, #0x118]
100448100: 9100a209    	add	x9, x16, #0x28
100448104: f90073e9    	str	x9, [sp, #0xe0]
100448108: d10343a9    	sub	x9, x29, #0xd0
10044810c: b2400129    	orr	x9, x9, #0x1
100448110: f90077e9    	str	x9, [sp, #0xe8]
100448114: d103c3a9    	sub	x9, x29, #0xf0
100448118: b2400129    	orr	x9, x9, #0x1
10044811c: a90cafe9    	stp	x9, x11, [sp, #0xc8]
100448120: 9109c3e9    	add	x9, sp, #0x270
100448124: b2400129    	orr	x9, x9, #0x1
100448128: f90033e9    	str	x9, [sp, #0x60]
10044812c: 910b83e9    	add	x9, sp, #0x2e0
100448130: b240012b    	orr	x11, x9, #0x1
100448134: d10403a9    	sub	x9, x29, #0x100
100448138: b2400129    	orr	x9, x9, #0x1
10044813c: a9092fe9    	stp	x9, x11, [sp, #0x90]
100448140: 910bc3e9    	add	x9, sp, #0x2f0
100448144: b2400129    	orr	x9, x9, #0x1
100448148: f90053e9    	str	x9, [sp, #0xa0]
10044814c: 9108a3e9    	add	x9, sp, #0x228
100448150: b240012b    	orr	x11, x9, #0x1
100448154: 9107e3e9    	add	x9, sp, #0x1f8
100448158: b2400129    	orr	x9, x9, #0x1
10044815c: f9005fe9    	str	x9, [sp, #0xb8]
100448160: 910723e9    	add	x9, sp, #0x1c8
100448164: b2400129    	orr	x9, x9, #0x1
100448168: f9005be9    	str	x9, [sp, #0xb0]
10044816c: 910763e9    	add	x9, sp, #0x1d8
100448170: b2400129    	orr	x9, x9, #0x1
100448174: a90827eb    	stp	x11, x9, [sp, #0x80]
100448178: 9107a3e9    	add	x9, sp, #0x1e8
10044817c: b240012b    	orr	x11, x9, #0x1
100448180: 9106e3e9    	add	x9, sp, #0x1b8
100448184: b2400129    	orr	x9, x9, #0x1
100448188: f90063e9    	str	x9, [sp, #0xc0]
10044818c: 9105e3e9    	add	x9, sp, #0x178
100448190: b2400129    	orr	x9, x9, #0x1
100448194: f90057e9    	str	x9, [sp, #0xa8]
100448198: 910563e9    	add	x9, sp, #0x158
10044819c: b2400129    	orr	x9, x9, #0x1
1004481a0: a9072fe9    	stp	x9, x11, [sp, #0x70]
1004481a4: 9105a3e9    	add	x9, sp, #0x168
1004481a8: b2400129    	orr	x9, x9, #0x1
1004481ac: f90037e9    	str	x9, [sp, #0x68]
1004481b0: 910643e9    	add	x9, sp, #0x190
1004481b4: b240012b    	orr	x11, x9, #0x1
1004481b8: 9106a3e9    	add	x9, sp, #0x1a8
1004481bc: b2400129    	orr	x9, x9, #0x1
1004481c0: a9052fe9    	stp	x9, x11, [sp, #0x50]
1004481c4: d00011e9    	adrp	x9, 0x100686000 <dyld_stub_binder+0x100686000>
1004481c8: 91358129    	add	x9, x9, #0xd60
1004481cc: f90027e9    	str	x9, [sp, #0x48]
1004481d0: aa0a03e9    	mov	x9, x10
1004481d4: f9402908    	ldr	x8, [x8, #0x50]
1004481d8: 91004118    	add	x24, x8, #0x10
1004481dc: b8697b13    	ldr	w19, [x24, x9, lsl #2]
1004481e0: 53106668    	ubfx	w8, w19, #16, #10
1004481e4: 71039d1f    	cmp	w8, #0xe7
1004481e8: 5402d242    	b.hs	0x10044dc30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c9c>
1004481ec: b0000a09    	adrp	x9, 0x100589000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16e90>
1004481f0: 9124e129    	add	x9, x9, #0x938
1004481f4: 7868593b    	ldrh	w27, [x9, w8, uxtw #1]
1004481f8: 531d7a68    	ubfx	w8, w19, #29, #2
1004481fc: f94093ea    	ldr	x10, [sp, #0x120]
100448200: 0b080149    	add	w9, w10, w8
100448204: 11000534    	add	w20, w9, #0x1
100448208: 531c7e79    	lsr	w25, w19, #28
10044820c: a91373f8    	stp	x24, x28, [sp, #0x130]
100448210: 53127e69    	lsr	w9, w19, #18
100448214: 12180529    	and	w9, w9, #0x300
100448218: b90143f3    	str	w19, [sp, #0x140]
10044821c: b90147ea    	str	w10, [sp, #0x144]
100448220: 331c7269    	bfxil	w9, w19, #28, #1
100448224: b9014bf4    	str	w20, [sp, #0x148]
100448228: 79029bfb    	strh	w27, [sp, #0x14c]
10044822c: 79029fe9    	strh	w9, [sp, #0x14e]
100448230: f900abf4    	str	x20, [sp, #0x150]
100448234: 360000f9    	tbz	w25, #0x0, 0x100448250 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2bc>
100448238: 2a0a03e9    	mov	w9, w10
10044823c: 91000536    	add	x22, x9, #0x1
100448240: eb1c02df    	cmp	x22, x28
100448244: 54036982    	b.hs	0x10044ef74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fe0>
100448248: b8767b09    	ldr	w9, [x24, x22, lsl #2]
10044824c: 14000002    	b	0x100448254 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2c0>
100448250: 12003e69    	and	w9, w19, #0xffff
100448254: f90083e9    	str	x9, [sp, #0x100]
100448258: 71039b7f    	cmp	w27, #0xe6
10044825c: 540311c8    	b.hi	0x10044e494 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6500>
100448260: f000090b    	adrp	x11, 0x10056b000 <dyld_stub_binder+0x10056b000>
100448264: 9126b16b    	add	x11, x11, #0x9ac
100448268: 10000089    	adr	x9, 0x100448278 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2e4>
10044826c: 787b796a    	ldrh	w10, [x11, x27, lsl #1]
100448270: 8b0a0929    	add	x9, x9, x10, lsl #2
100448274: d61f0120    	br	x9
100448278: f9408fe8    	ldr	x8, [sp, #0x118]
10044827c: f9402113    	ldr	x19, [x8, #0x40]
100448280: f1000a7f    	cmp	x19, #0x2
100448284: 54033003    	b.lo	0x10044e884 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68f0>
100448288: f9407be8    	ldr	x8, [sp, #0xf0]
10044828c: f9400108    	ldr	x8, [x8]
100448290: 8b130101    	add	x1, x8, x19
100448294: d1000836    	sub	x22, x1, #0x2
100448298: eb16003f    	cmp	x1, x22
10044829c: f94087e8    	ldr	x8, [sp, #0x108]
1004482a0: 54032f83    	b.lo	0x10044e890 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68fc>
1004482a4: f940091c    	ldr	x28, [x8, #0x10]
1004482a8: eb1c003f    	cmp	x1, x28
1004482ac: 54032f88    	b.hi	0x10044e89c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6908>
1004482b0: f9400514    	ldr	x20, [x8, #0x8]
1004482b4: 8b161298    	add	x24, x20, x22, lsl #4
1004482b8: 39400308    	ldrb	w8, [x24]
1004482bc: 7100391f    	cmp	w8, #0xe
1004482c0: 5402cfc0    	b.eq	0x10044dcb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d24>
1004482c4: 7100251f    	cmp	w8, #0x9
1004482c8: 5402cf88    	b.hi	0x10044dcb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d24>
1004482cc: 3940430a    	ldrb	w10, [x24, #0x10]
1004482d0: 51002949    	sub	w9, w10, #0xa
1004482d4: 7100153f    	cmp	w9, #0x5
1004482d8: 5402cf03    	b.lo	0x10044dcb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d24>
1004482dc: 7100111f    	cmp	w8, #0x4
1004482e0: 54000180    	b.eq	0x100448310 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37c>
1004482e4: 71000d1f    	cmp	w8, #0x3
1004482e8: 5402cee1    	b.ne	0x10044dcc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d30>
1004482ec: b9400708    	ldr	w8, [x24, #0x4]
1004482f0: 71000d5f    	cmp	w10, #0x3
1004482f4: 54000860    	b.eq	0x100448400 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x46c>
1004482f8: 7100115f    	cmp	w10, #0x4
1004482fc: 5402ce41    	b.ne	0x10044dcc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d30>
100448300: 5280000a    	mov	w10, #0x0               ; =0
100448304: f9400f0d    	ldr	x13, [x24, #0x18]
100448308: 5280002b    	mov	w11, #0x1               ; =1
10044830c: 14000040    	b	0x10044840c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x478>
100448310: f9400709    	ldr	x9, [x24, #0x8]
100448314: 71000d5f    	cmp	w10, #0x3
100448318: 540006c0    	b.eq	0x1004483f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x45c>
10044831c: 7100115f    	cmp	w10, #0x4
100448320: 5402cd21    	b.ne	0x10044dcc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d30>
100448324: f9400f0d    	ldr	x13, [x24, #0x18]
100448328: 5280002a    	mov	w10, #0x1               ; =1
10044832c: 5280002b    	mov	w11, #0x1               ; =1
100448330: 14000037    	b	0x10044840c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x478>
100448334: f9408fe2    	ldr	x2, [sp, #0x118]
100448338: f940204c    	ldr	x12, [x2, #0x40]
10044833c: b40330ec    	cbz	x12, 0x10044e958 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x69c4>
100448340: f94087f1    	ldr	x17, [sp, #0x108]
100448344: f9400a3c    	ldr	x28, [x17, #0x10]
100448348: f9407be8    	ldr	x8, [sp, #0xf0]
10044834c: f940010e    	ldr	x14, [x8]
100448350: d1000588    	sub	x8, x12, #0x1
100448354: 8b0801d6    	add	x22, x14, x8
100448358: eb1c02df    	cmp	x22, x28
10044835c: 540346e2    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
100448360: f9400634    	ldr	x20, [x17, #0x8]
100448364: 8b161289    	add	x9, x20, x22, lsl #4
100448368: 3940012a    	ldrb	w10, [x9]
10044836c: 5100294b    	sub	w11, w10, #0xa
100448370: 7100117f    	cmp	w11, #0x4
100448374: 5402cfa9    	b.ls	0x10044dd68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5dd4>
100448378: 7100115f    	cmp	w10, #0x4
10044837c: 54000be0    	b.eq	0x1004484f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x564>
100448380: 71000d5f    	cmp	w10, #0x3
100448384: 5402cf81    	b.ne	0x10044dd74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5de0>
100448388: 5280000a    	mov	w10, #0x0               ; =0
10044838c: b940052b    	ldr	w11, [x9, #0x4]
100448390: 1400005c    	b	0x100448500 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56c>
100448394: 52800028    	mov	w8, #0x1                ; =1
100448398: 6a53711f    	tst	w8, w19, lsr #28
10044839c: 9a880508    	cinc	x8, x8, ne
1004483a0: f94093e9    	ldr	x9, [sp, #0x120]
1004483a4: 2a0903f9    	mov	w25, w9
1004483a8: 8b190116    	add	x22, x8, x25
1004483ac: eb1c02df    	cmp	x22, x28
1004483b0: 54034602    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
1004483b4: 11009f6a    	add	w10, w27, #0x27
1004483b8: 12001d4c    	and	w12, w10, #0xff
1004483bc: b8767b14    	ldr	w20, [x24, x22, lsl #2]
1004483c0: 1100cb68    	add	w8, w27, #0x32
1004483c4: 12001d09    	and	w9, w8, #0xff
1004483c8: 7100313f    	cmp	w9, #0xc
1004483cc: 1a9f97e9    	cset	w9, hi
1004483d0: f9408feb    	ldr	x11, [sp, #0x118]
1004483d4: f940216b    	ldr	x11, [x11, #0x40]
1004483d8: 71000d9f    	cmp	w12, #0x3
1004483dc: 54001722    	b.hs	0x1004486c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x72c>
1004483e0: b100117f    	cmn	x11, #0x4
1004483e4: 5401f788    	b.hi	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
1004483e8: 91000d6b    	add	x11, x11, #0x3
1004483ec: 140000b8    	b	0x1004486cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x738>
1004483f0: 5280000b    	mov	w11, #0x0               ; =0
1004483f4: b940170c    	ldr	w12, [x24, #0x14]
1004483f8: 5280002a    	mov	w10, #0x1               ; =1
1004483fc: 14000004    	b	0x10044840c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x478>
100448400: 5280000a    	mov	w10, #0x0               ; =0
100448404: 5280000b    	mov	w11, #0x0               ; =0
100448408: b940170c    	ldr	w12, [x24, #0x14]
10044840c: 292623aa    	stp	w10, w8, [x29, #-0xd0]
100448410: f81383a9    	stur	x9, [x29, #-0xc8]
100448414: 292833ab    	stp	w11, w12, [x29, #-0xc0]
100448418: f81483ad    	stur	x13, [x29, #-0xb8]
10044841c: d103c3a0    	sub	x0, x29, #0xf0
100448420: d10343a2    	sub	x2, x29, #0xd0
100448424: d10343a8    	sub	x8, x29, #0xd0
100448428: 91004103    	add	x3, x8, #0x10
10044842c: aa1b03e1    	mov	x1, x27
100448430: 94002b0c    	bl	0x100453060 <__ZN13quickjs_oxide6engine2vm7execute20binary_number_result17ha217492dc6eb4815E>
100448434: eb1c02df    	cmp	x22, x28
100448438: 54034102    	b.hs	0x10044ec58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cc4>
10044843c: 3cd103a0    	ldur	q0, [x29, #-0xf0]
100448440: 3d800300    	str	q0, [x24]
100448444: 910006d6    	add	x22, x22, #0x1
100448448: eb1c02df    	cmp	x22, x28
10044844c: 540340c2    	b.hs	0x10044ec64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cd0>
100448450: d37ceec8    	lsl	x8, x22, #4
100448454: 528001c9    	mov	w9, #0xe                ; =14
100448458: 38286a89    	strb	w9, [x20, x8]
10044845c: d1000668    	sub	x8, x19, #0x1
100448460: f9408fe9    	ldr	x9, [sp, #0x118]
100448464: f9002128    	str	x8, [x9, #0x40]
100448468: 140011da    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044846c: f94083e8    	ldr	x8, [sp, #0x100]
100448470: 12003d13    	and	w19, w8, #0xffff
100448474: f94073e8    	ldr	x8, [sp, #0xe0]
100448478: f9400108    	ldr	x8, [x8]
10044847c: f9407be9    	ldr	x9, [sp, #0xf0]
100448480: f9400129    	ldr	x9, [x9]
100448484: eb08012a    	subs	x10, x9, x8
100448488: 9a8a33ea    	csel	x10, xzr, x10, lo
10044848c: eb13015f    	cmp	x10, x19
100448490: 5402cc89    	b.ls	0x10044de20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e8c>
100448494: f94087ef    	ldr	x15, [sp, #0x108]
100448498: f94009fc    	ldr	x28, [x15, #0x10]
10044849c: 8b130116    	add	x22, x8, x19
1004484a0: eb1c02df    	cmp	x22, x28
1004484a4: 54034c42    	b.hs	0x10044ee2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6e98>
1004484a8: f94005e8    	ldr	x8, [x15, #0x8]
1004484ac: d37ceeca    	lsl	x10, x22, #4
1004484b0: 386a690b    	ldrb	w11, [x8, x10]
1004484b4: 7100397f    	cmp	w11, #0xe
1004484b8: f9408fee    	ldr	x14, [sp, #0x118]
1004484bc: f9407ff0    	ldr	x16, [sp, #0xf8]
1004484c0: 5402cf80    	b.eq	0x10044deb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f1c>
1004484c4: 5100af6a    	sub	w10, w27, #0x2b
1004484c8: 5100296c    	sub	w12, w11, #0xa
1004484cc: d100256d    	sub	x13, x11, #0x9
1004484d0: 7100119f    	cmp	w12, #0x4
1004484d4: 9a9f31ac    	csel	x12, x13, xzr, lo
1004484d8: f100099f    	cmp	x12, #0x2
1004484dc: 910963ed    	add	x13, sp, #0x258
1004484e0: 5400dd4c    	b.gt	0x10044a088 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x20f4>
1004484e4: d100058c    	sub	x12, x12, #0x1
1004484e8: f100099f    	cmp	x12, #0x2
1004484ec: 5400eaa2    	b.hs	0x10044a240 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22ac>
1004484f0: 5280008b    	mov	w11, #0x4               ; =4
1004484f4: 14000756    	b	0x10044a24c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22b8>
1004484f8: fd40052a    	ldr	d10, [x9, #0x8]
1004484fc: 5280002a    	mov	w10, #0x1               ; =1
100448500: 5101d76d    	sub	w13, w27, #0x75
100448504: 12001daf    	and	w15, w13, #0xff
100448508: 710005ff    	cmp	w15, #0x1
10044850c: 540000e8    	b.hi	0x100448528 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x594>
100448510: b100059f    	cmn	x12, #0x1
100448514: 5402c300    	b.eq	0x10044dd74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5de0>
100448518: f9401c4f    	ldr	x15, [x2, #0x38]
10044851c: cb0e01ee    	sub	x14, x15, x14
100448520: eb0e019f    	cmp	x12, x14
100448524: 5402c282    	b.hs	0x10044dd74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5de0>
100448528: 7101c77f    	cmp	w27, #0x71
10044852c: 54005c40    	b.eq	0x1004490b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1120>
100448530: 7101cb7f    	cmp	w27, #0x72
100448534: 54005bc0    	b.eq	0x1004490ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1118>
100448538: 1e620160    	scvtf	d0, w11
10044853c: 7101df7f    	cmp	w27, #0x77
100448540: 54005c21    	b.ne	0x1004490c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1130>
100448544: 7100015f    	cmp	w10, #0x0
100448548: 1e601d40    	fcsel	d0, d10, d0, ne
10044854c: 9e66000a    	fmov	x10, d0
100448550: 9240f94a    	and	x10, x10, #0x7fffffffffffffff
100448554: d2effe0b    	mov	x11, #0x7ff0000000000000 ; =9218868437227405312
100448558: eb0b015f    	cmp	x10, x11
10044855c: fa4bd144    	ccmp	x10, x11, #0x4, le
100448560: fa401944    	ccmp	x10, #0x0, #0x4, ne
100448564: 5400fc01    	b.ne	0x10044a4e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2550>
100448568: 5280000a    	mov	w10, #0x0               ; =0
10044856c: 12800013    	mov	w19, #-0x1              ; =-1
100448570: 140007f6    	b	0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
100448574: 7103377f    	cmp	w27, #0xcd
100448578: 1a9f17f3    	cset	w19, eq
10044857c: 71032f7f    	cmp	w27, #0xcb
100448580: 54005e00    	b.eq	0x100449140 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x11ac>
100448584: f9408fe8    	ldr	x8, [sp, #0x118]
100448588: f9402108    	ldr	x8, [x8, #0x40]
10044858c: f94087ea    	ldr	x10, [sp, #0x108]
100448590: f9407fec    	ldr	x12, [sp, #0xf8]
100448594: b4032768    	cbz	x8, 0x10044ea80 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aec>
100448598: f940095c    	ldr	x28, [x10, #0x10]
10044859c: f9407be9    	ldr	x9, [sp, #0xf0]
1004485a0: f9400129    	ldr	x9, [x9]
1004485a4: 8b090109    	add	x9, x8, x9
1004485a8: d1000536    	sub	x22, x9, #0x1
1004485ac: eb1c02df    	cmp	x22, x28
1004485b0: 54033442    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
1004485b4: f9400549    	ldr	x9, [x10, #0x8]
1004485b8: 8b161134    	add	x20, x9, x22, lsl #4
1004485bc: 3940028a    	ldrb	w10, [x20]
1004485c0: 5100294b    	sub	w11, w10, #0xa
1004485c4: 7100117f    	cmp	w11, #0x4
1004485c8: 5402eec9    	b.ls	0x10044e3a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x640c>
1004485cc: 7100155f    	cmp	w10, #0x5
1004485d0: 54010980    	b.eq	0x10044a700 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x276c>
1004485d4: 71000d5f    	cmp	w10, #0x3
1004485d8: 54031e81    	b.ne	0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
1004485dc: b9400694    	ldr	w20, [x20, #0x4]
1004485e0: 36f93bb4    	tbz	w20, #0x1f, 0x10044ad54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2dc0>
1004485e4: 140018f1    	b	0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
1004485e8: f9407fe8    	ldr	x8, [sp, #0xf8]
1004485ec: f9400114    	ldr	x20, [x8]
1004485f0: 7101bf7f    	cmp	w27, #0x6f
1004485f4: 54007720    	b.eq	0x1004494d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1544>
1004485f8: 7101c37f    	cmp	w27, #0x70
1004485fc: f9408fe1    	ldr	x1, [sp, #0x118]
100448600: f94087e0    	ldr	x0, [sp, #0x108]
100448604: 54007781    	b.ne	0x1004494f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1560>
100448608: 52800023    	mov	w3, #0x1                ; =1
10044860c: 140003b6    	b	0x1004494e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1550>
100448610: 71014b7f    	cmp	w27, #0x52
100448614: 52800068    	mov	w8, #0x3                ; =3
100448618: 9a880508    	cinc	x8, x8, ne
10044861c: 7101477f    	cmp	w27, #0x51
100448620: 52800049    	mov	w9, #0x2                ; =2
100448624: 9a880124    	csel	x4, x9, x8, eq
100448628: d1000489    	sub	x9, x4, #0x1
10044862c: f9408fe1    	ldr	x1, [sp, #0x118]
100448630: f9402028    	ldr	x8, [x1, #0x40]
100448634: eb09011f    	cmp	x8, x9
100448638: 54031d69    	b.ls	0x10044e9e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a50>
10044863c: f94087e0    	ldr	x0, [sp, #0x108]
100448640: f940081c    	ldr	x28, [x0, #0x10]
100448644: a94f2be9    	ldp	x9, x10, [sp, #0xf0]
100448648: f9400129    	ldr	x9, [x9]
10044864c: cb040108    	sub	x8, x8, x4
100448650: 8b080136    	add	x22, x9, x8
100448654: eb1c02df    	cmp	x22, x28
100448658: 54032f02    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044865c: f9400408    	ldr	x8, [x0, #0x8]
100448660: d37ceec9    	lsl	x9, x22, #4
100448664: 38696908    	ldrb	w8, [x8, x9]
100448668: 51002908    	sub	w8, w8, #0xa
10044866c: 7100111f    	cmp	w8, #0x4
100448670: 5402cf89    	b.ls	0x10044e060 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60cc>
100448674: f9400142    	ldr	x2, [x10]
100448678: d2800003    	mov	x3, #0x0                ; =0
10044867c: 9400297d    	bl	0x100452c70 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots11insert_copy17hb51f0c3ae63d8d95E>
100448680: 14001153    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100448684: 71015b7f    	cmp	w27, #0x56
100448688: 52800068    	mov	w8, #0x3                ; =3
10044868c: 9a880508    	cinc	x8, x8, ne
100448690: 7101577f    	cmp	w27, #0x55
100448694: 52800049    	mov	w9, #0x2                ; =2
100448698: 9a880125    	csel	x5, x9, x8, eq
10044869c: f94087e8    	ldr	x8, [sp, #0x108]
1004486a0: a9408500    	ldp	x0, x1, [x8, #0x8]
1004486a4: f9408fe8    	ldr	x8, [sp, #0x118]
1004486a8: f9401902    	ldr	x2, [x8, #0x30]
1004486ac: f9402103    	ldr	x3, [x8, #0x40]
1004486b0: 52800024    	mov	w4, #0x1                ; =1
1004486b4: 52800006    	mov	w6, #0x0                ; =0
1004486b8: 94002a17    	bl	0x100452f14 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23rotate_operands_current17ha464970b12b6798cE>
1004486bc: 14001144    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
1004486c0: b1000d7f    	cmn	x11, #0x3
1004486c4: 5401e088    	b.hi	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
1004486c8: 9100096b    	add	x11, x11, #0x2
1004486cc: f9408fed    	ldr	x13, [sp, #0x118]
1004486d0: a94331ad    	ldp	x13, x12, [x13, #0x30]
1004486d4: cb0d018c    	sub	x12, x12, x13
1004486d8: eb0c017f    	cmp	x11, x12
1004486dc: 5401dfc8    	b.hi	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
1004486e0: 7210029f    	tst	w20, #0x10000
1004486e4: f9406beb    	ldr	x11, [sp, #0xd0]
1004486e8: f94073ed    	ldr	x13, [sp, #0xe0]
1004486ec: 9a8b01ab    	csel	x11, x13, x11, eq
1004486f0: f9407bec    	ldr	x12, [sp, #0xf0]
1004486f4: 9a8d018c    	csel	x12, x12, x13, eq
1004486f8: f9400181    	ldr	x1, [x12]
1004486fc: f9400176    	ldr	x22, [x11]
100448700: eb16002c    	subs	x12, x1, x22
100448704: 54031603    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
100448708: f94087eb    	ldr	x11, [sp, #0x108]
10044870c: f940096b    	ldr	x11, [x11, #0x10]
100448710: eb0b003f    	cmp	x1, x11
100448714: 54031c68    	b.hi	0x10044eaa0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b0c>
100448718: 92403e80    	and	x0, x20, #0xffff
10044871c: eb00019f    	cmp	x12, x0
100448720: 5401dda9    	b.ls	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
100448724: f94087ec    	ldr	x12, [sp, #0x108]
100448728: f940058c    	ldr	x12, [x12, #0x8]
10044872c: 8b16118d    	add	x13, x12, x22, lsl #4
100448730: 8b0011ad    	add	x13, x13, x0, lsl #4
100448734: 394001ae    	ldrb	w14, [x13]
100448738: 710025df    	cmp	w14, #0x9
10044873c: 5401dcc8    	b.hi	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
100448740: 71000ddf    	cmp	w14, #0x3
100448744: 5401b020    	b.eq	0x10044bd48 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3db4>
100448748: 710011df    	cmp	w14, #0x4
10044874c: 5401dc41    	b.ne	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
100448750: fd4005a0    	ldr	d0, [x13, #0x8]
100448754: 720f029f    	tst	w20, #0x20000
100448758: 1e7e1001    	fmov	d1, #-1.00000000
10044875c: 1e6e1002    	fmov	d2, #1.00000000
100448760: 1e610c41    	fcsel	d1, d2, d1, eq
100448764: 1e60282a    	fadd	d10, d1, d0
100448768: 5280002e    	mov	w14, #0x1               ; =1
10044876c: 52800031    	mov	w17, #0x1               ; =1
100448770: 14000eae    	b	0x10044c228 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4294>
100448774: 52800028    	mov	w8, #0x1                ; =1
100448778: 6a53711f    	tst	w8, w19, lsr #28
10044877c: 9a88050a    	cinc	x10, x8, ne
100448780: f94093e9    	ldr	x9, [sp, #0x120]
100448784: 2a0903e9    	mov	w9, w9
100448788: 8b090156    	add	x22, x10, x9
10044878c: eb1c02df    	cmp	x22, x28
100448790: 54032702    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100448794: 6a53711f    	tst	w8, w19, lsr #28
100448798: 52800048    	mov	w8, #0x2                ; =2
10044879c: 9a880508    	cinc	x8, x8, ne
1004487a0: 8b090108    	add	x8, x8, x9
1004487a4: eb1c011f    	cmp	x8, x28
1004487a8: 54033582    	b.hs	0x10044ee58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ec4>
1004487ac: b8687b08    	ldr	w8, [x24, x8, lsl #2]
1004487b0: 92402509    	and	x9, x8, #0x3ff
1004487b4: 7103993f    	cmp	w9, #0xe6
1004487b8: 54033668    	b.hi	0x10044ee84 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ef0>
1004487bc: b8767b07    	ldr	w7, [x24, x22, lsl #2]
1004487c0: b0000a0a    	adrp	x10, 0x100589000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16e90>
1004487c4: 9124e14a    	add	x10, x10, #0x938
1004487c8: 78697949    	ldrh	w9, [x10, x9, lsl #1]
1004487cc: 53107cea    	lsr	w10, w7, #16
1004487d0: 71037b7f    	cmp	w27, #0xde
1004487d4: 1a9f07e4    	cset	w4, ne
1004487d8: 530a2906    	ubfx	w6, w8, #10, #1
1004487dc: 530b3108    	ubfx	w8, w8, #11, #2
1004487e0: a94f97eb    	ldp	x11, x5, [sp, #0xf8]
1004487e4: f9400163    	ldr	x3, [x11]
1004487e8: 79000fe9    	strh	w9, [sp, #0x6]
1004487ec: 79000bea    	strh	w10, [sp, #0x4]
1004487f0: 910983e0    	add	x0, sp, #0x260
1004487f4: b90003e8    	str	w8, [sp]
1004487f8: f94087e1    	ldr	x1, [sp, #0x108]
1004487fc: f9408fe2    	ldr	x2, [sp, #0x118]
100448800: 94003674    	bl	0x1004561d0 <__ZN13quickjs_oxide6engine2vm7execute22try_dense_index_binary17h232961ecf1767ddeE>
100448804: b94263e8    	ldr	w8, [sp, #0x260]
100448808: 7100091f    	cmp	w8, #0x2
10044880c: 5401bfe1    	b.ne	0x10044c008 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4074>
100448810: 71037b7f    	cmp	w27, #0xde
100448814: f9408fe1    	ldr	x1, [sp, #0x118]
100448818: f94087e0    	ldr	x0, [sp, #0x108]
10044881c: 5400cc20    	b.eq	0x10044a1a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x220c>
100448820: 52800022    	mov	w2, #0x1                ; =1
100448824: f94083e3    	ldr	x3, [sp, #0x100]
100448828: 94002754    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044882c: f9408fe4    	ldr	x4, [sp, #0x118]
100448830: f94087e3    	ldr	x3, [sp, #0x108]
100448834: 910963eb    	add	x11, sp, #0x258
100448838: b4018e20    	cbz	x0, 0x10044b9fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3a68>
10044883c: 39400008    	ldrb	w8, [x0]
100448840: 71001d1f    	cmp	w8, #0x7
100448844: 54018dc8    	b.hi	0x10044b9fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3a68>
100448848: 52800029    	mov	w9, #0x1                ; =1
10044884c: 1ac82129    	lsl	w9, w9, w8
100448850: 5280138a    	mov	w10, #0x9c              ; =156
100448854: 6a0a013f    	tst	w9, w10
100448858: 54018b80    	b.eq	0x10044b9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3a34>
10044885c: f8401009    	ldur	x9, [x0, #0x1]
100448860: f9017be9    	str	x9, [sp, #0x2f0]
100448864: f9400409    	ldr	x9, [x0, #0x8]
100448868: f809f169    	stur	x9, [x11, #0x9f]
10044886c: 14000c59    	b	0x10044b9d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3a3c>
100448870: f9408fe8    	ldr	x8, [sp, #0x118]
100448874: f9402108    	ldr	x8, [x8, #0x40]
100448878: b100051f    	cmn	x8, #0x1
10044887c: 540009a0    	b.eq	0x1004489b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xa1c>
100448880: f9408fea    	ldr	x10, [sp, #0x118]
100448884: a943254a    	ldp	x10, x9, [x10, #0x30]
100448888: cb0a0129    	sub	x9, x9, x10
10044888c: eb09011f    	cmp	x8, x9
100448890: 54000902    	b.hs	0x1004489b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xa1c>
100448894: 71034b7f    	cmp	w27, #0xd2
100448898: f9406be8    	ldr	x8, [sp, #0xd0]
10044889c: f94073ea    	ldr	x10, [sp, #0xe0]
1004488a0: 9a880148    	csel	x8, x10, x8, eq
1004488a4: f9407be9    	ldr	x9, [sp, #0xf0]
1004488a8: 9a8a0129    	csel	x9, x9, x10, eq
1004488ac: f9400121    	ldr	x1, [x9]
1004488b0: f9400116    	ldr	x22, [x8]
1004488b4: eb160028    	subs	x8, x1, x22
1004488b8: 54030863    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
1004488bc: f94087e9    	ldr	x9, [sp, #0x108]
1004488c0: f9400929    	ldr	x9, [x9, #0x10]
1004488c4: eb09003f    	cmp	x1, x9
1004488c8: 54030e08    	b.hi	0x10044ea88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6af4>
1004488cc: f94083e9    	ldr	x9, [sp, #0x100]
1004488d0: 12003d29    	and	w9, w9, #0xffff
1004488d4: eb09011f    	cmp	x8, x9
1004488d8: 540006c9    	b.ls	0x1004489b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xa1c>
1004488dc: f94087e8    	ldr	x8, [sp, #0x108]
1004488e0: f9400508    	ldr	x8, [x8, #0x8]
1004488e4: 8b161108    	add	x8, x8, x22, lsl #4
1004488e8: 8b091102    	add	x2, x8, x9, lsl #4
1004488ec: 39400048    	ldrb	w8, [x2]
1004488f0: 7100251f    	cmp	w8, #0x9
1004488f4: 540005e1    	b.ne	0x1004489b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xa1c>
1004488f8: 52800048    	mov	w8, #0x2                ; =2
1004488fc: b81403a8    	stur	w8, [x29, #-0xc0]
100448900: 52800028    	mov	w8, #0x1                ; =1
100448904: 6a53711f    	tst	w8, w19, lsr #28
100448908: 9a880508    	cinc	x8, x8, ne
10044890c: f94093e9    	ldr	x9, [sp, #0x120]
100448910: 2a0903f6    	mov	w22, w9
100448914: 8b160100    	add	x0, x8, x22
100448918: eb1c001f    	cmp	x0, x28
10044891c: 54033542    	b.hs	0x10044efc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7030>
100448920: b8607b05    	ldr	w5, [x24, x0, lsl #2]
100448924: f9407fe8    	ldr	x8, [sp, #0xf8]
100448928: f9400101    	ldr	x1, [x8]
10044892c: d103c3a0    	sub	x0, x29, #0xf0
100448930: d10343a7    	sub	x7, x29, #0xd0
100448934: f9408be3    	ldr	x3, [sp, #0x110]
100448938: aa1403e4    	mov	x4, x20
10044893c: 52800026    	mov	w6, #0x1                ; =1
100448940: 94002d94    	bl	0x100453f90 <__ZN13quickjs_oxide6engine6object16ordinary_storage2ic62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$21property_ic_read_fast17h51aad55151f8a4d2E>
100448944: 385103b9    	ldurb	w25, [x29, #-0xf0]
100448948: f94067e9    	ldr	x9, [sp, #0xc8]
10044894c: b9400128    	ldr	w8, [x9]
100448950: b902f3e8    	str	w8, [sp, #0x2f0]
100448954: b8403128    	ldur	w8, [x9, #0x3]
100448958: 910963e9    	add	x9, sp, #0x258
10044895c: b809b128    	stur	w8, [x9, #0x9b]
100448960: f85183b4    	ldur	x20, [x29, #-0xe8]
100448964: b85403a8    	ldur	w8, [x29, #-0xc0]
100448968: 7100091f    	cmp	w8, #0x2
10044896c: 540000e0    	b.eq	0x100448988 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x9f4>
100448970: f85303a0    	ldur	x0, [x29, #-0xd0]
100448974: f9400008    	ldr	x8, [x0]
100448978: f1000508    	subs	x8, x8, #0x1
10044897c: f9000008    	str	x8, [x0]
100448980: 54000041    	b.ne	0x100448988 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x9f4>
100448984: 97eff0e6    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
100448988: 71002f3f    	cmp	w25, #0xb
10044898c: 5402b960    	b.eq	0x10044e0b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6124>
100448990: b942f3e8    	ldr	w8, [sp, #0x2f0]
100448994: b901a3e8    	str	w8, [sp, #0x1a0]
100448998: 910963e8    	add	x8, sp, #0x258
10044899c: b849b108    	ldur	w8, [x8, #0x9b]
1004489a0: 910293e9    	add	x9, sp, #0xa4
1004489a4: b80ff128    	stur	w8, [x9, #0xff]
1004489a8: 71002b3f    	cmp	w25, #0xa
1004489ac: 540234c1    	b.ne	0x10044d044 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x50b0>
1004489b0: 71034b7f    	cmp	w27, #0xd2
1004489b4: 5400b361    	b.ne	0x10044a020 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x208c>
1004489b8: a95003e3    	ldp	x3, x0, [sp, #0x100]
1004489bc: f9408fe1    	ldr	x1, [sp, #0x118]
1004489c0: 52800002    	mov	w2, #0x0                ; =0
1004489c4: 940026ed    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004489c8: f9408fe4    	ldr	x4, [sp, #0x118]
1004489cc: f94087e3    	ldr	x3, [sp, #0x108]
1004489d0: 910963eb    	add	x11, sp, #0x258
1004489d4: b4016e00    	cbz	x0, 0x10044b794 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3800>
1004489d8: 39400008    	ldrb	w8, [x0]
1004489dc: 71001d1f    	cmp	w8, #0x7
1004489e0: 54016da8    	b.hi	0x10044b794 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3800>
1004489e4: 52800029    	mov	w9, #0x1                ; =1
1004489e8: 1ac82129    	lsl	w9, w9, w8
1004489ec: 5280138a    	mov	w10, #0x9c              ; =156
1004489f0: 6a0a013f    	tst	w9, w10
1004489f4: 54016b60    	b.eq	0x10044b760 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37cc>
1004489f8: f8401009    	ldur	x9, [x0, #0x1]
1004489fc: f9017be9    	str	x9, [sp, #0x2f0]
100448a00: f9400409    	ldr	x9, [x0, #0x8]
100448a04: f809f169    	stur	x9, [x11, #0x9f]
100448a08: 14000b58    	b	0x10044b768 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37d4>
100448a0c: 52800028    	mov	w8, #0x1                ; =1
100448a10: 6a53711f    	tst	w8, w19, lsr #28
100448a14: 9a880509    	cinc	x9, x8, ne
100448a18: f94093e8    	ldr	x8, [sp, #0x120]
100448a1c: 2a0803e8    	mov	w8, w8
100448a20: 8b080136    	add	x22, x9, x8
100448a24: eb1c02df    	cmp	x22, x28
100448a28: 54031242    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100448a2c: f9408fe9    	ldr	x9, [sp, #0x118]
100448a30: f9402129    	ldr	x9, [x9, #0x40]
100448a34: b1000d3f    	cmn	x9, #0x3
100448a38: 5401e4c8    	b.hi	0x10044c6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x473c>
100448a3c: 91000929    	add	x9, x9, #0x2
100448a40: f9408feb    	ldr	x11, [sp, #0x118]
100448a44: a943296b    	ldp	x11, x10, [x11, #0x30]
100448a48: cb0b014a    	sub	x10, x10, x11
100448a4c: eb0a013f    	cmp	x9, x10
100448a50: 5401e408    	b.hi	0x10044c6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x473c>
100448a54: 7103537f    	cmp	w27, #0xd4
100448a58: f9406be9    	ldr	x9, [sp, #0xd0]
100448a5c: f94073eb    	ldr	x11, [sp, #0xe0]
100448a60: 9a890169    	csel	x9, x11, x9, eq
100448a64: f9407bea    	ldr	x10, [sp, #0xf0]
100448a68: 9a8b014a    	csel	x10, x10, x11, eq
100448a6c: f9400141    	ldr	x1, [x10]
100448a70: f9400129    	ldr	x9, [x9]
100448a74: eb09002c    	subs	x12, x1, x9
100448a78: 540307c3    	b.lo	0x10044eb70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6bdc>
100448a7c: f94087eb    	ldr	x11, [sp, #0x108]
100448a80: f940096a    	ldr	x10, [x11, #0x10]
100448a84: eb0a003f    	cmp	x1, x10
100448a88: 540301a8    	b.hi	0x10044eabc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b28>
100448a8c: f94083ed    	ldr	x13, [sp, #0x100]
100448a90: 12003dad    	and	w13, w13, #0xffff
100448a94: f940056b    	ldr	x11, [x11, #0x8]
100448a98: 2f00e400    	movi	d0, #0000000000000000
100448a9c: eb0d019f    	cmp	x12, x13
100448aa0: 5400c969    	b.ls	0x10044a3cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2438>
100448aa4: 8b091169    	add	x9, x11, x9, lsl #4
100448aa8: 8b0d1129    	add	x9, x9, x13, lsl #4
100448aac: 3940012c    	ldrb	w12, [x9]
100448ab0: 7100259f    	cmp	w12, #0x9
100448ab4: 5400c8c8    	b.hi	0x10044a3cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2438>
100448ab8: 71000d9f    	cmp	w12, #0x3
100448abc: 5401df40    	b.eq	0x10044c6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4710>
100448ac0: 7100119f    	cmp	w12, #0x4
100448ac4: 5400c841    	b.ne	0x10044a3cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2438>
100448ac8: 5280000c    	mov	w12, #0x0               ; =0
100448acc: fd400520    	ldr	d0, [x9, #0x8]
100448ad0: 14000640    	b	0x10044a3d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x243c>
100448ad4: f9408fec    	ldr	x12, [sp, #0x118]
100448ad8: f9402188    	ldr	x8, [x12, #0x40]
100448adc: b402fa88    	cbz	x8, 0x10044ea2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a98>
100448ae0: f94087eb    	ldr	x11, [sp, #0x108]
100448ae4: f940097c    	ldr	x28, [x11, #0x10]
100448ae8: f9407be9    	ldr	x9, [sp, #0xf0]
100448aec: f940012a    	ldr	x10, [x9]
100448af0: d1000509    	sub	x9, x8, #0x1
100448af4: 8b0a0136    	add	x22, x9, x10
100448af8: eb1c02df    	cmp	x22, x28
100448afc: 540309e2    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
100448b00: f9400568    	ldr	x8, [x11, #0x8]
100448b04: 8b16110a    	add	x10, x8, x22, lsl #4
100448b08: 39400148    	ldrb	w8, [x10]
100448b0c: 5100290b    	sub	w11, w8, #0xa
100448b10: 7100117f    	cmp	w11, #0x4
100448b14: 5402b389    	b.ls	0x10044e184 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61f0>
100448b18: 7100150b    	subs	w11, w8, #0x5
100448b1c: 7a472904    	ccmp	w8, #0x7, #0x4, hs
100448b20: 5402e9e1    	b.ne	0x10044e85c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
100448b24: f9002189    	str	x9, [x12, #0x40]
100448b28: 3940054c    	ldrb	w12, [x10, #0x1]
100448b2c: b940054d    	ldr	w13, [x10, #0x4]
100448b30: f9400549    	ldr	x9, [x10, #0x8]
100448b34: 528001ce    	mov	w14, #0xe               ; =14
100448b38: 3900014e    	strb	w14, [x10]
100448b3c: 71000d1f    	cmp	w8, #0x3
100448b40: 5400a9ac    	b.gt	0x10044a074 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x20e0>
100448b44: 7100091f    	cmp	w8, #0x2
100448b48: 5400f162    	b.hs	0x10044a974 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x29e0>
100448b4c: 71025b7f    	cmp	w27, #0x96
100448b50: 54020400    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
100448b54: 140009d1    	b	0x10044b298 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3304>
100448b58: 52800028    	mov	w8, #0x1                ; =1
100448b5c: 6a53711f    	tst	w8, w19, lsr #28
100448b60: 9a880508    	cinc	x8, x8, ne
100448b64: f94093e9    	ldr	x9, [sp, #0x120]
100448b68: 2a0903f4    	mov	w20, w9
100448b6c: 8b140116    	add	x22, x8, x20
100448b70: eb1c02df    	cmp	x22, x28
100448b74: 540307e2    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100448b78: f9408fe8    	ldr	x8, [sp, #0x118]
100448b7c: f9402108    	ldr	x8, [x8, #0x40]
100448b80: b1000d1f    	cmn	x8, #0x3
100448b84: 54000828    	b.hi	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448b88: 91000908    	add	x8, x8, #0x2
100448b8c: f9408fea    	ldr	x10, [sp, #0x118]
100448b90: a943254a    	ldp	x10, x9, [x10, #0x30]
100448b94: cb0a0129    	sub	x9, x9, x10
100448b98: eb09011f    	cmp	x8, x9
100448b9c: 54000768    	b.hi	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448ba0: b8767b09    	ldr	w9, [x24, x22, lsl #2]
100448ba4: 7210013f    	tst	w9, #0x10000
100448ba8: f9406be8    	ldr	x8, [sp, #0xd0]
100448bac: f94073eb    	ldr	x11, [sp, #0xe0]
100448bb0: 9a880168    	csel	x8, x11, x8, eq
100448bb4: f9407bea    	ldr	x10, [sp, #0xf0]
100448bb8: 9a8b014a    	csel	x10, x10, x11, eq
100448bbc: f9400141    	ldr	x1, [x10]
100448bc0: f9400116    	ldr	x22, [x8]
100448bc4: eb16002a    	subs	x10, x1, x22
100448bc8: 5402efe3    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
100448bcc: f94087e8    	ldr	x8, [sp, #0x108]
100448bd0: f9400908    	ldr	x8, [x8, #0x10]
100448bd4: eb08003f    	cmp	x1, x8
100448bd8: 5402fbc8    	b.hi	0x10044eb50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6bbc>
100448bdc: 92403d2b    	and	x11, x9, #0xffff
100448be0: eb0b015f    	cmp	x10, x11
100448be4: 54000529    	b.ls	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448be8: f94087e9    	ldr	x9, [sp, #0x108]
100448bec: f9400529    	ldr	x9, [x9, #0x8]
100448bf0: 8b16112a    	add	x10, x9, x22, lsl #4
100448bf4: 8b0b114a    	add	x10, x10, x11, lsl #4
100448bf8: 3940014b    	ldrb	w11, [x10]
100448bfc: 71000d7f    	cmp	w11, #0x3
100448c00: 54000441    	b.ne	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448c04: b9400543    	ldr	w3, [x10, #0x4]
100448c08: 37f80403    	tbnz	w3, #0x1f, 0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448c0c: 7103437f    	cmp	w27, #0xd0
100448c10: f9406bea    	ldr	x10, [sp, #0xd0]
100448c14: f94073ec    	ldr	x12, [sp, #0xe0]
100448c18: 9a8a018a    	csel	x10, x12, x10, eq
100448c1c: f9407beb    	ldr	x11, [sp, #0xf0]
100448c20: 9a8c016b    	csel	x11, x11, x12, eq
100448c24: f9400161    	ldr	x1, [x11]
100448c28: f9400156    	ldr	x22, [x10]
100448c2c: eb16002a    	subs	x10, x1, x22
100448c30: 5402eca3    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
100448c34: eb08003f    	cmp	x1, x8
100448c38: 5402f8c8    	b.hi	0x10044eb50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6bbc>
100448c3c: f94083e8    	ldr	x8, [sp, #0x100]
100448c40: 12003d08    	and	w8, w8, #0xffff
100448c44: eb08015f    	cmp	x10, x8
100448c48: 54000209    	b.ls	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448c4c: 8b161129    	add	x9, x9, x22, lsl #4
100448c50: 8b081122    	add	x2, x9, x8, lsl #4
100448c54: 39400048    	ldrb	w8, [x2]
100448c58: 7100251f    	cmp	w8, #0x9
100448c5c: 54000168    	b.hi	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448c60: f9407fe8    	ldr	x8, [sp, #0xf8]
100448c64: f9400101    	ldr	x1, [x8]
100448c68: d10343a0    	sub	x0, x29, #0xd0
100448c6c: 94003304    	bl	0x10045587c <__ZN13quickjs_oxide6engine6object16ordinary_storage62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$24peek_dense_number_result17hacaa150a333ea219E>
100448c70: b85303a8    	ldur	w8, [x29, #-0xd0]
100448c74: 7100051f    	cmp	w8, #0x1
100448c78: 54000080    	b.eq	0x100448c88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xcf4>
100448c7c: b85383a8    	ldur	w8, [x29, #-0xc8]
100448c80: 7100091f    	cmp	w8, #0x2
100448c84: 54025a61    	b.ne	0x10044d7d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
100448c88: 7103437f    	cmp	w27, #0xd0
100448c8c: 54009761    	b.ne	0x100449f78 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1fe4>
100448c90: a95003e3    	ldp	x3, x0, [sp, #0x100]
100448c94: f9408fe1    	ldr	x1, [sp, #0x118]
100448c98: 52800002    	mov	w2, #0x0                ; =0
100448c9c: 94002637    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100448ca0: f9408fe4    	ldr	x4, [sp, #0x118]
100448ca4: f94087e3    	ldr	x3, [sp, #0x108]
100448ca8: 910963eb    	add	x11, sp, #0x258
100448cac: b40144e0    	cbz	x0, 0x10044b548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x35b4>
100448cb0: 39400008    	ldrb	w8, [x0]
100448cb4: 71001d1f    	cmp	w8, #0x7
100448cb8: 54014488    	b.hi	0x10044b548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x35b4>
100448cbc: 52800029    	mov	w9, #0x1                ; =1
100448cc0: 1ac82129    	lsl	w9, w9, w8
100448cc4: 5280138a    	mov	w10, #0x9c              ; =156
100448cc8: 6a0a013f    	tst	w9, w10
100448ccc: 54014240    	b.eq	0x10044b514 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3580>
100448cd0: f8401009    	ldur	x9, [x0, #0x1]
100448cd4: f9017be9    	str	x9, [sp, #0x2f0]
100448cd8: f9400409    	ldr	x9, [x0, #0x8]
100448cdc: f809f169    	stur	x9, [x11, #0x9f]
100448ce0: 14000a0f    	b	0x10044b51c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3588>
100448ce4: 52800048    	mov	w8, #0x2                ; =2
100448ce8: b81403a8    	stur	w8, [x29, #-0xc0]
100448cec: f94087e8    	ldr	x8, [sp, #0x108]
100448cf0: a940e51c    	ldp	x28, x25, [x8, #0x8]
100448cf4: 71032b7f    	cmp	w27, #0xca
100448cf8: 1a9f17f3    	cset	w19, eq
100448cfc: f9408ff6    	ldr	x22, [sp, #0x118]
100448d00: 5400f221    	b.ne	0x10044ab44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2bb0>
100448d04: aa1c03e0    	mov	x0, x28
100448d08: aa1903e1    	mov	x1, x25
100448d0c: aa1603e2    	mov	x2, x22
100448d10: 97f4d487    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
100448d14: aa0103f4    	mov	x20, x1
100448d18: 3702bf80    	tbnz	w0, #0x0, 0x10044e508 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6574>
100448d1c: b400f120    	cbz	x0, 0x10044ab40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2bac>
100448d20: f9401688    	ldr	x8, [x20, #0x28]
100448d24: f9408ff6    	ldr	x22, [sp, #0x118]
100448d28: b4000068    	cbz	x8, 0x100448d34 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xda0>
100448d2c: f9401a80    	ldr	x0, [x20, #0x30]
100448d30: 94033b65    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
100448d34: f9402280    	ldr	x0, [x20, #0x40]
100448d38: b4000040    	cbz	x0, 0x100448d40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xdac>
100448d3c: 94033b62    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
100448d40: aa1403e0    	mov	x0, x20
100448d44: 94033b60    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
100448d48: 1400077f    	b	0x10044ab44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2bb0>
100448d4c: 7101b77f    	cmp	w27, #0x6d
100448d50: 1a9f17e9    	cset	w9, eq
100448d54: f9408fe8    	ldr	x8, [sp, #0x118]
100448d58: f9402108    	ldr	x8, [x8, #0x40]
100448d5c: eb09011f    	cmp	x8, x9
100448d60: 5402e609    	b.ls	0x10044ea20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a8c>
100448d64: f94087ec    	ldr	x12, [sp, #0x108]
100448d68: f940099c    	ldr	x28, [x12, #0x10]
100448d6c: f9407bea    	ldr	x10, [sp, #0xf0]
100448d70: f940014a    	ldr	x10, [x10]
100448d74: aa2903e9    	mvn	x9, x9
100448d78: 8b0a010b    	add	x11, x8, x10
100448d7c: 8b090176    	add	x22, x11, x9
100448d80: eb1c02df    	cmp	x22, x28
100448d84: 5402f5a2    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
100448d88: f9400589    	ldr	x9, [x12, #0x8]
100448d8c: d37ceecb    	lsl	x11, x22, #4
100448d90: 386b692b    	ldrb	w11, [x9, x11]
100448d94: 5100296c    	sub	w12, w11, #0xa
100448d98: 7100119f    	cmp	w12, #0x4
100448d9c: 54029ee9    	b.ls	0x10044e178 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61e4>
100448da0: 7100217f    	cmp	w11, #0x8
100448da4: 54002822    	b.hs	0x1004492a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1314>
100448da8: 528013ec    	mov	w12, #0x9f              ; =159
100448dac: 1acb258b    	lsr	w11, w12, w11
100448db0: 360027cb    	tbz	w11, #0x0, 0x1004492a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1314>
100448db4: 5280002c    	mov	w12, #0x1               ; =1
100448db8: 14000140    	b	0x1004492b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1324>
100448dbc: f94083e8    	ldr	x8, [sp, #0x100]
100448dc0: 12003d13    	and	w19, w8, #0xffff
100448dc4: f9406be8    	ldr	x8, [sp, #0xd0]
100448dc8: f9400108    	ldr	x8, [x8]
100448dcc: f94073e9    	ldr	x9, [sp, #0xe0]
100448dd0: f9400129    	ldr	x9, [x9]
100448dd4: eb080129    	subs	x9, x9, x8
100448dd8: 9a8933e9    	csel	x9, xzr, x9, lo
100448ddc: eb13013f    	cmp	x9, x19
100448de0: 54029969    	b.ls	0x10044e10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6178>
100448de4: f94087ec    	ldr	x12, [sp, #0x108]
100448de8: f940099c    	ldr	x28, [x12, #0x10]
100448dec: 8b130116    	add	x22, x8, x19
100448df0: eb1c02df    	cmp	x22, x28
100448df4: f9408fe2    	ldr	x2, [sp, #0x118]
100448df8: f9407fed    	ldr	x13, [sp, #0xf8]
100448dfc: 54030382    	b.hs	0x10044ee6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ed8>
100448e00: f9400580    	ldr	x0, [x12, #0x8]
100448e04: d37ceec8    	lsl	x8, x22, #4
100448e08: 38686808    	ldrb	w8, [x0, x8]
100448e0c: 7100391f    	cmp	w8, #0xe
100448e10: 54029f20    	b.eq	0x10044e1f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6260>
100448e14: 51002909    	sub	w9, w8, #0xa
100448e18: d100250a    	sub	x10, x8, #0x9
100448e1c: 7100113f    	cmp	w9, #0x4
100448e20: 9a9f3149    	csel	x9, x10, xzr, lo
100448e24: d100052a    	sub	x10, x9, #0x1
100448e28: f1000d5f    	cmp	x10, #0x3
100448e2c: 910963ee    	add	x14, sp, #0x258
100448e30: 5400dbc2    	b.hs	0x10044a9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2a14>
100448e34: f94097e8    	ldr	x8, [sp, #0x128]
100448e38: f9401108    	ldr	x8, [x8, #0x20]
100448e3c: b4027768    	cbz	x8, 0x10044dd28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d94>
100448e40: 7100bf7f    	cmp	w27, #0x2f
100448e44: 5400a941    	b.ne	0x10044a36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x23d8>
100448e48: f9402048    	ldr	x8, [x2, #0x40]
100448e4c: b402e468    	cbz	x8, 0x10044ead8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b44>
100448e50: f940099c    	ldr	x28, [x12, #0x10]
100448e54: f9407be9    	ldr	x9, [sp, #0xf0]
100448e58: f9400129    	ldr	x9, [x9]
100448e5c: 8b090108    	add	x8, x8, x9
100448e60: d1000516    	sub	x22, x8, #0x1
100448e64: eb1c02df    	cmp	x22, x28
100448e68: 5402ee82    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
100448e6c: f9400588    	ldr	x8, [x12, #0x8]
100448e70: 8b161102    	add	x2, x8, x22, lsl #4
100448e74: 39400048    	ldrb	w8, [x2]
100448e78: 51002909    	sub	w9, w8, #0xa
100448e7c: 7100113f    	cmp	w9, #0x4
100448e80: 5402bfa9    	b.ls	0x10044e674 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e0>
100448e84: 71001d1f    	cmp	w8, #0x7
100448e88: 54018a48    	b.hi	0x10044bfd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x403c>
100448e8c: 52800029    	mov	w9, #0x1                ; =1
100448e90: 1ac82129    	lsl	w9, w9, w8
100448e94: 5280138a    	mov	w10, #0x9c              ; =156
100448e98: 6a0a013f    	tst	w9, w10
100448e9c: 54010da0    	b.eq	0x10044b050 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30bc>
100448ea0: f8401049    	ldur	x9, [x2, #0x1]
100448ea4: f81103a9    	stur	x9, [x29, #-0xf0]
100448ea8: f9400449    	ldr	x9, [x2, #0x8]
100448eac: f80ff1c9    	stur	x9, [x14, #0xff]
100448eb0: 1400086a    	b	0x10044b058 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30c4>
100448eb4: 52800028    	mov	w8, #0x1                ; =1
100448eb8: 6a53711f    	tst	w8, w19, lsr #28
100448ebc: 9a88050a    	cinc	x10, x8, ne
100448ec0: f94093e9    	ldr	x9, [sp, #0x120]
100448ec4: 2a0903e9    	mov	w9, w9
100448ec8: 8b090156    	add	x22, x10, x9
100448ecc: eb1c02df    	cmp	x22, x28
100448ed0: 5402ed02    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100448ed4: 6a53711f    	tst	w8, w19, lsr #28
100448ed8: 52800048    	mov	w8, #0x2                ; =2
100448edc: 9a880508    	cinc	x8, x8, ne
100448ee0: 8b090108    	add	x8, x8, x9
100448ee4: eb1c011f    	cmp	x8, x28
100448ee8: 5402fb82    	b.hs	0x10044ee58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ec4>
100448eec: b8687b08    	ldr	w8, [x24, x8, lsl #2]
100448ef0: 92402509    	and	x9, x8, #0x3ff
100448ef4: 7103993f    	cmp	w9, #0xe6
100448ef8: 5402fc08    	b.hi	0x10044ee78 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ee4>
100448efc: b8767b07    	ldr	w7, [x24, x22, lsl #2]
100448f00: b0000a0a    	adrp	x10, 0x100589000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16e90>
100448f04: 9124e14a    	add	x10, x10, #0x938
100448f08: 78697949    	ldrh	w9, [x10, x9, lsl #1]
100448f0c: 53107cea    	lsr	w10, w7, #16
100448f10: 7103737f    	cmp	w27, #0xdc
100448f14: 1a9f07e4    	cset	w4, ne
100448f18: 530a2906    	ubfx	w6, w8, #10, #1
100448f1c: 530b3108    	ubfx	w8, w8, #11, #2
100448f20: a94f97eb    	ldp	x11, x5, [sp, #0xf8]
100448f24: f9400163    	ldr	x3, [x11]
100448f28: 79000fe9    	strh	w9, [sp, #0x6]
100448f2c: 79000bea    	strh	w10, [sp, #0x4]
100448f30: 9109c3e0    	add	x0, sp, #0x270
100448f34: b90003e8    	str	w8, [sp]
100448f38: f94087e1    	ldr	x1, [sp, #0x108]
100448f3c: f9408fe2    	ldr	x2, [sp, #0x118]
100448f40: 940033f6    	bl	0x100455f18 <__ZN13quickjs_oxide6engine2vm7execute21try_dense_read_binary17h5dcc40f912362179E>
100448f44: 3949c3e9    	ldrb	w9, [sp, #0x270]
100448f48: 7100293f    	cmp	w9, #0xa
100448f4c: 540186c1    	b.ne	0x10044c024 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4090>
100448f50: 7103737f    	cmp	w27, #0xdc
100448f54: f9408fe1    	ldr	x1, [sp, #0x118]
100448f58: f94087e0    	ldr	x0, [sp, #0x108]
100448f5c: 540094a0    	b.eq	0x10044a1f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x225c>
100448f60: 52800022    	mov	w2, #0x1                ; =1
100448f64: f94083e3    	ldr	x3, [sp, #0x100]
100448f68: 94002584    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100448f6c: f9408fe4    	ldr	x4, [sp, #0x118]
100448f70: f94087e3    	ldr	x3, [sp, #0x108]
100448f74: 910963eb    	add	x11, sp, #0x258
100448f78: b4015dc0    	cbz	x0, 0x10044bb30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b9c>
100448f7c: 39400008    	ldrb	w8, [x0]
100448f80: 71001d1f    	cmp	w8, #0x7
100448f84: 54015d68    	b.hi	0x10044bb30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b9c>
100448f88: 52800029    	mov	w9, #0x1                ; =1
100448f8c: 1ac82129    	lsl	w9, w9, w8
100448f90: 5280138a    	mov	w10, #0x9c              ; =156
100448f94: 6a0a013f    	tst	w9, w10
100448f98: 54015b20    	b.eq	0x10044bafc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b68>
100448f9c: f8401009    	ldur	x9, [x0, #0x1]
100448fa0: f9017be9    	str	x9, [sp, #0x2f0]
100448fa4: f9400409    	ldr	x9, [x0, #0x8]
100448fa8: f809f169    	stur	x9, [x11, #0x9f]
100448fac: 14000ad6    	b	0x10044bb04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b70>
100448fb0: f94083e9    	ldr	x9, [sp, #0x100]
100448fb4: 12003133    	and	w19, w9, #0x1fff
100448fb8: f9408fea    	ldr	x10, [sp, #0x118]
100448fbc: f9402148    	ldr	x8, [x10, #0x40]
100448fc0: 37701a89    	tbnz	w9, #0xe, 0x100449310 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x137c>
100448fc4: b100051f    	cmn	x8, #0x1
100448fc8: f94087e0    	ldr	x0, [sp, #0x108]
100448fcc: 54001d80    	b.eq	0x10044937c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e8>
100448fd0: a9432541    	ldp	x1, x9, [x10, #0x30]
100448fd4: cb010129    	sub	x9, x9, x1
100448fd8: eb09011f    	cmp	x8, x9
100448fdc: 54001aa3    	b.lo	0x100449330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x139c>
100448fe0: 140000e7    	b	0x10044937c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e8>
100448fe4: 52800028    	mov	w8, #0x1                ; =1
100448fe8: 6a53711f    	tst	w8, w19, lsr #28
100448fec: 9a880509    	cinc	x9, x8, ne
100448ff0: f94093e8    	ldr	x8, [sp, #0x120]
100448ff4: 2a0803e8    	mov	w8, w8
100448ff8: 8b080136    	add	x22, x9, x8
100448ffc: eb1c02df    	cmp	x22, x28
100449000: 5402e382    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100449004: f9408fe9    	ldr	x9, [sp, #0x118]
100449008: f9402129    	ldr	x9, [x9, #0x40]
10044900c: b1000d3f    	cmn	x9, #0x3
100449010: 5401c548    	b.hi	0x10044c8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4924>
100449014: 91000929    	add	x9, x9, #0x2
100449018: f9408feb    	ldr	x11, [sp, #0x118]
10044901c: a943296b    	ldp	x11, x10, [x11, #0x30]
100449020: cb0b014a    	sub	x10, x10, x11
100449024: eb0a013f    	cmp	x9, x10
100449028: 5401c488    	b.hi	0x10044c8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4924>
10044902c: 71038b7f    	cmp	w27, #0xe2
100449030: f9406be9    	ldr	x9, [sp, #0xd0]
100449034: f94073eb    	ldr	x11, [sp, #0xe0]
100449038: 9a890169    	csel	x9, x11, x9, eq
10044903c: f9407bea    	ldr	x10, [sp, #0xf0]
100449040: 9a8b014a    	csel	x10, x10, x11, eq
100449044: f9400141    	ldr	x1, [x10]
100449048: f940012b    	ldr	x11, [x9]
10044904c: eb0b002c    	subs	x12, x1, x11
100449050: 5402d883    	b.lo	0x10044eb60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6bcc>
100449054: f94087ea    	ldr	x10, [sp, #0x108]
100449058: f9400949    	ldr	x9, [x10, #0x10]
10044905c: eb09003f    	cmp	x1, x9
100449060: 5402d148    	b.hi	0x10044ea88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6af4>
100449064: f94083ed    	ldr	x13, [sp, #0x100]
100449068: 12003dad    	and	w13, w13, #0xffff
10044906c: f940054a    	ldr	x10, [x10, #0x8]
100449070: 2f00e400    	movi	d0, #0000000000000000
100449074: eb0d019f    	cmp	x12, x13
100449078: 54009e49    	b.ls	0x10044a440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24ac>
10044907c: 8b0b114b    	add	x11, x10, x11, lsl #4
100449080: 8b0d116c    	add	x12, x11, x13, lsl #4
100449084: 3940018b    	ldrb	w11, [x12]
100449088: 7100257f    	cmp	w11, #0x9
10044908c: 54009da8    	b.hi	0x10044a440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24ac>
100449090: 71000d7f    	cmp	w11, #0x3
100449094: 5401b120    	b.eq	0x10044c6b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4724>
100449098: 7100117f    	cmp	w11, #0x4
10044909c: 54009d21    	b.ne	0x10044a440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24ac>
1004490a0: 5280000b    	mov	w11, #0x0               ; =0
1004490a4: fd400580    	ldr	d0, [x12, #0x8]
1004490a8: 140004e7    	b	0x10044a444 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24b0>
1004490ac: aa0b03f3    	mov	x19, x11
1004490b0: 14000526    	b	0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
1004490b4: 3600a00a    	tbz	w10, #0x0, 0x10044a4b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2520>
1004490b8: 1e61414a    	fneg	d10, d10
1004490bc: 5280002a    	mov	w10, #0x1               ; =1
1004490c0: 14000522    	b	0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
1004490c4: 5101cf6c    	sub	w12, w27, #0x73
1004490c8: 121e798c    	and	w12, w12, #0xfffffffd
1004490cc: 7100056e    	subs	w14, w11, #0x1
1004490d0: 1a9f77ef    	cset	w15, vs
1004490d4: 3100056b    	adds	w11, w11, #0x1
1004490d8: 1a9f77f0    	cset	w16, vs
1004490dc: 72001d9f    	tst	w12, #0xff
1004490e0: 1a8f020f    	csel	w15, w16, w15, eq
1004490e4: 1e7e1001    	fmov	d1, #-1.00000000
1004490e8: 1e6e1002    	fmov	d2, #1.00000000
1004490ec: 1e610c43    	fcsel	d3, d2, d1, eq
1004490f0: 1e602860    	fadd	d0, d3, d0
1004490f4: 52800030    	mov	w16, #0x1               ; =1
1004490f8: 72001d9f    	tst	w12, #0xff
1004490fc: 1a8e016b    	csel	w11, w11, w14, eq
100449100: 720001ff    	tst	w15, #0x1
100449104: 1e601c00    	fcsel	d0, d0, d0, ne
100449108: 1a8b110b    	csel	w11, w8, w11, ne
10044910c: 1a9f120e    	csel	w14, w16, wzr, ne
100449110: 72001d9f    	tst	w12, #0xff
100449114: 1e610c41    	fcsel	d1, d2, d1, eq
100449118: 1e6a2821    	fadd	d1, d1, d10
10044911c: 5280002c    	mov	w12, #0x1               ; =1
100449120: 7100015f    	cmp	w10, #0x0
100449124: 1a880173    	csel	w19, w11, w8, eq
100449128: 1e610c0a    	fcsel	d10, d0, d1, eq
10044912c: 1a8c01ca    	csel	w10, w14, w12, eq
100449130: 12001dab    	and	w11, w13, #0xff
100449134: 7100057f    	cmp	w11, #0x1
100449138: 5400a088    	b.hi	0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
10044913c: 14000507    	b	0x10044a558 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25c4>
100449140: f9408fe8    	ldr	x8, [sp, #0x118]
100449144: f9402108    	ldr	x8, [x8, #0x40]
100449148: f100050b    	subs	x11, x8, #0x1
10044914c: f94087ea    	ldr	x10, [sp, #0x108]
100449150: 5402cca9    	b.ls	0x10044eae4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b50>
100449154: f9407be9    	ldr	x9, [sp, #0xf0]
100449158: f9400129    	ldr	x9, [x9]
10044915c: 8b080121    	add	x1, x9, x8
100449160: d1000836    	sub	x22, x1, #0x2
100449164: eb16003f    	cmp	x1, x22
100449168: 5402cc63    	b.lo	0x10044eaf4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b60>
10044916c: f9001beb    	str	x11, [sp, #0x30]
100449170: f940095c    	ldr	x28, [x10, #0x10]
100449174: eb1c003f    	cmp	x1, x28
100449178: 5402cc48    	b.hi	0x10044eb00 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b6c>
10044917c: f9400548    	ldr	x8, [x10, #0x8]
100449180: f90017e8    	str	x8, [sp, #0x28]
100449184: 8b16110a    	add	x10, x8, x22, lsl #4
100449188: 39400148    	ldrb	w8, [x10]
10044918c: 7100391f    	cmp	w8, #0xe
100449190: 54026440    	b.eq	0x10044de18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e84>
100449194: 7100251f    	cmp	w8, #0x9
100449198: 54026408    	b.hi	0x10044de18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e84>
10044919c: 38410149    	ldurb	w9, [x10, #0x10]
1004491a0: 71000d3f    	cmp	w9, #0x3
1004491a4: f90023ea    	str	x10, [sp, #0x40]
1004491a8: 5400ab40    	b.eq	0x10044a710 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x277c>
1004491ac: 7100153f    	cmp	w9, #0x5
1004491b0: 540262e1    	b.ne	0x10044de0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e78>
1004491b4: f9407fe8    	ldr	x8, [sp, #0xf8]
1004491b8: f9400101    	ldr	x1, [x8]
1004491bc: d10343a0    	sub	x0, x29, #0xd0
1004491c0: 91004142    	add	x2, x10, #0x10
1004491c4: f90083e1    	str	x1, [sp, #0x100]
1004491c8: 94002ffe    	bl	0x1004551c0 <__ZN13quickjs_oxide6engine4heap14slot_ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$36slot_value_release_readiness_jsvalue17h2038e983b6e2b9bfE>
1004491cc: 385303a8    	ldurb	w8, [x29, #-0xd0]
1004491d0: 71002d1f    	cmp	w8, #0xb
1004491d4: 54027761    	b.ne	0x10044e0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
1004491d8: 385313a9    	ldurb	w9, [x29, #-0xcf]
1004491dc: 35027729    	cbnz	w9, 0x10044e0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
1004491e0: f94083ea    	ldr	x10, [sp, #0x100]
1004491e4: f9401548    	ldr	x8, [x10, #0x28]
1004491e8: b27ff7e9    	mov	x9, #0x7ffffffffffffffe ; =9223372036854775806
1004491ec: eb09011f    	cmp	x8, x9
1004491f0: 5402edc8    	b.hi	0x10044efa8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7014>
1004491f4: 91000508    	add	x8, x8, #0x1
1004491f8: f9001548    	str	x8, [x10, #0x28]
1004491fc: f94023e8    	ldr	x8, [sp, #0x40]
100449200: b9401500    	ldr	w0, [x8, #0x14]
100449204: f9409941    	ldr	x1, [x10, #0x130]
100449208: eb00003f    	cmp	x1, x0
10044920c: 5402ed49    	b.ls	0x10044efb4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7020>
100449210: f9409548    	ldr	x8, [x10, #0x128]
100449214: 52800309    	mov	w9, #0x18               ; =24
100449218: 9ba92008    	umaddl	x8, w0, w9, x8
10044921c: f9400109    	ldr	x9, [x8]
100449220: 927f052a    	and	x10, x9, #0x6
100449224: f100095f    	cmp	x10, #0x2
100449228: 5402c880    	b.eq	0x10044eb38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ba4>
10044922c: b940150a    	ldr	w10, [x8, #0x14]
100449230: 3402c84a    	cbz	w10, 0x10044eb38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ba4>
100449234: f100113f    	cmp	x9, #0x4
100449238: 5402cc21    	b.ne	0x10044ebbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c28>
10044923c: f9400509    	ldr	x9, [x8, #0x8]
100449240: f940012a    	ldr	x10, [x9]
100449244: b100054a    	adds	x10, x10, #0x1
100449248: f900012a    	str	x10, [x9]
10044924c: 5402ec42    	b.hs	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
100449250: f9400514    	ldr	x20, [x8, #0x8]
100449254: f9018bf4    	str	x20, [sp, #0x310]
100449258: f94083e9    	ldr	x9, [sp, #0x100]
10044925c: f9401528    	ldr	x8, [x9, #0x28]
100449260: d1000508    	sub	x8, x8, #0x1
100449264: f9001528    	str	x8, [x9, #0x28]
100449268: aa1403e0    	mov	x0, x20
10044926c: 97f0f88c    	bl	0x10008749c <__ZN13quickjs_oxide6engine4atom29parse_canonical_u32_js_string17hf7015ef7cc8bce8eE>
100449270: b9003be1    	str	w1, [sp, #0x38]
100449274: f9400288    	ldr	x8, [x20]
100449278: d1000508    	sub	x8, x8, #0x1
10044927c: f9000288    	str	x8, [x20]
100449280: 3602a400    	tbz	w0, #0x0, 0x10044e700 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x676c>
100449284: b9403be9    	ldr	w9, [sp, #0x38]
100449288: 3100053f    	cmn	w9, #0x1
10044928c: 5402a3a0    	b.eq	0x10044e700 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x676c>
100449290: b5000068    	cbnz	x8, 0x10044929c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1308>
100449294: 910c43e0    	add	x0, sp, #0x310
100449298: 97efa12c    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044929c: f94023e8    	ldr	x8, [sp, #0x40]
1004492a0: 39400108    	ldrb	w8, [x8]
1004492a4: 14000521    	b	0x10044a728 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2794>
1004492a8: f94097eb    	ldr	x11, [sp, #0x128]
1004492ac: f940116b    	ldr	x11, [x11, #0x20]
1004492b0: b40253cb    	cbz	x11, 0x10044dd28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d94>
1004492b4: 5280000c    	mov	w12, #0x0               ; =0
1004492b8: 7101b37f    	cmp	w27, #0x6c
1004492bc: 54006ee1    	b.ne	0x10044a098 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2104>
1004492c0: d100050b    	sub	x11, x8, #0x1
1004492c4: 8b0b0156    	add	x22, x10, x11
1004492c8: eb1c02df    	cmp	x22, x28
1004492cc: 5402cb62    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
1004492d0: 8b161128    	add	x8, x9, x22, lsl #4
1004492d4: 39400118    	ldrb	w24, [x8]
1004492d8: 51002b09    	sub	w9, w24, #0xa
1004492dc: 7100153f    	cmp	w9, #0x5
1004492e0: 540261e3    	b.lo	0x10044df1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f88>
1004492e4: f9408fe9    	ldr	x9, [sp, #0x118]
1004492e8: f900212b    	str	x11, [x9, #0x40]
1004492ec: b8401109    	ldur	w9, [x8, #0x1]
1004492f0: b902f3e9    	str	w9, [sp, #0x2f0]
1004492f4: b9400509    	ldr	w9, [x8, #0x4]
1004492f8: 910963ea    	add	x10, sp, #0x258
1004492fc: b809b149    	stur	w9, [x10, #0x9b]
100449300: f940051b    	ldr	x27, [x8, #0x8]
100449304: 528001c9    	mov	w9, #0xe                ; =14
100449308: 39000109    	strb	w9, [x8]
10044930c: 140005f5    	b	0x10044aae0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2b4c>
100449310: b1000d1f    	cmn	x8, #0x3
100449314: f94087e0    	ldr	x0, [sp, #0x108]
100449318: 54000328    	b.hi	0x10044937c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e8>
10044931c: 91000908    	add	x8, x8, #0x2
100449320: a9432541    	ldp	x1, x9, [x10, #0x30]
100449324: cb010129    	sub	x9, x9, x1
100449328: eb09011f    	cmp	x8, x9
10044932c: 54000288    	b.hi	0x10044937c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e8>
100449330: f94073e8    	ldr	x8, [sp, #0xe0]
100449334: f9400116    	ldr	x22, [x8]
100449338: eb160028    	subs	x8, x1, x22
10044933c: 5402b443    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
100449340: f940081c    	ldr	x28, [x0, #0x10]
100449344: eb1c003f    	cmp	x1, x28
100449348: 5402c1c8    	b.hi	0x10044eb80 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6bec>
10044934c: eb13011f    	cmp	x8, x19
100449350: 54000169    	b.ls	0x10044937c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e8>
100449354: f9400408    	ldr	x8, [x0, #0x8]
100449358: 8b161108    	add	x8, x8, x22, lsl #4
10044935c: d37cee69    	lsl	x9, x19, #4
100449360: 38696909    	ldrb	w9, [x8, x9]
100449364: 7100253f    	cmp	w9, #0x9
100449368: 540000a8    	b.hi	0x10044937c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e8>
10044936c: 71000d3f    	cmp	w9, #0x3
100449370: 5401e460    	b.eq	0x10044cffc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5068>
100449374: 7100113f    	cmp	w9, #0x4
100449378: 5401e300    	b.eq	0x10044cfd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5044>
10044937c: 7103637f    	cmp	w27, #0xd8
100449380: 54006261    	b.ne	0x100449fcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2038>
100449384: aa0a03e1    	mov	x1, x10
100449388: 52800002    	mov	w2, #0x0                ; =0
10044938c: aa1303e3    	mov	x3, x19
100449390: 9400247a    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449394: f9408fe4    	ldr	x4, [sp, #0x118]
100449398: f94087e3    	ldr	x3, [sp, #0x108]
10044939c: 910963eb    	add	x11, sp, #0x258
1004493a0: b40116e0    	cbz	x0, 0x10044b67c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36e8>
1004493a4: 39400008    	ldrb	w8, [x0]
1004493a8: 71001d1f    	cmp	w8, #0x7
1004493ac: 54011688    	b.hi	0x10044b67c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36e8>
1004493b0: 52800029    	mov	w9, #0x1                ; =1
1004493b4: 1ac82129    	lsl	w9, w9, w8
1004493b8: 5280138a    	mov	w10, #0x9c              ; =156
1004493bc: 6a0a013f    	tst	w9, w10
1004493c0: 54011440    	b.eq	0x10044b648 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36b4>
1004493c4: f8401009    	ldur	x9, [x0, #0x1]
1004493c8: f9017be9    	str	x9, [sp, #0x2f0]
1004493cc: f9400409    	ldr	x9, [x0, #0x8]
1004493d0: f809f169    	stur	x9, [x11, #0x9f]
1004493d4: 1400089f    	b	0x10044b650 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36bc>
1004493d8: f94083e8    	ldr	x8, [sp, #0x100]
1004493dc: 79031fe8    	strh	w8, [sp, #0x18e]
1004493e0: 52800028    	mov	w8, #0x1                ; =1
1004493e4: 6a53711f    	tst	w8, w19, lsr #28
1004493e8: 9a88050a    	cinc	x10, x8, ne
1004493ec: f94093e9    	ldr	x9, [sp, #0x120]
1004493f0: 2a0903e9    	mov	w9, w9
1004493f4: 8b090156    	add	x22, x10, x9
1004493f8: eb1c02df    	cmp	x22, x28
1004493fc: 5402c3a2    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100449400: b8767b0a    	ldr	w10, [x24, x22, lsl #2]
100449404: 7905c3ea    	strh	w10, [sp, #0x2e0]
100449408: 53107d4a    	lsr	w10, w10, #16
10044940c: 781003aa    	sturh	w10, [x29, #-0x100]
100449410: 6a53711f    	tst	w8, w19, lsr #28
100449414: 52800048    	mov	w8, #0x2                ; =2
100449418: 9a880508    	cinc	x8, x8, ne
10044941c: 8b090116    	add	x22, x8, x9
100449420: eb1c02df    	cmp	x22, x28
100449424: 5402c262    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100449428: d37ef6c8    	lsl	x8, x22, #2
10044942c: 78e86b08    	ldrsh	w8, [x24, x8]
100449430: b90313e8    	str	w8, [sp, #0x310]
100449434: 91063be8    	add	x8, sp, #0x18e
100449438: d10403a9    	sub	x9, x29, #0x100
10044943c: a93327a8    	stp	x8, x9, [x29, #-0xd0]
100449440: 910c43e8    	add	x8, sp, #0x310
100449444: 910b83e9    	add	x9, sp, #0x2e0
100449448: a93427a8    	stp	x8, x9, [x29, #-0xc0]
10044944c: f9407fe8    	ldr	x8, [sp, #0xf8]
100449450: f81503a8    	stur	x8, [x29, #-0xb0]
100449454: d103c3a0    	sub	x0, x29, #0xf0
100449458: d10343a1    	sub	x1, x29, #0xd0
10044945c: f94087e2    	ldr	x2, [sp, #0x108]
100449460: f9408fe3    	ldr	x3, [sp, #0x118]
100449464: 94003428    	bl	0x100456504 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h1c0d50ae0b05e20aE>
100449468: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044946c: 7100051f    	cmp	w8, #0x1
100449470: 54026160    	b.eq	0x10044e09c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6108>
100449474: 385113a8    	ldurb	w8, [x29, #-0xef]
100449478: f9408fe1    	ldr	x1, [sp, #0x118]
10044947c: f94087e0    	ldr	x0, [sp, #0x108]
100449480: 3701c428    	tbnz	w8, #0x0, 0x10044cd04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d70>
100449484: 79431ff4    	ldrh	w20, [sp, #0x18e]
100449488: 52800002    	mov	w2, #0x0                ; =0
10044948c: aa1403e3    	mov	x3, x20
100449490: 9400243a    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449494: f9408fe4    	ldr	x4, [sp, #0x118]
100449498: f94087e3    	ldr	x3, [sp, #0x108]
10044949c: 910963eb    	add	x11, sp, #0x258
1004494a0: b4014280    	cbz	x0, 0x10044bcf0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d5c>
1004494a4: 39400008    	ldrb	w8, [x0]
1004494a8: 71001d1f    	cmp	w8, #0x7
1004494ac: 54014228    	b.hi	0x10044bcf0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d5c>
1004494b0: 52800029    	mov	w9, #0x1                ; =1
1004494b4: 1ac82129    	lsl	w9, w9, w8
1004494b8: 5280138a    	mov	w10, #0x9c              ; =156
1004494bc: 6a0a013f    	tst	w9, w10
1004494c0: 54013fe0    	b.eq	0x10044bcbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d28>
1004494c4: f8401009    	ldur	x9, [x0, #0x1]
1004494c8: f9017be9    	str	x9, [sp, #0x2f0]
1004494cc: f9400409    	ldr	x9, [x0, #0x8]
1004494d0: f809f169    	stur	x9, [x11, #0x9f]
1004494d4: 140009fc    	b	0x10044bcc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d30>
1004494d8: d2800003    	mov	x3, #0x0                ; =0
1004494dc: f9408fe1    	ldr	x1, [sp, #0x118]
1004494e0: f94087e0    	ldr	x0, [sp, #0x108]
1004494e4: aa1403e2    	mov	x2, x20
1004494e8: aa0303e4    	mov	x4, x3
1004494ec: 940025e1    	bl	0x100452c70 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots11insert_copy17hb51f0c3ae63d8d95E>
1004494f0: 14000db7    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
1004494f4: f9402033    	ldr	x19, [x1, #0x40]
1004494f8: f1000e7f    	cmp	x19, #0x3
1004494fc: 54021903    	b.lo	0x10044d81c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5888>
100449500: f940081c    	ldr	x28, [x0, #0x10]
100449504: f9401828    	ldr	x8, [x1, #0x30]
100449508: d1000d18    	sub	x24, x8, #0x3
10044950c: 8b130316    	add	x22, x24, x19
100449510: eb1c02df    	cmp	x22, x28
100449514: 5402b922    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
100449518: f940040a    	ldr	x10, [x0, #0x8]
10044951c: 8b161142    	add	x2, x10, x22, lsl #4
100449520: 39400059    	ldrb	w25, [x2]
100449524: 51002b29    	sub	w9, w25, #0xa
100449528: 7100113f    	cmp	w9, #0x4
10044952c: 54009d88    	b.hi	0x10044a8dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2948>
100449530: 94032398    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
100449534: 14000da6    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449538: 37fa7c94    	tbnz	w20, #0x1f, 0x10044e4c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6534>
10044953c: b81343b4    	stur	w20, [x29, #-0xcc]
100449540: 52800068    	mov	w8, #0x3                ; =3
100449544: 381303a8    	sturb	w8, [x29, #-0xd0]
100449548: d10343a2    	sub	x2, x29, #0xd0
10044954c: f94087e0    	ldr	x0, [sp, #0x108]
100449550: f9408fe1    	ldr	x1, [sp, #0x118]
100449554: 940023eb    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449558: b400ea00    	cbz	x0, 0x10044b298 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3304>
10044955c: 14001384    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
100449560: d10343a0    	sub	x0, x29, #0xd0
100449564: f94087e1    	ldr	x1, [sp, #0x108]
100449568: f9408fe2    	ldr	x2, [sp, #0x118]
10044956c: 940029d2    	bl	0x100453cb4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
100449570: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449574: 71000d1f    	cmp	w8, #0x3
100449578: 5401b2c0    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044957c: 1400130c    	b	0x10044e1ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6218>
100449580: d10343a0    	sub	x0, x29, #0xd0
100449584: a9500be1    	ldp	x1, x2, [sp, #0x100]
100449588: f9408fe3    	ldr	x3, [sp, #0x118]
10044958c: 94002a3b    	bl	0x100453e78 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h70fd3eeee0d1a43eE>
100449590: b85303a8    	ldur	w8, [x29, #-0xd0]
100449594: 7100091f    	cmp	w8, #0x2
100449598: 5401a821    	b.ne	0x10044ca9c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b08>
10044959c: a95003e3    	ldp	x3, x0, [sp, #0x100]
1004495a0: f9408fe1    	ldr	x1, [sp, #0x118]
1004495a4: 52800022    	mov	w2, #0x1                ; =1
1004495a8: 940023f4    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004495ac: f9408fe4    	ldr	x4, [sp, #0x118]
1004495b0: f94087e3    	ldr	x3, [sp, #0x108]
1004495b4: 910963eb    	add	x11, sp, #0x258
1004495b8: b4011880    	cbz	x0, 0x10044b8c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3934>
1004495bc: 39400008    	ldrb	w8, [x0]
1004495c0: 71001d1f    	cmp	w8, #0x7
1004495c4: 54011828    	b.hi	0x10044b8c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3934>
1004495c8: 52800029    	mov	w9, #0x1                ; =1
1004495cc: 1ac82129    	lsl	w9, w9, w8
1004495d0: 5280138a    	mov	w10, #0x9c              ; =156
1004495d4: 6a0a013f    	tst	w9, w10
1004495d8: 540115e0    	b.eq	0x10044b894 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3900>
1004495dc: f8401009    	ldur	x9, [x0, #0x1]
1004495e0: f9017be9    	str	x9, [sp, #0x2f0]
1004495e4: f9400409    	ldr	x9, [x0, #0x8]
1004495e8: f809f169    	stur	x9, [x11, #0x9f]
1004495ec: 140008ac    	b	0x10044b89c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3908>
1004495f0: f94097e8    	ldr	x8, [sp, #0x128]
1004495f4: f9400508    	ldr	x8, [x8, #0x8]
1004495f8: b1000513    	adds	x19, x8, #0x1
1004495fc: 54024962    	b.hs	0x10044df28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f94>
100449600: f94087e0    	ldr	x0, [sp, #0x108]
100449604: f9408fe1    	ldr	x1, [sp, #0x118]
100449608: 52800022    	mov	w2, #0x1                ; =1
10044960c: 97ff9914    	bl	0x10042fa5c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
100449610: aa0103f4    	mov	x20, x1
100449614: 37025520    	tbnz	w0, #0x0, 0x10044e0b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6124>
100449618: 39400288    	ldrb	w8, [x20]
10044961c: 71000d1f    	cmp	w8, #0x3
100449620: f9408fe1    	ldr	x1, [sp, #0x118]
100449624: f94087e0    	ldr	x0, [sp, #0x108]
100449628: 54024801    	b.ne	0x10044df28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f94>
10044962c: b9400696    	ldr	w22, [x20, #0x4]
100449630: 37fa47d6    	tbnz	w22, #0x1f, 0x10044df28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f94>
100449634: 52800042    	mov	w2, #0x2                ; =2
100449638: 97ff9909    	bl	0x10042fa5c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
10044963c: aa0103f4    	mov	x20, x1
100449640: 370253c0    	tbnz	w0, #0x0, 0x10044e0b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6124>
100449644: f94087e0    	ldr	x0, [sp, #0x108]
100449648: f9408fe1    	ldr	x1, [sp, #0x118]
10044964c: d2800002    	mov	x2, #0x0                ; =0
100449650: 97ff9903    	bl	0x10042fa5c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
100449654: aa0103fb    	mov	x27, x1
100449658: 37028240    	tbnz	w0, #0x0, 0x10044e6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x670c>
10044965c: f9407fe8    	ldr	x8, [sp, #0xf8]
100449660: f9400101    	ldr	x1, [x8]
100449664: 910a03e0    	add	x0, sp, #0x280
100449668: aa1403e2    	mov	x2, x20
10044966c: aa1603e3    	mov	x3, x22
100449670: aa1b03e4    	mov	x4, x27
100449674: 94002d44    	bl	0x100454b84 <__ZN13quickjs_oxide6engine6object16ordinary_storage2ic62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$28try_dense_array_write_scalar17h2b1073b890e82808E>
100449678: 394a03e8    	ldrb	w8, [sp, #0x280]
10044967c: 71002d1f    	cmp	w8, #0xb
100449680: 54028241    	b.ne	0x10044e6c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6734>
100449684: 394a07e8    	ldrb	w8, [sp, #0x281]
100449688: f9408fe3    	ldr	x3, [sp, #0x118]
10044968c: f94087e9    	ldr	x9, [sp, #0x108]
100449690: 37013908    	tbnz	w8, #0x0, 0x10044bdb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3e1c>
100449694: 39400368    	ldrb	w8, [x27]
100449698: 7100111f    	cmp	w8, #0x4
10044969c: 54013780    	b.eq	0x10044bd8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3df8>
1004496a0: 71000d1f    	cmp	w8, #0x3
1004496a4: 54024421    	b.ne	0x10044df28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f94>
1004496a8: bd400760    	ldr	s0, [x27, #0x4]
1004496ac: 0f20a400    	sshll.2d	v0, v0, #0x0
1004496b0: 5e61d800    	scvtf	d0, d0
1004496b4: 140009b7    	b	0x10044bd90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3dfc>
1004496b8: d10343a0    	sub	x0, x29, #0xd0
1004496bc: a9500be1    	ldp	x1, x2, [sp, #0x100]
1004496c0: f9408fe3    	ldr	x3, [sp, #0x118]
1004496c4: 94002bf7    	bl	0x1004546a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h82601e3ca496b491E>
1004496c8: 385303a8    	ldurb	w8, [x29, #-0xd0]
1004496cc: 7100051f    	cmp	w8, #0x1
1004496d0: 54026f60    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
1004496d4: 385313a8    	ldurb	w8, [x29, #-0xcf]
1004496d8: 71000d1f    	cmp	w8, #0x3
1004496dc: 54009840    	b.eq	0x10044a9e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2a50>
1004496e0: 7100091f    	cmp	w8, #0x2
1004496e4: f94087ea    	ldr	x10, [sp, #0x108]
1004496e8: f9407fec    	ldr	x12, [sp, #0xf8]
1004496ec: 54028360    	b.eq	0x10044e758 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67c4>
1004496f0: f94097e8    	ldr	x8, [sp, #0x128]
1004496f4: f9401108    	ldr	x8, [x8, #0x20]
1004496f8: b4023188    	cbz	x8, 0x10044dd28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d94>
1004496fc: f94083e8    	ldr	x8, [sp, #0x100]
100449700: 12003d13    	and	w19, w8, #0xffff
100449704: f94073e8    	ldr	x8, [sp, #0xe0]
100449708: f9400108    	ldr	x8, [x8]
10044970c: f9407be9    	ldr	x9, [sp, #0xf0]
100449710: f9400129    	ldr	x9, [x9]
100449714: eb080129    	subs	x9, x9, x8
100449718: 9a8933e9    	csel	x9, xzr, x9, lo
10044971c: eb13013f    	cmp	x9, x19
100449720: 54028429    	b.ls	0x10044e7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6810>
100449724: f940095c    	ldr	x28, [x10, #0x10]
100449728: 8b130116    	add	x22, x8, x19
10044972c: eb1c02df    	cmp	x22, x28
100449730: 5402b7e2    	b.hs	0x10044ee2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6e98>
100449734: f9400548    	ldr	x8, [x10, #0x8]
100449738: 8b161108    	add	x8, x8, x22, lsl #4
10044973c: 39400109    	ldrb	w9, [x8]
100449740: 7100393f    	cmp	w9, #0xe
100449744: 540283e0    	b.eq	0x10044e7c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x682c>
100449748: b840110a    	ldur	w10, [x8, #0x1]
10044974c: f9403feb    	ldr	x11, [sp, #0x78]
100449750: b900016a    	str	w10, [x11]
100449754: b940050a    	ldr	w10, [x8, #0x4]
100449758: b800316a    	stur	w10, [x11, #0x3]
10044975c: f940050a    	ldr	x10, [x8, #0x8]
100449760: 5280018b    	mov	w11, #0xc               ; =12
100449764: 3900010b    	strb	w11, [x8]
100449768: 3907a3e9    	strb	w9, [sp, #0x1e8]
10044976c: f900fbea    	str	x10, [sp, #0x1f0]
100449770: a95223e1    	ldp	x1, x8, [sp, #0x120]
100449774: f9401103    	ldr	x3, [x8, #0x20]
100449778: f9400182    	ldr	x2, [x12]
10044977c: f9406fe0    	ldr	x0, [sp, #0xd8]
100449780: 94002b89    	bl	0x1004545a4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
100449784: b5025f40    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
100449788: f9407fe8    	ldr	x8, [sp, #0xf8]
10044978c: f9400100    	ldr	x0, [x8]
100449790: 9107a3e1    	add	x1, sp, #0x1e8
100449794: 97f463a2    	bl	0x10016261c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
100449798: b40092a0    	cbz	x0, 0x10044a9ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2a58>
10044979c: 140012f4    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
1004497a0: f94087e0    	ldr	x0, [sp, #0x108]
1004497a4: f9408fe1    	ldr	x1, [sp, #0x118]
1004497a8: d2800002    	mov	x2, #0x0                ; =0
1004497ac: 97ff98ac    	bl	0x10042fa5c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
1004497b0: 37026260    	tbnz	w0, #0x0, 0x10044e3fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6468>
1004497b4: 39400028    	ldrb	w8, [x1]
1004497b8: 71001d1f    	cmp	w8, #0x7
1004497bc: f9408fe3    	ldr	x3, [sp, #0x118]
1004497c0: f94087e2    	ldr	x2, [sp, #0x108]
1004497c4: f9407fea    	ldr	x10, [sp, #0xf8]
1004497c8: 540000c8    	b.hi	0x1004497e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x184c>
1004497cc: 52800029    	mov	w9, #0x1                ; =1
1004497d0: 1ac82128    	lsl	w8, w9, w8
1004497d4: 528013e9    	mov	w9, #0x9f               ; =159
1004497d8: 6a09011f    	tst	w8, w9
1004497dc: 540133e1    	b.ne	0x10044be58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ec4>
1004497e0: f9400141    	ldr	x1, [x10]
1004497e4: d10343a0    	sub	x0, x29, #0xd0
1004497e8: 94002e23    	bl	0x100455074 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17hed8eb2f3e93c22dbE>
1004497ec: 385303a8    	ldurb	w8, [x29, #-0xd0]
1004497f0: 7100051f    	cmp	w8, #0x1
1004497f4: 54026640    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
1004497f8: f94097e8    	ldr	x8, [sp, #0x128]
1004497fc: f9401108    	ldr	x8, [x8, #0x20]
100449800: f9408fe2    	ldr	x2, [sp, #0x118]
100449804: f94087e1    	ldr	x1, [sp, #0x108]
100449808: b4022908    	cbz	x8, 0x10044dd28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d94>
10044980c: 385313b3    	ldurb	w19, [x29, #-0xcf]
100449810: d10343a0    	sub	x0, x29, #0xd0
100449814: 94002928    	bl	0x100453cb4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
100449818: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044981c: 7100291f    	cmp	w8, #0xa
100449820: 540264e0    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
100449824: f94077ea    	ldr	x10, [sp, #0xe8]
100449828: b9400149    	ldr	w9, [x10]
10044982c: f94043eb    	ldr	x11, [sp, #0x80]
100449830: b9000169    	str	w9, [x11]
100449834: b8403149    	ldur	w9, [x10, #0x3]
100449838: b8003169    	stur	w9, [x11, #0x3]
10044983c: f85383a9    	ldur	x9, [x29, #-0xc8]
100449840: 3908a3e8    	strb	w8, [sp, #0x228]
100449844: f9011be9    	str	x9, [sp, #0x230]
100449848: 52800028    	mov	w8, #0x1                ; =1
10044984c: 0a330108    	bic	w8, w8, w19
100449850: 381313a8    	sturb	w8, [x29, #-0xcf]
100449854: 52800048    	mov	w8, #0x2                ; =2
100449858: 381303a8    	sturb	w8, [x29, #-0xd0]
10044985c: d10343a2    	sub	x2, x29, #0xd0
100449860: f94087e0    	ldr	x0, [sp, #0x108]
100449864: f9408fe1    	ldr	x1, [sp, #0x118]
100449868: 94002326    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044986c: b5025800    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
100449870: a95223e1    	ldp	x1, x8, [sp, #0x120]
100449874: f9401103    	ldr	x3, [x8, #0x20]
100449878: f9407fe8    	ldr	x8, [sp, #0xf8]
10044987c: f9400102    	ldr	x2, [x8]
100449880: f9406fe0    	ldr	x0, [sp, #0xd8]
100449884: 94002b48    	bl	0x1004545a4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
100449888: b5025720    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044988c: f9407fe8    	ldr	x8, [sp, #0xf8]
100449890: f9400101    	ldr	x1, [x8]
100449894: 9108e3e0    	add	x0, sp, #0x238
100449898: 9108a3e2    	add	x2, sp, #0x228
10044989c: 97f03634    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
1004498a0: 3948e3e8    	ldrb	w8, [sp, #0x238]
1004498a4: 71002d1f    	cmp	w8, #0xb
1004498a8: 54019940    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
1004498ac: 14001391    	b	0x10044e6f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x675c>
1004498b0: f94087e8    	ldr	x8, [sp, #0x108]
1004498b4: a9408500    	ldp	x0, x1, [x8, #0x8]
1004498b8: f9408fe8    	ldr	x8, [sp, #0x118]
1004498bc: f9401902    	ldr	x2, [x8, #0x30]
1004498c0: f9402103    	ldr	x3, [x8, #0x40]
1004498c4: d2800004    	mov	x4, #0x0                ; =0
1004498c8: 52800085    	mov	w5, #0x4                ; =4
1004498cc: 52800026    	mov	w6, #0x1                ; =1
1004498d0: 94002591    	bl	0x100452f14 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23rotate_operands_current17ha464970b12b6798cE>
1004498d4: 14000cbe    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
1004498d8: f9408be8    	ldr	x8, [sp, #0x110]
1004498dc: f9400108    	ldr	x8, [x8]
1004498e0: f940551c    	ldr	x28, [x8, #0xa8]
1004498e4: f94083e9    	ldr	x9, [sp, #0x100]
1004498e8: 12003d36    	and	w22, w9, #0xffff
1004498ec: eb16039f    	cmp	x28, x22
1004498f0: 5402af49    	b.ls	0x10044eed8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f44>
1004498f4: f9405108    	ldr	x8, [x8, #0xa0]
1004498f8: 8b161508    	add	x8, x8, x22, lsl #5
1004498fc: 3940ad13    	ldrb	w19, [x8, #0x2b]
100449900: 71001a7f    	cmp	w19, #0x6
100449904: f9408fe3    	ldr	x3, [sp, #0x118]
100449908: f94087e2    	ldr	x2, [sp, #0x108]
10044990c: 54025fa0    	b.eq	0x10044e500 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x656c>
100449910: 3940a114    	ldrb	w20, [x8, #0x28]
100449914: d10343a0    	sub	x0, x29, #0xd0
100449918: f94083e1    	ldr	x1, [sp, #0x100]
10044991c: 94002b61    	bl	0x1004546a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h82601e3ca496b491E>
100449920: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449924: 7100051f    	cmp	w8, #0x1
100449928: 54025ca0    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044992c: 385313a8    	ldurb	w8, [x29, #-0xcf]
100449930: 7100027f    	cmp	w19, #0x0
100449934: 7a420900    	ccmp	w8, #0x2, #0x0, eq
100449938: f9408fe3    	ldr	x3, [sp, #0x118]
10044993c: f94087e2    	ldr	x2, [sp, #0x108]
100449940: 54026720    	b.eq	0x10044e624 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6690>
100449944: 36025374    	tbz	w20, #0x0, 0x10044e3b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x641c>
100449948: 71000d1f    	cmp	w8, #0x3
10044994c: 54025328    	b.hi	0x10044e3b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x641c>
100449950: 7100091f    	cmp	w8, #0x2
100449954: 540252e0    	b.eq	0x10044e3b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x641c>
100449958: f94097e8    	ldr	x8, [sp, #0x128]
10044995c: f9401108    	ldr	x8, [x8, #0x20]
100449960: b4021e48    	cbz	x8, 0x10044dd28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d94>
100449964: d10343a0    	sub	x0, x29, #0xd0
100449968: f94083e1    	ldr	x1, [sp, #0x100]
10044996c: 94002bcc    	bl	0x10045489c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h7ea5ad6467471964E>
100449970: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449974: 7100391f    	cmp	w8, #0xe
100449978: 54025a20    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044997c: f94077ea    	ldr	x10, [sp, #0xe8]
100449980: b9400149    	ldr	w9, [x10]
100449984: f94047eb    	ldr	x11, [sp, #0x88]
100449988: b9000169    	str	w9, [x11]
10044998c: b8403149    	ldur	w9, [x10, #0x3]
100449990: b8003169    	stur	w9, [x11, #0x3]
100449994: f85383a9    	ldur	x9, [x29, #-0xc8]
100449998: 390763e8    	strb	w8, [sp, #0x1d8]
10044999c: f900f3e9    	str	x9, [sp, #0x1e0]
1004499a0: a95223e1    	ldp	x1, x8, [sp, #0x120]
1004499a4: f9401103    	ldr	x3, [x8, #0x20]
1004499a8: f9407fe8    	ldr	x8, [sp, #0xf8]
1004499ac: f9400102    	ldr	x2, [x8]
1004499b0: f9406fe0    	ldr	x0, [sp, #0xd8]
1004499b4: 94002afc    	bl	0x1004545a4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
1004499b8: b5024da0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
1004499bc: f9407fe8    	ldr	x8, [sp, #0xf8]
1004499c0: f9400100    	ldr	x0, [x8]
1004499c4: 910763e1    	add	x1, sp, #0x1d8
1004499c8: 97f46315    	bl	0x10016261c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
1004499cc: 14000c80    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
1004499d0: f8490348    	ldur	x8, [x26, #0x90]
1004499d4: f100091f    	cmp	x8, #0x2
1004499d8: 54028340    	b.eq	0x10044ea40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aac>
1004499dc: f9407fe8    	ldr	x8, [sp, #0xf8]
1004499e0: f9400102    	ldr	x2, [x8]
1004499e4: 384b8348    	ldurb	w8, [x26, #0xb8]
1004499e8: 71001d1f    	cmp	w8, #0x7
1004499ec: f9408fe1    	ldr	x1, [sp, #0x118]
1004499f0: f94087e0    	ldr	x0, [sp, #0x108]
1004499f4: 910963eb    	add	x11, sp, #0x258
1004499f8: 54012668    	b.hi	0x10044bec4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f30>
1004499fc: 52800029    	mov	w9, #0x1                ; =1
100449a00: 1ac82129    	lsl	w9, w9, w8
100449a04: 5280138a    	mov	w10, #0x9c              ; =156
100449a08: 6a0a013f    	tst	w9, w10
100449a0c: 5400a800    	b.eq	0x10044af0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2f78>
100449a10: f84b9349    	ldur	x9, [x26, #0xb9]
100449a14: f81103a9    	stur	x9, [x29, #-0xf0]
100449a18: f84c0349    	ldur	x9, [x26, #0xc0]
100449a1c: f80ff169    	stur	x9, [x11, #0xff]
100449a20: 1400053d    	b	0x10044af14 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2f80>
100449a24: a95003e8    	ldp	x8, x0, [sp, #0x100]
100449a28: 2a0803e8    	mov	w8, w8
100449a2c: a95107e9    	ldp	x9, x1, [sp, #0x110]
100449a30: f9400129    	ldr	x9, [x9]
100449a34: f940452a    	ldr	x10, [x9, #0x88]
100449a38: eb08015f    	cmp	x10, x8
100449a3c: 540235c9    	b.ls	0x10044e0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6160>
100449a40: f9404129    	ldr	x9, [x9, #0x80]
100449a44: 5280030a    	mov	w10, #0x18              ; =24
100449a48: 9baa2508    	umaddl	x8, w8, w10, x9
100449a4c: b8410d09    	ldr	w9, [x8, #0x10]!
100449a50: 35023529    	cbnz	w9, 0x10044e0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6160>
100449a54: 39402109    	ldrb	w9, [x8, #0x8]
100449a58: 71000d3f    	cmp	w9, #0x3
100449a5c: 54007d4c    	b.gt	0x10044aa04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2a70>
100449a60: 7100053f    	cmp	w9, #0x1
100449a64: 5400c22c    	b.gt	0x10044b2a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3314>
100449a68: 340130a9    	cbz	w9, 0x10044c07c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40e8>
100449a6c: 7100053f    	cmp	w9, #0x1
100449a70: 54023421    	b.ne	0x10044e0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6160>
100449a74: 52800028    	mov	w8, #0x1                ; =1
100449a78: 381303a8    	sturb	w8, [x29, #-0xd0]
100449a7c: d10343a2    	sub	x2, x29, #0xd0
100449a80: 940022a0    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449a84: 14000c52    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449a88: 381303bf    	sturb	wzr, [x29, #-0xd0]
100449a8c: d10343a2    	sub	x2, x29, #0xd0
100449a90: f94087e0    	ldr	x0, [sp, #0x108]
100449a94: f9408fe1    	ldr	x1, [sp, #0x118]
100449a98: 9400229a    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449a9c: 14000c4c    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449aa0: a95003ea    	ldp	x10, x0, [sp, #0x100]
100449aa4: 13003d49    	sxth	w9, w10
100449aa8: 7100011f    	cmp	w8, #0x0
100449aac: 1a8a0128    	csel	w8, w9, w10, eq
100449ab0: b81343a8    	stur	w8, [x29, #-0xcc]
100449ab4: 52800068    	mov	w8, #0x3                ; =3
100449ab8: 381303a8    	sturb	w8, [x29, #-0xd0]
100449abc: d10343a2    	sub	x2, x29, #0xd0
100449ac0: f9408fe1    	ldr	x1, [sp, #0x118]
100449ac4: 9400228f    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449ac8: 14000c41    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449acc: f9407749    	ldr	x9, [x26, #0xe8]
100449ad0: b40000a9    	cbz	x9, 0x100449ae4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1b50>
100449ad4: aa0903e2    	mov	x2, x9
100449ad8: 38440c48    	ldrb	w8, [x2, #0x40]!
100449adc: 7100291f    	cmp	w8, #0xa
100449ae0: 5400a961    	b.ne	0x10044b00c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3078>
100449ae4: a95133e8    	ldp	x8, x12, [sp, #0x110]
100449ae8: f9400108    	ldr	x8, [x8]
100449aec: 3944290a    	ldrb	w10, [x8, #0x10a]
100449af0: f8490349    	ldur	x9, [x26, #0x90]
100449af4: f94087e0    	ldr	x0, [sp, #0x108]
100449af8: f9407fed    	ldr	x13, [sp, #0xf8]
100449afc: 910963ee    	add	x14, sp, #0x258
100449b00: f100093f    	cmp	x9, #0x2
100449b04: 3600a2aa    	tbz	w10, #0x0, 0x10044af58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2fc4>
100449b08: 540279c0    	b.eq	0x10044ea40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aac>
100449b0c: 384a8348    	ldurb	w8, [x26, #0xa8]
100449b10: f94001a1    	ldr	x1, [x13]
100449b14: 71001d1f    	cmp	w8, #0x7
100449b18: 54012aa8    	b.hi	0x10044c06c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40d8>
100449b1c: 52800029    	mov	w9, #0x1                ; =1
100449b20: 1ac82129    	lsl	w9, w9, w8
100449b24: 5280138a    	mov	w10, #0x9c              ; =156
100449b28: 6a0a013f    	tst	w9, w10
100449b2c: 54012680    	b.eq	0x10044bffc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4068>
100449b30: f84a9349    	ldur	x9, [x26, #0xa9]
100449b34: f81103a9    	stur	x9, [x29, #-0xf0]
100449b38: f84b0349    	ldur	x9, [x26, #0xb0]
100449b3c: 14000543    	b	0x10044b048 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30b4>
100449b40: 52800028    	mov	w8, #0x1                ; =1
100449b44: 381303a8    	sturb	w8, [x29, #-0xd0]
100449b48: d10343a2    	sub	x2, x29, #0xd0
100449b4c: f94087e0    	ldr	x0, [sp, #0x108]
100449b50: f9408fe1    	ldr	x1, [sp, #0x118]
100449b54: 9400226b    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449b58: 14000c1d    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449b5c: 52802048    	mov	w8, #0x102              ; =258
100449b60: 781303a8    	sturh	w8, [x29, #-0xd0]
100449b64: d10343a2    	sub	x2, x29, #0xd0
100449b68: f94087e0    	ldr	x0, [sp, #0x108]
100449b6c: f9408fe1    	ldr	x1, [sp, #0x118]
100449b70: 94002264    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449b74: 14000c16    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449b78: a95003e3    	ldp	x3, x0, [sp, #0x100]
100449b7c: f9408fe1    	ldr	x1, [sp, #0x118]
100449b80: 52800002    	mov	w2, #0x0                ; =0
100449b84: 9400227d    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449b88: f9408fe4    	ldr	x4, [sp, #0x118]
100449b8c: f94087e3    	ldr	x3, [sp, #0x108]
100449b90: 910963eb    	add	x11, sp, #0x258
100449b94: b400bfc0    	cbz	x0, 0x10044b38c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x33f8>
100449b98: 39400008    	ldrb	w8, [x0]
100449b9c: 71001d1f    	cmp	w8, #0x7
100449ba0: 5400bf68    	b.hi	0x10044b38c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x33f8>
100449ba4: 52800029    	mov	w9, #0x1                ; =1
100449ba8: 1ac82129    	lsl	w9, w9, w8
100449bac: 5280138a    	mov	w10, #0x9c              ; =156
100449bb0: 6a0a013f    	tst	w9, w10
100449bb4: 5400bd20    	b.eq	0x10044b358 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x33c4>
100449bb8: f8401009    	ldur	x9, [x0, #0x1]
100449bbc: f9017be9    	str	x9, [sp, #0x2f0]
100449bc0: f9400409    	ldr	x9, [x0, #0x8]
100449bc4: f809f169    	stur	x9, [x11, #0x9f]
100449bc8: 140005e6    	b	0x10044b360 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x33cc>
100449bcc: f94083e8    	ldr	x8, [sp, #0x100]
100449bd0: 79031be8    	strh	w8, [sp, #0x18c]
100449bd4: 52800028    	mov	w8, #0x1                ; =1
100449bd8: 6a53711f    	tst	w8, w19, lsr #28
100449bdc: 9a880508    	cinc	x8, x8, ne
100449be0: f94093e9    	ldr	x9, [sp, #0x120]
100449be4: 8b294116    	add	x22, x8, w9, uxtw
100449be8: eb1c02df    	cmp	x22, x28
100449bec: 54028422    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
100449bf0: b8767b08    	ldr	w8, [x24, x22, lsl #2]
100449bf4: 781003a8    	sturh	w8, [x29, #-0x100]
100449bf8: 53107d08    	lsr	w8, w8, #16
100449bfc: b90313e8    	str	w8, [sp, #0x310]
100449c00: 910633e8    	add	x8, sp, #0x18c
100449c04: d10403a9    	sub	x9, x29, #0x100
100449c08: a93327a8    	stp	x8, x9, [x29, #-0xd0]
100449c0c: f9407fe9    	ldr	x9, [sp, #0xf8]
100449c10: a950a3e2    	ldp	x2, x8, [sp, #0x108]
100449c14: a93423a9    	stp	x9, x8, [x29, #-0xc0]
100449c18: 910543e8    	add	x8, sp, #0x150
100449c1c: 910c43e9    	add	x9, sp, #0x310
100449c20: a93527a8    	stp	x8, x9, [x29, #-0xb0]
100449c24: d103c3a0    	sub	x0, x29, #0xf0
100449c28: d10343a1    	sub	x1, x29, #0xd0
100449c2c: f9408fe3    	ldr	x3, [sp, #0x118]
100449c30: 940032ef    	bl	0x1004567ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17hc7efe337c4f9c0c8E>
100449c34: 385103a8    	ldurb	w8, [x29, #-0xf0]
100449c38: 7100051f    	cmp	w8, #0x1
100449c3c: 54022300    	b.eq	0x10044e09c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6108>
100449c40: 385113a8    	ldurb	w8, [x29, #-0xef]
100449c44: f9408fe1    	ldr	x1, [sp, #0x118]
100449c48: f94087e0    	ldr	x0, [sp, #0x108]
100449c4c: 37018588    	tbnz	w8, #0x0, 0x10044ccfc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d68>
100449c50: 79431bf4    	ldrh	w20, [sp, #0x18c]
100449c54: 52800002    	mov	w2, #0x0                ; =0
100449c58: aa1403e3    	mov	x3, x20
100449c5c: 94002247    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449c60: f9408fe4    	ldr	x4, [sp, #0x118]
100449c64: f94087e3    	ldr	x3, [sp, #0x108]
100449c68: 910963eb    	add	x11, sp, #0x258
100449c6c: b400ffc0    	cbz	x0, 0x10044bc64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cd0>
100449c70: 39400008    	ldrb	w8, [x0]
100449c74: 71001d1f    	cmp	w8, #0x7
100449c78: 5400ff68    	b.hi	0x10044bc64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cd0>
100449c7c: 52800029    	mov	w9, #0x1                ; =1
100449c80: 1ac82129    	lsl	w9, w9, w8
100449c84: 5280138a    	mov	w10, #0x9c              ; =156
100449c88: 6a0a013f    	tst	w9, w10
100449c8c: 5400fd20    	b.eq	0x10044bc30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c9c>
100449c90: f8401009    	ldur	x9, [x0, #0x1]
100449c94: f9017be9    	str	x9, [sp, #0x2f0]
100449c98: f9400409    	ldr	x9, [x0, #0x8]
100449c9c: f809f169    	stur	x9, [x11, #0x9f]
100449ca0: 140007e6    	b	0x10044bc38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ca4>
100449ca4: 52800048    	mov	w8, #0x2                ; =2
100449ca8: 781303a8    	sturh	w8, [x29, #-0xd0]
100449cac: d10343a2    	sub	x2, x29, #0xd0
100449cb0: f94087e0    	ldr	x0, [sp, #0x108]
100449cb4: f9408fe1    	ldr	x1, [sp, #0x118]
100449cb8: 94002212    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449cbc: 14000bc4    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449cc0: f94083e8    	ldr	x8, [sp, #0x100]
100449cc4: 12002508    	and	w8, w8, #0x3ff
100449cc8: 71039d1f    	cmp	w8, #0xe7
100449ccc: 540240c2    	b.hs	0x10044e4e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6550>
100449cd0: 90000a09    	adrp	x9, 0x100589000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16e90>
100449cd4: 9124e129    	add	x9, x9, #0x938
100449cd8: 78685934    	ldrh	w20, [x9, w8, uxtw #1]
100449cdc: f94087e8    	ldr	x8, [sp, #0x108]
100449ce0: a9408901    	ldp	x1, x2, [x8, #0x8]
100449ce4: d10343a0    	sub	x0, x29, #0xd0
100449ce8: f9408fe3    	ldr	x3, [sp, #0x118]
100449cec: aa1403e4    	mov	x4, x20
100449cf0: 94002f71    	bl	0x100455ab4 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore26number_pair_branch_current17h41edf09839af6f39E>
100449cf4: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449cf8: 7100051f    	cmp	w8, #0x1
100449cfc: 54023e00    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
100449d00: 385313a8    	ldurb	w8, [x29, #-0xcf]
100449d04: 7100091f    	cmp	w8, #0x2
100449d08: 54018021    	b.ne	0x10044cd0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d78>
100449d0c: f94087e8    	ldr	x8, [sp, #0x108]
100449d10: a9408901    	ldp	x1, x2, [x8, #0x8]
100449d14: d10343a0    	sub	x0, x29, #0xd0
100449d18: f9408fe3    	ldr	x3, [sp, #0x118]
100449d1c: aa1403e4    	mov	x4, x20
100449d20: 94003007    	bl	0x100455d3c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore21binary_number_current17h067062137fa236b9E>
100449d24: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449d28: 7100051f    	cmp	w8, #0x1
100449d2c: 54023c80    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
100449d30: 385313a8    	ldurb	w8, [x29, #-0xcf]
100449d34: 370174e8    	tbnz	w8, #0x0, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
100449d38: 14001255    	b	0x10044e68c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66f8>
100449d3c: b9400348    	ldr	w8, [x26]
100449d40: 36026808    	tbz	w8, #0x0, 0x10044ea40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aac>
100449d44: 29425b54    	ldp	w20, w22, [x26, #0x10]
100449d48: f9400741    	ldr	x1, [x26, #0x8]
100449d4c: 910bc3e0    	add	x0, sp, #0x2f0
100449d50: aa1403e2    	mov	x2, x20
100449d54: aa1603e3    	mov	x3, x22
100449d58: 97f07b61    	bl	0x100068adc <__ZN13quickjs_oxide6engine4heap9ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$20retain_object_handle17ha34447950ade3eebE>
100449d5c: 394bc3e8    	ldrb	w8, [sp, #0x2f0]
100449d60: 7100191f    	cmp	w8, #0x6
100449d64: 54024121    	b.ne	0x10044e588 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65f4>
100449d68: 2926dbb4    	stp	w20, w22, [x29, #-0xcc]
100449d6c: 52800128    	mov	w8, #0x9                ; =9
100449d70: 381303a8    	sturb	w8, [x29, #-0xd0]
100449d74: f9407fe8    	ldr	x8, [sp, #0xf8]
100449d78: f9400102    	ldr	x2, [x8]
100449d7c: d10343a3    	sub	x3, x29, #0xd0
100449d80: f94087e0    	ldr	x0, [sp, #0x108]
100449d84: f9408fe1    	ldr	x1, [sp, #0x118]
100449d88: 94002242    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449d8c: 14000b90    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449d90: a95003e3    	ldp	x3, x0, [sp, #0x100]
100449d94: f9408fe1    	ldr	x1, [sp, #0x118]
100449d98: 52800002    	mov	w2, #0x0                ; =0
100449d9c: 940021f7    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449da0: f9408fe4    	ldr	x4, [sp, #0x118]
100449da4: f94087e3    	ldr	x3, [sp, #0x108]
100449da8: 910963eb    	add	x11, sp, #0x258
100449dac: b400b340    	cbz	x0, 0x10044b414 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3480>
100449db0: 39400008    	ldrb	w8, [x0]
100449db4: 71001d1f    	cmp	w8, #0x7
100449db8: 5400b2e8    	b.hi	0x10044b414 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3480>
100449dbc: 52800029    	mov	w9, #0x1                ; =1
100449dc0: 1ac82129    	lsl	w9, w9, w8
100449dc4: 5280138a    	mov	w10, #0x9c              ; =156
100449dc8: 6a0a013f    	tst	w9, w10
100449dcc: 5400b0a0    	b.eq	0x10044b3e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x344c>
100449dd0: f8401009    	ldur	x9, [x0, #0x1]
100449dd4: f9017be9    	str	x9, [sp, #0x2f0]
100449dd8: f9400409    	ldr	x9, [x0, #0x8]
100449ddc: f809f169    	stur	x9, [x11, #0x9f]
100449de0: 14000582    	b	0x10044b3e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3454>
100449de4: a95003e3    	ldp	x3, x0, [sp, #0x100]
100449de8: f9408fe1    	ldr	x1, [sp, #0x118]
100449dec: 52800022    	mov	w2, #0x1                ; =1
100449df0: 940021e2    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449df4: f9408fe4    	ldr	x4, [sp, #0x118]
100449df8: f94087e3    	ldr	x3, [sp, #0x108]
100449dfc: 910963eb    	add	x11, sp, #0x258
100449e00: b400b4e0    	cbz	x0, 0x10044b49c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3508>
100449e04: 39400008    	ldrb	w8, [x0]
100449e08: 71001d1f    	cmp	w8, #0x7
100449e0c: 5400b488    	b.hi	0x10044b49c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3508>
100449e10: 52800029    	mov	w9, #0x1                ; =1
100449e14: 1ac82129    	lsl	w9, w9, w8
100449e18: 5280138a    	mov	w10, #0x9c              ; =156
100449e1c: 6a0a013f    	tst	w9, w10
100449e20: 5400b240    	b.eq	0x10044b468 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x34d4>
100449e24: f8401009    	ldur	x9, [x0, #0x1]
100449e28: f9017be9    	str	x9, [sp, #0x2f0]
100449e2c: f9400409    	ldr	x9, [x0, #0x8]
100449e30: f809f169    	stur	x9, [x11, #0x9f]
100449e34: 1400058f    	b	0x10044b470 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x34dc>
100449e38: f94087e8    	ldr	x8, [sp, #0x108]
100449e3c: a9408500    	ldp	x0, x1, [x8, #0x8]
100449e40: f9408fe8    	ldr	x8, [sp, #0x118]
100449e44: f9401902    	ldr	x2, [x8, #0x30]
100449e48: f9402103    	ldr	x3, [x8, #0x40]
100449e4c: d2800004    	mov	x4, #0x0                ; =0
100449e50: 52800045    	mov	w5, #0x2                ; =2
100449e54: 52800006    	mov	w6, #0x0                ; =0
100449e58: 9400242f    	bl	0x100452f14 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23rotate_operands_current17ha464970b12b6798cE>
100449e5c: 14000b5c    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
100449e60: d10343a0    	sub	x0, x29, #0xd0
100449e64: a9500be1    	ldp	x1, x2, [sp, #0x100]
100449e68: f9408fe3    	ldr	x3, [sp, #0x118]
100449e6c: 94002a0d    	bl	0x1004546a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h82601e3ca496b491E>
100449e70: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449e74: 7100051f    	cmp	w8, #0x1
100449e78: 54023220    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
100449e7c: 385313a8    	ldurb	w8, [x29, #-0xcf]
100449e80: 7100091f    	cmp	w8, #0x2
100449e84: 540239c0    	b.eq	0x10044e5bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6628>
100449e88: f9407349    	ldr	x9, [x26, #0xe0]
100449e8c: f94083e8    	ldr	x8, [sp, #0x100]
100449e90: 92403d08    	and	x8, x8, #0xffff
100449e94: eb08013f    	cmp	x9, x8
100449e98: 540169c9    	b.ls	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
100449e9c: f9406f49    	ldr	x9, [x26, #0xd8]
100449ea0: 3828693f    	strb	wzr, [x9, x8]
100449ea4: 14000b4b    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
100449ea8: f8490348    	ldur	x8, [x26, #0x90]
100449eac: f100091f    	cmp	x8, #0x2
100449eb0: 54025c80    	b.eq	0x10044ea40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aac>
100449eb4: 384b8348    	ldurb	w8, [x26, #0xb8]
100449eb8: 350168c8    	cbnz	w8, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
100449ebc: 140011ae    	b	0x10044e574 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65e0>
100449ec0: d10343a0    	sub	x0, x29, #0xd0
100449ec4: f94087e1    	ldr	x1, [sp, #0x108]
100449ec8: f9408fe2    	ldr	x2, [sp, #0x118]
100449ecc: 9400277a    	bl	0x100453cb4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
100449ed0: 385303a8    	ldurb	w8, [x29, #-0xd0]
100449ed4: 71000d1f    	cmp	w8, #0x3
100449ed8: 540217c1    	b.ne	0x10044e1d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x623c>
100449edc: b85343a1    	ldur	w1, [x29, #-0xcc]
100449ee0: 37fa3941    	tbnz	w1, #0x1f, 0x10044e608 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6674>
100449ee4: f900abe1    	str	x1, [sp, #0x150]
100449ee8: f9408be8    	ldr	x8, [sp, #0x110]
100449eec: f9400108    	ldr	x8, [x8]
100449ef0: 91014100    	add	x0, x8, #0x50
100449ef4: 97f133a6    	bl	0x100096d8c <__ZN13quickjs_oxide6engine4code4exec8ExecCode14opcode_at_exec17he9da0ed9fc7f2c79E>
100449ef8: 12003c08    	and	w8, w0, #0xffff
100449efc: 71039d1f    	cmp	w8, #0xe7
100449f00: 54016681    	b.ne	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
100449f04: 140011d2    	b	0x10044e64c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66b8>
100449f08: d10343a0    	sub	x0, x29, #0xd0
100449f0c: a9500be1    	ldp	x1, x2, [sp, #0x100]
100449f10: f9408fe3    	ldr	x3, [sp, #0x118]
100449f14: 94002793    	bl	0x100453d60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h6225c2cdb0985a83E>
100449f18: b85303a8    	ldur	w8, [x29, #-0xd0]
100449f1c: 7100091f    	cmp	w8, #0x2
100449f20: 54015cc1    	b.ne	0x10044cab8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b24>
100449f24: a95003e3    	ldp	x3, x0, [sp, #0x100]
100449f28: f9408fe1    	ldr	x1, [sp, #0x118]
100449f2c: 52800002    	mov	w2, #0x0                ; =0
100449f30: 94002192    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449f34: f9408fe4    	ldr	x4, [sp, #0x118]
100449f38: f94087e3    	ldr	x3, [sp, #0x108]
100449f3c: 910963eb    	add	x11, sp, #0x258
100449f40: b400d1a0    	cbz	x0, 0x10044b974 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39e0>
100449f44: 39400008    	ldrb	w8, [x0]
100449f48: 71001d1f    	cmp	w8, #0x7
100449f4c: 5400d148    	b.hi	0x10044b974 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39e0>
100449f50: 52800029    	mov	w9, #0x1                ; =1
100449f54: 1ac82129    	lsl	w9, w9, w8
100449f58: 5280138a    	mov	w10, #0x9c              ; =156
100449f5c: 6a0a013f    	tst	w9, w10
100449f60: 5400cf00    	b.eq	0x10044b940 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39ac>
100449f64: f8401009    	ldur	x9, [x0, #0x1]
100449f68: f9017be9    	str	x9, [sp, #0x2f0]
100449f6c: f9400409    	ldr	x9, [x0, #0x8]
100449f70: f809f169    	stur	x9, [x11, #0x9f]
100449f74: 14000675    	b	0x10044b948 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39b4>
100449f78: a95003e3    	ldp	x3, x0, [sp, #0x100]
100449f7c: f9408fe1    	ldr	x1, [sp, #0x118]
100449f80: 52800022    	mov	w2, #0x1                ; =1
100449f84: 9400217d    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449f88: f9408fe4    	ldr	x4, [sp, #0x118]
100449f8c: f94087e3    	ldr	x3, [sp, #0x108]
100449f90: 910963eb    	add	x11, sp, #0x258
100449f94: b400b1e0    	cbz	x0, 0x10044b5d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x363c>
100449f98: 39400008    	ldrb	w8, [x0]
100449f9c: 71001d1f    	cmp	w8, #0x7
100449fa0: 5400b188    	b.hi	0x10044b5d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x363c>
100449fa4: 52800029    	mov	w9, #0x1                ; =1
100449fa8: 1ac82129    	lsl	w9, w9, w8
100449fac: 5280138a    	mov	w10, #0x9c              ; =156
100449fb0: 6a0a013f    	tst	w9, w10
100449fb4: 5400af40    	b.eq	0x10044b59c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3608>
100449fb8: f8401009    	ldur	x9, [x0, #0x1]
100449fbc: f9017be9    	str	x9, [sp, #0x2f0]
100449fc0: f9400409    	ldr	x9, [x0, #0x8]
100449fc4: f809f169    	stur	x9, [x11, #0x9f]
100449fc8: 14000577    	b	0x10044b5a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3610>
100449fcc: aa0a03e1    	mov	x1, x10
100449fd0: 52800002    	mov	w2, #0x0                ; =0
100449fd4: aa1303e3    	mov	x3, x19
100449fd8: 94002168    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449fdc: f9408fe4    	ldr	x4, [sp, #0x118]
100449fe0: f94087e3    	ldr	x3, [sp, #0x108]
100449fe4: 910963eb    	add	x11, sp, #0x258
100449fe8: b400b900    	cbz	x0, 0x10044b708 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3774>
100449fec: 39400008    	ldrb	w8, [x0]
100449ff0: 71001d1f    	cmp	w8, #0x7
100449ff4: 5400b8a8    	b.hi	0x10044b708 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3774>
100449ff8: 52800029    	mov	w9, #0x1                ; =1
100449ffc: 1ac82129    	lsl	w9, w9, w8
10044a000: 5280138a    	mov	w10, #0x9c              ; =156
10044a004: 6a0a013f    	tst	w9, w10
10044a008: 5400b660    	b.eq	0x10044b6d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3740>
10044a00c: f8401009    	ldur	x9, [x0, #0x1]
10044a010: f9017be9    	str	x9, [sp, #0x2f0]
10044a014: f9400409    	ldr	x9, [x0, #0x8]
10044a018: f809f169    	stur	x9, [x11, #0x9f]
10044a01c: 140005b0    	b	0x10044b6dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3748>
10044a020: a95003e3    	ldp	x3, x0, [sp, #0x100]
10044a024: f9408fe1    	ldr	x1, [sp, #0x118]
10044a028: 52800022    	mov	w2, #0x1                ; =1
10044a02c: 94002153    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044a030: f9408fe4    	ldr	x4, [sp, #0x118]
10044a034: f94087e3    	ldr	x3, [sp, #0x108]
10044a038: 910963eb    	add	x11, sp, #0x258
10044a03c: b400bf00    	cbz	x0, 0x10044b81c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3888>
10044a040: 39400008    	ldrb	w8, [x0]
10044a044: 71001d1f    	cmp	w8, #0x7
10044a048: 5400bea8    	b.hi	0x10044b81c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3888>
10044a04c: 52800029    	mov	w9, #0x1                ; =1
10044a050: 1ac82129    	lsl	w9, w9, w8
10044a054: 5280138a    	mov	w10, #0x9c              ; =156
10044a058: 6a0a013f    	tst	w9, w10
10044a05c: 5400bc60    	b.eq	0x10044b7e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3854>
10044a060: f8401009    	ldur	x9, [x0, #0x1]
10044a064: f9017be9    	str	x9, [sp, #0x2f0]
10044a068: f9400409    	ldr	x9, [x0, #0x8]
10044a06c: f809f169    	stur	x9, [x11, #0x9f]
10044a070: 140005e0    	b	0x10044b7f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x385c>
10044a074: 7100097f    	cmp	w11, #0x2
10044a078: 54004882    	b.hs	0x10044a988 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x29f4>
10044a07c: 71025b7f    	cmp	w27, #0x96
10044a080: 54015a81    	b.ne	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044a084: 14000485    	b	0x10044b298 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3304>
10044a088: f1000d9f    	cmp	x12, #0x3
10044a08c: 540229c1    	b.ne	0x10044e5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6630>
10044a090: 5280006b    	mov	w11, #0x3               ; =3
10044a094: 1400006e    	b	0x10044a24c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22b8>
10044a098: f100091f    	cmp	x8, #0x2
10044a09c: 54024e23    	b.lo	0x10044ea60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6acc>
10044a0a0: 8b0a010b    	add	x11, x8, x10
10044a0a4: d1000976    	sub	x22, x11, #0x2
10044a0a8: eb1c02df    	cmp	x22, x28
10044a0ac: 54025c62    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044a0b0: d37ceecb    	lsl	x11, x22, #4
10044a0b4: 386b692b    	ldrb	w11, [x9, x11]
10044a0b8: 5100296b    	sub	w11, w11, #0xa
10044a0bc: 7100157f    	cmp	w11, #0x5
10044a0c0: 5401f2e3    	b.lo	0x10044df1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f88>
10044a0c4: d1000508    	sub	x8, x8, #0x1
10044a0c8: 8b080156    	add	x22, x10, x8
10044a0cc: eb1c02df    	cmp	x22, x28
10044a0d0: 54025b42    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044a0d4: 8b161129    	add	x9, x9, x22, lsl #4
10044a0d8: 39400139    	ldrb	w25, [x9]
10044a0dc: 51002b2a    	sub	w10, w25, #0xa
10044a0e0: 7100155f    	cmp	w10, #0x5
10044a0e4: 5401f1c3    	b.lo	0x10044df1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f88>
10044a0e8: f9408fea    	ldr	x10, [sp, #0x118]
10044a0ec: f9002148    	str	x8, [x10, #0x40]
10044a0f0: b840112a    	ldur	w10, [x9, #0x1]
10044a0f4: b81103aa    	stur	w10, [x29, #-0xf0]
10044a0f8: b940052a    	ldr	w10, [x9, #0x4]
10044a0fc: 910963eb    	add	x11, sp, #0x258
10044a100: b80fb16a    	stur	w10, [x11, #0xfb]
10044a104: f9400533    	ldr	x19, [x9, #0x8]
10044a108: 528001ca    	mov	w10, #0xe               ; =14
10044a10c: 3900012a    	strb	w10, [x9]
10044a110: f94087eb    	ldr	x11, [sp, #0x108]
10044a114: b4024a68    	cbz	x8, 0x10044ea60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6acc>
10044a118: f940097c    	ldr	x28, [x11, #0x10]
10044a11c: f9407be9    	ldr	x9, [sp, #0xf0]
10044a120: f940012a    	ldr	x10, [x9]
10044a124: d1000509    	sub	x9, x8, #0x1
10044a128: 8b090156    	add	x22, x10, x9
10044a12c: eb1c02df    	cmp	x22, x28
10044a130: 54025842    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044a134: f9400568    	ldr	x8, [x11, #0x8]
10044a138: 8b161108    	add	x8, x8, x22, lsl #4
10044a13c: 39400118    	ldrb	w24, [x8]
10044a140: 51002b0a    	sub	w10, w24, #0xa
10044a144: 7100115f    	cmp	w10, #0x4
10044a148: 5401eea9    	b.ls	0x10044df1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f88>
10044a14c: b90103ec    	str	w12, [sp, #0x100]
10044a150: f9408fe2    	ldr	x2, [sp, #0x118]
10044a154: f9002049    	str	x9, [x2, #0x40]
10044a158: b8401109    	ldur	w9, [x8, #0x1]
10044a15c: b81303a9    	stur	w9, [x29, #-0xd0]
10044a160: b9400509    	ldr	w9, [x8, #0x4]
10044a164: d10343aa    	sub	x10, x29, #0xd0
10044a168: b8003149    	stur	w9, [x10, #0x3]
10044a16c: f940051b    	ldr	x27, [x8, #0x8]
10044a170: 528001c9    	mov	w9, #0xe                ; =14
10044a174: 39000109    	strb	w9, [x8]
10044a178: f94087e8    	ldr	x8, [sp, #0x108]
10044a17c: a940f114    	ldp	x20, x28, [x8, #0x8]
10044a180: aa1403e0    	mov	x0, x20
10044a184: aa1c03e1    	mov	x1, x28
10044a188: 97f4cf69    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044a18c: aa0103f6    	mov	x22, x1
10044a190: 36004800    	tbz	w0, #0x0, 0x10044aa90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2afc>
10044a194: 910963ea    	add	x10, sp, #0x258
10044a198: b4004996    	cbz	x22, 0x10044aac8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2b34>
10044a19c: 14001254    	b	0x10044eaec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b58>
10044a1a0: 52800002    	mov	w2, #0x0                ; =0
10044a1a4: f94083e3    	ldr	x3, [sp, #0x100]
10044a1a8: 940020f4    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044a1ac: f9408fe4    	ldr	x4, [sp, #0x118]
10044a1b0: f94087e3    	ldr	x3, [sp, #0x108]
10044a1b4: 910963eb    	add	x11, sp, #0x258
10044a1b8: b400c780    	cbz	x0, 0x10044baa8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b14>
10044a1bc: 39400008    	ldrb	w8, [x0]
10044a1c0: 71001d1f    	cmp	w8, #0x7
10044a1c4: 5400c728    	b.hi	0x10044baa8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b14>
10044a1c8: 52800029    	mov	w9, #0x1                ; =1
10044a1cc: 1ac82129    	lsl	w9, w9, w8
10044a1d0: 5280138a    	mov	w10, #0x9c              ; =156
10044a1d4: 6a0a013f    	tst	w9, w10
10044a1d8: 5400c4e0    	b.eq	0x10044ba74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ae0>
10044a1dc: f8401009    	ldur	x9, [x0, #0x1]
10044a1e0: f9017be9    	str	x9, [sp, #0x2f0]
10044a1e4: f9400409    	ldr	x9, [x0, #0x8]
10044a1e8: f809f169    	stur	x9, [x11, #0x9f]
10044a1ec: 14000624    	b	0x10044ba7c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ae8>
10044a1f0: 52800002    	mov	w2, #0x0                ; =0
10044a1f4: f94083e3    	ldr	x3, [sp, #0x100]
10044a1f8: 940020e0    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044a1fc: f9408fe4    	ldr	x4, [sp, #0x118]
10044a200: f94087e3    	ldr	x3, [sp, #0x108]
10044a204: 910963eb    	add	x11, sp, #0x258
10044a208: b400cea0    	cbz	x0, 0x10044bbdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c48>
10044a20c: 39400008    	ldrb	w8, [x0]
10044a210: 71001d1f    	cmp	w8, #0x7
10044a214: 5400ce48    	b.hi	0x10044bbdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c48>
10044a218: 52800029    	mov	w9, #0x1                ; =1
10044a21c: 1ac82129    	lsl	w9, w9, w8
10044a220: 5280138a    	mov	w10, #0x9c              ; =156
10044a224: 6a0a013f    	tst	w9, w10
10044a228: 5400cc00    	b.eq	0x10044bba8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c14>
10044a22c: f8401009    	ldur	x9, [x0, #0x1]
10044a230: f9017be9    	str	x9, [sp, #0x2f0]
10044a234: f9400409    	ldr	x9, [x0, #0x8]
10044a238: f809f169    	stur	x9, [x11, #0x9f]
10044a23c: 1400065d    	b	0x10044bbb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c1c>
10044a240: 51000d6b    	sub	w11, w11, #0x3
10044a244: 7100097f    	cmp	w11, #0x2
10044a248: 1a9f27eb    	cset	w11, lo
10044a24c: 12001d4a    	and	w10, w10, #0xff
10044a250: 7100095f    	cmp	w10, #0x2
10044a254: 540000c2    	b.hs	0x10044a26c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22d8>
10044a258: 7100057f    	cmp	w11, #0x1
10044a25c: 540000c0    	b.eq	0x10044a274 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22e0>
10044a260: 71000d7f    	cmp	w11, #0x3
10044a264: 54000201    	b.ne	0x10044a2a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2310>
10044a268: 140010e6    	b	0x10044e600 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x666c>
10044a26c: 7100057f    	cmp	w11, #0x1
10044a270: 540001a1    	b.ne	0x10044a2a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2310>
10044a274: f94021ca    	ldr	x10, [x14, #0x40]
10044a278: b400016a    	cbz	x10, 0x10044a2a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2310>
10044a27c: d100054a    	sub	x10, x10, #0x1
10044a280: 8b0a012c    	add	x12, x9, x10
10044a284: eb1c019f    	cmp	x12, x28
10044a288: 540000e2    	b.hs	0x10044a2a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2310>
10044a28c: d37ced8b    	lsl	x11, x12, #4
10044a290: 386b690b    	ldrb	w11, [x8, x11]
10044a294: 7100117f    	cmp	w11, #0x4
10044a298: 54015dc0    	b.eq	0x10044ce50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ebc>
10044a29c: 71000d7f    	cmp	w11, #0x3
10044a2a0: 54015d00    	b.eq	0x10044ce40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4eac>
10044a2a4: f94097ea    	ldr	x10, [sp, #0x128]
10044a2a8: f940114a    	ldr	x10, [x10, #0x20]
10044a2ac: b401d3ea    	cbz	x10, 0x10044dd28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d94>
10044a2b0: 71009b7f    	cmp	w27, #0x26
10044a2b4: 54000060    	b.eq	0x10044a2c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x232c>
10044a2b8: 7100b37f    	cmp	w27, #0x2c
10044a2bc: 54000301    	b.ne	0x10044a31c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2388>
10044a2c0: f94021ca    	ldr	x10, [x14, #0x40]
10044a2c4: b4023a8a    	cbz	x10, 0x10044ea14 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a80>
10044a2c8: 8b090149    	add	x9, x10, x9
10044a2cc: d1000536    	sub	x22, x9, #0x1
10044a2d0: eb1c02df    	cmp	x22, x28
10044a2d4: 54024b22    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044a2d8: 8b161102    	add	x2, x8, x22, lsl #4
10044a2dc: 39400048    	ldrb	w8, [x2]
10044a2e0: 51002909    	sub	w9, w8, #0xa
10044a2e4: 7100113f    	cmp	w9, #0x4
10044a2e8: 5401ee09    	b.ls	0x10044e0a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6114>
10044a2ec: 71001d1f    	cmp	w8, #0x7
10044a2f0: 54006788    	b.hi	0x10044afe0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x304c>
10044a2f4: 52800029    	mov	w9, #0x1                ; =1
10044a2f8: 1ac82129    	lsl	w9, w9, w8
10044a2fc: 5280138a    	mov	w10, #0x9c              ; =156
10044a300: 6a0a013f    	tst	w9, w10
10044a304: 54001520    	b.eq	0x10044a5a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2614>
10044a308: f8401049    	ldur	x9, [x2, #0x1]
10044a30c: f81103a9    	stur	x9, [x29, #-0xf0]
10044a310: f9400449    	ldr	x9, [x2, #0x8]
10044a314: f80ff1a9    	stur	x9, [x13, #0xff]
10044a318: 140000a6    	b	0x10044a5b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x261c>
10044a31c: f94021ca    	ldr	x10, [x14, #0x40]
10044a320: b4023c8a    	cbz	x10, 0x10044eab0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b1c>
10044a324: d100054b    	sub	x11, x10, #0x1
10044a328: 8b0b0136    	add	x22, x9, x11
10044a32c: eb1c02df    	cmp	x22, x28
10044a330: 54024842    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044a334: 8b16110a    	add	x10, x8, x22, lsl #4
10044a338: 39400148    	ldrb	w8, [x10]
10044a33c: 51002909    	sub	w9, w8, #0xa
10044a340: 7100113f    	cmp	w9, #0x4
10044a344: 54020449    	b.ls	0x10044e3cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6438>
10044a348: f90021cb    	str	x11, [x14, #0x40]
10044a34c: b8401149    	ldur	w9, [x10, #0x1]
10044a350: b902f3e9    	str	w9, [sp, #0x2f0]
10044a354: b9400549    	ldr	w9, [x10, #0x4]
10044a358: b809b1a9    	stur	w9, [x13, #0x9b]
10044a35c: f9400549    	ldr	x9, [x10, #0x8]
10044a360: 528001cb    	mov	w11, #0xe               ; =14
10044a364: 3900014b    	strb	w11, [x10]
10044a368: 1400009c    	b	0x10044a5d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2644>
10044a36c: f9402048    	ldr	x8, [x2, #0x40]
10044a370: b4023ae8    	cbz	x8, 0x10044eacc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b38>
10044a374: f940099c    	ldr	x28, [x12, #0x10]
10044a378: f9407be9    	ldr	x9, [sp, #0xf0]
10044a37c: f940012a    	ldr	x10, [x9]
10044a380: d1000509    	sub	x9, x8, #0x1
10044a384: 8b090156    	add	x22, x10, x9
10044a388: eb1c02df    	cmp	x22, x28
10044a38c: 54024562    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044a390: f9400588    	ldr	x8, [x12, #0x8]
10044a394: 8b16110a    	add	x10, x8, x22, lsl #4
10044a398: 39400148    	ldrb	w8, [x10]
10044a39c: 5100290b    	sub	w11, w8, #0xa
10044a3a0: 7100117f    	cmp	w11, #0x4
10044a3a4: 54021629    	b.ls	0x10044e668 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66d4>
10044a3a8: f9002049    	str	x9, [x2, #0x40]
10044a3ac: b8401149    	ldur	w9, [x10, #0x1]
10044a3b0: b902f3e9    	str	w9, [sp, #0x2f0]
10044a3b4: b9400549    	ldr	w9, [x10, #0x4]
10044a3b8: b809b1c9    	stur	w9, [x14, #0x9b]
10044a3bc: f9400549    	ldr	x9, [x10, #0x8]
10044a3c0: 528001cb    	mov	w11, #0xe               ; =14
10044a3c4: 3900014b    	strb	w11, [x10]
10044a3c8: 1400032e    	b	0x10044b080 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30ec>
10044a3cc: 5280002c    	mov	w12, #0x1               ; =1
10044a3d0: b8767b09    	ldr	w9, [x24, x22, lsl #2]
10044a3d4: 7210013f    	tst	w9, #0x10000
10044a3d8: f9406bed    	ldr	x13, [sp, #0xd0]
10044a3dc: f94073ef    	ldr	x15, [sp, #0xe0]
10044a3e0: 9a8d01ed    	csel	x13, x15, x13, eq
10044a3e4: f9407bee    	ldr	x14, [sp, #0xf0]
10044a3e8: 9a8f01ce    	csel	x14, x14, x15, eq
10044a3ec: f94001c1    	ldr	x1, [x14]
10044a3f0: f94001b6    	ldr	x22, [x13]
10044a3f4: eb16002d    	subs	x13, x1, x22
10044a3f8: 54022e63    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
10044a3fc: eb0a003f    	cmp	x1, x10
10044a400: 540235e8    	b.hi	0x10044eabc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b28>
10044a404: 92403d2a    	and	x10, x9, #0xffff
10044a408: eb0a01bf    	cmp	x13, x10
10044a40c: 54011629    	b.ls	0x10044c6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x473c>
10044a410: 8b16116b    	add	x11, x11, x22, lsl #4
10044a414: 8b0a116a    	add	x10, x11, x10, lsl #4
10044a418: 3940014b    	ldrb	w11, [x10]
10044a41c: 7100297f    	cmp	w11, #0xa
10044a420: 54011582    	b.hs	0x10044c6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x473c>
10044a424: 71000d7f    	cmp	w11, #0x3
10044a428: 54011520    	b.eq	0x10044c6cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4738>
10044a42c: 7100117f    	cmp	w11, #0x4
10044a430: 54011501    	b.ne	0x10044c6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x473c>
10044a434: 370114ec    	tbnz	w12, #0x0, 0x10044c6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x473c>
10044a438: fd400541    	ldr	d1, [x10, #0x8]
10044a43c: 14000cb5    	b	0x10044d710 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x577c>
10044a440: 5280002b    	mov	w11, #0x1               ; =1
10044a444: b8767b0c    	ldr	w12, [x24, x22, lsl #2]
10044a448: 7210019f    	tst	w12, #0x10000
10044a44c: f9406bed    	ldr	x13, [sp, #0xd0]
10044a450: f94073ef    	ldr	x15, [sp, #0xe0]
10044a454: 9a8d01ed    	csel	x13, x15, x13, eq
10044a458: f9407bee    	ldr	x14, [sp, #0xf0]
10044a45c: 9a8f01ce    	csel	x14, x14, x15, eq
10044a460: f94001c1    	ldr	x1, [x14]
10044a464: f94001b6    	ldr	x22, [x13]
10044a468: eb16002d    	subs	x13, x1, x22
10044a46c: 54022ac3    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
10044a470: eb09003f    	cmp	x1, x9
10044a474: 540230a8    	b.hi	0x10044ea88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6af4>
10044a478: 92403d89    	and	x9, x12, #0xffff
10044a47c: eb0901bf    	cmp	x13, x9
10044a480: 540121c9    	b.ls	0x10044c8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4924>
10044a484: 8b16114a    	add	x10, x10, x22, lsl #4
10044a488: 8b091149    	add	x9, x10, x9, lsl #4
10044a48c: 3940012a    	ldrb	w10, [x9]
10044a490: 7100295f    	cmp	w10, #0xa
10044a494: 54012122    	b.hs	0x10044c8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4924>
10044a498: 71000d5f    	cmp	w10, #0x3
10044a49c: 540120c0    	b.eq	0x10044c8b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4920>
10044a4a0: 7100115f    	cmp	w10, #0x4
10044a4a4: 540120a1    	b.ne	0x10044c8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4924>
10044a4a8: 3701208b    	tbnz	w11, #0x0, 0x10044c8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4924>
10044a4ac: fd400521    	ldr	d1, [x9, #0x8]
10044a4b0: 14000cac    	b	0x10044d760 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57cc>
10044a4b4: 1e620160    	scvtf	d0, w11
10044a4b8: 1e61400a    	fneg	d10, d0
10044a4bc: 1e780153    	fcvtzs	w19, d10
10044a4c0: 1e620260    	scvtf	d0, w19
10044a4c4: 1e602140    	fcmp	d10, d0
10044a4c8: 540000a1    	b.ne	0x10044a4dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2548>
10044a4cc: 5280000a    	mov	w10, #0x0               ; =0
10044a4d0: 350003cb    	cbnz	w11, 0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
10044a4d4: 9e66014b    	fmov	x11, d10
10044a4d8: b6f8038b    	tbz	x11, #0x3f, 0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
10044a4dc: 5280002a    	mov	w10, #0x1               ; =1
10044a4e0: 1400001a    	b	0x10044a548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
10044a4e4: 5280000a    	mov	w10, #0x0               ; =0
10044a4e8: 1e65c000    	frintz	d0, d0
10044a4ec: d2e7be0b    	mov	x11, #0x3df0000000000000 ; =4463067230724161536
10044a4f0: 9e670161    	fmov	d1, x11
10044a4f4: 1e610801    	fmul	d1, d0, d1
10044a4f8: 1e65c021    	frintz	d1, d1
10044a4fc: d2f83e0b    	mov	x11, #-0x3e10000000000000 ; =-4472074429978902528
10044a500: 9e670162    	fmov	d2, x11
10044a504: 1f420021    	fmadd	d1, d1, d2, d0
10044a508: 6f07e7e3    	movi.2d	v3, #0xffffffffffffffff
10044a50c: 6ee0f863    	fneg.2d	v3, v3
10044a510: 6ea31c20    	bit.16b	v0, v1, v3
10044a514: d2e83e0b    	mov	x11, #0x41f0000000000000 ; =4751297606875873280
10044a518: 9e670161    	fmov	d1, x11
10044a51c: 1e612801    	fadd	d1, d0, d1
10044a520: 1e602008    	fcmp	d0, #0.0
10044a524: 1e604c20    	fcsel	d0, d1, d0, mi
10044a528: 1e622801    	fadd	d1, d0, d2
10044a52c: 1e78002b    	fcvtzs	w11, d1
10044a530: d2e83c0c    	mov	x12, #0x41e0000000000000 ; =4746794007248502784
10044a534: 9e670181    	fmov	d1, x12
10044a538: 1e78000c    	fcvtzs	w12, d0
10044a53c: 1e612000    	fcmp	d0, d1
10044a540: 1a8bb18b    	csel	w11, w12, w11, lt
10044a544: 2a2b03f3    	mvn	w19, w11
10044a548: f9002048    	str	x8, [x2, #0x40]
10044a54c: 528001c8    	mov	w8, #0xe                ; =14
10044a550: 39000128    	strb	w8, [x9]
10044a554: a940f234    	ldp	x20, x28, [x17, #0x8]
10044a558: 7100015f    	cmp	w10, #0x0
10044a55c: 52800068    	mov	w8, #0x3                ; =3
10044a560: 1a880518    	cinc	w24, w8, ne
10044a564: aa1403e0    	mov	x0, x20
10044a568: aa1c03e1    	mov	x1, x28
10044a56c: 97f4ce70    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044a570: aa0103f6    	mov	x22, x1
10044a574: 36000060    	tbz	w0, #0x0, 0x10044a580 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25ec>
10044a578: b40132d6    	cbz	x22, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044a57c: 1400115c    	b	0x10044eaec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b58>
10044a580: eb16039f    	cmp	x28, x22
10044a584: 54024649    	b.ls	0x10044ee4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6eb8>
10044a588: 8b161288    	add	x8, x20, x22, lsl #4
10044a58c: 39000118    	strb	w24, [x8]
10044a590: b9000513    	str	w19, [x8, #0x4]
10044a594: fd00050a    	str	d10, [x8, #0x8]
10044a598: f9408fe9    	ldr	x9, [sp, #0x118]
10044a59c: f9402128    	ldr	x8, [x9, #0x40]
10044a5a0: 91000508    	add	x8, x8, #0x1
10044a5a4: 17fff7b0    	b	0x100448464 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d0>
10044a5a8: 7200053f    	tst	w9, #0x3
10044a5ac: 540051a0    	b.eq	0x10044afe0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x304c>
10044a5b0: f85103a9    	ldur	x9, [x29, #-0xf0]
10044a5b4: f94077ea    	ldr	x10, [sp, #0xe8]
10044a5b8: f9000149    	str	x9, [x10]
10044a5bc: f84ff1a9    	ldur	x9, [x13, #0xff]
10044a5c0: f8007149    	stur	x9, [x10, #0x7]
10044a5c4: b9400149    	ldr	w9, [x10]
10044a5c8: b902f3e9    	str	w9, [sp, #0x2f0]
10044a5cc: b8403149    	ldur	w9, [x10, #0x3]
10044a5d0: b809b1a9    	stur	w9, [x13, #0x9b]
10044a5d4: f85383a9    	ldur	x9, [x29, #-0xc8]
10044a5d8: f94073ea    	ldr	x10, [sp, #0xe0]
10044a5dc: f940014a    	ldr	x10, [x10]
10044a5e0: f9407beb    	ldr	x11, [sp, #0xf0]
10044a5e4: f940016b    	ldr	x11, [x11]
10044a5e8: eb0a016b    	subs	x11, x11, x10
10044a5ec: 9a8b33eb    	csel	x11, xzr, x11, lo
10044a5f0: eb13017f    	cmp	x11, x19
10044a5f4: 5401ca29    	b.ls	0x10044df38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fa4>
10044a5f8: f94009fc    	ldr	x28, [x15, #0x10]
10044a5fc: 8b130156    	add	x22, x10, x19
10044a600: eb1c02df    	cmp	x22, x28
10044a604: 54024142    	b.hs	0x10044ee2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6e98>
10044a608: f94005ea    	ldr	x10, [x15, #0x8]
10044a60c: 8b16114a    	add	x10, x10, x22, lsl #4
10044a610: 3940014b    	ldrb	w11, [x10]
10044a614: 7100397f    	cmp	w11, #0xe
10044a618: 5401cde0    	b.eq	0x10044dfd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6040>
10044a61c: b840114c    	ldur	w12, [x10, #0x1]
10044a620: f94063ee    	ldr	x14, [sp, #0xc0]
10044a624: b90001cc    	str	w12, [x14]
10044a628: b940054c    	ldr	w12, [x10, #0x4]
10044a62c: b80031cc    	stur	w12, [x14, #0x3]
10044a630: f940054c    	ldr	x12, [x10, #0x8]
10044a634: 39000148    	strb	w8, [x10]
10044a638: b942f3e8    	ldr	w8, [sp, #0x2f0]
10044a63c: b8001148    	stur	w8, [x10, #0x1]
10044a640: b849b1a8    	ldur	w8, [x13, #0x9b]
10044a644: b9000548    	str	w8, [x10, #0x4]
10044a648: f9000549    	str	x9, [x10, #0x8]
10044a64c: 3906e3eb    	strb	w11, [sp, #0x1b8]
10044a650: f900e3ec    	str	x12, [sp, #0x1c0]
10044a654: a9522beb    	ldp	x11, x10, [sp, #0x120]
10044a658: f9401149    	ldr	x9, [x10, #0x20]
10044a65c: f9400208    	ldr	x8, [x16]
10044a660: f900154b    	str	x11, [x10, #0x28]
10044a664: f940150a    	ldr	x10, [x8, #0x28]
10044a668: b502202a    	cbnz	x10, 0x10044ea6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ad8>
10044a66c: 9280000a    	mov	x10, #-0x1              ; =-1
10044a670: f900150a    	str	x10, [x8, #0x28]
10044a674: 3947210a    	ldrb	w10, [x8, #0x1c8]
10044a678: 7100095f    	cmp	w10, #0x2
10044a67c: 540000e1    	b.ne	0x10044a698 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2704>
10044a680: f940e10a    	ldr	x10, [x8, #0x1c0]
10044a684: b401e60a    	cbz	x10, 0x10044e344 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63b0>
10044a688: f940dd0b    	ldr	x11, [x8, #0x1b8]
10044a68c: 8b0a196a    	add	x10, x11, x10, lsl #6
10044a690: d101014a    	sub	x10, x10, #0x40
10044a694: 14000002    	b	0x10044a69c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2708>
10044a698: 9107210a    	add	x10, x8, #0x1c8
10044a69c: f940114b    	ldr	x11, [x10, #0x20]
10044a6a0: eb09017f    	cmp	x11, x9
10044a6a4: 5401cce1    	b.ne	0x10044e040 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60ac>
10044a6a8: 39400149    	ldrb	w9, [x10]
10044a6ac: 3701cd29    	tbnz	w9, #0x0, 0x10044e050 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60bc>
10044a6b0: b9401149    	ldr	w9, [x10, #0x10]
10044a6b4: 7100053f    	cmp	w9, #0x1
10044a6b8: 540000e1    	b.ne	0x10044a6d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2740>
10044a6bc: f9400d49    	ldr	x9, [x10, #0x18]
10044a6c0: f94093eb    	ldr	x11, [sp, #0x120]
10044a6c4: eb0b013f    	cmp	x9, x11
10044a6c8: 54000061    	b.ne	0x10044a6d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2740>
10044a6cc: d2800009    	mov	x9, #0x0                ; =0
10044a6d0: 14000007    	b	0x10044a6ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2758>
10044a6d4: 52800029    	mov	w9, #0x1                ; =1
10044a6d8: f9000949    	str	x9, [x10, #0x10]
10044a6dc: f94093e9    	ldr	x9, [sp, #0x120]
10044a6e0: f9000d49    	str	x9, [x10, #0x18]
10044a6e4: f9401509    	ldr	x9, [x8, #0x28]
10044a6e8: 91000529    	add	x9, x9, #0x1
10044a6ec: f9001509    	str	x9, [x8, #0x28]
10044a6f0: f9400200    	ldr	x0, [x16]
10044a6f4: 9106e3e1    	add	x1, sp, #0x1b8
10044a6f8: 97f45fc9    	bl	0x10016261c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
10044a6fc: 14000934    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044a700: 7103377f    	cmp	w27, #0xcd
10044a704: 54002b41    	b.ne	0x10044ac6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2cd8>
10044a708: f9400196    	ldr	x22, [x12]
10044a70c: 14000162    	b	0x10044ac94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2d00>
10044a710: b9401549    	ldr	w9, [x10, #0x14]
10044a714: b9003be9    	str	w9, [sp, #0x38]
10044a718: 37fa1469    	tbnz	w9, #0x1f, 0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044a71c: f9407fe9    	ldr	x9, [sp, #0xf8]
10044a720: f9400129    	ldr	x9, [x9]
10044a724: f90083e9    	str	x9, [sp, #0x100]
10044a728: 7100251f    	cmp	w8, #0x9
10044a72c: f94083e1    	ldr	x1, [sp, #0x100]
10044a730: 540213a1    	b.ne	0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044a734: d10343a0    	sub	x0, x29, #0xd0
10044a738: f94023e2    	ldr	x2, [sp, #0x40]
10044a73c: 94002aa1    	bl	0x1004551c0 <__ZN13quickjs_oxide6engine4heap14slot_ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$36slot_value_release_readiness_jsvalue17h2038e983b6e2b9bfE>
10044a740: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044a744: 71002d1f    	cmp	w8, #0xb
10044a748: 5401cbc1    	b.ne	0x10044e0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
10044a74c: 385313a9    	ldurb	w9, [x29, #-0xcf]
10044a750: 3501cb89    	cbnz	w9, 0x10044e0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
10044a754: f94083e9    	ldr	x9, [sp, #0x100]
10044a758: f9401528    	ldr	x8, [x9, #0x28]
10044a75c: b5021248    	cbnz	x8, 0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044a760: 92800008    	mov	x8, #-0x1               ; =-1
10044a764: f9001528    	str	x8, [x9, #0x28]
10044a768: f94023e8    	ldr	x8, [sp, #0x40]
10044a76c: 2940a10a    	ldp	w10, w8, [x8, #0x4]
10044a770: a94fe521    	ldp	x1, x25, [x9, #0xf8]
10044a774: b90027e8    	str	w8, [sp, #0x24]
10044a778: b9031be8    	str	w8, [sp, #0x318]
10044a77c: aa0a03f8    	mov	x24, x10
10044a780: b90317ea    	str	w10, [sp, #0x314]
10044a784: b90313ff    	str	wzr, [sp, #0x310]
10044a788: d10343a0    	sub	x0, x29, #0xd0
10044a78c: 910c43e3    	add	x3, sp, #0x310
10044a790: aa0103f3    	mov	x19, x1
10044a794: aa1903e2    	mov	x2, x25
10044a798: 97ef9e77    	bl	0x100032174 <__ZN13quickjs_oxide6engine4heap14object_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$22validate_slot_identity17h9c24141d9f2cc0c4E>
10044a79c: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044a7a0: 7100191f    	cmp	w8, #0x6
10044a7a4: 54020f81    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a7a8: f85383b4    	ldur	x20, [x29, #-0xc8]
10044a7ac: f9000ff9    	str	x25, [sp, #0x18]
10044a7b0: eb19029f    	cmp	x20, x25
10044a7b4: 540237c2    	b.hs	0x10044eeac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f18>
10044a7b8: 52802308    	mov	w8, #0x118              ; =280
10044a7bc: 9b084e99    	madd	x25, x20, x8, x19
10044a7c0: 39400328    	ldrb	w8, [x25]
10044a7c4: 7100051f    	cmp	w8, #0x1
10044a7c8: 54020e61    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a7cc: aa1903e2    	mov	x2, x25
10044a7d0: f8408c48    	ldr	x8, [x2, #0x8]!
10044a7d4: f100051f    	cmp	x8, #0x1
10044a7d8: 54020de8    	b.hi	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a7dc: aa1303e1    	mov	x1, x19
10044a7e0: 3943b729    	ldrb	w9, [x25, #0xed]
10044a7e4: 39416328    	ldrb	w8, [x25, #0x58]
10044a7e8: 71000d3f    	cmp	w9, #0x3
10044a7ec: 540002a1    	b.ne	0x10044a840 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x28ac>
10044a7f0: 7100091f    	cmp	w8, #0x2
10044a7f4: 54000261    	b.ne	0x10044a840 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x28ac>
10044a7f8: f9403328    	ldr	x8, [x25, #0x60]
10044a7fc: d2f00009    	mov	x9, #-0x8000000000000000 ; =-9223372036854775808
10044a800: eb09011f    	cmp	x8, x9
10044a804: 54005101    	b.ne	0x10044b224 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3290>
10044a808: d10343a0    	sub	x0, x29, #0xd0
10044a80c: f94083e8    	ldr	x8, [sp, #0x100]
10044a810: 9100c101    	add	x1, x8, #0x30
10044a814: b9403be3    	ldr	w3, [sp, #0x38]
10044a818: 94002b30    	bl	0x1004554d8 <__ZN13quickjs_oxide6engine6object16ordinary_storage29materialized_array_own_number17h87c9af0567eda137E>
10044a81c: b85303a8    	ldur	w8, [x29, #-0xd0]
10044a820: 7100091f    	cmp	w8, #0x2
10044a824: 54020b80    	b.eq	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a828: 3600bc28    	tbz	w8, #0x0, 0x10044bfac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4018>
10044a82c: fc5383a0    	ldur	d0, [x29, #-0xc8]
10044a830: 52800088    	mov	w8, #0x4                ; =4
10044a834: 390b43e8    	strb	w8, [sp, #0x2d0]
10044a838: fd016fe0    	str	d0, [sp, #0x2d8]
10044a83c: 140005e0    	b	0x10044bfbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4028>
10044a840: 71000d1f    	cmp	w8, #0x3
10044a844: 54004721    	b.ne	0x10044b128 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3194>
10044a848: b9403be8    	ldr	w8, [sp, #0x38]
10044a84c: 37fa0a48    	tbnz	w8, #0x1f, 0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a850: d10343a0    	sub	x0, x29, #0xd0
10044a854: b9403be8    	ldr	w8, [sp, #0x38]
10044a858: 32010105    	orr	w5, w8, #0x80000000
10044a85c: f9400fe2    	ldr	x2, [sp, #0x18]
10044a860: aa1803e3    	mov	x3, x24
10044a864: b94027e4    	ldr	w4, [sp, #0x24]
10044a868: 97f08534    	bl	0x10006bd38 <__ZN13quickjs_oxide6engine6object16ordinary_storage6locate17heb1b0d901cfe2fdaE>
10044a86c: 385303a0    	ldurb	w0, [x29, #-0xd0]
10044a870: f85383b4    	ldur	x20, [x29, #-0xc8]
10044a874: 71002c1f    	cmp	w0, #0xb
10044a878: 540206a1    	b.ne	0x10044e94c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x69b8>
10044a87c: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044a880: 121f1908    	and	w8, w8, #0xfe
10044a884: 7100091f    	cmp	w8, #0x2
10044a888: 54020860    	b.eq	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a88c: 91008320    	add	x0, x25, #0x20
10044a890: 97f08671    	bl	0x10006c254 <__ZN94_$LT$quickjs_oxide..engine..heap..object_records..Slots$u20$as$u20$core..ops..deref..Deref$GT$5deref17ha37b741159f3cf0eE>
10044a894: eb01029f    	cmp	x20, x1
10044a898: 54023a02    	b.hs	0x10044efd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7044>
10044a89c: 52800308    	mov	w8, #0x18               ; =24
10044a8a0: 9b080288    	madd	x8, x20, x8, x0
10044a8a4: b9400109    	ldr	w9, [x8]
10044a8a8: 3400c1c9    	cbz	w9, 0x10044c0e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x414c>
10044a8ac: 7100053f    	cmp	w9, #0x1
10044a8b0: 54020721    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a8b4: 29409103    	ldp	w3, w4, [x8, #0x4]
10044a8b8: f94083e8    	ldr	x8, [sp, #0x100]
10044a8bc: a94f8901    	ldp	x1, x2, [x8, #0xf8]
10044a8c0: d10343a0    	sub	x0, x29, #0xd0
10044a8c4: 97f12113    	bl	0x100092d10 <__ZN13quickjs_oxide6engine4heap15binding_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$7var_ref17hdb209f11d90c7491E>
10044a8c8: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044a8cc: 7100191f    	cmp	w8, #0x6
10044a8d0: 54020621    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044a8d4: f85383a1    	ldur	x1, [x29, #-0xc8]
10044a8d8: 14000603    	b	0x10044c0e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4150>
10044a8dc: f9408fe9    	ldr	x9, [sp, #0x118]
10044a8e0: f9401d29    	ldr	x9, [x9, #0x38]
10044a8e4: eb080128    	subs	x8, x9, x8
10044a8e8: 9a8833e8    	csel	x8, xzr, x8, lo
10044a8ec: cb130108    	sub	x8, x8, x19
10044a8f0: f100091f    	cmp	x8, #0x2
10044a8f4: 54004a88    	b.hi	0x10044b244 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x32b0>
10044a8f8: d00013b3    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044a8fc: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a900: 52800574    	mov	w20, #0x2b              ; =43
10044a904: 52800560    	mov	w0, #0x2b               ; =43
10044a908: 9403348a    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044a90c: b4022ec0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044a910: 90000b08    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044a914: 91061508    	add	x8, x8, #0x185
10044a918: ad400500    	ldp	q0, q1, [x8]
10044a91c: ad000400    	stp	q0, q1, [x0]
10044a920: 3cc1b100    	ldur	q0, [x8, #0x1b]
10044a924: 3c81b000    	stur	q0, [x0, #0x1b]
10044a928: 528000a8    	mov	w8, #0x5                ; =5
10044a92c: 381783a8    	sturb	w8, [x29, #-0x88]
10044a930: 52800568    	mov	w8, #0x2b               ; =43
10044a934: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044a938: f81703bf    	stur	xzr, [x29, #-0x90]
10044a93c: f81583a8    	stur	x8, [x29, #-0xa8]
10044a940: f81303bf    	stur	xzr, [x29, #-0xd0]
10044a944: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a948: 52800a00    	mov	w0, #0x50               ; =80
10044a94c: 94033479    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044a950: b4021640    	cbz	x0, 0x10044ec18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c84>
10044a954: ad7a87a0    	ldp	q0, q1, [x29, #-0xb0]
10044a958: ad010400    	stp	q0, q1, [x0, #0x20]
10044a95c: 3cd703a0    	ldur	q0, [x29, #-0x90]
10044a960: 3d801000    	str	q0, [x0, #0x40]
10044a964: ad7983a1    	ldp	q1, q0, [x29, #-0xd0]
10044a968: ad000001    	stp	q1, q0, [x0]
10044a96c: b4011320    	cbz	x0, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044a970: 14000e7f    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044a974: 710001bf    	cmp	w13, #0x0
10044a978: 1a9f07e9    	cset	w9, ne
10044a97c: 7100091f    	cmp	w8, #0x2
10044a980: 1a890188    	csel	w8, w12, w9, eq
10044a984: 14000240    	b	0x10044b284 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x32f0>
10044a988: 7100111f    	cmp	w8, #0x4
10044a98c: 54004781    	b.ne	0x10044b27c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x32e8>
10044a990: 9e670120    	fmov	d0, x9
10044a994: 1e602008    	fcmp	d0, #0.0
10044a998: 54ff0da0    	b.eq	0x100448b4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xbb8>
10044a99c: 1e602000    	fcmp	d0, d0
10044a9a0: 1a9f67e8    	cset	w8, vc
10044a9a4: 14000238    	b	0x10044b284 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x32f0>
10044a9a8: b501ee89    	cbnz	x9, 0x10044e778 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67e4>
10044a9ac: 51000d08    	sub	w8, w8, #0x3
10044a9b0: 7100091f    	cmp	w8, #0x2
10044a9b4: 54ff2402    	b.hs	0x100448e34 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xea0>
10044a9b8: 7100bf7f    	cmp	w27, #0x2f
10044a9bc: 1a9f17e4    	cset	w4, eq
10044a9c0: aa1c03e1    	mov	x1, x28
10044a9c4: f94083e3    	ldr	x3, [sp, #0x100]
10044a9c8: 94002849    	bl	0x100454aec <__ZN13quickjs_oxide6engine2vm5stack6number61_$LT$impl$u20$quickjs_oxide..engine..vm..stack..SlotStore$GT$35store_proven_number_operand_current17h5730d3b1bb9520a0E>
10044a9cc: f9408fe2    	ldr	x2, [sp, #0x118]
10044a9d0: f94087ec    	ldr	x12, [sp, #0x108]
10044a9d4: f9407fed    	ldr	x13, [sp, #0xf8]
10044a9d8: 910963ee    	add	x14, sp, #0x258
10044a9dc: 34ff22c0    	cbz	w0, 0x100448e34 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xea0>
10044a9e0: 1400087c    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044a9e4: f94083e8    	ldr	x8, [sp, #0x100]
10044a9e8: 12003d13    	and	w19, w8, #0xffff
10044a9ec: f9407348    	ldr	x8, [x26, #0xe0]
10044a9f0: eb13011f    	cmp	x8, x19
10044a9f4: 54010ee9    	b.ls	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044a9f8: f9406f48    	ldr	x8, [x26, #0xd8]
10044a9fc: 3833691f    	strb	wzr, [x8, x19]
10044aa00: 14000874    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044aa04: 7100153f    	cmp	w9, #0x5
10044aa08: 5400466c    	b.gt	0x10044b2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3340>
10044aa0c: 7100113f    	cmp	w9, #0x4
10044aa10: 5400b3e0    	b.eq	0x10044c08c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40f8>
10044aa14: 7100153f    	cmp	w9, #0x5
10044aa18: 5401b6e1    	b.ne	0x10044e0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6160>
10044aa1c: fc40c100    	ldur	d0, [x8, #0xc]
10044aa20: d10343a8    	sub	x8, x29, #0xd0
10044aa24: fc004100    	stur	d0, [x8, #0x4]
10044aa28: 528000c8    	mov	w8, #0x6                ; =6
10044aa2c: 381303a8    	sturb	w8, [x29, #-0xd0]
10044aa30: f9407fe8    	ldr	x8, [sp, #0xf8]
10044aa34: f9400101    	ldr	x1, [x8]
10044aa38: d103c3a0    	sub	x0, x29, #0xf0
10044aa3c: d10343a2    	sub	x2, x29, #0xd0
10044aa40: 97ff7dbe    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044aa44: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044aa48: 7100291f    	cmp	w8, #0xa
10044aa4c: 5401b280    	b.eq	0x10044e09c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6108>
10044aa50: f94067ea    	ldr	x10, [sp, #0xc8]
10044aa54: b9400149    	ldr	w9, [x10]
10044aa58: f94037eb    	ldr	x11, [sp, #0x68]
10044aa5c: b9000169    	str	w9, [x11]
10044aa60: b8403149    	ldur	w9, [x10, #0x3]
10044aa64: b8003169    	stur	w9, [x11, #0x3]
10044aa68: f85183a9    	ldur	x9, [x29, #-0xe8]
10044aa6c: 3905a3e8    	strb	w8, [sp, #0x168]
10044aa70: f900bbe9    	str	x9, [sp, #0x170]
10044aa74: f9407fe8    	ldr	x8, [sp, #0xf8]
10044aa78: f9400102    	ldr	x2, [x8]
10044aa7c: 9105a3e3    	add	x3, sp, #0x168
10044aa80: f94087e0    	ldr	x0, [sp, #0x108]
10044aa84: f9408fe1    	ldr	x1, [sp, #0x118]
10044aa88: 94001f02    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044aa8c: 14000850    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044aa90: eb16039f    	cmp	x28, x22
10044aa94: 54021dc9    	b.ls	0x10044ee4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6eb8>
10044aa98: 8b161288    	add	x8, x20, x22, lsl #4
10044aa9c: 39000119    	strb	w25, [x8]
10044aaa0: b85103a9    	ldur	w9, [x29, #-0xf0]
10044aaa4: b8001109    	stur	w9, [x8, #0x1]
10044aaa8: 910963ea    	add	x10, sp, #0x258
10044aaac: b84fb149    	ldur	w9, [x10, #0xfb]
10044aab0: b9000509    	str	w9, [x8, #0x4]
10044aab4: f9000513    	str	x19, [x8, #0x8]
10044aab8: f9408fe9    	ldr	x9, [sp, #0x118]
10044aabc: f9402128    	ldr	x8, [x9, #0x40]
10044aac0: 91000508    	add	x8, x8, #0x1
10044aac4: f9002128    	str	x8, [x9, #0x40]
10044aac8: b85303a8    	ldur	w8, [x29, #-0xd0]
10044aacc: b902f3e8    	str	w8, [sp, #0x2f0]
10044aad0: d10343a8    	sub	x8, x29, #0xd0
10044aad4: b8403108    	ldur	w8, [x8, #0x3]
10044aad8: b809b148    	stur	w8, [x10, #0x9b]
10044aadc: b94103ec    	ldr	w12, [sp, #0x100]
10044aae0: b942f3e8    	ldr	w8, [sp, #0x2f0]
10044aae4: f9405fe9    	ldr	x9, [sp, #0xb8]
10044aae8: b9000128    	str	w8, [x9]
10044aaec: b849b148    	ldur	w8, [x10, #0x9b]
10044aaf0: b8003128    	stur	w8, [x9, #0x3]
10044aaf4: 3907e3f8    	strb	w24, [sp, #0x1f8]
10044aaf8: f90103fb    	str	x27, [sp, #0x200]
10044aafc: 370106ac    	tbnz	w12, #0x0, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ab00: a95223e1    	ldp	x1, x8, [sp, #0x120]
10044ab04: f9401103    	ldr	x3, [x8, #0x20]
10044ab08: f9407fe8    	ldr	x8, [sp, #0xf8]
10044ab0c: f9400102    	ldr	x2, [x8]
10044ab10: f9406fe0    	ldr	x0, [sp, #0xd8]
10044ab14: 940026a4    	bl	0x1004545a4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
10044ab18: b501c2a0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ab1c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044ab20: f9400101    	ldr	x1, [x8]
10044ab24: 910823e0    	add	x0, sp, #0x208
10044ab28: 9107e3e2    	add	x2, sp, #0x1f8
10044ab2c: 97f03190    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
10044ab30: 394823e8    	ldrb	w8, [sp, #0x208]
10044ab34: 71002d1f    	cmp	w8, #0xb
10044ab38: 540104c0    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ab3c: 14000edb    	b	0x10044e6a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6714>
10044ab40: f9408ff6    	ldr	x22, [sp, #0x118]
10044ab44: f94022d8    	ldr	x24, [x22, #0x40]
10044ab48: b401f798    	cbz	x24, 0x10044ea38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aa4>
10044ab4c: f9407be8    	ldr	x8, [sp, #0xf0]
10044ab50: f9400108    	ldr	x8, [x8]
10044ab54: 8b080308    	add	x8, x24, x8
10044ab58: d1000508    	sub	x8, x8, #0x1
10044ab5c: eb19011f    	cmp	x8, x25
10044ab60: 540219e2    	b.hs	0x10044ee9c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f08>
10044ab64: 8b081396    	add	x22, x28, x8, lsl #4
10044ab68: 394002c8    	ldrb	w8, [x22]
10044ab6c: 51002908    	sub	w8, w8, #0xa
10044ab70: 7100111f    	cmp	w8, #0x4
10044ab74: 5401b129    	b.ls	0x10044e198 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6204>
10044ab78: 71032b7f    	cmp	w27, #0xca
10044ab7c: 1a9f17e6    	cset	w6, eq
10044ab80: a94f97e8    	ldp	x8, x5, [sp, #0xf8]
10044ab84: f9400101    	ldr	x1, [x8]
10044ab88: 910c43e0    	add	x0, sp, #0x310
10044ab8c: d10343a7    	sub	x7, x29, #0xd0
10044ab90: aa1603e2    	mov	x2, x22
10044ab94: f9408be3    	ldr	x3, [sp, #0x110]
10044ab98: f94093e4    	ldr	x4, [sp, #0x120]
10044ab9c: 940024fd    	bl	0x100453f90 <__ZN13quickjs_oxide6engine6object16ordinary_storage2ic62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$21property_ic_read_fast17h51aad55151f8a4d2E>
10044aba0: 394c43e8    	ldrb	w8, [sp, #0x310]
10044aba4: 7100291f    	cmp	w8, #0xa
10044aba8: 5401cc60    	b.eq	0x10044e534 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65a0>
10044abac: 3dc0c7e0    	ldr	q0, [sp, #0x310]
10044abb0: 3c9003a0    	stur	q0, [x29, #-0x100]
10044abb4: 71032b7f    	cmp	w27, #0xca
10044abb8: 54000121    	b.ne	0x10044abdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2c48>
10044abbc: eb19029f    	cmp	x20, x25
10044abc0: 540219c2    	b.hs	0x10044eef8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f64>
10044abc4: 3cd003a0    	ldur	q0, [x29, #-0x100]
10044abc8: 3cb47b80    	str	q0, [x28, x20, lsl #4]
10044abcc: 91000708    	add	x8, x24, #0x1
10044abd0: f9408fe9    	ldr	x9, [sp, #0x118]
10044abd4: f9002128    	str	x8, [x9, #0x40]
10044abd8: 1400001b    	b	0x10044ac44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2cb0>
10044abdc: 394002c8    	ldrb	w8, [x22]
10044abe0: f84012c9    	ldur	x9, [x22, #0x1]
10044abe4: f9018be9    	str	x9, [sp, #0x310]
10044abe8: f94006c9    	ldr	x9, [x22, #0x8]
10044abec: 910963ea    	add	x10, sp, #0x258
10044abf0: f80bf149    	stur	x9, [x10, #0xbf]
10044abf4: 3cd003a0    	ldur	q0, [x29, #-0x100]
10044abf8: 3d8002c0    	str	q0, [x22]
10044abfc: 51002909    	sub	w9, w8, #0xa
10044ac00: 7100113f    	cmp	w9, #0x4
10044ac04: 54000209    	b.ls	0x10044ac44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2cb0>
10044ac08: 390bc3e8    	strb	w8, [sp, #0x2f0]
10044ac0c: f9418be8    	ldr	x8, [sp, #0x310]
10044ac10: f94053e9    	ldr	x9, [sp, #0xa0]
10044ac14: f9000128    	str	x8, [x9]
10044ac18: 910963e8    	add	x8, sp, #0x258
10044ac1c: f84bf108    	ldur	x8, [x8, #0xbf]
10044ac20: f8007128    	stur	x8, [x9, #0x7]
10044ac24: f9407fe8    	ldr	x8, [sp, #0xf8]
10044ac28: f9400101    	ldr	x1, [x8]
10044ac2c: d103c3a0    	sub	x0, x29, #0xf0
10044ac30: 910bc3e2    	add	x2, sp, #0x2f0
10044ac34: 97f0314e    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
10044ac38: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044ac3c: 71002d1f    	cmp	w8, #0xb
10044ac40: 5401dba1    	b.ne	0x10044e7b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6820>
10044ac44: b85403a8    	ldur	w8, [x29, #-0xc0]
10044ac48: 7100091f    	cmp	w8, #0x2
10044ac4c: 5400fc20    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ac50: f85303a0    	ldur	x0, [x29, #-0xd0]
10044ac54: f9400008    	ldr	x8, [x0]
10044ac58: f1000508    	subs	x8, x8, #0x1
10044ac5c: f9000008    	str	x8, [x0]
10044ac60: 5400fb81    	b.ne	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ac64: 97efe82e    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044ac68: 140007da    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ac6c: f9400196    	ldr	x22, [x12]
10044ac70: d10343a0    	sub	x0, x29, #0xd0
10044ac74: aa1603e1    	mov	x1, x22
10044ac78: aa1403e2    	mov	x2, x20
10044ac7c: 94002951    	bl	0x1004551c0 <__ZN13quickjs_oxide6engine4heap14slot_ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$36slot_value_release_readiness_jsvalue17h2038e983b6e2b9bfE>
10044ac80: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044ac84: 71002d1f    	cmp	w8, #0xb
10044ac88: 5401a1c1    	b.ne	0x10044e0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
10044ac8c: 385313a9    	ldurb	w9, [x29, #-0xcf]
10044ac90: 3501a189    	cbnz	w9, 0x10044e0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
10044ac94: f94016c8    	ldr	x8, [x22, #0x28]
10044ac98: 92f00009    	mov	x9, #0x7fffffffffffffff ; =9223372036854775807
10044ac9c: eb09011f    	cmp	x8, x9
10044aca0: 54021762    	b.hs	0x10044ef8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ff8>
10044aca4: 91000508    	add	x8, x8, #0x1
10044aca8: f90016c8    	str	x8, [x22, #0x28]
10044acac: b9400680    	ldr	w0, [x20, #0x4]
10044acb0: f9409ac1    	ldr	x1, [x22, #0x130]
10044acb4: eb00003f    	cmp	x1, x0
10044acb8: 54021709    	b.ls	0x10044ef98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7004>
10044acbc: f94096c8    	ldr	x8, [x22, #0x128]
10044acc0: 52800309    	mov	w9, #0x18               ; =24
10044acc4: 9ba92008    	umaddl	x8, w0, w9, x8
10044acc8: f9400109    	ldr	x9, [x8]
10044accc: 927f052a    	and	x10, x9, #0x6
10044acd0: f100095f    	cmp	x10, #0x2
10044acd4: 5401f1c0    	b.eq	0x10044eb0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b78>
10044acd8: b940150a    	ldr	w10, [x8, #0x14]
10044acdc: 3401f18a    	cbz	w10, 0x10044eb0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b78>
10044ace0: f100113f    	cmp	x9, #0x4
10044ace4: 5401f541    	b.ne	0x10044eb8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6bf8>
10044ace8: f9400509    	ldr	x9, [x8, #0x8]
10044acec: f940012a    	ldr	x10, [x9]
10044acf0: b100054a    	adds	x10, x10, #0x1
10044acf4: f900012a    	str	x10, [x9]
10044acf8: 540216e2    	b.hs	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044acfc: f940051c    	ldr	x28, [x8, #0x8]
10044ad00: f9018bfc    	str	x28, [sp, #0x310]
10044ad04: f94016c8    	ldr	x8, [x22, #0x28]
10044ad08: d1000508    	sub	x8, x8, #0x1
10044ad0c: f90016c8    	str	x8, [x22, #0x28]
10044ad10: aa1c03e0    	mov	x0, x28
10044ad14: 97f0f1e2    	bl	0x10008749c <__ZN13quickjs_oxide6engine4atom29parse_canonical_u32_js_string17hf7015ef7cc8bce8eE>
10044ad18: f9400388    	ldr	x8, [x28]
10044ad1c: d1000508    	sub	x8, x8, #0x1
10044ad20: f9000388    	str	x8, [x28]
10044ad24: 3601cde0    	tbz	w0, #0x0, 0x10044e6e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x674c>
10044ad28: aa0103f4    	mov	x20, x1
10044ad2c: 3100043f    	cmn	w1, #0x1
10044ad30: 5401cd80    	b.eq	0x10044e6e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x674c>
10044ad34: b5000068    	cbnz	x8, 0x10044ad40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2dac>
10044ad38: 910c43e0    	add	x0, sp, #0x310
10044ad3c: 97ef9a83    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044ad40: f94087e8    	ldr	x8, [sp, #0x108]
10044ad44: a940f109    	ldp	x9, x28, [x8, #0x8]
10044ad48: f9408fe8    	ldr	x8, [sp, #0x118]
10044ad4c: f9402108    	ldr	x8, [x8, #0x40]
10044ad50: f9407fec    	ldr	x12, [sp, #0xf8]
10044ad54: f100091f    	cmp	x8, #0x2
10044ad58: 5401ea03    	b.lo	0x10044ea98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b04>
10044ad5c: f9407bea    	ldr	x10, [sp, #0xf0]
10044ad60: f940014a    	ldr	x10, [x10]
10044ad64: 8b0a0108    	add	x8, x8, x10
10044ad68: d1000916    	sub	x22, x8, #0x2
10044ad6c: eb1c02df    	cmp	x22, x28
10044ad70: 5401f642    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044ad74: 8b161128    	add	x8, x9, x22, lsl #4
10044ad78: 39400109    	ldrb	w9, [x8]
10044ad7c: 5100292a    	sub	w10, w9, #0xa
10044ad80: 7100115f    	cmp	w10, #0x4
10044ad84: 5401b129    	b.ls	0x10044e3a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6414>
10044ad88: 7100253f    	cmp	w9, #0x9
10044ad8c: 5401e0e1    	b.ne	0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
10044ad90: f9400198    	ldr	x24, [x12]
10044ad94: f9401719    	ldr	x25, [x24, #0x28]
10044ad98: 92f00009    	mov	x9, #0x7fffffffffffffff ; =9223372036854775807
10044ad9c: eb09033f    	cmp	x25, x9
10044ada0: 5401ec22    	b.hs	0x10044eb24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b90>
10044ada4: 91000729    	add	x9, x25, #0x1
10044ada8: f9001709    	str	x9, [x24, #0x28]
10044adac: a94fdb1c    	ldp	x28, x22, [x24, #0xf8]
10044adb0: fc404100    	ldur	d0, [x8, #0x4]
10044adb4: 910963e8    	add	x8, sp, #0x258
10044adb8: fc08c100    	stur	d0, [x8, #0x8c]
10044adbc: b902e3ff    	str	wzr, [sp, #0x2e0]
10044adc0: d10343a0    	sub	x0, x29, #0xd0
10044adc4: 910b83e3    	add	x3, sp, #0x2e0
10044adc8: aa1c03e1    	mov	x1, x28
10044adcc: aa1603e2    	mov	x2, x22
10044add0: 97ef9ce9    	bl	0x100032174 <__ZN13quickjs_oxide6engine4heap14object_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$22validate_slot_identity17h9c24141d9f2cc0c4E>
10044add4: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044add8: 7100191f    	cmp	w8, #0x6
10044addc: 54017aa1    	b.ne	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ade0: f85383a0    	ldur	x0, [x29, #-0xc8]
10044ade4: eb16001f    	cmp	x0, x22
10044ade8: 540206e2    	b.hs	0x10044eec4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f30>
10044adec: 52802308    	mov	w8, #0x118              ; =280
10044adf0: 9b087008    	madd	x8, x0, x8, x28
10044adf4: 39400109    	ldrb	w9, [x8]
10044adf8: 7100053f    	cmp	w9, #0x1
10044adfc: 540179a1    	b.ne	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ae00: f9400509    	ldr	x9, [x8, #0x8]
10044ae04: f100053f    	cmp	x9, #0x1
10044ae08: 54017948    	b.hi	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ae0c: 3943b509    	ldrb	w9, [x8, #0xed]
10044ae10: 71000d3f    	cmp	w9, #0x3
10044ae14: 540178e1    	b.ne	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ae18: 39416109    	ldrb	w9, [x8, #0x58]
10044ae1c: 7100093f    	cmp	w9, #0x2
10044ae20: 54017881    	b.ne	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ae24: f9403109    	ldr	x9, [x8, #0x60]
10044ae28: d2f0000a    	mov	x10, #-0x8000000000000000 ; =-9223372036854775808
10044ae2c: eb0a013f    	cmp	x9, x10
10044ae30: 54017800    	b.eq	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ae34: f940390a    	ldr	x10, [x8, #0x70]
10044ae38: 2a1403e9    	mov	w9, w20
10044ae3c: eb09015f    	cmp	x10, x9
10044ae40: 54017789    	b.ls	0x10044dd30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d9c>
10044ae44: f9403508    	ldr	x8, [x8, #0x68]
10044ae48: 8b091101    	add	x1, x8, x9, lsl #4
10044ae4c: 910c43e0    	add	x0, sp, #0x310
10044ae50: 94002a5e    	bl	0x1004557c8 <__ZN13quickjs_oxide6engine6object16ordinary_storage23immediate_value_jsvalue17hc5220bf7094f18e2E>
10044ae54: f9001719    	str	x25, [x24, #0x28]
10044ae58: 394c43e8    	ldrb	w8, [sp, #0x310]
10044ae5c: 7100291f    	cmp	w8, #0xa
10044ae60: 5401da40    	b.eq	0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
10044ae64: 3dc0c7e0    	ldr	q0, [sp, #0x310]
10044ae68: 3c9003a0    	stur	q0, [x29, #-0x100]
10044ae6c: 7103377f    	cmp	w27, #0xcd
10044ae70: f94087e8    	ldr	x8, [sp, #0x108]
10044ae74: 54000340    	b.eq	0x10044aedc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2f48>
10044ae78: a9408901    	ldp	x1, x2, [x8, #0x8]
10044ae7c: d10343a0    	sub	x0, x29, #0xd0
10044ae80: f9408fe3    	ldr	x3, [sp, #0x118]
10044ae84: 97ff955f    	bl	0x100430400 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
10044ae88: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044ae8c: 7100291f    	cmp	w8, #0xa
10044ae90: 5401c5e0    	b.eq	0x10044e74c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67b8>
10044ae94: f94077ea    	ldr	x10, [sp, #0xe8]
10044ae98: b9400149    	ldr	w9, [x10]
10044ae9c: f94053eb    	ldr	x11, [sp, #0xa0]
10044aea0: b9000169    	str	w9, [x11]
10044aea4: b8403149    	ldur	w9, [x10, #0x3]
10044aea8: b8003169    	stur	w9, [x11, #0x3]
10044aeac: f85383a9    	ldur	x9, [x29, #-0xc8]
10044aeb0: 390bc3e8    	strb	w8, [sp, #0x2f0]
10044aeb4: f9017fe9    	str	x9, [sp, #0x2f8]
10044aeb8: f9407fe8    	ldr	x8, [sp, #0xf8]
10044aebc: f9400101    	ldr	x1, [x8]
10044aec0: d103c3a0    	sub	x0, x29, #0xf0
10044aec4: 910bc3e2    	add	x2, sp, #0x2f0
10044aec8: 97f030a9    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
10044aecc: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044aed0: 71002d1f    	cmp	w8, #0xb
10044aed4: f94087e8    	ldr	x8, [sp, #0x108]
10044aed8: 5401c4a1    	b.ne	0x10044e76c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67d8>
10044aedc: a940f114    	ldp	x20, x28, [x8, #0x8]
10044aee0: aa1403e0    	mov	x0, x20
10044aee4: aa1c03e1    	mov	x1, x28
10044aee8: f9408fe2    	ldr	x2, [sp, #0x118]
10044aeec: 97f4cc10    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044aef0: aa0103f6    	mov	x22, x1
10044aef4: 3707b420    	tbnz	w0, #0x0, 0x10044a578 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25e4>
10044aef8: eb16039f    	cmp	x28, x22
10044aefc: 5401fa89    	b.ls	0x10044ee4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6eb8>
10044af00: 3cd003a0    	ldur	q0, [x29, #-0x100]
10044af04: 3cb67a80    	str	q0, [x20, x22, lsl #4]
10044af08: 17fffda4    	b	0x10044a598 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2604>
10044af0c: 7200053f    	tst	w9, #0x3
10044af10: 54007da0    	b.eq	0x10044bec4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f30>
10044af14: 381303a8    	sturb	w8, [x29, #-0xd0]
10044af18: f85103a9    	ldur	x9, [x29, #-0xf0]
10044af1c: f94077ea    	ldr	x10, [sp, #0xe8]
10044af20: f9000149    	str	x9, [x10]
10044af24: f84ff169    	ldur	x9, [x11, #0xff]
10044af28: f8007149    	stur	x9, [x10, #0x7]
10044af2c: b9400149    	ldr	w9, [x10]
10044af30: f94057eb    	ldr	x11, [sp, #0xa8]
10044af34: b9000169    	str	w9, [x11]
10044af38: b8403149    	ldur	w9, [x10, #0x3]
10044af3c: b8003169    	stur	w9, [x11, #0x3]
10044af40: f85383a9    	ldur	x9, [x29, #-0xc8]
10044af44: 3905e3e8    	strb	w8, [sp, #0x178]
10044af48: f900c3e9    	str	x9, [sp, #0x180]
10044af4c: 9105e3e3    	add	x3, sp, #0x178
10044af50: 94001dd0    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044af54: 1400071e    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044af58: 5401d740    	b.eq	0x10044ea40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6aac>
10044af5c: 384a8349    	ldurb	w9, [x26, #0xa8]
10044af60: 7100093f    	cmp	w9, #0x2
10044af64: 540087e2    	b.hs	0x10044c060 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40cc>
10044af68: 295a0d02    	ldp	w2, w3, [x8, #0xd0]
10044af6c: f94001a1    	ldr	x1, [x13]
10044af70: 91024340    	add	x0, x26, #0x90
10044af74: 97ffe043    	bl	0x100443080 <__ZN13quickjs_oxide6engine2vm8protocol9CallInput13callee_global17hccaf868ce3e72756E>
10044af78: 3701a420    	tbnz	w0, #0x0, 0x10044e3fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6468>
10044af7c: fd400420    	ldr	d0, [x1, #0x8]
10044af80: d10343a8    	sub	x8, x29, #0xd0
10044af84: fc004100    	stur	d0, [x8, #0x4]
10044af88: 52800128    	mov	w8, #0x9                ; =9
10044af8c: 381303a8    	sturb	w8, [x29, #-0xd0]
10044af90: f9407fe8    	ldr	x8, [sp, #0xf8]
10044af94: f9400101    	ldr	x1, [x8]
10044af98: d103c3a0    	sub	x0, x29, #0xf0
10044af9c: d10343a2    	sub	x2, x29, #0xd0
10044afa0: 97ff7c66    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044afa4: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044afa8: 7100291f    	cmp	w8, #0xa
10044afac: 54018780    	b.eq	0x10044e09c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6108>
10044afb0: f94067ea    	ldr	x10, [sp, #0xc8]
10044afb4: b9400149    	ldr	w9, [x10]
10044afb8: b902f3e9    	str	w9, [sp, #0x2f0]
10044afbc: b8403149    	ldur	w9, [x10, #0x3]
10044afc0: 910963ee    	add	x14, sp, #0x258
10044afc4: b809b1c9    	stur	w9, [x14, #0x9b]
10044afc8: f85183a9    	ldur	x9, [x29, #-0xe8]
10044afcc: f9408fec    	ldr	x12, [sp, #0x118]
10044afd0: f94087e0    	ldr	x0, [sp, #0x108]
10044afd4: f9407fed    	ldr	x13, [sp, #0xf8]
10044afd8: f94077ea    	ldr	x10, [sp, #0xe8]
10044afdc: 140006f2    	b	0x10044cba4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c10>
10044afe0: f9400201    	ldr	x1, [x16]
10044afe4: d10343a0    	sub	x0, x29, #0xd0
10044afe8: 97ff7c54    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044afec: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044aff0: 7100291f    	cmp	w8, #0xa
10044aff4: f94087ef    	ldr	x15, [sp, #0x108]
10044aff8: f9407ff0    	ldr	x16, [sp, #0xf8]
10044affc: 910963ed    	add	x13, sp, #0x258
10044b000: f94077ea    	ldr	x10, [sp, #0xe8]
10044b004: 54ffae01    	b.ne	0x10044a5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2630>
10044b008: 14000b6a    	b	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044b00c: f9407fed    	ldr	x13, [sp, #0xf8]
10044b010: f94001a1    	ldr	x1, [x13]
10044b014: 71001d1f    	cmp	w8, #0x7
10044b018: f9408fec    	ldr	x12, [sp, #0x118]
10044b01c: f94087e0    	ldr	x0, [sp, #0x108]
10044b020: 910963ee    	add	x14, sp, #0x258
10044b024: 5400da28    	b.hi	0x10044cb68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bd4>
10044b028: 5280002a    	mov	w10, #0x1               ; =1
10044b02c: 1ac8214a    	lsl	w10, w10, w8
10044b030: 5280138b    	mov	w11, #0x9c              ; =156
10044b034: 6a0b015f    	tst	w10, w11
10044b038: 54008de0    	b.eq	0x10044c1f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4260>
10044b03c: f844112a    	ldur	x10, [x9, #0x41]
10044b040: f81103aa    	stur	x10, [x29, #-0xf0]
10044b044: f9402529    	ldr	x9, [x9, #0x48]
10044b048: f80ff1c9    	stur	x9, [x14, #0xff]
10044b04c: 1400046c    	b	0x10044c1fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4268>
10044b050: 7200053f    	tst	w9, #0x3
10044b054: 54007be0    	b.eq	0x10044bfd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x403c>
10044b058: f85103a9    	ldur	x9, [x29, #-0xf0]
10044b05c: f94077ea    	ldr	x10, [sp, #0xe8]
10044b060: f9000149    	str	x9, [x10]
10044b064: f84ff1c9    	ldur	x9, [x14, #0xff]
10044b068: f8007149    	stur	x9, [x10, #0x7]
10044b06c: b9400149    	ldr	w9, [x10]
10044b070: b902f3e9    	str	w9, [sp, #0x2f0]
10044b074: b8403149    	ldur	w9, [x10, #0x3]
10044b078: b809b1c9    	stur	w9, [x14, #0x9b]
10044b07c: f85383a9    	ldur	x9, [x29, #-0xc8]
10044b080: f9406bea    	ldr	x10, [sp, #0xd0]
10044b084: f940014a    	ldr	x10, [x10]
10044b088: f94073eb    	ldr	x11, [sp, #0xe0]
10044b08c: f940016b    	ldr	x11, [x11]
10044b090: eb0a016b    	subs	x11, x11, x10
10044b094: 9a8b33eb    	csel	x11, xzr, x11, lo
10044b098: eb13017f    	cmp	x11, x19
10044b09c: 54018e29    	b.ls	0x10044e260 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62cc>
10044b0a0: f940099c    	ldr	x28, [x12, #0x10]
10044b0a4: 8b130156    	add	x22, x10, x19
10044b0a8: eb1c02df    	cmp	x22, x28
10044b0ac: 5401ee02    	b.hs	0x10044ee6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ed8>
10044b0b0: f940058a    	ldr	x10, [x12, #0x8]
10044b0b4: 8b16114a    	add	x10, x10, x22, lsl #4
10044b0b8: 3940014b    	ldrb	w11, [x10]
10044b0bc: 7100397f    	cmp	w11, #0xe
10044b0c0: 54019060    	b.eq	0x10044e2cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6338>
10044b0c4: b840114c    	ldur	w12, [x10, #0x1]
10044b0c8: f9405bef    	ldr	x15, [sp, #0xb0]
10044b0cc: b90001ec    	str	w12, [x15]
10044b0d0: b940054c    	ldr	w12, [x10, #0x4]
10044b0d4: b80031ec    	stur	w12, [x15, #0x3]
10044b0d8: f940054c    	ldr	x12, [x10, #0x8]
10044b0dc: 39000148    	strb	w8, [x10]
10044b0e0: b942f3e8    	ldr	w8, [sp, #0x2f0]
10044b0e4: b8001148    	stur	w8, [x10, #0x1]
10044b0e8: b849b1c8    	ldur	w8, [x14, #0x9b]
10044b0ec: b9000548    	str	w8, [x10, #0x4]
10044b0f0: f9000549    	str	x9, [x10, #0x8]
10044b0f4: 390723eb    	strb	w11, [sp, #0x1c8]
10044b0f8: f900ebec    	str	x12, [sp, #0x1d0]
10044b0fc: a95223e1    	ldp	x1, x8, [sp, #0x120]
10044b100: f9401103    	ldr	x3, [x8, #0x20]
10044b104: f94001a2    	ldr	x2, [x13]
10044b108: f9406fe0    	ldr	x0, [sp, #0xd8]
10044b10c: 94002526    	bl	0x1004545a4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
10044b110: b50192e0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b114: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b118: f9400100    	ldr	x0, [x8]
10044b11c: 910723e1    	add	x1, sp, #0x1c8
10044b120: 97f45d3f    	bl	0x10016261c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
10044b124: 140006aa    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b128: b94027e8    	ldr	w8, [sp, #0x24]
10044b12c: b9033fe8    	str	w8, [sp, #0x33c]
10044b130: b9033bf8    	str	w24, [sp, #0x338]
10044b134: b90337ff    	str	wzr, [sp, #0x334]
10044b138: d10343a0    	sub	x0, x29, #0xd0
10044b13c: 910cd3e3    	add	x3, sp, #0x334
10044b140: f9400ff9    	ldr	x25, [sp, #0x18]
10044b144: aa1903e2    	mov	x2, x25
10044b148: 97ef9c0b    	bl	0x100032174 <__ZN13quickjs_oxide6engine4heap14object_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$22validate_slot_identity17h9c24141d9f2cc0c4E>
10044b14c: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044b150: 7100191f    	cmp	w8, #0x6
10044b154: 5401c201    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b158: f85383b4    	ldur	x20, [x29, #-0xc8]
10044b15c: eb19029f    	cmp	x20, x25
10044b160: 5401ea62    	b.hs	0x10044eeac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f18>
10044b164: 52802308    	mov	w8, #0x118              ; =280
10044b168: 9b084e88    	madd	x8, x20, x8, x19
10044b16c: 39400109    	ldrb	w9, [x8]
10044b170: 7100053f    	cmp	w9, #0x1
10044b174: 5401c101    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b178: f9400509    	ldr	x9, [x8, #0x8]
10044b17c: f100053f    	cmp	x9, #0x1
10044b180: 5401c0a8    	b.hi	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b184: 39416109    	ldrb	w9, [x8, #0x58]
10044b188: 7100753f    	cmp	w9, #0x1d
10044b18c: 5401c041    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b190: b9406d09    	ldr	w9, [x8, #0x6c]
10044b194: 3941c114    	ldrb	w20, [x8, #0x70]
10044b198: 3cc5c100    	ldur	q0, [x8, #0x5c]
10044b19c: 3d80c7e0    	str	q0, [sp, #0x310]
10044b1a0: b90323e9    	str	w9, [sp, #0x320]
10044b1a4: 390c93f4    	strb	w20, [sp, #0x324]
10044b1a8: 51001e88    	sub	w8, w20, #0x7
10044b1ac: 7100091f    	cmp	w8, #0x2
10044b1b0: 5401bf23    	b.lo	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b1b4: b9403be8    	ldr	w8, [sp, #0x38]
10044b1b8: 2a0803e4    	mov	w4, w8
10044b1bc: d10343a0    	sub	x0, x29, #0xd0
10044b1c0: 910c43e3    	add	x3, sp, #0x310
10044b1c4: aa1303e1    	mov	x1, x19
10044b1c8: aa1903e2    	mov	x2, x25
10044b1cc: d2800005    	mov	x5, #0x0                ; =0
10044b1d0: 97f0f4d2    	bl	0x100088518 <__ZN13quickjs_oxide6engine8builtins12array_buffer11typed_array33ordinary_typed_array_word_in_heap17h3136afd441ee1654E>
10044b1d4: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044b1d8: 71002d1f    	cmp	w8, #0xb
10044b1dc: 5401bc41    	b.ne	0x10044e964 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x69d0>
10044b1e0: 385313a8    	ldurb	w8, [x29, #-0xcf]
10044b1e4: 7100091f    	cmp	w8, #0x2
10044b1e8: 5401bd61    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b1ec: d10343a8    	sub	x8, x29, #0xd0
10044b1f0: f8402102    	ldur	x2, [x8, #0x2]
10044b1f4: d10343a0    	sub	x0, x29, #0xd0
10044b1f8: aa1403e1    	mov	x1, x20
10044b1fc: 97f0f6eb    	bl	0x100088da8 <__ZN13quickjs_oxide6engine8builtins12array_buffer11typed_array25typed_array_decode_number17hf2a2b840b4fe4c9bE>
10044b200: fc5383a0    	ldur	d0, [x29, #-0xc8]
10044b204: 296627a8    	ldp	w8, w9, [x29, #-0xd0]
10044b208: 7100011f    	cmp	w8, #0x0
10044b20c: 52800068    	mov	w8, #0x3                ; =3
10044b210: 1a880508    	cinc	w8, w8, ne
10044b214: 390b43e8    	strb	w8, [sp, #0x2d0]
10044b218: b902d7e9    	str	w9, [sp, #0x2d4]
10044b21c: fd016fe0    	str	d0, [sp, #0x2d8]
10044b220: 14000367    	b	0x10044bfbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4028>
10044b224: f9403b29    	ldr	x9, [x25, #0x70]
10044b228: b9403be8    	ldr	w8, [sp, #0x38]
10044b22c: 2a0803e8    	mov	w8, w8
10044b230: eb08013f    	cmp	x9, x8
10044b234: 5401bb09    	b.ls	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044b238: f9403729    	ldr	x9, [x25, #0x68]
10044b23c: 8b081121    	add	x1, x9, x8, lsl #4
10044b240: 140003a9    	b	0x10044c0e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4150>
10044b244: 71001f3f    	cmp	w25, #0x7
10044b248: f90083ea    	str	x10, [sp, #0x100]
10044b24c: 5400cd48    	b.hi	0x10044cbf4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c60>
10044b250: 52800028    	mov	w8, #0x1                ; =1
10044b254: 1ad92108    	lsl	w8, w8, w25
10044b258: 52801389    	mov	w9, #0x9c               ; =156
10044b25c: 6a09011f    	tst	w8, w9
10044b260: 54009a60    	b.eq	0x10044c5ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4618>
10044b264: f8401048    	ldur	x8, [x2, #0x1]
10044b268: f81103a8    	stur	x8, [x29, #-0xf0]
10044b26c: f9400448    	ldr	x8, [x2, #0x8]
10044b270: 910963e9    	add	x9, sp, #0x258
10044b274: f80ff128    	stur	x8, [x9, #0xff]
10044b278: 140004cf    	b	0x10044c5b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4620>
10044b27c: f100013f    	cmp	x9, #0x0
10044b280: 1a9f07e8    	cset	w8, ne
10044b284: 71025b7f    	cmp	w27, #0x96
10044b288: 1a9f07e9    	cset	w9, ne
10044b28c: 12000108    	and	w8, w8, #0x1
10044b290: 6b08013f    	cmp	w9, w8
10044b294: 5400c9e0    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044b298: f94083e8    	ldr	x8, [sp, #0x100]
10044b29c: 2a0803e8    	mov	w8, w8
10044b2a0: f900abe8    	str	x8, [sp, #0x150]
10044b2a4: 1400064b    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044b2a8: 7100093f    	cmp	w9, #0x2
10044b2ac: 54006fe0    	b.eq	0x10044c0a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4114>
10044b2b0: 71000d3f    	cmp	w9, #0x3
10044b2b4: 54017201    	b.ne	0x10044e0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6160>
10044b2b8: b9400d08    	ldr	w8, [x8, #0xc]
10044b2bc: b81343a8    	stur	w8, [x29, #-0xcc]
10044b2c0: 52800068    	mov	w8, #0x3                ; =3
10044b2c4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b2c8: d10343a2    	sub	x2, x29, #0xd0
10044b2cc: 94001c8d    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044b2d0: 1400063f    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b2d4: 7100193f    	cmp	w9, #0x6
10044b2d8: 54006f60    	b.eq	0x10044c0c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4130>
10044b2dc: 71001d3f    	cmp	w9, #0x7
10044b2e0: 540170a1    	b.ne	0x10044e0f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6160>
10044b2e4: fc40c100    	ldur	d0, [x8, #0xc]
10044b2e8: d10343a8    	sub	x8, x29, #0xd0
10044b2ec: fc004100    	stur	d0, [x8, #0x4]
10044b2f0: 528000a8    	mov	w8, #0x5                ; =5
10044b2f4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b2f8: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b2fc: f9400101    	ldr	x1, [x8]
10044b300: d103c3a0    	sub	x0, x29, #0xf0
10044b304: d10343a2    	sub	x2, x29, #0xd0
10044b308: 97ff7b8c    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044b30c: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044b310: 7100291f    	cmp	w8, #0xa
10044b314: 54016c40    	b.eq	0x10044e09c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6108>
10044b318: f94067ea    	ldr	x10, [sp, #0xc8]
10044b31c: b9400149    	ldr	w9, [x10]
10044b320: f9403beb    	ldr	x11, [sp, #0x70]
10044b324: b9000169    	str	w9, [x11]
10044b328: b8403149    	ldur	w9, [x10, #0x3]
10044b32c: b8003169    	stur	w9, [x11, #0x3]
10044b330: f85183a9    	ldur	x9, [x29, #-0xe8]
10044b334: 390563e8    	strb	w8, [sp, #0x158]
10044b338: f900b3e9    	str	x9, [sp, #0x160]
10044b33c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b340: f9400102    	ldr	x2, [x8]
10044b344: 910563e3    	add	x3, sp, #0x158
10044b348: f94087e0    	ldr	x0, [sp, #0x108]
10044b34c: f9408fe1    	ldr	x1, [sp, #0x118]
10044b350: 94001cd0    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b354: 1400061e    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b358: 7200053f    	tst	w9, #0x3
10044b35c: 54000180    	b.eq	0x10044b38c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x33f8>
10044b360: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b364: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b368: f94077e9    	ldr	x9, [sp, #0xe8]
10044b36c: f9000128    	str	x8, [x9]
10044b370: f849f168    	ldur	x8, [x11, #0x9f]
10044b374: f8007128    	stur	x8, [x9, #0x7]
10044b378: d10343a2    	sub	x2, x29, #0xd0
10044b37c: aa0303e0    	mov	x0, x3
10044b380: aa0403e1    	mov	x1, x4
10044b384: 94001ca5    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b388: 14000611    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b38c: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b390: f9400102    	ldr	x2, [x8]
10044b394: d10343a0    	sub	x0, x29, #0xd0
10044b398: 94001d13    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b39c: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b3a0: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b3a4: 7100091f    	cmp	w8, #0x2
10044b3a8: 54017e20    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b3ac: 92401c09    	and	x9, x0, #0xff
10044b3b0: f100293f    	cmp	x9, #0xa
10044b3b4: f9408fe1    	ldr	x1, [sp, #0x118]
10044b3b8: f94087e9    	ldr	x9, [sp, #0x108]
10044b3bc: f9407fea    	ldr	x10, [sp, #0xf8]
10044b3c0: 54017e20    	b.eq	0x10044e384 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63f0>
10044b3c4: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b3c8: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b3cc: f9400142    	ldr	x2, [x10]
10044b3d0: d103c3a3    	sub	x3, x29, #0xf0
10044b3d4: aa0903e0    	mov	x0, x9
10044b3d8: 94001cae    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b3dc: 140005fc    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b3e0: 7200053f    	tst	w9, #0x3
10044b3e4: 54000180    	b.eq	0x10044b414 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3480>
10044b3e8: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b3ec: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b3f0: f94077e9    	ldr	x9, [sp, #0xe8]
10044b3f4: f9000128    	str	x8, [x9]
10044b3f8: f849f168    	ldur	x8, [x11, #0x9f]
10044b3fc: f8007128    	stur	x8, [x9, #0x7]
10044b400: d10343a2    	sub	x2, x29, #0xd0
10044b404: aa0303e0    	mov	x0, x3
10044b408: aa0403e1    	mov	x1, x4
10044b40c: 94001c83    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b410: 140005ef    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b414: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b418: f9400102    	ldr	x2, [x8]
10044b41c: d10343a0    	sub	x0, x29, #0xd0
10044b420: 94001cf1    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b424: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b428: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b42c: 7100091f    	cmp	w8, #0x2
10044b430: 540179e0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b434: 92401c08    	and	x8, x0, #0xff
10044b438: f100291f    	cmp	x8, #0xa
10044b43c: f9408fe1    	ldr	x1, [sp, #0x118]
10044b440: f94087e9    	ldr	x9, [sp, #0x108]
10044b444: f9407fea    	ldr	x10, [sp, #0xf8]
10044b448: 54014b80    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044b44c: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b450: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b454: f9400142    	ldr	x2, [x10]
10044b458: d103c3a3    	sub	x3, x29, #0xf0
10044b45c: aa0903e0    	mov	x0, x9
10044b460: 94001c8c    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b464: 140005da    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b468: 7200053f    	tst	w9, #0x3
10044b46c: 54000180    	b.eq	0x10044b49c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3508>
10044b470: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b474: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b478: f94077e9    	ldr	x9, [sp, #0xe8]
10044b47c: f9000128    	str	x8, [x9]
10044b480: f849f168    	ldur	x8, [x11, #0x9f]
10044b484: f8007128    	stur	x8, [x9, #0x7]
10044b488: d10343a2    	sub	x2, x29, #0xd0
10044b48c: aa0303e0    	mov	x0, x3
10044b490: aa0403e1    	mov	x1, x4
10044b494: 94001c61    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b498: 140005cd    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b49c: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b4a0: f9400102    	ldr	x2, [x8]
10044b4a4: d10343a0    	sub	x0, x29, #0xd0
10044b4a8: 94001d66    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044b4ac: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044b4b0: 71002d1f    	cmp	w8, #0xb
10044b4b4: 540147e0    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044b4b8: f94077ea    	ldr	x10, [sp, #0xe8]
10044b4bc: b9400149    	ldr	w9, [x10]
10044b4c0: b90313e9    	str	w9, [sp, #0x310]
10044b4c4: b8403149    	ldur	w9, [x10, #0x3]
10044b4c8: 910963eb    	add	x11, sp, #0x258
10044b4cc: b80bb169    	stur	w9, [x11, #0xbb]
10044b4d0: 7100291f    	cmp	w8, #0xa
10044b4d4: f9408fe1    	ldr	x1, [sp, #0x118]
10044b4d8: f94087e0    	ldr	x0, [sp, #0x108]
10044b4dc: f9407fea    	ldr	x10, [sp, #0xf8]
10044b4e0: 54014da0    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044b4e4: f85383a9    	ldur	x9, [x29, #-0xc8]
10044b4e8: 381103a8    	sturb	w8, [x29, #-0xf0]
10044b4ec: b94313e8    	ldr	w8, [sp, #0x310]
10044b4f0: f94067ec    	ldr	x12, [sp, #0xc8]
10044b4f4: b9000188    	str	w8, [x12]
10044b4f8: b84bb168    	ldur	w8, [x11, #0xbb]
10044b4fc: b8003188    	stur	w8, [x12, #0x3]
10044b500: f81183a9    	stur	x9, [x29, #-0xe8]
10044b504: f9400142    	ldr	x2, [x10]
10044b508: d103c3a3    	sub	x3, x29, #0xf0
10044b50c: 94001c61    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b510: 140005af    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b514: 7200053f    	tst	w9, #0x3
10044b518: 54000180    	b.eq	0x10044b548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x35b4>
10044b51c: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b520: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b524: f94077e9    	ldr	x9, [sp, #0xe8]
10044b528: f9000128    	str	x8, [x9]
10044b52c: f849f168    	ldur	x8, [x11, #0x9f]
10044b530: f8007128    	stur	x8, [x9, #0x7]
10044b534: d10343a2    	sub	x2, x29, #0xd0
10044b538: aa0303e0    	mov	x0, x3
10044b53c: aa0403e1    	mov	x1, x4
10044b540: 94001c36    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b544: 140005a2    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b548: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b54c: f9400102    	ldr	x2, [x8]
10044b550: d10343a0    	sub	x0, x29, #0xd0
10044b554: 94001ca4    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b558: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b55c: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b560: 7100091f    	cmp	w8, #0x2
10044b564: 54017040    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b568: 92401c08    	and	x8, x0, #0xff
10044b56c: f100291f    	cmp	x8, #0xa
10044b570: f9408fe1    	ldr	x1, [sp, #0x118]
10044b574: f94087e9    	ldr	x9, [sp, #0x108]
10044b578: f9407fea    	ldr	x10, [sp, #0xf8]
10044b57c: 540141e0    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044b580: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b584: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b588: f9400142    	ldr	x2, [x10]
10044b58c: d103c3a3    	sub	x3, x29, #0xf0
10044b590: aa0903e0    	mov	x0, x9
10044b594: 94001c3f    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b598: 1400058d    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b59c: 7200053f    	tst	w9, #0x3
10044b5a0: 54000180    	b.eq	0x10044b5d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x363c>
10044b5a4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b5a8: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b5ac: f94077e9    	ldr	x9, [sp, #0xe8]
10044b5b0: f9000128    	str	x8, [x9]
10044b5b4: f849f168    	ldur	x8, [x11, #0x9f]
10044b5b8: f8007128    	stur	x8, [x9, #0x7]
10044b5bc: d10343a2    	sub	x2, x29, #0xd0
10044b5c0: aa0303e0    	mov	x0, x3
10044b5c4: aa0403e1    	mov	x1, x4
10044b5c8: 94001c14    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b5cc: 14000580    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b5d0: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b5d4: f9400102    	ldr	x2, [x8]
10044b5d8: d10343a0    	sub	x0, x29, #0xd0
10044b5dc: 94001d19    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044b5e0: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044b5e4: 71002d1f    	cmp	w8, #0xb
10044b5e8: 54013e40    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044b5ec: f94077ea    	ldr	x10, [sp, #0xe8]
10044b5f0: b9400149    	ldr	w9, [x10]
10044b5f4: b90313e9    	str	w9, [sp, #0x310]
10044b5f8: b8403149    	ldur	w9, [x10, #0x3]
10044b5fc: 910963eb    	add	x11, sp, #0x258
10044b600: b80bb169    	stur	w9, [x11, #0xbb]
10044b604: 7100291f    	cmp	w8, #0xa
10044b608: f9408fe1    	ldr	x1, [sp, #0x118]
10044b60c: f94087e0    	ldr	x0, [sp, #0x108]
10044b610: f9407fea    	ldr	x10, [sp, #0xf8]
10044b614: 54014400    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044b618: f85383a9    	ldur	x9, [x29, #-0xc8]
10044b61c: 381103a8    	sturb	w8, [x29, #-0xf0]
10044b620: b94313e8    	ldr	w8, [sp, #0x310]
10044b624: f94067ec    	ldr	x12, [sp, #0xc8]
10044b628: b9000188    	str	w8, [x12]
10044b62c: b84bb168    	ldur	w8, [x11, #0xbb]
10044b630: b8003188    	stur	w8, [x12, #0x3]
10044b634: f81183a9    	stur	x9, [x29, #-0xe8]
10044b638: f9400142    	ldr	x2, [x10]
10044b63c: d103c3a3    	sub	x3, x29, #0xf0
10044b640: 94001c14    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b644: 14000562    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b648: 7200053f    	tst	w9, #0x3
10044b64c: 54000180    	b.eq	0x10044b67c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36e8>
10044b650: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b654: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b658: f94077e9    	ldr	x9, [sp, #0xe8]
10044b65c: f9000128    	str	x8, [x9]
10044b660: f849f168    	ldur	x8, [x11, #0x9f]
10044b664: f8007128    	stur	x8, [x9, #0x7]
10044b668: d10343a2    	sub	x2, x29, #0xd0
10044b66c: aa0303e0    	mov	x0, x3
10044b670: aa0403e1    	mov	x1, x4
10044b674: 94001be9    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b678: 14000555    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b67c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b680: f9400102    	ldr	x2, [x8]
10044b684: d10343a0    	sub	x0, x29, #0xd0
10044b688: aa1303e1    	mov	x1, x19
10044b68c: 94001c56    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b690: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b694: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b698: 7100091f    	cmp	w8, #0x2
10044b69c: 54016680    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b6a0: 92401c09    	and	x9, x0, #0xff
10044b6a4: f100293f    	cmp	x9, #0xa
10044b6a8: f9408fe1    	ldr	x1, [sp, #0x118]
10044b6ac: f94087e9    	ldr	x9, [sp, #0x108]
10044b6b0: f9407fea    	ldr	x10, [sp, #0xf8]
10044b6b4: 540189c0    	b.eq	0x10044e7ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6858>
10044b6b8: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b6bc: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b6c0: f9400142    	ldr	x2, [x10]
10044b6c4: d103c3a3    	sub	x3, x29, #0xf0
10044b6c8: aa0903e0    	mov	x0, x9
10044b6cc: 94001bf1    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b6d0: 1400053f    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b6d4: 7200053f    	tst	w9, #0x3
10044b6d8: 54000180    	b.eq	0x10044b708 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3774>
10044b6dc: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b6e0: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b6e4: f94077e9    	ldr	x9, [sp, #0xe8]
10044b6e8: f9000128    	str	x8, [x9]
10044b6ec: f849f168    	ldur	x8, [x11, #0x9f]
10044b6f0: f8007128    	stur	x8, [x9, #0x7]
10044b6f4: d10343a2    	sub	x2, x29, #0xd0
10044b6f8: aa0303e0    	mov	x0, x3
10044b6fc: aa0403e1    	mov	x1, x4
10044b700: 94001bc6    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b704: 14000532    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b708: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b70c: f9400102    	ldr	x2, [x8]
10044b710: d10343a0    	sub	x0, x29, #0xd0
10044b714: aa1303e1    	mov	x1, x19
10044b718: 94001c33    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b71c: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b720: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b724: 7100091f    	cmp	w8, #0x2
10044b728: 54016220    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b72c: 92401c08    	and	x8, x0, #0xff
10044b730: f100291f    	cmp	x8, #0xa
10044b734: f9408fe1    	ldr	x1, [sp, #0x118]
10044b738: f94087e9    	ldr	x9, [sp, #0x108]
10044b73c: f9407fea    	ldr	x10, [sp, #0xf8]
10044b740: 540184e0    	b.eq	0x10044e7dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6848>
10044b744: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b748: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b74c: f9400142    	ldr	x2, [x10]
10044b750: d103c3a3    	sub	x3, x29, #0xf0
10044b754: aa0903e0    	mov	x0, x9
10044b758: 94001bce    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b75c: 1400051c    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b760: 7200053f    	tst	w9, #0x3
10044b764: 54000180    	b.eq	0x10044b794 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3800>
10044b768: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b76c: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b770: f94077e9    	ldr	x9, [sp, #0xe8]
10044b774: f9000128    	str	x8, [x9]
10044b778: f849f168    	ldur	x8, [x11, #0x9f]
10044b77c: f8007128    	stur	x8, [x9, #0x7]
10044b780: d10343a2    	sub	x2, x29, #0xd0
10044b784: aa0303e0    	mov	x0, x3
10044b788: aa0403e1    	mov	x1, x4
10044b78c: 94001ba3    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b790: 1400050f    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b794: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b798: f9400102    	ldr	x2, [x8]
10044b79c: d10343a0    	sub	x0, x29, #0xd0
10044b7a0: 94001c11    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b7a4: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b7a8: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b7ac: 7100091f    	cmp	w8, #0x2
10044b7b0: 54015de0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b7b4: 92401c08    	and	x8, x0, #0xff
10044b7b8: f100291f    	cmp	x8, #0xa
10044b7bc: f9408fe1    	ldr	x1, [sp, #0x118]
10044b7c0: f94087e9    	ldr	x9, [sp, #0x108]
10044b7c4: f9407fea    	ldr	x10, [sp, #0xf8]
10044b7c8: 54012f80    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044b7cc: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b7d0: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b7d4: f9400142    	ldr	x2, [x10]
10044b7d8: d103c3a3    	sub	x3, x29, #0xf0
10044b7dc: aa0903e0    	mov	x0, x9
10044b7e0: 94001bac    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b7e4: 140004fa    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b7e8: 7200053f    	tst	w9, #0x3
10044b7ec: 54000180    	b.eq	0x10044b81c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3888>
10044b7f0: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b7f4: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b7f8: f94077e9    	ldr	x9, [sp, #0xe8]
10044b7fc: f9000128    	str	x8, [x9]
10044b800: f849f168    	ldur	x8, [x11, #0x9f]
10044b804: f8007128    	stur	x8, [x9, #0x7]
10044b808: d10343a2    	sub	x2, x29, #0xd0
10044b80c: aa0303e0    	mov	x0, x3
10044b810: aa0403e1    	mov	x1, x4
10044b814: 94001b81    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b818: 140004ed    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b81c: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b820: f9400102    	ldr	x2, [x8]
10044b824: d10343a0    	sub	x0, x29, #0xd0
10044b828: 94001c86    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044b82c: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044b830: 71002d1f    	cmp	w8, #0xb
10044b834: 54012be0    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044b838: f94077ea    	ldr	x10, [sp, #0xe8]
10044b83c: b9400149    	ldr	w9, [x10]
10044b840: b90313e9    	str	w9, [sp, #0x310]
10044b844: b8403149    	ldur	w9, [x10, #0x3]
10044b848: 910963eb    	add	x11, sp, #0x258
10044b84c: b80bb169    	stur	w9, [x11, #0xbb]
10044b850: 7100291f    	cmp	w8, #0xa
10044b854: f9408fe1    	ldr	x1, [sp, #0x118]
10044b858: f94087e0    	ldr	x0, [sp, #0x108]
10044b85c: f9407fea    	ldr	x10, [sp, #0xf8]
10044b860: 540131a0    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044b864: f85383a9    	ldur	x9, [x29, #-0xc8]
10044b868: 381103a8    	sturb	w8, [x29, #-0xf0]
10044b86c: b94313e8    	ldr	w8, [sp, #0x310]
10044b870: f94067ec    	ldr	x12, [sp, #0xc8]
10044b874: b9000188    	str	w8, [x12]
10044b878: b84bb168    	ldur	w8, [x11, #0xbb]
10044b87c: b8003188    	stur	w8, [x12, #0x3]
10044b880: f81183a9    	stur	x9, [x29, #-0xe8]
10044b884: f9400142    	ldr	x2, [x10]
10044b888: d103c3a3    	sub	x3, x29, #0xf0
10044b88c: 94001b81    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b890: 140004cf    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b894: 7200053f    	tst	w9, #0x3
10044b898: 54000180    	b.eq	0x10044b8c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3934>
10044b89c: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b8a0: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b8a4: f94077e9    	ldr	x9, [sp, #0xe8]
10044b8a8: f9000128    	str	x8, [x9]
10044b8ac: f849f168    	ldur	x8, [x11, #0x9f]
10044b8b0: f8007128    	stur	x8, [x9, #0x7]
10044b8b4: d10343a2    	sub	x2, x29, #0xd0
10044b8b8: aa0303e0    	mov	x0, x3
10044b8bc: aa0403e1    	mov	x1, x4
10044b8c0: 94001b56    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b8c4: 140004c2    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b8c8: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b8cc: f9400102    	ldr	x2, [x8]
10044b8d0: d10343a0    	sub	x0, x29, #0xd0
10044b8d4: 94001c5b    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044b8d8: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044b8dc: 71002d1f    	cmp	w8, #0xb
10044b8e0: 54012680    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044b8e4: f94077ea    	ldr	x10, [sp, #0xe8]
10044b8e8: b9400149    	ldr	w9, [x10]
10044b8ec: b90313e9    	str	w9, [sp, #0x310]
10044b8f0: b8403149    	ldur	w9, [x10, #0x3]
10044b8f4: 910963eb    	add	x11, sp, #0x258
10044b8f8: b80bb169    	stur	w9, [x11, #0xbb]
10044b8fc: 7100291f    	cmp	w8, #0xa
10044b900: f9408fe1    	ldr	x1, [sp, #0x118]
10044b904: f94087e0    	ldr	x0, [sp, #0x108]
10044b908: f9407fea    	ldr	x10, [sp, #0xf8]
10044b90c: 54012c40    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044b910: f85383a9    	ldur	x9, [x29, #-0xc8]
10044b914: 381103a8    	sturb	w8, [x29, #-0xf0]
10044b918: b94313e8    	ldr	w8, [sp, #0x310]
10044b91c: f94067ec    	ldr	x12, [sp, #0xc8]
10044b920: b9000188    	str	w8, [x12]
10044b924: b84bb168    	ldur	w8, [x11, #0xbb]
10044b928: b8003188    	stur	w8, [x12, #0x3]
10044b92c: f81183a9    	stur	x9, [x29, #-0xe8]
10044b930: f9400142    	ldr	x2, [x10]
10044b934: d103c3a3    	sub	x3, x29, #0xf0
10044b938: 94001b56    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b93c: 140004a4    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b940: 7200053f    	tst	w9, #0x3
10044b944: 54000180    	b.eq	0x10044b974 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39e0>
10044b948: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b94c: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b950: f94077e9    	ldr	x9, [sp, #0xe8]
10044b954: f9000128    	str	x8, [x9]
10044b958: f849f168    	ldur	x8, [x11, #0x9f]
10044b95c: f8007128    	stur	x8, [x9, #0x7]
10044b960: d10343a2    	sub	x2, x29, #0xd0
10044b964: aa0303e0    	mov	x0, x3
10044b968: aa0403e1    	mov	x1, x4
10044b96c: 94001b2b    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b970: 14000497    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b974: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044b978: f9400102    	ldr	x2, [x8]
10044b97c: d10343a0    	sub	x0, x29, #0xd0
10044b980: 94001b99    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044b984: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044b988: f85303a0    	ldur	x0, [x29, #-0xd0]
10044b98c: 7100091f    	cmp	w8, #0x2
10044b990: 54014ee0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044b994: 92401c08    	and	x8, x0, #0xff
10044b998: f100291f    	cmp	x8, #0xa
10044b99c: f9408fe1    	ldr	x1, [sp, #0x118]
10044b9a0: f94087e9    	ldr	x9, [sp, #0x108]
10044b9a4: f9407fea    	ldr	x10, [sp, #0xf8]
10044b9a8: 54012080    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044b9ac: f85383a8    	ldur	x8, [x29, #-0xc8]
10044b9b0: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044b9b4: f9400142    	ldr	x2, [x10]
10044b9b8: d103c3a3    	sub	x3, x29, #0xf0
10044b9bc: aa0903e0    	mov	x0, x9
10044b9c0: 94001b34    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044b9c4: 14000482    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b9c8: 7200053f    	tst	w9, #0x3
10044b9cc: 54000180    	b.eq	0x10044b9fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3a68>
10044b9d0: 381303a8    	sturb	w8, [x29, #-0xd0]
10044b9d4: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b9d8: f94077e9    	ldr	x9, [sp, #0xe8]
10044b9dc: f9000128    	str	x8, [x9]
10044b9e0: f849f168    	ldur	x8, [x11, #0x9f]
10044b9e4: f8007128    	stur	x8, [x9, #0x7]
10044b9e8: d10343a2    	sub	x2, x29, #0xd0
10044b9ec: aa0303e0    	mov	x0, x3
10044b9f0: aa0403e1    	mov	x1, x4
10044b9f4: 94001b09    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044b9f8: 14000475    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044b9fc: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044ba00: f9400102    	ldr	x2, [x8]
10044ba04: d10343a0    	sub	x0, x29, #0xd0
10044ba08: 94001c0e    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044ba0c: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044ba10: 71002d1f    	cmp	w8, #0xb
10044ba14: 54011ce0    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044ba18: f94077ea    	ldr	x10, [sp, #0xe8]
10044ba1c: b9400149    	ldr	w9, [x10]
10044ba20: b90313e9    	str	w9, [sp, #0x310]
10044ba24: b8403149    	ldur	w9, [x10, #0x3]
10044ba28: 910963eb    	add	x11, sp, #0x258
10044ba2c: b80bb169    	stur	w9, [x11, #0xbb]
10044ba30: 7100291f    	cmp	w8, #0xa
10044ba34: f9408fe1    	ldr	x1, [sp, #0x118]
10044ba38: f94087e0    	ldr	x0, [sp, #0x108]
10044ba3c: f9407fea    	ldr	x10, [sp, #0xf8]
10044ba40: 540122a0    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044ba44: f85383a9    	ldur	x9, [x29, #-0xc8]
10044ba48: 381103a8    	sturb	w8, [x29, #-0xf0]
10044ba4c: b94313e8    	ldr	w8, [sp, #0x310]
10044ba50: f94067ec    	ldr	x12, [sp, #0xc8]
10044ba54: b9000188    	str	w8, [x12]
10044ba58: b84bb168    	ldur	w8, [x11, #0xbb]
10044ba5c: b8003188    	stur	w8, [x12, #0x3]
10044ba60: f81183a9    	stur	x9, [x29, #-0xe8]
10044ba64: f9400142    	ldr	x2, [x10]
10044ba68: d103c3a3    	sub	x3, x29, #0xf0
10044ba6c: 94001b09    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044ba70: 14000457    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044ba74: 7200053f    	tst	w9, #0x3
10044ba78: 54000180    	b.eq	0x10044baa8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b14>
10044ba7c: 381303a8    	sturb	w8, [x29, #-0xd0]
10044ba80: f9417be8    	ldr	x8, [sp, #0x2f0]
10044ba84: f94077e9    	ldr	x9, [sp, #0xe8]
10044ba88: f9000128    	str	x8, [x9]
10044ba8c: f849f168    	ldur	x8, [x11, #0x9f]
10044ba90: f8007128    	stur	x8, [x9, #0x7]
10044ba94: d10343a2    	sub	x2, x29, #0xd0
10044ba98: aa0303e0    	mov	x0, x3
10044ba9c: aa0403e1    	mov	x1, x4
10044baa0: 94001ade    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044baa4: 1400044a    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044baa8: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044baac: f9400102    	ldr	x2, [x8]
10044bab0: d10343a0    	sub	x0, x29, #0xd0
10044bab4: 94001b4c    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044bab8: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044babc: f85303a0    	ldur	x0, [x29, #-0xd0]
10044bac0: 7100091f    	cmp	w8, #0x2
10044bac4: 54014540    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044bac8: 92401c08    	and	x8, x0, #0xff
10044bacc: f100291f    	cmp	x8, #0xa
10044bad0: f9408fe1    	ldr	x1, [sp, #0x118]
10044bad4: f94087e9    	ldr	x9, [sp, #0x108]
10044bad8: f9407fea    	ldr	x10, [sp, #0xf8]
10044badc: 540116e0    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044bae0: f85383a8    	ldur	x8, [x29, #-0xc8]
10044bae4: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044bae8: f9400142    	ldr	x2, [x10]
10044baec: d103c3a3    	sub	x3, x29, #0xf0
10044baf0: aa0903e0    	mov	x0, x9
10044baf4: 94001ae7    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044baf8: 14000435    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bafc: 7200053f    	tst	w9, #0x3
10044bb00: 54000180    	b.eq	0x10044bb30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b9c>
10044bb04: 381303a8    	sturb	w8, [x29, #-0xd0]
10044bb08: f9417be8    	ldr	x8, [sp, #0x2f0]
10044bb0c: f94077e9    	ldr	x9, [sp, #0xe8]
10044bb10: f9000128    	str	x8, [x9]
10044bb14: f849f168    	ldur	x8, [x11, #0x9f]
10044bb18: f8007128    	stur	x8, [x9, #0x7]
10044bb1c: d10343a2    	sub	x2, x29, #0xd0
10044bb20: aa0303e0    	mov	x0, x3
10044bb24: aa0403e1    	mov	x1, x4
10044bb28: 94001abc    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044bb2c: 14000428    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bb30: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044bb34: f9400102    	ldr	x2, [x8]
10044bb38: d10343a0    	sub	x0, x29, #0xd0
10044bb3c: 94001bc1    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044bb40: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044bb44: 71002d1f    	cmp	w8, #0xb
10044bb48: 54011340    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044bb4c: f94077ea    	ldr	x10, [sp, #0xe8]
10044bb50: b9400149    	ldr	w9, [x10]
10044bb54: b90313e9    	str	w9, [sp, #0x310]
10044bb58: b8403149    	ldur	w9, [x10, #0x3]
10044bb5c: 910963eb    	add	x11, sp, #0x258
10044bb60: b80bb169    	stur	w9, [x11, #0xbb]
10044bb64: 7100291f    	cmp	w8, #0xa
10044bb68: f9408fe1    	ldr	x1, [sp, #0x118]
10044bb6c: f94087e0    	ldr	x0, [sp, #0x108]
10044bb70: f9407fea    	ldr	x10, [sp, #0xf8]
10044bb74: 54011900    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044bb78: f85383a9    	ldur	x9, [x29, #-0xc8]
10044bb7c: 381103a8    	sturb	w8, [x29, #-0xf0]
10044bb80: b94313e8    	ldr	w8, [sp, #0x310]
10044bb84: f94067ec    	ldr	x12, [sp, #0xc8]
10044bb88: b9000188    	str	w8, [x12]
10044bb8c: b84bb168    	ldur	w8, [x11, #0xbb]
10044bb90: b8003188    	stur	w8, [x12, #0x3]
10044bb94: f81183a9    	stur	x9, [x29, #-0xe8]
10044bb98: f9400142    	ldr	x2, [x10]
10044bb9c: d103c3a3    	sub	x3, x29, #0xf0
10044bba0: 94001abc    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044bba4: 1400040a    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bba8: 7200053f    	tst	w9, #0x3
10044bbac: 54000180    	b.eq	0x10044bbdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c48>
10044bbb0: 381303a8    	sturb	w8, [x29, #-0xd0]
10044bbb4: f9417be8    	ldr	x8, [sp, #0x2f0]
10044bbb8: f94077e9    	ldr	x9, [sp, #0xe8]
10044bbbc: f9000128    	str	x8, [x9]
10044bbc0: f849f168    	ldur	x8, [x11, #0x9f]
10044bbc4: f8007128    	stur	x8, [x9, #0x7]
10044bbc8: d10343a2    	sub	x2, x29, #0xd0
10044bbcc: aa0303e0    	mov	x0, x3
10044bbd0: aa0403e1    	mov	x1, x4
10044bbd4: 94001a91    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044bbd8: 140003fd    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bbdc: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044bbe0: f9400102    	ldr	x2, [x8]
10044bbe4: d10343a0    	sub	x0, x29, #0xd0
10044bbe8: 94001aff    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044bbec: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044bbf0: f85303a0    	ldur	x0, [x29, #-0xd0]
10044bbf4: 7100091f    	cmp	w8, #0x2
10044bbf8: 54013ba0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044bbfc: 92401c08    	and	x8, x0, #0xff
10044bc00: f100291f    	cmp	x8, #0xa
10044bc04: f9408fe1    	ldr	x1, [sp, #0x118]
10044bc08: f94087e9    	ldr	x9, [sp, #0x108]
10044bc0c: f9407fea    	ldr	x10, [sp, #0xf8]
10044bc10: 54010d40    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044bc14: f85383a8    	ldur	x8, [x29, #-0xc8]
10044bc18: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044bc1c: f9400142    	ldr	x2, [x10]
10044bc20: d103c3a3    	sub	x3, x29, #0xf0
10044bc24: aa0903e0    	mov	x0, x9
10044bc28: 94001a9a    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044bc2c: 140003e8    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bc30: 7200053f    	tst	w9, #0x3
10044bc34: 54000180    	b.eq	0x10044bc64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cd0>
10044bc38: 381303a8    	sturb	w8, [x29, #-0xd0]
10044bc3c: f9417be8    	ldr	x8, [sp, #0x2f0]
10044bc40: f94077e9    	ldr	x9, [sp, #0xe8]
10044bc44: f9000128    	str	x8, [x9]
10044bc48: f849f168    	ldur	x8, [x11, #0x9f]
10044bc4c: f8007128    	stur	x8, [x9, #0x7]
10044bc50: d10343a2    	sub	x2, x29, #0xd0
10044bc54: aa0303e0    	mov	x0, x3
10044bc58: aa0403e1    	mov	x1, x4
10044bc5c: 94001a6f    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044bc60: 140003db    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bc64: f9407fe8    	ldr	x8, [sp, #0xf8]
10044bc68: f9400102    	ldr	x2, [x8]
10044bc6c: d10343a0    	sub	x0, x29, #0xd0
10044bc70: aa1403e1    	mov	x1, x20
10044bc74: 94001adc    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044bc78: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044bc7c: f85303a0    	ldur	x0, [x29, #-0xd0]
10044bc80: 7100091f    	cmp	w8, #0x2
10044bc84: 54013740    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044bc88: 92401c08    	and	x8, x0, #0xff
10044bc8c: f100291f    	cmp	x8, #0xa
10044bc90: f9408fe1    	ldr	x1, [sp, #0x118]
10044bc94: f94087e9    	ldr	x9, [sp, #0x108]
10044bc98: f9407fea    	ldr	x10, [sp, #0xf8]
10044bc9c: 540150e0    	b.eq	0x10044e6b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6724>
10044bca0: f85383a8    	ldur	x8, [x29, #-0xc8]
10044bca4: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044bca8: f9400142    	ldr	x2, [x10]
10044bcac: d103c3a3    	sub	x3, x29, #0xf0
10044bcb0: aa0903e0    	mov	x0, x9
10044bcb4: 94001a77    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044bcb8: 140003c5    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bcbc: 7200053f    	tst	w9, #0x3
10044bcc0: 54000180    	b.eq	0x10044bcf0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d5c>
10044bcc4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044bcc8: f9417be8    	ldr	x8, [sp, #0x2f0]
10044bccc: f94077e9    	ldr	x9, [sp, #0xe8]
10044bcd0: f9000128    	str	x8, [x9]
10044bcd4: f849f168    	ldur	x8, [x11, #0x9f]
10044bcd8: f8007128    	stur	x8, [x9, #0x7]
10044bcdc: d10343a2    	sub	x2, x29, #0xd0
10044bce0: aa0303e0    	mov	x0, x3
10044bce4: aa0403e1    	mov	x1, x4
10044bce8: 94001a4c    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044bcec: 140003b8    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bcf0: f9407fe8    	ldr	x8, [sp, #0xf8]
10044bcf4: f9400102    	ldr	x2, [x8]
10044bcf8: d10343a0    	sub	x0, x29, #0xd0
10044bcfc: aa1403e1    	mov	x1, x20
10044bd00: 94001ab9    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044bd04: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044bd08: f85303a0    	ldur	x0, [x29, #-0xd0]
10044bd0c: 7100091f    	cmp	w8, #0x2
10044bd10: 540132e0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044bd14: 92401c08    	and	x8, x0, #0xff
10044bd18: f100291f    	cmp	x8, #0xa
10044bd1c: f9408fe1    	ldr	x1, [sp, #0x118]
10044bd20: f94087e9    	ldr	x9, [sp, #0x108]
10044bd24: f9407fea    	ldr	x10, [sp, #0xf8]
10044bd28: 54014c80    	b.eq	0x10044e6b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6724>
10044bd2c: f85383a8    	ldur	x8, [x29, #-0xc8]
10044bd30: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044bd34: f9400142    	ldr	x2, [x10]
10044bd38: d103c3a3    	sub	x3, x29, #0xf0
10044bd3c: aa0903e0    	mov	x0, x9
10044bd40: 94001a54    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044bd44: 140003a2    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bd48: b94005ad    	ldr	w13, [x13, #0x4]
10044bd4c: 710005af    	subs	w15, w13, #0x1
10044bd50: 1a9f77ee    	cset	w14, vs
10044bd54: 310005b0    	adds	w16, w13, #0x1
10044bd58: 1a9f77f1    	cset	w17, vs
10044bd5c: 720f029f    	tst	w20, #0x20000
10044bd60: 1a8e022e    	csel	w14, w17, w14, eq
10044bd64: 360025ae    	tbz	w14, #0x0, 0x10044c218 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4284>
10044bd68: 5280000e    	mov	w14, #0x0               ; =0
10044bd6c: 720f029f    	tst	w20, #0x20000
10044bd70: 1e7e1000    	fmov	d0, #-1.00000000
10044bd74: 1e6e1001    	fmov	d1, #1.00000000
10044bd78: 1e600c20    	fcsel	d0, d1, d0, eq
10044bd7c: 1e6201a1    	scvtf	d1, w13
10044bd80: 1e61280a    	fadd	d10, d0, d1
10044bd84: 52800031    	mov	w17, #0x1               ; =1
10044bd88: 14000128    	b	0x10044c228 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4294>
10044bd8c: fd400760    	ldr	d0, [x27, #0x8]
10044bd90: f9407fe8    	ldr	x8, [sp, #0xf8]
10044bd94: f9400100    	ldr	x0, [x8]
10044bd98: aa1403e1    	mov	x1, x20
10044bd9c: aa1603e2    	mov	x2, x22
10044bda0: 94002403    	bl	0x100454dac <__ZN13quickjs_oxide6engine8builtins12array_buffer11typed_array62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$28try_typed_array_number_write17hf73ee774e7469726E>
10044bda4: f9408fe3    	ldr	x3, [sp, #0x118]
10044bda8: f94087e9    	ldr	x9, [sp, #0x108]
10044bdac: 34010be0    	cbz	w0, 0x10044df28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f94>
10044bdb0: a9408921    	ldp	x1, x2, [x9, #0x8]
10044bdb4: d103c3a0    	sub	x0, x29, #0xf0
10044bdb8: 97ff9192    	bl	0x100430400 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
10044bdbc: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044bdc0: 7100291f    	cmp	w8, #0xa
10044bdc4: 54011780    	b.eq	0x10044e0b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6120>
10044bdc8: f94087e8    	ldr	x8, [sp, #0x108]
10044bdcc: a9408901    	ldp	x1, x2, [x8, #0x8]
10044bdd0: d103c3a0    	sub	x0, x29, #0xf0
10044bdd4: f9408fe3    	ldr	x3, [sp, #0x118]
10044bdd8: 97ff918a    	bl	0x100430400 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
10044bddc: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044bde0: 7100291f    	cmp	w8, #0xa
10044bde4: 54011680    	b.eq	0x10044e0b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6120>
10044bde8: f94087e8    	ldr	x8, [sp, #0x108]
10044bdec: a9408901    	ldp	x1, x2, [x8, #0x8]
10044bdf0: d103c3a0    	sub	x0, x29, #0xf0
10044bdf4: f9408fe3    	ldr	x3, [sp, #0x118]
10044bdf8: 97ff9182    	bl	0x100430400 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
10044bdfc: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044be00: 7100291f    	cmp	w8, #0xa
10044be04: 54011580    	b.eq	0x10044e0b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6120>
10044be08: f94067eb    	ldr	x11, [sp, #0xc8]
10044be0c: b9400169    	ldr	w9, [x11]
10044be10: f94077ea    	ldr	x10, [sp, #0xe8]
10044be14: b9000149    	str	w9, [x10]
10044be18: b8403169    	ldur	w9, [x11, #0x3]
10044be1c: b8003149    	stur	w9, [x10, #0x3]
10044be20: f85183a9    	ldur	x9, [x29, #-0xe8]
10044be24: 381303a8    	sturb	w8, [x29, #-0xd0]
10044be28: f81383a9    	stur	x9, [x29, #-0xc8]
10044be2c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044be30: f9400101    	ldr	x1, [x8]
10044be34: 910a83e0    	add	x0, sp, #0x2a0
10044be38: d10343a2    	sub	x2, x29, #0xd0
10044be3c: 97f02ccc    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
10044be40: 394a83e8    	ldrb	w8, [sp, #0x2a0]
10044be44: 71002d1f    	cmp	w8, #0xb
10044be48: 54014441    	b.ne	0x10044e6d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x673c>
10044be4c: f94097e8    	ldr	x8, [sp, #0x128]
10044be50: f9000513    	str	x19, [x8, #0x8]
10044be54: 1400035f    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044be58: d10343a0    	sub	x0, x29, #0xd0
10044be5c: aa0203e1    	mov	x1, x2
10044be60: aa0303e2    	mov	x2, x3
10044be64: 94001f94    	bl	0x100453cb4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
10044be68: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044be6c: 7100291f    	cmp	w8, #0xa
10044be70: 5400fa00    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044be74: f94077ea    	ldr	x10, [sp, #0xe8]
10044be78: b9400149    	ldr	w9, [x10]
10044be7c: f94067eb    	ldr	x11, [sp, #0xc8]
10044be80: b9000169    	str	w9, [x11]
10044be84: b8403149    	ldur	w9, [x10, #0x3]
10044be88: b8003169    	stur	w9, [x11, #0x3]
10044be8c: f85383a9    	ldur	x9, [x29, #-0xc8]
10044be90: 381103a8    	sturb	w8, [x29, #-0xf0]
10044be94: f81183a9    	stur	x9, [x29, #-0xe8]
10044be98: d103c3a0    	sub	x0, x29, #0xf0
10044be9c: 940024a3    	bl	0x100455128 <__ZN13quickjs_oxide6engine5value8js_value7JsValue20to_boolean_primitive17h5fda868505f7d3b0E>
10044bea0: 52000008    	eor	w8, w0, #0x1
10044bea4: 381313a8    	sturb	w8, [x29, #-0xcf]
10044bea8: 52800048    	mov	w8, #0x2                ; =2
10044beac: 381303a8    	sturb	w8, [x29, #-0xd0]
10044beb0: d10343a2    	sub	x2, x29, #0xd0
10044beb4: f94087e0    	ldr	x0, [sp, #0x108]
10044beb8: f9408fe1    	ldr	x1, [sp, #0x118]
10044bebc: 94001991    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044bec0: 14000343    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044bec4: d10343a0    	sub	x0, x29, #0xd0
10044bec8: aa0203e1    	mov	x1, x2
10044becc: 9102e342    	add	x2, x26, #0xb8
10044bed0: 97ff789a    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044bed4: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044bed8: 7100291f    	cmp	w8, #0xa
10044bedc: 54012f00    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044bee0: f9407fe9    	ldr	x9, [sp, #0xf8]
10044bee4: f9400122    	ldr	x2, [x9]
10044bee8: f9408fe1    	ldr	x1, [sp, #0x118]
10044beec: f94087e0    	ldr	x0, [sp, #0x108]
10044bef0: f94077ea    	ldr	x10, [sp, #0xe8]
10044bef4: 17fffc0e    	b	0x10044af2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2f98>
10044bef8: f9408be8    	ldr	x8, [sp, #0x110]
10044befc: f9400109    	ldr	x9, [x8]
10044bf00: f9403928    	ldr	x8, [x9, #0x70]
10044bf04: b4014ec8    	cbz	x8, 0x10044e8dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6948>
10044bf08: f9403d2a    	ldr	x10, [x9, #0x78]
10044bf0c: f94083e9    	ldr	x9, [sp, #0x100]
10044bf10: 2a0903e9    	mov	w9, w9
10044bf14: eb09015f    	cmp	x10, x9
10044bf18: 54014e29    	b.ls	0x10044e8dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6948>
10044bf1c: 5280090a    	mov	w10, #0x48              ; =72
10044bf20: 9baa2128    	umaddl	x8, w9, w10, x8
10044bf24: 91004116    	add	x22, x8, #0x10
10044bf28: 3940a11b    	ldrb	w27, [x8, #0x28]
10044bf2c: 71000b7f    	cmp	w27, #0x2
10044bf30: 54005d22    	b.hs	0x10044cad4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b40>
10044bf34: 79408ac8    	ldrh	w8, [x22, #0x44]
10044bf38: f9408fe9    	ldr	x9, [sp, #0x118]
10044bf3c: f9402129    	ldr	x9, [x9, #0x40]
10044bf40: ab08013f    	cmn	x9, x8
10044bf44: 54008ec2    	b.hs	0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044bf48: 8b080128    	add	x8, x9, x8
10044bf4c: f9408fea    	ldr	x10, [sp, #0x118]
10044bf50: a943254a    	ldp	x10, x9, [x10, #0x30]
10044bf54: cb0a0129    	sub	x9, x9, x10
10044bf58: eb09011f    	cmp	x8, x9
10044bf5c: 54008e08    	b.hi	0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044bf60: 794036c8    	ldrh	w8, [x22, #0x1a]
10044bf64: b9003be8    	str	w8, [sp, #0x38]
10044bf68: 294422c9    	ldp	w9, w8, [x22, #0x20]
10044bf6c: b90103e9    	str	w9, [sp, #0x100]
10044bf70: b90043e8    	str	w8, [sp, #0x40]
10044bf74: f94016d4    	ldr	x20, [x22, #0x28]
10044bf78: b94002c8    	ldr	w8, [x22]
10044bf7c: 51000908    	sub	w8, w8, #0x2
10044bf80: 52800049    	mov	w9, #0x2                ; =2
10044bf84: 7100091f    	cmp	w8, #0x2
10044bf88: 1a893108    	csel	w8, w8, w9, lo
10044bf8c: 7100091f    	cmp	w8, #0x2
10044bf90: 54007ba0    	b.eq	0x10044cf04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f70>
10044bf94: 7100051f    	cmp	w8, #0x1
10044bf98: 54007bc1    	b.ne	0x10044cf10 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f7c>
10044bf9c: bd4006c0    	ldr	s0, [x22, #0x4]
10044bfa0: 0f20a400    	sshll.2d	v0, v0, #0x0
10044bfa4: 5e61d80a    	scvtf	d10, d0
10044bfa8: 14000455    	b	0x10044d0fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5168>
10044bfac: b85343a8    	ldur	w8, [x29, #-0xcc]
10044bfb0: 52800069    	mov	w9, #0x3                ; =3
10044bfb4: 390b43e9    	strb	w9, [sp, #0x2d0]
10044bfb8: b902d7e8    	str	w8, [sp, #0x2d4]
10044bfbc: f94083f3    	ldr	x19, [sp, #0x100]
10044bfc0: f9401668    	ldr	x8, [x19, #0x28]
10044bfc4: 91000508    	add	x8, x8, #0x1
10044bfc8: f9001668    	str	x8, [x19, #0x28]
10044bfcc: 1400004f    	b	0x10044c108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4174>
10044bfd0: f94001a1    	ldr	x1, [x13]
10044bfd4: d10343a0    	sub	x0, x29, #0xd0
10044bfd8: 97ff7858    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044bfdc: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044bfe0: 7100291f    	cmp	w8, #0xa
10044bfe4: f94087ec    	ldr	x12, [sp, #0x108]
10044bfe8: f9407fed    	ldr	x13, [sp, #0xf8]
10044bfec: 910963ee    	add	x14, sp, #0x258
10044bff0: f94077ea    	ldr	x10, [sp, #0xe8]
10044bff4: 54ff83c1    	b.ne	0x10044b06c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30d8>
10044bff8: 1400076e    	b	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044bffc: 7200053f    	tst	w9, #0x3
10044c000: 54000fe1    	b.ne	0x10044c1fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4268>
10044c004: 1400001a    	b	0x10044c06c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40d8>
10044c008: f94137e9    	ldr	x9, [sp, #0x268]
10044c00c: 71000d1f    	cmp	w8, #0x3
10044c010: 5400f3e0    	b.eq	0x10044de8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ef8>
10044c014: 36006028    	tbz	w8, #0x0, 0x10044cc18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c84>
10044c018: f81383a9    	stur	x9, [x29, #-0xc8]
10044c01c: 52800088    	mov	w8, #0x4                ; =4
10044c020: 14000301    	b	0x10044cc24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c90>
10044c024: f9413fe8    	ldr	x8, [sp, #0x278]
10044c028: 71002d3f    	cmp	w9, #0xb
10044c02c: 54010b20    	b.eq	0x10044e190 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61fc>
10044c030: 390643e9    	strb	w9, [sp, #0x190]
10044c034: a945abeb    	ldp	x11, x10, [sp, #0x58]
10044c038: b9400149    	ldr	w9, [x10]
10044c03c: b9000169    	str	w9, [x11]
10044c040: b8403149    	ldur	w9, [x10, #0x3]
10044c044: b8003169    	stur	w9, [x11, #0x3]
10044c048: f900cfe8    	str	x8, [sp, #0x198]
10044c04c: 910643e2    	add	x2, sp, #0x190
10044c050: f94087e0    	ldr	x0, [sp, #0x108]
10044c054: f9408fe1    	ldr	x1, [sp, #0x118]
10044c058: 9400192a    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044c05c: 140002f7    	b	0x10044cc38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ca4>
10044c060: 7100253f    	cmp	w9, #0x9
10044c064: 54014bc1    	b.ne	0x10044e9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a48>
10044c068: f94001a1    	ldr	x1, [x13]
10044c06c: d10343a0    	sub	x0, x29, #0xd0
10044c070: 9102a342    	add	x2, x26, #0xa8
10044c074: 97ff7831    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044c078: 140002be    	b	0x10044cb70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bdc>
10044c07c: 381303bf    	sturb	wzr, [x29, #-0xd0]
10044c080: d10343a2    	sub	x2, x29, #0xd0
10044c084: 9400191f    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044c088: 140002d1    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c08c: fd400900    	ldr	d0, [x8, #0x10]
10044c090: fc1383a0    	stur	d0, [x29, #-0xc8]
10044c094: 52800088    	mov	w8, #0x4                ; =4
10044c098: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c09c: d10343a2    	sub	x2, x29, #0xd0
10044c0a0: 94001918    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044c0a4: 140002ca    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c0a8: 39402508    	ldrb	w8, [x8, #0x9]
10044c0ac: 381313a8    	sturb	w8, [x29, #-0xcf]
10044c0b0: 52800048    	mov	w8, #0x2                ; =2
10044c0b4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c0b8: d10343a2    	sub	x2, x29, #0xd0
10044c0bc: 94001911    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044c0c0: 140002c3    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c0c4: f9400908    	ldr	x8, [x8, #0x10]
10044c0c8: f81383a8    	stur	x8, [x29, #-0xc8]
10044c0cc: 528000e8    	mov	w8, #0x7                ; =7
10044c0d0: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c0d4: d10343a2    	sub	x2, x29, #0xd0
10044c0d8: 9400190a    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044c0dc: 140002bc    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c0e0: 91002101    	add	x1, x8, #0x8
10044c0e4: 910b43e0    	add	x0, sp, #0x2d0
10044c0e8: 940025b8    	bl	0x1004557c8 <__ZN13quickjs_oxide6engine6object16ordinary_storage23immediate_value_jsvalue17hc5220bf7094f18e2E>
10044c0ec: 394b43e8    	ldrb	w8, [sp, #0x2d0]
10044c0f0: f94083f3    	ldr	x19, [sp, #0x100]
10044c0f4: f9401669    	ldr	x9, [x19, #0x28]
10044c0f8: 91000529    	add	x9, x9, #0x1
10044c0fc: f9001669    	str	x9, [x19, #0x28]
10044c100: 7100291f    	cmp	w8, #0xa
10044c104: 54014500    	b.eq	0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044c108: 3dc0b7e0    	ldr	q0, [sp, #0x2d0]
10044c10c: 3d80b3e0    	str	q0, [sp, #0x2c0]
10044c110: 910006c8    	add	x8, x22, #0x1
10044c114: eb1c011f    	cmp	x8, x28
10044c118: 54017222    	b.hs	0x10044ef5c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fc8>
10044c11c: f94017e9    	ldr	x9, [sp, #0x28]
10044c120: 8b081128    	add	x8, x9, x8, lsl #4
10044c124: 39400109    	ldrb	w9, [x8]
10044c128: 528001ca    	mov	w10, #0xe               ; =14
10044c12c: 3900010a    	strb	w10, [x8]
10044c130: 5100292a    	sub	w10, w9, #0xa
10044c134: 7100115f    	cmp	w10, #0x4
10044c138: f94023ee    	ldr	x14, [sp, #0x40]
10044c13c: 54016ea9    	b.ls	0x10044ef10 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f7c>
10044c140: 390b83e9    	strb	w9, [sp, #0x2e0]
10044c144: f8401109    	ldur	x9, [x8, #0x1]
10044c148: f9404fea    	ldr	x10, [sp, #0x98]
10044c14c: f9000149    	str	x9, [x10]
10044c150: f9400508    	ldr	x8, [x8, #0x8]
10044c154: f8007148    	stur	x8, [x10, #0x7]
10044c158: eb1c02df    	cmp	x22, x28
10044c15c: f9408fea    	ldr	x10, [sp, #0x118]
10044c160: f9407feb    	ldr	x11, [sp, #0xf8]
10044c164: f9401bed    	ldr	x13, [sp, #0x30]
10044c168: 54017002    	b.hs	0x10044ef68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fd4>
10044c16c: 394001c8    	ldrb	w8, [x14]
10044c170: f84011c9    	ldur	x9, [x14, #0x1]
10044c174: f81303a9    	stur	x9, [x29, #-0xd0]
10044c178: f94005c9    	ldr	x9, [x14, #0x8]
10044c17c: d10343ac    	sub	x12, x29, #0xd0
10044c180: f8007189    	stur	x9, [x12, #0x7]
10044c184: 3dc0b3e0    	ldr	q0, [sp, #0x2c0]
10044c188: 3d8001c0    	str	q0, [x14]
10044c18c: 51002909    	sub	w9, w8, #0xa
10044c190: 7100113f    	cmp	w9, #0x4
10044c194: 54016ca9    	b.ls	0x10044ef28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f94>
10044c198: 381003a8    	sturb	w8, [x29, #-0x100]
10044c19c: f85303a8    	ldur	x8, [x29, #-0xd0]
10044c1a0: f9404be9    	ldr	x9, [sp, #0x90]
10044c1a4: f9000128    	str	x8, [x9]
10044c1a8: f8407188    	ldur	x8, [x12, #0x7]
10044c1ac: f8007128    	stur	x8, [x9, #0x7]
10044c1b0: f900214d    	str	x13, [x10, #0x40]
10044c1b4: f9400161    	ldr	x1, [x11]
10044c1b8: 910bc3e0    	add	x0, sp, #0x2f0
10044c1bc: 910b83e2    	add	x2, sp, #0x2e0
10044c1c0: 97f02beb    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
10044c1c4: 394bc3e8    	ldrb	w8, [sp, #0x2f0]
10044c1c8: 71002d1f    	cmp	w8, #0xb
10044c1cc: 540123c1    	b.ne	0x10044e644 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66b0>
10044c1d0: f9407fe8    	ldr	x8, [sp, #0xf8]
10044c1d4: f9400101    	ldr	x1, [x8]
10044c1d8: d103c3a0    	sub	x0, x29, #0xf0
10044c1dc: d10403a2    	sub	x2, x29, #0x100
10044c1e0: 97f02be3    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
10044c1e4: 385103a8    	ldurb	w8, [x29, #-0xf0]
10044c1e8: 71002d1f    	cmp	w8, #0xb
10044c1ec: 54004f20    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044c1f0: 14000924    	b	0x10044e680 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66ec>
10044c1f4: 7200055f    	tst	w10, #0x3
10044c1f8: 54004b80    	b.eq	0x10044cb68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bd4>
10044c1fc: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c200: f85103a9    	ldur	x9, [x29, #-0xf0]
10044c204: f94077ea    	ldr	x10, [sp, #0xe8]
10044c208: f9000149    	str	x9, [x10]
10044c20c: f84ff1c9    	ldur	x9, [x14, #0xff]
10044c210: f8007149    	stur	x9, [x10, #0x7]
10044c214: 1400025f    	b	0x10044cb90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bfc>
10044c218: 5280000e    	mov	w14, #0x0               ; =0
10044c21c: 52800011    	mov	w17, #0x0               ; =0
10044c220: 720f029f    	tst	w20, #0x20000
10044c224: 1a8f020f    	csel	w15, w16, w15, eq
10044c228: 12001d4a    	and	w10, w10, #0xff
10044c22c: 71000d5f    	cmp	w10, #0x3
10044c230: 1a8f31a3    	csel	w3, w13, w15, lo
10044c234: 1a9131ca    	csel	w10, w14, w17, lo
10044c238: 37f804e3    	tbnz	w3, #0x1f, 0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
10044c23c: 370004ca    	tbnz	w10, #0x0, 0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
10044c240: 5280ffca    	mov	w10, #0x7fe             ; =2046
10044c244: 1ac82548    	lsr	w8, w10, w8
10044c248: 2a080128    	orr	w8, w9, w8
10044c24c: 7200011f    	tst	w8, #0x1
10044c250: f9406be8    	ldr	x8, [sp, #0xd0]
10044c254: f94073ea    	ldr	x10, [sp, #0xe0]
10044c258: 9a8a1108    	csel	x8, x8, x10, ne
10044c25c: f9407be9    	ldr	x9, [sp, #0xf0]
10044c260: 9a891149    	csel	x9, x10, x9, ne
10044c264: f9400121    	ldr	x1, [x9]
10044c268: f9400116    	ldr	x22, [x8]
10044c26c: eb160028    	subs	x8, x1, x22
10044c270: 54013aa3    	b.lo	0x10044e9c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a30>
10044c274: eb0b003f    	cmp	x1, x11
10044c278: 54014148    	b.hi	0x10044eaa0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b0c>
10044c27c: f94083e9    	ldr	x9, [sp, #0x100]
10044c280: 12003d29    	and	w9, w9, #0xffff
10044c284: eb09011f    	cmp	x8, x9
10044c288: 54000269    	b.ls	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
10044c28c: 8b161188    	add	x8, x12, x22, lsl #4
10044c290: 8b091102    	add	x2, x8, x9, lsl #4
10044c294: 39400048    	ldrb	w8, [x2]
10044c298: 7100251f    	cmp	w8, #0x9
10044c29c: 540001c8    	b.hi	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
10044c2a0: b9003bf1    	str	w17, [sp, #0x38]
10044c2a4: b90033ef    	str	w15, [sp, #0x30]
10044c2a8: f90023e0    	str	x0, [sp, #0x40]
10044c2ac: f9407fe8    	ldr	x8, [sp, #0xf8]
10044c2b0: f9400101    	ldr	x1, [x8]
10044c2b4: d10343a0    	sub	x0, x29, #0xd0
10044c2b8: 94002571    	bl	0x10045587c <__ZN13quickjs_oxide6engine6object16ordinary_storage62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$24peek_dense_number_result17hacaa150a333ea219E>
10044c2bc: b85303a8    	ldur	w8, [x29, #-0xd0]
10044c2c0: 7100051f    	cmp	w8, #0x1
10044c2c4: 54000080    	b.eq	0x10044c2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4340>
10044c2c8: b85383a8    	ldur	w8, [x29, #-0xc8]
10044c2cc: 7100091f    	cmp	w8, #0x2
10044c2d0: 5400aaa1    	b.ne	0x10044d824 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5890>
10044c2d4: 71036b7f    	cmp	w27, #0xda
10044c2d8: 540002e1    	b.ne	0x10044c334 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43a0>
10044c2dc: a95003e3    	ldp	x3, x0, [sp, #0x100]
10044c2e0: f9408fe1    	ldr	x1, [sp, #0x118]
10044c2e4: 52800002    	mov	w2, #0x0                ; =0
10044c2e8: 940018a4    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c2ec: f9408fe4    	ldr	x4, [sp, #0x118]
10044c2f0: f94087e3    	ldr	x3, [sp, #0x108]
10044c2f4: f9407feb    	ldr	x11, [sp, #0xf8]
10044c2f8: 910963ec    	add	x12, sp, #0x258
10044c2fc: b4000940    	cbz	x0, 0x10044c424 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4490>
10044c300: 39400008    	ldrb	w8, [x0]
10044c304: 71001d1f    	cmp	w8, #0x7
10044c308: 540008e8    	b.hi	0x10044c424 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4490>
10044c30c: 52800029    	mov	w9, #0x1                ; =1
10044c310: 1ac82129    	lsl	w9, w9, w8
10044c314: 5280138a    	mov	w10, #0x9c              ; =156
10044c318: 6a0a013f    	tst	w9, w10
10044c31c: 540006a0    	b.eq	0x10044c3f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x445c>
10044c320: f8401009    	ldur	x9, [x0, #0x1]
10044c324: f9017be9    	str	x9, [sp, #0x2f0]
10044c328: f9400409    	ldr	x9, [x0, #0x8]
10044c32c: f809f189    	stur	x9, [x12, #0x9f]
10044c330: 14000032    	b	0x10044c3f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4464>
10044c334: 71033b7f    	cmp	w27, #0xce
10044c338: f9408fe1    	ldr	x1, [sp, #0x118]
10044c33c: f94087e0    	ldr	x0, [sp, #0x108]
10044c340: 54000060    	b.eq	0x10044c34c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43b8>
10044c344: 7103677f    	cmp	w27, #0xd9
10044c348: 540002c1    	b.ne	0x10044c3a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x440c>
10044c34c: 52800002    	mov	w2, #0x0                ; =0
10044c350: f94083e3    	ldr	x3, [sp, #0x100]
10044c354: 94001889    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c358: f9408fe4    	ldr	x4, [sp, #0x118]
10044c35c: f94087e3    	ldr	x3, [sp, #0x108]
10044c360: f9407feb    	ldr	x11, [sp, #0xf8]
10044c364: 910963ec    	add	x12, sp, #0x258
10044c368: b4000a20    	cbz	x0, 0x10044c4ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4518>
10044c36c: 39400008    	ldrb	w8, [x0]
10044c370: 71001d1f    	cmp	w8, #0x7
10044c374: 540009c8    	b.hi	0x10044c4ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4518>
10044c378: 52800029    	mov	w9, #0x1                ; =1
10044c37c: 1ac82129    	lsl	w9, w9, w8
10044c380: 5280138a    	mov	w10, #0x9c              ; =156
10044c384: 6a0a013f    	tst	w9, w10
10044c388: 54000780    	b.eq	0x10044c478 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
10044c38c: f8401009    	ldur	x9, [x0, #0x1]
10044c390: f9017be9    	str	x9, [sp, #0x2f0]
10044c394: f9400409    	ldr	x9, [x0, #0x8]
10044c398: f809f189    	stur	x9, [x12, #0x9f]
10044c39c: 14000039    	b	0x10044c480 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44ec>
10044c3a0: 52800022    	mov	w2, #0x1                ; =1
10044c3a4: f94083e3    	ldr	x3, [sp, #0x100]
10044c3a8: 94001874    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c3ac: f9408fe4    	ldr	x4, [sp, #0x118]
10044c3b0: f94087e3    	ldr	x3, [sp, #0x108]
10044c3b4: 910963eb    	add	x11, sp, #0x258
10044c3b8: b4000be0    	cbz	x0, 0x10044c534 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x45a0>
10044c3bc: 39400008    	ldrb	w8, [x0]
10044c3c0: 71001d1f    	cmp	w8, #0x7
10044c3c4: 54000b88    	b.hi	0x10044c534 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x45a0>
10044c3c8: 52800029    	mov	w9, #0x1                ; =1
10044c3cc: 1ac82129    	lsl	w9, w9, w8
10044c3d0: 5280138a    	mov	w10, #0x9c              ; =156
10044c3d4: 6a0a013f    	tst	w9, w10
10044c3d8: 54000940    	b.eq	0x10044c500 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x456c>
10044c3dc: f8401009    	ldur	x9, [x0, #0x1]
10044c3e0: f9017be9    	str	x9, [sp, #0x2f0]
10044c3e4: f9400409    	ldr	x9, [x0, #0x8]
10044c3e8: f809f169    	stur	x9, [x11, #0x9f]
10044c3ec: 14000047    	b	0x10044c508 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4574>
10044c3f0: 7200053f    	tst	w9, #0x3
10044c3f4: 54000180    	b.eq	0x10044c424 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4490>
10044c3f8: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c3fc: f9417be8    	ldr	x8, [sp, #0x2f0]
10044c400: f94077e9    	ldr	x9, [sp, #0xe8]
10044c404: f9000128    	str	x8, [x9]
10044c408: f849f188    	ldur	x8, [x12, #0x9f]
10044c40c: f8007128    	stur	x8, [x9, #0x7]
10044c410: d10343a2    	sub	x2, x29, #0xd0
10044c414: aa0303e0    	mov	x0, x3
10044c418: aa0403e1    	mov	x1, x4
10044c41c: 9400187f    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044c420: 140001eb    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c424: f9400162    	ldr	x2, [x11]
10044c428: d10343a0    	sub	x0, x29, #0xd0
10044c42c: f94083e1    	ldr	x1, [sp, #0x100]
10044c430: 940018ed    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044c434: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044c438: f85303a0    	ldur	x0, [x29, #-0xd0]
10044c43c: 7100091f    	cmp	w8, #0x2
10044c440: 5400f960    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044c444: 92401c09    	and	x9, x0, #0xff
10044c448: f100293f    	cmp	x9, #0xa
10044c44c: f9408fe1    	ldr	x1, [sp, #0x118]
10044c450: f94087e9    	ldr	x9, [sp, #0x108]
10044c454: f9407fea    	ldr	x10, [sp, #0xf8]
10044c458: 5400f960    	b.eq	0x10044e384 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63f0>
10044c45c: f85383a8    	ldur	x8, [x29, #-0xc8]
10044c460: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044c464: f9400142    	ldr	x2, [x10]
10044c468: d103c3a3    	sub	x3, x29, #0xf0
10044c46c: aa0903e0    	mov	x0, x9
10044c470: 94001888    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044c474: 140001d6    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c478: 7200053f    	tst	w9, #0x3
10044c47c: 54000180    	b.eq	0x10044c4ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4518>
10044c480: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c484: f9417be8    	ldr	x8, [sp, #0x2f0]
10044c488: f94077e9    	ldr	x9, [sp, #0xe8]
10044c48c: f9000128    	str	x8, [x9]
10044c490: f849f188    	ldur	x8, [x12, #0x9f]
10044c494: f8007128    	stur	x8, [x9, #0x7]
10044c498: d10343a2    	sub	x2, x29, #0xd0
10044c49c: aa0303e0    	mov	x0, x3
10044c4a0: aa0403e1    	mov	x1, x4
10044c4a4: 9400185d    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044c4a8: 140001c9    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c4ac: f9400162    	ldr	x2, [x11]
10044c4b0: d10343a0    	sub	x0, x29, #0xd0
10044c4b4: f94083e1    	ldr	x1, [sp, #0x100]
10044c4b8: 940018cb    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044c4bc: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044c4c0: f85303a0    	ldur	x0, [x29, #-0xd0]
10044c4c4: 7100091f    	cmp	w8, #0x2
10044c4c8: 5400f520    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044c4cc: 92401c08    	and	x8, x0, #0xff
10044c4d0: f100291f    	cmp	x8, #0xa
10044c4d4: f9408fe1    	ldr	x1, [sp, #0x118]
10044c4d8: f94087e9    	ldr	x9, [sp, #0x108]
10044c4dc: f9407fea    	ldr	x10, [sp, #0xf8]
10044c4e0: 5400c6c0    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044c4e4: f85383a8    	ldur	x8, [x29, #-0xc8]
10044c4e8: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044c4ec: f9400142    	ldr	x2, [x10]
10044c4f0: d103c3a3    	sub	x3, x29, #0xf0
10044c4f4: aa0903e0    	mov	x0, x9
10044c4f8: 94001866    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044c4fc: 140001b4    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c500: 7200053f    	tst	w9, #0x3
10044c504: 54000180    	b.eq	0x10044c534 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x45a0>
10044c508: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c50c: f9417be8    	ldr	x8, [sp, #0x2f0]
10044c510: f94077e9    	ldr	x9, [sp, #0xe8]
10044c514: f9000128    	str	x8, [x9]
10044c518: f849f168    	ldur	x8, [x11, #0x9f]
10044c51c: f8007128    	stur	x8, [x9, #0x7]
10044c520: d10343a2    	sub	x2, x29, #0xd0
10044c524: aa0303e0    	mov	x0, x3
10044c528: aa0403e1    	mov	x1, x4
10044c52c: 9400183b    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044c530: 140001a7    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c534: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044c538: f9400102    	ldr	x2, [x8]
10044c53c: d10343a0    	sub	x0, x29, #0xd0
10044c540: 94001940    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044c544: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044c548: 71002d1f    	cmp	w8, #0xb
10044c54c: 5400c320    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044c550: f94077ea    	ldr	x10, [sp, #0xe8]
10044c554: b9400149    	ldr	w9, [x10]
10044c558: b90313e9    	str	w9, [sp, #0x310]
10044c55c: b8403149    	ldur	w9, [x10, #0x3]
10044c560: 910963eb    	add	x11, sp, #0x258
10044c564: b80bb169    	stur	w9, [x11, #0xbb]
10044c568: 7100291f    	cmp	w8, #0xa
10044c56c: f9408fe1    	ldr	x1, [sp, #0x118]
10044c570: f94087e0    	ldr	x0, [sp, #0x108]
10044c574: f9407fea    	ldr	x10, [sp, #0xf8]
10044c578: 5400c8e0    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044c57c: f85383a9    	ldur	x9, [x29, #-0xc8]
10044c580: 381103a8    	sturb	w8, [x29, #-0xf0]
10044c584: b94313e8    	ldr	w8, [sp, #0x310]
10044c588: f94067ec    	ldr	x12, [sp, #0xc8]
10044c58c: b9000188    	str	w8, [x12]
10044c590: b84bb168    	ldur	w8, [x11, #0xbb]
10044c594: b8003188    	stur	w8, [x12, #0x3]
10044c598: f81183a9    	stur	x9, [x29, #-0xe8]
10044c59c: f9400142    	ldr	x2, [x10]
10044c5a0: d103c3a3    	sub	x3, x29, #0xf0
10044c5a4: 9400183b    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044c5a8: 14000189    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c5ac: 7200051f    	tst	w8, #0x3
10044c5b0: 54003220    	b.eq	0x10044cbf4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c60>
10044c5b4: f85103a8    	ldur	x8, [x29, #-0xf0]
10044c5b8: f94077ea    	ldr	x10, [sp, #0xe8]
10044c5bc: f9000148    	str	x8, [x10]
10044c5c0: 910963e9    	add	x9, sp, #0x258
10044c5c4: f84ff128    	ldur	x8, [x9, #0xff]
10044c5c8: f8007148    	stur	x8, [x10, #0x7]
10044c5cc: aa0a03e8    	mov	x8, x10
10044c5d0: aa0803ea    	mov	x10, x8
10044c5d4: b9400108    	ldr	w8, [x8]
10044c5d8: b902f3e8    	str	w8, [sp, #0x2f0]
10044c5dc: b8403148    	ldur	w8, [x10, #0x3]
10044c5e0: b809b128    	stur	w8, [x9, #0x9b]
10044c5e4: f85383bb    	ldur	x27, [x29, #-0xc8]
10044c5e8: f94083e0    	ldr	x0, [sp, #0x100]
10044c5ec: aa1c03e1    	mov	x1, x28
10044c5f0: f9408fe2    	ldr	x2, [sp, #0x118]
10044c5f4: 97f4c64e    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044c5f8: aa0003e8    	mov	x8, x0
10044c5fc: aa0103e0    	mov	x0, x1
10044c600: 36000068    	tbz	w8, #0x0, 0x10044c60c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4678>
10044c604: b5002e40    	cbnz	x0, 0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c608: 14000012    	b	0x10044c650 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x46bc>
10044c60c: eb00039f    	cmp	x28, x0
10044c610: 54014ec9    	b.ls	0x10044efe8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7054>
10044c614: f94083e8    	ldr	x8, [sp, #0x100]
10044c618: 8b001108    	add	x8, x8, x0, lsl #4
10044c61c: 39000119    	strb	w25, [x8]
10044c620: b942f3e9    	ldr	w9, [sp, #0x2f0]
10044c624: b8001109    	stur	w9, [x8, #0x1]
10044c628: 910963e9    	add	x9, sp, #0x258
10044c62c: b849b129    	ldur	w9, [x9, #0x9b]
10044c630: b9000509    	str	w9, [x8, #0x4]
10044c634: f900051b    	str	x27, [x8, #0x8]
10044c638: 91000673    	add	x19, x19, #0x1
10044c63c: f9408fe8    	ldr	x8, [sp, #0x118]
10044c640: f9002113    	str	x19, [x8, #0x40]
10044c644: f1000e7f    	cmp	x19, #0x3
10044c648: 5400a843    	b.lo	0x10044db50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bbc>
10044c64c: 8b180276    	add	x22, x19, x24
10044c650: f94083e8    	ldr	x8, [sp, #0x100]
10044c654: eb1c02df    	cmp	x22, x28
10044c658: 54012f02    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044c65c: 8b161102    	add	x2, x8, x22, lsl #4
10044c660: 39400059    	ldrb	w25, [x2]
10044c664: 51002b28    	sub	w8, w25, #0xa
10044c668: 7100151f    	cmp	w8, #0x5
10044c66c: 54003cc3    	b.lo	0x10044ce04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e70>
10044c670: 71001f3f    	cmp	w25, #0x7
10044c674: 54004368    	b.hi	0x10044cee0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f4c>
10044c678: 52800028    	mov	w8, #0x1                ; =1
10044c67c: 1ad92108    	lsl	w8, w8, w25
10044c680: 52801389    	mov	w9, #0x9c               ; =156
10044c684: 6a09011f    	tst	w8, w9
10044c688: 540035c0    	b.eq	0x10044cd40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4dac>
10044c68c: f8401048    	ldur	x8, [x2, #0x1]
10044c690: f81103a8    	stur	x8, [x29, #-0xf0]
10044c694: f9400448    	ldr	x8, [x2, #0x8]
10044c698: 910963e9    	add	x9, sp, #0x258
10044c69c: f80ff128    	stur	x8, [x9, #0xff]
10044c6a0: 140001aa    	b	0x10044cd48 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4db4>
10044c6a4: 5280000c    	mov	w12, #0x0               ; =0
10044c6a8: bd400520    	ldr	s0, [x9, #0x4]
10044c6ac: 0f20a400    	sshll.2d	v0, v0, #0x0
10044c6b0: 5e61d800    	scvtf	d0, d0
10044c6b4: 17fff747    	b	0x10044a3d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x243c>
10044c6b8: 5280000b    	mov	w11, #0x0               ; =0
10044c6bc: bd400580    	ldr	s0, [x12, #0x4]
10044c6c0: 0f20a400    	sshll.2d	v0, v0, #0x0
10044c6c4: 5e61d800    	scvtf	d0, d0
10044c6c8: 17fff75f    	b	0x10044a444 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24b0>
10044c6cc: 360081cc    	tbz	w12, #0x0, 0x10044d704 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5770>
10044c6d0: 7103537f    	cmp	w27, #0xd4
10044c6d4: 540002c1    	b.ne	0x10044c72c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4798>
10044c6d8: a95003e3    	ldp	x3, x0, [sp, #0x100]
10044c6dc: f9408fe1    	ldr	x1, [sp, #0x118]
10044c6e0: 52800002    	mov	w2, #0x0                ; =0
10044c6e4: 940017a5    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c6e8: f9408fe4    	ldr	x4, [sp, #0x118]
10044c6ec: f94087e3    	ldr	x3, [sp, #0x108]
10044c6f0: 910963eb    	add	x11, sp, #0x258
10044c6f4: b4000600    	cbz	x0, 0x10044c7b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4820>
10044c6f8: 39400008    	ldrb	w8, [x0]
10044c6fc: 71001d1f    	cmp	w8, #0x7
10044c700: 540005a8    	b.hi	0x10044c7b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4820>
10044c704: 52800029    	mov	w9, #0x1                ; =1
10044c708: 1ac82129    	lsl	w9, w9, w8
10044c70c: 5280138a    	mov	w10, #0x9c              ; =156
10044c710: 6a0a013f    	tst	w9, w10
10044c714: 54000360    	b.eq	0x10044c780 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47ec>
10044c718: f8401009    	ldur	x9, [x0, #0x1]
10044c71c: f9017be9    	str	x9, [sp, #0x2f0]
10044c720: f9400409    	ldr	x9, [x0, #0x8]
10044c724: f809f169    	stur	x9, [x11, #0x9f]
10044c728: 14000018    	b	0x10044c788 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47f4>
10044c72c: a95003e3    	ldp	x3, x0, [sp, #0x100]
10044c730: f9408fe1    	ldr	x1, [sp, #0x118]
10044c734: 52800022    	mov	w2, #0x1                ; =1
10044c738: 94001790    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c73c: f9408fe4    	ldr	x4, [sp, #0x118]
10044c740: f94087e3    	ldr	x3, [sp, #0x108]
10044c744: 910963eb    	add	x11, sp, #0x258
10044c748: b40007a0    	cbz	x0, 0x10044c83c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x48a8>
10044c74c: 39400008    	ldrb	w8, [x0]
10044c750: 71001d1f    	cmp	w8, #0x7
10044c754: 54000748    	b.hi	0x10044c83c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x48a8>
10044c758: 52800029    	mov	w9, #0x1                ; =1
10044c75c: 1ac82129    	lsl	w9, w9, w8
10044c760: 5280138a    	mov	w10, #0x9c              ; =156
10044c764: 6a0a013f    	tst	w9, w10
10044c768: 54000500    	b.eq	0x10044c808 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4874>
10044c76c: f8401009    	ldur	x9, [x0, #0x1]
10044c770: f9017be9    	str	x9, [sp, #0x2f0]
10044c774: f9400409    	ldr	x9, [x0, #0x8]
10044c778: f809f169    	stur	x9, [x11, #0x9f]
10044c77c: 14000025    	b	0x10044c810 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x487c>
10044c780: 7200053f    	tst	w9, #0x3
10044c784: 54000180    	b.eq	0x10044c7b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4820>
10044c788: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c78c: f9417be8    	ldr	x8, [sp, #0x2f0]
10044c790: f94077e9    	ldr	x9, [sp, #0xe8]
10044c794: f9000128    	str	x8, [x9]
10044c798: f849f168    	ldur	x8, [x11, #0x9f]
10044c79c: f8007128    	stur	x8, [x9, #0x7]
10044c7a0: d10343a2    	sub	x2, x29, #0xd0
10044c7a4: aa0303e0    	mov	x0, x3
10044c7a8: aa0403e1    	mov	x1, x4
10044c7ac: 9400179b    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044c7b0: 14000107    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c7b4: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044c7b8: f9400102    	ldr	x2, [x8]
10044c7bc: d10343a0    	sub	x0, x29, #0xd0
10044c7c0: 94001809    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044c7c4: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044c7c8: f85303a0    	ldur	x0, [x29, #-0xd0]
10044c7cc: 7100091f    	cmp	w8, #0x2
10044c7d0: 5400dce0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044c7d4: 92401c08    	and	x8, x0, #0xff
10044c7d8: f100291f    	cmp	x8, #0xa
10044c7dc: f9408fe1    	ldr	x1, [sp, #0x118]
10044c7e0: f94087e9    	ldr	x9, [sp, #0x108]
10044c7e4: f9407fea    	ldr	x10, [sp, #0xf8]
10044c7e8: 5400ae80    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044c7ec: f85383a8    	ldur	x8, [x29, #-0xc8]
10044c7f0: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044c7f4: f9400142    	ldr	x2, [x10]
10044c7f8: d103c3a3    	sub	x3, x29, #0xf0
10044c7fc: aa0903e0    	mov	x0, x9
10044c800: 940017a4    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044c804: 140000f2    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c808: 7200053f    	tst	w9, #0x3
10044c80c: 54000180    	b.eq	0x10044c83c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x48a8>
10044c810: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c814: f9417be8    	ldr	x8, [sp, #0x2f0]
10044c818: f94077e9    	ldr	x9, [sp, #0xe8]
10044c81c: f9000128    	str	x8, [x9]
10044c820: f849f168    	ldur	x8, [x11, #0x9f]
10044c824: f8007128    	stur	x8, [x9, #0x7]
10044c828: d10343a2    	sub	x2, x29, #0xd0
10044c82c: aa0303e0    	mov	x0, x3
10044c830: aa0403e1    	mov	x1, x4
10044c834: 94001779    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044c838: 140000e5    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c83c: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044c840: f9400102    	ldr	x2, [x8]
10044c844: d10343a0    	sub	x0, x29, #0xd0
10044c848: 9400187e    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044c84c: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044c850: 71002d1f    	cmp	w8, #0xb
10044c854: 5400aae0    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044c858: f94077ea    	ldr	x10, [sp, #0xe8]
10044c85c: b9400149    	ldr	w9, [x10]
10044c860: b90313e9    	str	w9, [sp, #0x310]
10044c864: b8403149    	ldur	w9, [x10, #0x3]
10044c868: 910963eb    	add	x11, sp, #0x258
10044c86c: b80bb169    	stur	w9, [x11, #0xbb]
10044c870: 7100291f    	cmp	w8, #0xa
10044c874: f9408fe1    	ldr	x1, [sp, #0x118]
10044c878: f94087e0    	ldr	x0, [sp, #0x108]
10044c87c: f9407fea    	ldr	x10, [sp, #0xf8]
10044c880: 5400b0a0    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044c884: f85383a9    	ldur	x9, [x29, #-0xc8]
10044c888: 381103a8    	sturb	w8, [x29, #-0xf0]
10044c88c: b94313e8    	ldr	w8, [sp, #0x310]
10044c890: f94067ec    	ldr	x12, [sp, #0xc8]
10044c894: b9000188    	str	w8, [x12]
10044c898: b84bb168    	ldur	w8, [x11, #0xbb]
10044c89c: b8003188    	stur	w8, [x12, #0x3]
10044c8a0: f81183a9    	stur	x9, [x29, #-0xe8]
10044c8a4: f9400142    	ldr	x2, [x10]
10044c8a8: d103c3a3    	sub	x3, x29, #0xf0
10044c8ac: 94001779    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044c8b0: 140000c7    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c8b4: 3600750b    	tbz	w11, #0x0, 0x10044d754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57c0>
10044c8b8: 71038b7f    	cmp	w27, #0xe2
10044c8bc: 540002c1    	b.ne	0x10044c914 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4980>
10044c8c0: a95003e3    	ldp	x3, x0, [sp, #0x100]
10044c8c4: f9408fe1    	ldr	x1, [sp, #0x118]
10044c8c8: 52800002    	mov	w2, #0x0                ; =0
10044c8cc: 9400172b    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c8d0: f9408fe4    	ldr	x4, [sp, #0x118]
10044c8d4: f94087e3    	ldr	x3, [sp, #0x108]
10044c8d8: 910963eb    	add	x11, sp, #0x258
10044c8dc: b4000600    	cbz	x0, 0x10044c99c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a08>
10044c8e0: 39400008    	ldrb	w8, [x0]
10044c8e4: 71001d1f    	cmp	w8, #0x7
10044c8e8: 540005a8    	b.hi	0x10044c99c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a08>
10044c8ec: 52800029    	mov	w9, #0x1                ; =1
10044c8f0: 1ac82129    	lsl	w9, w9, w8
10044c8f4: 5280138a    	mov	w10, #0x9c              ; =156
10044c8f8: 6a0a013f    	tst	w9, w10
10044c8fc: 54000360    	b.eq	0x10044c968 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49d4>
10044c900: f8401009    	ldur	x9, [x0, #0x1]
10044c904: f9017be9    	str	x9, [sp, #0x2f0]
10044c908: f9400409    	ldr	x9, [x0, #0x8]
10044c90c: f809f169    	stur	x9, [x11, #0x9f]
10044c910: 14000018    	b	0x10044c970 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49dc>
10044c914: a95003e3    	ldp	x3, x0, [sp, #0x100]
10044c918: f9408fe1    	ldr	x1, [sp, #0x118]
10044c91c: 52800022    	mov	w2, #0x1                ; =1
10044c920: 94001716    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044c924: f9408fe4    	ldr	x4, [sp, #0x118]
10044c928: f94087e3    	ldr	x3, [sp, #0x108]
10044c92c: 910963eb    	add	x11, sp, #0x258
10044c930: b40007a0    	cbz	x0, 0x10044ca24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a90>
10044c934: 39400008    	ldrb	w8, [x0]
10044c938: 71001d1f    	cmp	w8, #0x7
10044c93c: 54000748    	b.hi	0x10044ca24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a90>
10044c940: 52800029    	mov	w9, #0x1                ; =1
10044c944: 1ac82129    	lsl	w9, w9, w8
10044c948: 5280138a    	mov	w10, #0x9c              ; =156
10044c94c: 6a0a013f    	tst	w9, w10
10044c950: 54000500    	b.eq	0x10044c9f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a5c>
10044c954: f8401009    	ldur	x9, [x0, #0x1]
10044c958: f9017be9    	str	x9, [sp, #0x2f0]
10044c95c: f9400409    	ldr	x9, [x0, #0x8]
10044c960: f809f169    	stur	x9, [x11, #0x9f]
10044c964: 14000025    	b	0x10044c9f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a64>
10044c968: 7200053f    	tst	w9, #0x3
10044c96c: 54000180    	b.eq	0x10044c99c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a08>
10044c970: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c974: f9417be8    	ldr	x8, [sp, #0x2f0]
10044c978: f94077e9    	ldr	x9, [sp, #0xe8]
10044c97c: f9000128    	str	x8, [x9]
10044c980: f849f168    	ldur	x8, [x11, #0x9f]
10044c984: f8007128    	stur	x8, [x9, #0x7]
10044c988: d10343a2    	sub	x2, x29, #0xd0
10044c98c: aa0303e0    	mov	x0, x3
10044c990: aa0403e1    	mov	x1, x4
10044c994: 94001721    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044c998: 1400008d    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c99c: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044c9a0: f9400102    	ldr	x2, [x8]
10044c9a4: d10343a0    	sub	x0, x29, #0xd0
10044c9a8: 9400178f    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044c9ac: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044c9b0: f85303a0    	ldur	x0, [x29, #-0xd0]
10044c9b4: 7100091f    	cmp	w8, #0x2
10044c9b8: 5400cda0    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044c9bc: 92401c08    	and	x8, x0, #0xff
10044c9c0: f100291f    	cmp	x8, #0xa
10044c9c4: f9408fe1    	ldr	x1, [sp, #0x118]
10044c9c8: f94087e9    	ldr	x9, [sp, #0x108]
10044c9cc: f9407fea    	ldr	x10, [sp, #0xf8]
10044c9d0: 54009f40    	b.eq	0x10044ddb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e24>
10044c9d4: f85383a8    	ldur	x8, [x29, #-0xc8]
10044c9d8: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044c9dc: f9400142    	ldr	x2, [x10]
10044c9e0: d103c3a3    	sub	x3, x29, #0xf0
10044c9e4: aa0903e0    	mov	x0, x9
10044c9e8: 9400172a    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044c9ec: 14000078    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044c9f0: 7200053f    	tst	w9, #0x3
10044c9f4: 54000180    	b.eq	0x10044ca24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a90>
10044c9f8: 381303a8    	sturb	w8, [x29, #-0xd0]
10044c9fc: f9417be8    	ldr	x8, [sp, #0x2f0]
10044ca00: f94077e9    	ldr	x9, [sp, #0xe8]
10044ca04: f9000128    	str	x8, [x9]
10044ca08: f849f168    	ldur	x8, [x11, #0x9f]
10044ca0c: f8007128    	stur	x8, [x9, #0x7]
10044ca10: d10343a2    	sub	x2, x29, #0xd0
10044ca14: aa0303e0    	mov	x0, x3
10044ca18: aa0403e1    	mov	x1, x4
10044ca1c: 940016ff    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044ca20: 1400006b    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044ca24: a94f87e8    	ldp	x8, x1, [sp, #0xf8]
10044ca28: f9400102    	ldr	x2, [x8]
10044ca2c: d10343a0    	sub	x0, x29, #0xd0
10044ca30: 94001804    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044ca34: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044ca38: 71002d1f    	cmp	w8, #0xb
10044ca3c: 54009ba0    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044ca40: f94077ea    	ldr	x10, [sp, #0xe8]
10044ca44: b9400149    	ldr	w9, [x10]
10044ca48: b90313e9    	str	w9, [sp, #0x310]
10044ca4c: b8403149    	ldur	w9, [x10, #0x3]
10044ca50: 910963eb    	add	x11, sp, #0x258
10044ca54: b80bb169    	stur	w9, [x11, #0xbb]
10044ca58: 7100291f    	cmp	w8, #0xa
10044ca5c: f9408fe1    	ldr	x1, [sp, #0x118]
10044ca60: f94087e0    	ldr	x0, [sp, #0x108]
10044ca64: f9407fea    	ldr	x10, [sp, #0xf8]
10044ca68: 5400a160    	b.eq	0x10044de94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
10044ca6c: f85383a9    	ldur	x9, [x29, #-0xc8]
10044ca70: 381103a8    	sturb	w8, [x29, #-0xf0]
10044ca74: b94313e8    	ldr	w8, [sp, #0x310]
10044ca78: f94067ec    	ldr	x12, [sp, #0xc8]
10044ca7c: b9000188    	str	w8, [x12]
10044ca80: b84bb168    	ldur	w8, [x11, #0xbb]
10044ca84: b8003188    	stur	w8, [x12, #0x3]
10044ca88: f81183a9    	stur	x9, [x29, #-0xe8]
10044ca8c: f9400142    	ldr	x2, [x10]
10044ca90: d103c3a3    	sub	x3, x29, #0xf0
10044ca94: 940016ff    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044ca98: 1400004d    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044ca9c: f85383a9    	ldur	x9, [x29, #-0xc8]
10044caa0: 71000d1f    	cmp	w8, #0x3
10044caa4: 54009f40    	b.eq	0x10044de8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ef8>
10044caa8: 36001f28    	tbz	w8, #0x0, 0x10044ce8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ef8>
10044caac: f81383a9    	stur	x9, [x29, #-0xc8]
10044cab0: 52800088    	mov	w8, #0x4                ; =4
10044cab4: 140000f9    	b	0x10044ce98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f04>
10044cab8: f85383a9    	ldur	x9, [x29, #-0xc8]
10044cabc: 71000d1f    	cmp	w8, #0x3
10044cac0: 54009e60    	b.eq	0x10044de8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ef8>
10044cac4: 36001f68    	tbz	w8, #0x0, 0x10044ceb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f1c>
10044cac8: f81383a9    	stur	x9, [x29, #-0xc8]
10044cacc: 52800088    	mov	w8, #0x4                ; =4
10044cad0: 140000fb    	b	0x10044cebc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f28>
10044cad4: 71000f7f    	cmp	w27, #0x3
10044cad8: 54000b61    	b.ne	0x10044cc44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4cb0>
10044cadc: 3dc00ac0    	ldr	q0, [x22, #0x20]
10044cae0: f9401ac8    	ldr	x8, [x22, #0x30]
10044cae4: f90183e8    	str	x8, [sp, #0x300]
10044cae8: 3d80bfe0    	str	q0, [sp, #0x2f0]
10044caec: 794036c8    	ldrh	w8, [x22, #0x1a]
10044caf0: 790623e8    	strh	w8, [sp, #0x310]
10044caf4: 394066d4    	ldrb	w20, [x22, #0x19]
10044caf8: 910bc3e8    	add	x8, sp, #0x2f0
10044cafc: a93323b6    	stp	x22, x8, [x29, #-0xd0]
10044cb00: 910c43e8    	add	x8, sp, #0x310
10044cb04: f9407fe9    	ldr	x9, [sp, #0xf8]
10044cb08: a93423a9    	stp	x9, x8, [x29, #-0xc0]
10044cb0c: d103c3a0    	sub	x0, x29, #0xf0
10044cb10: d10343a1    	sub	x1, x29, #0xd0
10044cb14: f94087e2    	ldr	x2, [sp, #0x108]
10044cb18: f9408fe3    	ldr	x3, [sp, #0x118]
10044cb1c: 940027fc    	bl	0x100456b0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h0a2b358663622ddcE>
10044cb20: b85103a8    	ldur	w8, [x29, #-0xf0]
10044cb24: f85183a0    	ldur	x0, [x29, #-0xe8]
10044cb28: 3700c228    	tbnz	w8, #0x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044cb2c: b5002f80    	cbnz	x0, 0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044cb30: f85203a8    	ldur	x8, [x29, #-0xe0]
10044cb34: 12000108    	and	w8, w8, #0x1
10044cb38: 6b08029f    	cmp	w20, w8
10044cb3c: 54002b20    	b.eq	0x10044d0a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x510c>
10044cb40: f9408be8    	ldr	x8, [sp, #0x110]
10044cb44: f9400109    	ldr	x9, [x8]
10044cb48: b94042c8    	ldr	w8, [x22, #0x40]
10044cb4c: f940352a    	ldr	x10, [x9, #0x68]
10044cb50: eb08015f    	cmp	x10, x8
10044cb54: 540104c9    	b.ls	0x10044ebec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c58>
10044cb58: f9403129    	ldr	x9, [x9, #0x60]
10044cb5c: 8b080928    	add	x8, x9, x8, lsl #2
10044cb60: b8410d09    	ldr	w9, [x8, #0x10]!
10044cb64: 1400001c    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044cb68: d10343a0    	sub	x0, x29, #0xd0
10044cb6c: 97ff7573    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044cb70: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044cb74: 7100291f    	cmp	w8, #0xa
10044cb78: f9408fec    	ldr	x12, [sp, #0x118]
10044cb7c: f94087e0    	ldr	x0, [sp, #0x108]
10044cb80: f9407fed    	ldr	x13, [sp, #0xf8]
10044cb84: 910963ee    	add	x14, sp, #0x258
10044cb88: f94077ea    	ldr	x10, [sp, #0xe8]
10044cb8c: 5400c980    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044cb90: b9400149    	ldr	w9, [x10]
10044cb94: b902f3e9    	str	w9, [sp, #0x2f0]
10044cb98: b8403149    	ldur	w9, [x10, #0x3]
10044cb9c: b809b1c9    	stur	w9, [x14, #0x9b]
10044cba0: f85383a9    	ldur	x9, [x29, #-0xc8]
10044cba4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044cba8: b942f3e8    	ldr	w8, [sp, #0x2f0]
10044cbac: b9000148    	str	w8, [x10]
10044cbb0: b849b1c8    	ldur	w8, [x14, #0x9b]
10044cbb4: b8003148    	stur	w8, [x10, #0x3]
10044cbb8: f81383a9    	stur	x9, [x29, #-0xc8]
10044cbbc: f94001a2    	ldr	x2, [x13]
10044cbc0: d10343a3    	sub	x3, x29, #0xd0
10044cbc4: aa0c03e1    	mov	x1, x12
10044cbc8: 940016b2    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044cbcc: b500bd00    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044cbd0: f940abe9    	ldr	x9, [sp, #0x150]
10044cbd4: f9408be8    	ldr	x8, [sp, #0x110]
10044cbd8: f9400108    	ldr	x8, [x8]
10044cbdc: f9402d1c    	ldr	x28, [x8, #0x58]
10044cbe0: f90093e9    	str	x9, [sp, #0x120]
10044cbe4: 2a0903e9    	mov	w9, w9
10044cbe8: eb09039f    	cmp	x28, x9
10044cbec: 54fdaf48    	b.hi	0x1004481d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x240>
10044cbf0: 14000410    	b	0x10044dc30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c9c>
10044cbf4: d10343a0    	sub	x0, x29, #0xd0
10044cbf8: aa1403e1    	mov	x1, x20
10044cbfc: 97ff754f    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044cc00: 385303b9    	ldurb	w25, [x29, #-0xd0]
10044cc04: 71002b3f    	cmp	w25, #0xa
10044cc08: 910963e9    	add	x9, sp, #0x258
10044cc0c: f94077e8    	ldr	x8, [sp, #0xe8]
10044cc10: 54ffce01    	b.ne	0x10044c5d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x463c>
10044cc14: 140002b9    	b	0x10044d6f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5764>
10044cc18: b94267e8    	ldr	w8, [sp, #0x264]
10044cc1c: b81343a8    	stur	w8, [x29, #-0xcc]
10044cc20: 52800068    	mov	w8, #0x3                ; =3
10044cc24: 381303a8    	sturb	w8, [x29, #-0xd0]
10044cc28: d10343a2    	sub	x2, x29, #0xd0
10044cc2c: f94087e0    	ldr	x0, [sp, #0x108]
10044cc30: f9408fe1    	ldr	x1, [sp, #0x118]
10044cc34: 94001633    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044cc38: b500b9a0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044cc3c: 11001289    	add	w9, w20, #0x4
10044cc40: 17ffffe5    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044cc44: 3dc00ac0    	ldr	q0, [x22, #0x20]
10044cc48: f9401ac8    	ldr	x8, [x22, #0x30]
10044cc4c: f90183e8    	str	x8, [sp, #0x300]
10044cc50: 3d80bfe0    	str	q0, [sp, #0x2f0]
10044cc54: f94097e8    	ldr	x8, [sp, #0x128]
10044cc58: f9400508    	ldr	x8, [x8, #0x8]
10044cc5c: b100051f    	cmn	x8, #0x1
10044cc60: 540079a0    	b.eq	0x10044db94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c00>
10044cc64: 91000508    	add	x8, x8, #0x1
10044cc68: f9018fe8    	str	x8, [sp, #0x318]
10044cc6c: 52800028    	mov	w8, #0x1                ; =1
10044cc70: f9018be8    	str	x8, [sp, #0x310]
10044cc74: 910c43e8    	add	x8, sp, #0x310
10044cc78: a9335ba8    	stp	x8, x22, [x29, #-0xd0]
10044cc7c: 910bc3e8    	add	x8, sp, #0x2f0
10044cc80: f81403a8    	stur	x8, [x29, #-0xc0]
10044cc84: f9407fe8    	ldr	x8, [sp, #0xf8]
10044cc88: f81483a8    	stur	x8, [x29, #-0xb8]
10044cc8c: d103c3a0    	sub	x0, x29, #0xf0
10044cc90: d10343a1    	sub	x1, x29, #0xd0
10044cc94: f94087e2    	ldr	x2, [sp, #0x108]
10044cc98: f9408fe3    	ldr	x3, [sp, #0x118]
10044cc9c: 94002953    	bl	0x1004571e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h1b2d50d590dd79e8E>
10044cca0: b85103a9    	ldur	w9, [x29, #-0xf0]
10044cca4: f85183a8    	ldur	x8, [x29, #-0xe8]
10044cca8: 7100053f    	cmp	w9, #0x1
10044ccac: 5400a720    	b.eq	0x10044e190 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61fc>
10044ccb0: b5002368    	cbnz	x8, 0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044ccb4: b94313e8    	ldr	w8, [sp, #0x310]
10044ccb8: 7100051f    	cmp	w8, #0x1
10044ccbc: 54010be1    	b.ne	0x10044ee38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ea4>
10044ccc0: f9418fe8    	ldr	x8, [sp, #0x318]
10044ccc4: f94097e9    	ldr	x9, [sp, #0x128]
10044ccc8: f9000528    	str	x8, [x9, #0x8]
10044cccc: 39453bf9    	ldrb	w25, [sp, #0x14e]
10044ccd0: 1e604120    	fmov	d0, d9
10044ccd4: 7200033f    	tst	w25, #0x1
10044ccd8: 52800028    	mov	w8, #0x1                ; =1
10044ccdc: 9a880508    	cinc	x8, x8, ne
10044cce0: f94093e9    	ldr	x9, [sp, #0x120]
10044cce4: 8b294116    	add	x22, x8, w9, uxtw
10044cce8: eb1c02df    	cmp	x22, x28
10044ccec: 5400fc22    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044ccf0: b8767b09    	ldr	w9, [x24, x22, lsl #2]
10044ccf4: 1e604009    	fmov	d9, d0
10044ccf8: 17ffffb7    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044ccfc: 11001689    	add	w9, w20, #0x5
10044cd00: 17ffffb5    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044cd04: 11002289    	add	w9, w20, #0x8
10044cd08: 17ffffb3    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044cd0c: f94083e9    	ldr	x9, [sp, #0x100]
10044cd10: 7216013f    	tst	w9, #0x400
10044cd14: 1a9f17e9    	cset	w9, eq
10044cd18: 4a080128    	eor	w8, w9, w8
10044cd1c: 36000dc8    	tbz	w8, #0x0, 0x10044ced4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f40>
10044cd20: 52800028    	mov	w8, #0x1                ; =1
10044cd24: 6a53711f    	tst	w8, w19, lsr #28
10044cd28: 9a880508    	cinc	x8, x8, ne
10044cd2c: f94093e9    	ldr	x9, [sp, #0x120]
10044cd30: 8b294116    	add	x22, x8, w9, uxtw
10044cd34: eb1c02df    	cmp	x22, x28
10044cd38: 54004ac3    	b.lo	0x10044d690 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56fc>
10044cd3c: 140007cd    	b	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044cd40: 7200051f    	tst	w8, #0x3
10044cd44: 54000ce0    	b.eq	0x10044cee0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f4c>
10044cd48: f85103a8    	ldur	x8, [x29, #-0xf0]
10044cd4c: f94077ea    	ldr	x10, [sp, #0xe8]
10044cd50: f9000148    	str	x8, [x10]
10044cd54: 910963e9    	add	x9, sp, #0x258
10044cd58: f84ff128    	ldur	x8, [x9, #0xff]
10044cd5c: f8007148    	stur	x8, [x10, #0x7]
10044cd60: aa0a03e8    	mov	x8, x10
10044cd64: aa0803ea    	mov	x10, x8
10044cd68: b9400108    	ldr	w8, [x8]
10044cd6c: b902f3e8    	str	w8, [sp, #0x2f0]
10044cd70: b8403148    	ldur	w8, [x10, #0x3]
10044cd74: b809b128    	stur	w8, [x9, #0x9b]
10044cd78: f85383bb    	ldur	x27, [x29, #-0xc8]
10044cd7c: f94083e0    	ldr	x0, [sp, #0x100]
10044cd80: aa1c03e1    	mov	x1, x28
10044cd84: f9408fe2    	ldr	x2, [sp, #0x118]
10044cd88: 97f4c469    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044cd8c: aa0003e8    	mov	x8, x0
10044cd90: aa0103e0    	mov	x0, x1
10044cd94: 36000068    	tbz	w8, #0x0, 0x10044cda0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e0c>
10044cd98: b5fff1a0    	cbnz	x0, 0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044cd9c: 14000012    	b	0x10044cde4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e50>
10044cda0: eb00039f    	cmp	x28, x0
10044cda4: 54011229    	b.ls	0x10044efe8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7054>
10044cda8: f94083e8    	ldr	x8, [sp, #0x100]
10044cdac: 8b001108    	add	x8, x8, x0, lsl #4
10044cdb0: 39000119    	strb	w25, [x8]
10044cdb4: b942f3e9    	ldr	w9, [sp, #0x2f0]
10044cdb8: b8001109    	stur	w9, [x8, #0x1]
10044cdbc: 910963e9    	add	x9, sp, #0x258
10044cdc0: b849b129    	ldur	w9, [x9, #0x9b]
10044cdc4: b9000509    	str	w9, [x8, #0x4]
10044cdc8: f900051b    	str	x27, [x8, #0x8]
10044cdcc: 91000673    	add	x19, x19, #0x1
10044cdd0: f9408fe8    	ldr	x8, [sp, #0x118]
10044cdd4: f9002113    	str	x19, [x8, #0x40]
10044cdd8: f1000e7f    	cmp	x19, #0x3
10044cddc: 54006ba3    	b.lo	0x10044db50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bbc>
10044cde0: 8b180276    	add	x22, x19, x24
10044cde4: eb1c02df    	cmp	x22, x28
10044cde8: 5400f282    	b.hs	0x10044ec38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ca4>
10044cdec: f94083e8    	ldr	x8, [sp, #0x100]
10044cdf0: 8b161102    	add	x2, x8, x22, lsl #4
10044cdf4: 39400056    	ldrb	w22, [x2]
10044cdf8: 51002ac8    	sub	w8, w22, #0xa
10044cdfc: 7100151f    	cmp	w8, #0x5
10044ce00: 54000062    	b.hs	0x10044ce0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e78>
10044ce04: 94031563    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044ce08: 17ffff71    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044ce0c: 71001edf    	cmp	w22, #0x7
10044ce10: 54004648    	b.hi	0x10044d6d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5744>
10044ce14: 52800028    	mov	w8, #0x1                ; =1
10044ce18: 1ad62108    	lsl	w8, w8, w22
10044ce1c: 52801389    	mov	w9, #0x9c               ; =156
10044ce20: 6a09011f    	tst	w8, w9
10044ce24: 54000880    	b.eq	0x10044cf34 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4fa0>
10044ce28: f8401048    	ldur	x8, [x2, #0x1]
10044ce2c: f81103a8    	stur	x8, [x29, #-0xf0]
10044ce30: f9400448    	ldr	x8, [x2, #0x8]
10044ce34: 910963e9    	add	x9, sp, #0x258
10044ce38: f80ff128    	stur	x8, [x9, #0xff]
10044ce3c: 14000040    	b	0x10044cf3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4fa8>
10044ce40: 8b161109    	add	x9, x8, x22, lsl #4
10044ce44: 8b0c1108    	add	x8, x8, x12, lsl #4
10044ce48: b940050c    	ldr	w12, [x8, #0x4]
10044ce4c: 14000004    	b	0x10044ce5c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ec8>
10044ce50: 8b161109    	add	x9, x8, x22, lsl #4
10044ce54: 8b0c1108    	add	x8, x8, x12, lsl #4
10044ce58: fd400500    	ldr	d0, [x8, #0x8]
10044ce5c: 3900012b    	strb	w11, [x9]
10044ce60: b900052c    	str	w12, [x9, #0x4]
10044ce64: fd000520    	str	d0, [x9, #0x8]
10044ce68: 71009b7f    	cmp	w27, #0x26
10044ce6c: 54ffeb20    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ce70: 7100b37f    	cmp	w27, #0x2c
10044ce74: 54ffeae0    	b.eq	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ce78: 528001c9    	mov	w9, #0xe                ; =14
10044ce7c: 39000109    	strb	w9, [x8]
10044ce80: f9408fe8    	ldr	x8, [sp, #0x118]
10044ce84: f900210a    	str	x10, [x8, #0x40]
10044ce88: 17ffff52    	b	0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044ce8c: b85343a8    	ldur	w8, [x29, #-0xcc]
10044ce90: b81343a8    	stur	w8, [x29, #-0xcc]
10044ce94: 52800068    	mov	w8, #0x3                ; =3
10044ce98: 381303a8    	sturb	w8, [x29, #-0xd0]
10044ce9c: d10343a2    	sub	x2, x29, #0xd0
10044cea0: f94087e0    	ldr	x0, [sp, #0x108]
10044cea4: f9408fe1    	ldr	x1, [sp, #0x118]
10044cea8: 94001596    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044ceac: 14000009    	b	0x10044ced0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f3c>
10044ceb0: b85343a8    	ldur	w8, [x29, #-0xcc]
10044ceb4: b81343a8    	stur	w8, [x29, #-0xcc]
10044ceb8: 52800068    	mov	w8, #0x3                ; =3
10044cebc: 381303a8    	sturb	w8, [x29, #-0xd0]
10044cec0: d10343a2    	sub	x2, x29, #0xd0
10044cec4: f94087e0    	ldr	x0, [sp, #0x108]
10044cec8: f9408fe1    	ldr	x1, [sp, #0x118]
10044cecc: 9400158d    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044ced0: b500a4e0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ced4: f940abe8    	ldr	x8, [sp, #0x150]
10044ced8: 91000909    	add	x9, x8, #0x2
10044cedc: 17ffff3e    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044cee0: d10343a0    	sub	x0, x29, #0xd0
10044cee4: aa1403e1    	mov	x1, x20
10044cee8: 97ff7494    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044ceec: 385303b9    	ldurb	w25, [x29, #-0xd0]
10044cef0: 71002b3f    	cmp	w25, #0xa
10044cef4: 910963e9    	add	x9, sp, #0x258
10044cef8: f94077e8    	ldr	x8, [sp, #0xe8]
10044cefc: 54fff341    	b.ne	0x10044cd64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4dd0>
10044cf00: 140001fe    	b	0x10044d6f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5764>
10044cf04: 3dc002c0    	ldr	q0, [x22]
10044cf08: 3c9303a0    	stur	q0, [x29, #-0xd0]
10044cf0c: 14000073    	b	0x10044d0d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5144>
10044cf10: 79400ac8    	ldrh	w8, [x22, #0x4]
10044cf14: 79400ec3    	ldrh	w3, [x22, #0x6]
10044cf18: 7100091f    	cmp	w8, #0x2
10044cf1c: 54000d62    	b.hs	0x10044d0c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5134>
10044cf20: d10343a0    	sub	x0, x29, #0xd0
10044cf24: f94087e1    	ldr	x1, [sp, #0x108]
10044cf28: f9408fe2    	ldr	x2, [sp, #0x118]
10044cf2c: 94002804    	bl	0x100456f3c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots15immediate_local17h49fd8b485127d92bE>
10044cf30: 1400006a    	b	0x10044d0d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5144>
10044cf34: 7200051f    	tst	w8, #0x3
10044cf38: 54003d00    	b.eq	0x10044d6d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5744>
10044cf3c: f85103a8    	ldur	x8, [x29, #-0xf0]
10044cf40: f94077ea    	ldr	x10, [sp, #0xe8]
10044cf44: f9000148    	str	x8, [x10]
10044cf48: 910963e9    	add	x9, sp, #0x258
10044cf4c: f84ff128    	ldur	x8, [x9, #0xff]
10044cf50: f8007148    	stur	x8, [x10, #0x7]
10044cf54: aa0a03e8    	mov	x8, x10
10044cf58: aa0803ea    	mov	x10, x8
10044cf5c: b9400108    	ldr	w8, [x8]
10044cf60: b902f3e8    	str	w8, [sp, #0x2f0]
10044cf64: b8403148    	ldur	w8, [x10, #0x3]
10044cf68: b809b128    	stur	w8, [x9, #0x9b]
10044cf6c: f85383b4    	ldur	x20, [x29, #-0xc8]
10044cf70: f94083e0    	ldr	x0, [sp, #0x100]
10044cf74: aa1c03e1    	mov	x1, x28
10044cf78: f9408fe2    	ldr	x2, [sp, #0x118]
10044cf7c: 97f4c3ec    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044cf80: aa0003e8    	mov	x8, x0
10044cf84: aa0103e0    	mov	x0, x1
10044cf88: 36000068    	tbz	w8, #0x0, 0x10044cf94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5000>
10044cf8c: b5ffe200    	cbnz	x0, 0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044cf90: 1400000f    	b	0x10044cfcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5038>
10044cf94: eb00039f    	cmp	x28, x0
10044cf98: 54010289    	b.ls	0x10044efe8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7054>
10044cf9c: f94083e8    	ldr	x8, [sp, #0x100]
10044cfa0: 8b001108    	add	x8, x8, x0, lsl #4
10044cfa4: 39000116    	strb	w22, [x8]
10044cfa8: b942f3e9    	ldr	w9, [sp, #0x2f0]
10044cfac: b8001109    	stur	w9, [x8, #0x1]
10044cfb0: 910963e9    	add	x9, sp, #0x258
10044cfb4: b849b129    	ldur	w9, [x9, #0x9b]
10044cfb8: b9000509    	str	w9, [x8, #0x4]
10044cfbc: f9000514    	str	x20, [x8, #0x8]
10044cfc0: 91000668    	add	x8, x19, #0x1
10044cfc4: f9408fe9    	ldr	x9, [sp, #0x118]
10044cfc8: f9002128    	str	x8, [x9, #0x40]
10044cfcc: d2800000    	mov	x0, #0x0                ; =0
10044cfd0: b4ffe01f    	cbz	xzr, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044cfd4: 140004e6    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044cfd8: 8b131108    	add	x8, x8, x19, lsl #4
10044cfdc: fd400500    	ldr	d0, [x8, #0x8]
10044cfe0: f94083e8    	ldr	x8, [sp, #0x100]
10044cfe4: 7213011f    	tst	w8, #0x2000
10044cfe8: 1e6e1001    	fmov	d1, #1.00000000
10044cfec: 1e7e1002    	fmov	d2, #-1.00000000
10044cff0: 1e610c41    	fcsel	d1, d2, d1, eq
10044cff4: 1e602820    	fadd	d0, d1, d0
10044cff8: 14000010    	b	0x10044d038 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x50a4>
10044cffc: 8b131108    	add	x8, x8, x19, lsl #4
10044d000: b9400508    	ldr	w8, [x8, #0x4]
10044d004: 71000509    	subs	w9, w8, #0x1
10044d008: 1a9f77eb    	cset	w11, vs
10044d00c: 3100050a    	adds	w10, w8, #0x1
10044d010: 1a9f77ec    	cset	w12, vs
10044d014: f94083ed    	ldr	x13, [sp, #0x100]
10044d018: 721301bf    	tst	w13, #0x2000
10044d01c: 1a8c016b    	csel	w11, w11, w12, eq
10044d020: 360033cb    	tbz	w11, #0x0, 0x10044d698 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5704>
10044d024: 1e6e1000    	fmov	d0, #1.00000000
10044d028: 1e7e1001    	fmov	d1, #-1.00000000
10044d02c: 1e600c20    	fcsel	d0, d1, d0, eq
10044d030: 1e620101    	scvtf	d1, w8
10044d034: 1e612800    	fadd	d0, d0, d1
10044d038: fc1383a0    	stur	d0, [x29, #-0xc8]
10044d03c: 52800028    	mov	w8, #0x1                ; =1
10044d040: 14000199    	b	0x10044d6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5710>
10044d044: 3906a3f9    	strb	w25, [sp, #0x1a8]
10044d048: b941a3e8    	ldr	w8, [sp, #0x1a0]
10044d04c: f9402be9    	ldr	x9, [sp, #0x50]
10044d050: b9000128    	str	w8, [x9]
10044d054: 910293e8    	add	x8, sp, #0xa4
10044d058: b84ff108    	ldur	w8, [x8, #0xff]
10044d05c: b8003128    	stur	w8, [x9, #0x3]
10044d060: f900dbf4    	str	x20, [sp, #0x1b0]
10044d064: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d068: f9400102    	ldr	x2, [x8]
10044d06c: 9106a3e3    	add	x3, sp, #0x1a8
10044d070: f94087e0    	ldr	x0, [sp, #0x108]
10044d074: f9408fe1    	ldr	x1, [sp, #0x118]
10044d078: 94001586    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044d07c: b5009780    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d080: 52800028    	mov	w8, #0x1                ; =1
10044d084: 6a53711f    	tst	w8, w19, lsr #28
10044d088: 52800048    	mov	w8, #0x2                ; =2
10044d08c: 9a880508    	cinc	x8, x8, ne
10044d090: 8b160116    	add	x22, x8, x22
10044d094: eb1c02df    	cmp	x22, x28
10044d098: 54002fc3    	b.lo	0x10044d690 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56fc>
10044d09c: 140006f5    	b	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044d0a0: 52800028    	mov	w8, #0x1                ; =1
10044d0a4: 6a53711f    	tst	w8, w19, lsr #28
10044d0a8: 9a880508    	cinc	x8, x8, ne
10044d0ac: f94093e9    	ldr	x9, [sp, #0x120]
10044d0b0: 8b294116    	add	x22, x8, w9, uxtw
10044d0b4: eb1c02df    	cmp	x22, x28
10044d0b8: 5400ddc2    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044d0bc: 8b160b08    	add	x8, x24, x22, lsl #2
10044d0c0: b9400109    	ldr	w9, [x8]
10044d0c4: 17fffec4    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044d0c8: d10343a0    	sub	x0, x29, #0xd0
10044d0cc: f94087e1    	ldr	x1, [sp, #0x108]
10044d0d0: f9408fe2    	ldr	x2, [sp, #0x118]
10044d0d4: 940027c1    	bl	0x100456fd8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots19immediate_parameter17h006e24dbcb03b212E>
10044d0d8: b85303a8    	ldur	w8, [x29, #-0xd0]
10044d0dc: 7100091f    	cmp	w8, #0x2
10044d0e0: 540001e0    	b.eq	0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044d0e4: fc5383a0    	ldur	d0, [x29, #-0xc8]
10044d0e8: bc5343a1    	ldur	s1, [x29, #-0xcc]
10044d0ec: 0f20a421    	sshll.2d	v1, v1, #0x0
10044d0f0: 5e61d821    	scvtf	d1, d1
10044d0f4: 7200011f    	tst	w8, #0x1
10044d0f8: 1e611c0a    	fcsel	d10, d0, d1, ne
10044d0fc: 1e602148    	fcmp	d10, #0.0
10044d100: b26b6be8    	mov	x8, #0xffffffe00000     ; =281474974613504
10044d104: f2e83de8    	movk	x8, #0x41ef, lsl #48
10044d108: 9e670100    	fmov	d0, x8
10044d10c: 1e60a540    	fccmp	d10, d0, #0x0, ge
10044d110: 1e65c140    	frintz	d0, d10
10044d114: 1e6a4400    	fccmp	d0, d10, #0x0, mi
10044d118: 54000e20    	b.eq	0x10044d2dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5348>
10044d11c: 394062c8    	ldrb	w8, [x22, #0x18]
10044d120: 34000348    	cbz	w8, 0x10044d188 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x51f4>
10044d124: 794072c8    	ldrh	w8, [x22, #0x38]
10044d128: 340005e8    	cbz	w8, 0x10044d1e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5250>
10044d12c: 7100051f    	cmp	w8, #0x1
10044d130: 54000841    	b.ne	0x10044d238 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52a4>
10044d134: 794076d4    	ldrh	w20, [x22, #0x3a]
10044d138: f94087e0    	ldr	x0, [sp, #0x108]
10044d13c: f9408fe1    	ldr	x1, [sp, #0x118]
10044d140: 52800002    	mov	w2, #0x0                ; =0
10044d144: aa1403e3    	mov	x3, x20
10044d148: 9400150c    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044d14c: b4000fa0    	cbz	x0, 0x10044d340 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53ac>
10044d150: 39400008    	ldrb	w8, [x0]
10044d154: 71001d1f    	cmp	w8, #0x7
10044d158: 54000f48    	b.hi	0x10044d340 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53ac>
10044d15c: 52800029    	mov	w9, #0x1                ; =1
10044d160: 1ac82129    	lsl	w9, w9, w8
10044d164: 5280138a    	mov	w10, #0x9c              ; =156
10044d168: 6a0a013f    	tst	w9, w10
10044d16c: 54000ce0    	b.eq	0x10044d308 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5374>
10044d170: f8401009    	ldur	x9, [x0, #0x1]
10044d174: f9017be9    	str	x9, [sp, #0x2f0]
10044d178: f9400409    	ldr	x9, [x0, #0x8]
10044d17c: 910963ea    	add	x10, sp, #0x258
10044d180: f809f149    	stur	x9, [x10, #0x9f]
10044d184: 14000063    	b	0x10044d310 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x537c>
10044d188: 794036d4    	ldrh	w20, [x22, #0x1a]
10044d18c: 394066c8    	ldrb	w8, [x22, #0x19]
10044d190: 360007e8    	tbz	w8, #0x0, 0x10044d28c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52f8>
10044d194: f94087e0    	ldr	x0, [sp, #0x108]
10044d198: f9408fe1    	ldr	x1, [sp, #0x118]
10044d19c: 52800002    	mov	w2, #0x0                ; =0
10044d1a0: aa1403e3    	mov	x3, x20
10044d1a4: 940014f5    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044d1a8: b4001cc0    	cbz	x0, 0x10044d540 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x55ac>
10044d1ac: 39400008    	ldrb	w8, [x0]
10044d1b0: 71001d1f    	cmp	w8, #0x7
10044d1b4: 54001c68    	b.hi	0x10044d540 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x55ac>
10044d1b8: 52800029    	mov	w9, #0x1                ; =1
10044d1bc: 1ac82129    	lsl	w9, w9, w8
10044d1c0: 5280138a    	mov	w10, #0x9c              ; =156
10044d1c4: 6a0a013f    	tst	w9, w10
10044d1c8: 54001a00    	b.eq	0x10044d508 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5574>
10044d1cc: f8401009    	ldur	x9, [x0, #0x1]
10044d1d0: f9017be9    	str	x9, [sp, #0x2f0]
10044d1d4: f9400409    	ldr	x9, [x0, #0x8]
10044d1d8: 910963ea    	add	x10, sp, #0x258
10044d1dc: f809f149    	stur	x9, [x10, #0x9f]
10044d1e0: 140000cc    	b	0x10044d510 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x557c>
10044d1e4: 794076d4    	ldrh	w20, [x22, #0x3a]
10044d1e8: f94087e0    	ldr	x0, [sp, #0x108]
10044d1ec: f9408fe1    	ldr	x1, [sp, #0x118]
10044d1f0: 52800002    	mov	w2, #0x0                ; =0
10044d1f4: aa1403e3    	mov	x3, x20
10044d1f8: 940014e0    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044d1fc: b4000ec0    	cbz	x0, 0x10044d3d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5440>
10044d200: 39400008    	ldrb	w8, [x0]
10044d204: 71001d1f    	cmp	w8, #0x7
10044d208: 54000e68    	b.hi	0x10044d3d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5440>
10044d20c: 52800029    	mov	w9, #0x1                ; =1
10044d210: 1ac82129    	lsl	w9, w9, w8
10044d214: 5280138a    	mov	w10, #0x9c              ; =156
10044d218: 6a0a013f    	tst	w9, w10
10044d21c: 54000c00    	b.eq	0x10044d39c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5408>
10044d220: f8401009    	ldur	x9, [x0, #0x1]
10044d224: f9017be9    	str	x9, [sp, #0x2f0]
10044d228: f9400409    	ldr	x9, [x0, #0x8]
10044d22c: 910963ea    	add	x10, sp, #0x258
10044d230: f809f149    	stur	x9, [x10, #0x9f]
10044d234: 1400005c    	b	0x10044d3a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5410>
10044d238: 794076d4    	ldrh	w20, [x22, #0x3a]
10044d23c: f94087e0    	ldr	x0, [sp, #0x108]
10044d240: f9408fe1    	ldr	x1, [sp, #0x118]
10044d244: 52800022    	mov	w2, #0x1                ; =1
10044d248: aa1403e3    	mov	x3, x20
10044d24c: 940014cb    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044d250: b40010c0    	cbz	x0, 0x10044d468 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54d4>
10044d254: 39400008    	ldrb	w8, [x0]
10044d258: 71001d1f    	cmp	w8, #0x7
10044d25c: 54001068    	b.hi	0x10044d468 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54d4>
10044d260: 52800029    	mov	w9, #0x1                ; =1
10044d264: 1ac82129    	lsl	w9, w9, w8
10044d268: 5280138a    	mov	w10, #0x9c              ; =156
10044d26c: 6a0a013f    	tst	w9, w10
10044d270: 54000e00    	b.eq	0x10044d430 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x549c>
10044d274: f8401009    	ldur	x9, [x0, #0x1]
10044d278: f9017be9    	str	x9, [sp, #0x2f0]
10044d27c: f9400409    	ldr	x9, [x0, #0x8]
10044d280: 910963ea    	add	x10, sp, #0x258
10044d284: f809f149    	stur	x9, [x10, #0x9f]
10044d288: 1400006c    	b	0x10044d438 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54a4>
10044d28c: f94087e0    	ldr	x0, [sp, #0x108]
10044d290: f9408fe1    	ldr	x1, [sp, #0x118]
10044d294: 52800002    	mov	w2, #0x0                ; =0
10044d298: aa1403e3    	mov	x3, x20
10044d29c: 940014b7    	bl	0x100452578 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044d2a0: b4001aa0    	cbz	x0, 0x10044d5f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5660>
10044d2a4: 39400008    	ldrb	w8, [x0]
10044d2a8: 71001d1f    	cmp	w8, #0x7
10044d2ac: 54001a48    	b.hi	0x10044d5f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5660>
10044d2b0: 52800029    	mov	w9, #0x1                ; =1
10044d2b4: 1ac82129    	lsl	w9, w9, w8
10044d2b8: 5280138a    	mov	w10, #0x9c              ; =156
10044d2bc: 6a0a013f    	tst	w9, w10
10044d2c0: 540017e0    	b.eq	0x10044d5bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5628>
10044d2c4: f8401009    	ldur	x9, [x0, #0x1]
10044d2c8: f9017be9    	str	x9, [sp, #0x2f0]
10044d2cc: f9400409    	ldr	x9, [x0, #0x8]
10044d2d0: 910963ea    	add	x10, sp, #0x258
10044d2d4: f809f149    	stur	x9, [x10, #0x9f]
10044d2d8: 140000bb    	b	0x10044d5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5630>
10044d2dc: b94103e8    	ldr	w8, [sp, #0x100]
10044d2e0: 51000908    	sub	w8, w8, #0x2
10044d2e4: 52800049    	mov	w9, #0x2                ; =2
10044d2e8: 7100091f    	cmp	w8, #0x2
10044d2ec: 1a893108    	csel	w8, w8, w9, lo
10044d2f0: 7100091f    	cmp	w8, #0x2
10044d2f4: 54002520    	b.eq	0x10044d798 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5804>
10044d2f8: 7100051f    	cmp	w8, #0x1
10044d2fc: 54002581    	b.ne	0x10044d7ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5818>
10044d300: b90103ff    	str	wzr, [sp, #0x100]
10044d304: 14000163    	b	0x10044d890 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58fc>
10044d308: 7200053f    	tst	w9, #0x3
10044d30c: 540001a0    	b.eq	0x10044d340 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53ac>
10044d310: 381303a8    	sturb	w8, [x29, #-0xd0]
10044d314: f9417be8    	ldr	x8, [sp, #0x2f0]
10044d318: f94077e9    	ldr	x9, [sp, #0xe8]
10044d31c: f9000128    	str	x8, [x9]
10044d320: 910963e8    	add	x8, sp, #0x258
10044d324: f849f108    	ldur	x8, [x8, #0x9f]
10044d328: f8007128    	stur	x8, [x9, #0x7]
10044d32c: d10343a2    	sub	x2, x29, #0xd0
10044d330: f94087e0    	ldr	x0, [sp, #0x108]
10044d334: f9408fe1    	ldr	x1, [sp, #0x118]
10044d338: 940014b8    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044d33c: 1400009c    	b	0x10044d5ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5618>
10044d340: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d344: f9400102    	ldr	x2, [x8]
10044d348: d10343a0    	sub	x0, x29, #0xd0
10044d34c: aa1403e1    	mov	x1, x20
10044d350: f94087e3    	ldr	x3, [sp, #0x108]
10044d354: f9408fe4    	ldr	x4, [sp, #0x118]
10044d358: 94001523    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044d35c: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044d360: f85303a0    	ldur	x0, [x29, #-0xd0]
10044d364: 7100091f    	cmp	w8, #0x2
10044d368: 54008020    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d36c: 92401c09    	and	x9, x0, #0xff
10044d370: f100293f    	cmp	x9, #0xa
10044d374: 54001020    	b.eq	0x10044d578 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x55e4>
10044d378: f85383a8    	ldur	x8, [x29, #-0xc8]
10044d37c: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044d380: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d384: f9400102    	ldr	x2, [x8]
10044d388: d103c3a3    	sub	x3, x29, #0xf0
10044d38c: f94087e0    	ldr	x0, [sp, #0x108]
10044d390: f9408fe1    	ldr	x1, [sp, #0x118]
10044d394: 940014bf    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044d398: 14000085    	b	0x10044d5ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5618>
10044d39c: 7200053f    	tst	w9, #0x3
10044d3a0: 540001a0    	b.eq	0x10044d3d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5440>
10044d3a4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044d3a8: f9417be8    	ldr	x8, [sp, #0x2f0]
10044d3ac: f94077e9    	ldr	x9, [sp, #0xe8]
10044d3b0: f9000128    	str	x8, [x9]
10044d3b4: 910963e8    	add	x8, sp, #0x258
10044d3b8: f849f108    	ldur	x8, [x8, #0x9f]
10044d3bc: f8007128    	stur	x8, [x9, #0x7]
10044d3c0: d10343a2    	sub	x2, x29, #0xd0
10044d3c4: f94087e0    	ldr	x0, [sp, #0x108]
10044d3c8: f9408fe1    	ldr	x1, [sp, #0x118]
10044d3cc: 94001493    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044d3d0: 140000a1    	b	0x10044d654 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56c0>
10044d3d4: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d3d8: f9400102    	ldr	x2, [x8]
10044d3dc: d10343a0    	sub	x0, x29, #0xd0
10044d3e0: aa1403e1    	mov	x1, x20
10044d3e4: f94087e3    	ldr	x3, [sp, #0x108]
10044d3e8: f9408fe4    	ldr	x4, [sp, #0x118]
10044d3ec: 940014fe    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044d3f0: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044d3f4: f85303a0    	ldur	x0, [x29, #-0xd0]
10044d3f8: 7100091f    	cmp	w8, #0x2
10044d3fc: 54007b80    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d400: 92401c08    	and	x8, x0, #0xff
10044d404: f100291f    	cmp	x8, #0xa
10044d408: 54001120    	b.eq	0x10044d62c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5698>
10044d40c: f85383a8    	ldur	x8, [x29, #-0xc8]
10044d410: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044d414: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d418: f9400102    	ldr	x2, [x8]
10044d41c: d103c3a3    	sub	x3, x29, #0xf0
10044d420: f94087e0    	ldr	x0, [sp, #0x108]
10044d424: f9408fe1    	ldr	x1, [sp, #0x118]
10044d428: 9400149a    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044d42c: 1400008a    	b	0x10044d654 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56c0>
10044d430: 7200053f    	tst	w9, #0x3
10044d434: 540001a0    	b.eq	0x10044d468 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54d4>
10044d438: 381303a8    	sturb	w8, [x29, #-0xd0]
10044d43c: f9417be8    	ldr	x8, [sp, #0x2f0]
10044d440: f94077e9    	ldr	x9, [sp, #0xe8]
10044d444: f9000128    	str	x8, [x9]
10044d448: 910963e8    	add	x8, sp, #0x258
10044d44c: f849f108    	ldur	x8, [x8, #0x9f]
10044d450: f8007128    	stur	x8, [x9, #0x7]
10044d454: d10343a2    	sub	x2, x29, #0xd0
10044d458: f94087e0    	ldr	x0, [sp, #0x108]
10044d45c: f9408fe1    	ldr	x1, [sp, #0x118]
10044d460: 9400146e    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044d464: 14000024    	b	0x10044d4f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5560>
10044d468: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d46c: f9400102    	ldr	x2, [x8]
10044d470: d10343a0    	sub	x0, x29, #0xd0
10044d474: aa1403e1    	mov	x1, x20
10044d478: f94087e3    	ldr	x3, [sp, #0x108]
10044d47c: f9408fe4    	ldr	x4, [sp, #0x118]
10044d480: 94001570    	bl	0x100452a40 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044d484: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044d488: 71002d1f    	cmp	w8, #0xb
10044d48c: 54004920    	b.eq	0x10044ddb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e1c>
10044d490: f94077ea    	ldr	x10, [sp, #0xe8]
10044d494: b9400149    	ldr	w9, [x10]
10044d498: b90313e9    	str	w9, [sp, #0x310]
10044d49c: b8403149    	ldur	w9, [x10, #0x3]
10044d4a0: 910963ea    	add	x10, sp, #0x258
10044d4a4: b80bb149    	stur	w9, [x10, #0xbb]
10044d4a8: 7100291f    	cmp	w8, #0xa
10044d4ac: 54000061    	b.ne	0x10044d4b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5524>
10044d4b0: 52800488    	mov	w8, #0x24               ; =36
10044d4b4: 14000012    	b	0x10044d4fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5568>
10044d4b8: f85383a9    	ldur	x9, [x29, #-0xc8]
10044d4bc: 381103a8    	sturb	w8, [x29, #-0xf0]
10044d4c0: b94313e8    	ldr	w8, [sp, #0x310]
10044d4c4: f94067ea    	ldr	x10, [sp, #0xc8]
10044d4c8: b9000148    	str	w8, [x10]
10044d4cc: 910963e8    	add	x8, sp, #0x258
10044d4d0: b84bb108    	ldur	w8, [x8, #0xbb]
10044d4d4: b8003148    	stur	w8, [x10, #0x3]
10044d4d8: f81183a9    	stur	x9, [x29, #-0xe8]
10044d4dc: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d4e0: f9400102    	ldr	x2, [x8]
10044d4e4: d103c3a3    	sub	x3, x29, #0xf0
10044d4e8: f94087e0    	ldr	x0, [sp, #0x108]
10044d4ec: f9408fe1    	ldr	x1, [sp, #0x118]
10044d4f0: 94001468    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044d4f4: b50073c0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d4f8: 52800928    	mov	w8, #0x49               ; =73
10044d4fc: 52800009    	mov	w9, #0x0                ; =0
10044d500: 5280004a    	mov	w10, #0x2               ; =2
10044d504: 14000058    	b	0x10044d664 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d0>
10044d508: 7200053f    	tst	w9, #0x3
10044d50c: 540001a0    	b.eq	0x10044d540 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x55ac>
10044d510: 381303a8    	sturb	w8, [x29, #-0xd0]
10044d514: f9417be8    	ldr	x8, [sp, #0x2f0]
10044d518: f94077e9    	ldr	x9, [sp, #0xe8]
10044d51c: f9000128    	str	x8, [x9]
10044d520: 910963e8    	add	x8, sp, #0x258
10044d524: f849f108    	ldur	x8, [x8, #0x9f]
10044d528: f8007128    	stur	x8, [x9, #0x7]
10044d52c: d10343a2    	sub	x2, x29, #0xd0
10044d530: f94087e0    	ldr	x0, [sp, #0x108]
10044d534: f9408fe1    	ldr	x1, [sp, #0x118]
10044d538: 94001438    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044d53c: 1400001c    	b	0x10044d5ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5618>
10044d540: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d544: f9400102    	ldr	x2, [x8]
10044d548: d10343a0    	sub	x0, x29, #0xd0
10044d54c: aa1403e1    	mov	x1, x20
10044d550: f94087e3    	ldr	x3, [sp, #0x108]
10044d554: f9408fe4    	ldr	x4, [sp, #0x118]
10044d558: 940014a3    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044d55c: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044d560: f85303a0    	ldur	x0, [x29, #-0xd0]
10044d564: 7100091f    	cmp	w8, #0x2
10044d568: 54007020    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d56c: 92401c09    	and	x9, x0, #0xff
10044d570: f100293f    	cmp	x9, #0xa
10044d574: 540000c1    	b.ne	0x10044d58c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x55f8>
10044d578: 7200011f    	tst	w8, #0x1
10044d57c: 52800468    	mov	w8, #0x23               ; =35
10044d580: 1a881508    	cinc	w8, w8, eq
10044d584: 52800029    	mov	w9, #0x1                ; =1
10044d588: 14000036    	b	0x10044d660 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56cc>
10044d58c: f85383a8    	ldur	x8, [x29, #-0xc8]
10044d590: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044d594: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d598: f9400102    	ldr	x2, [x8]
10044d59c: d103c3a3    	sub	x3, x29, #0xf0
10044d5a0: f94087e0    	ldr	x0, [sp, #0x108]
10044d5a4: f9408fe1    	ldr	x1, [sp, #0x118]
10044d5a8: 9400143a    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044d5ac: b5006e00    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d5b0: 52800928    	mov	w8, #0x49               ; =73
10044d5b4: 52800029    	mov	w9, #0x1                ; =1
10044d5b8: 1400002a    	b	0x10044d660 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56cc>
10044d5bc: 7200053f    	tst	w9, #0x3
10044d5c0: 540001a0    	b.eq	0x10044d5f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5660>
10044d5c4: 381303a8    	sturb	w8, [x29, #-0xd0]
10044d5c8: f9417be8    	ldr	x8, [sp, #0x2f0]
10044d5cc: f94077e9    	ldr	x9, [sp, #0xe8]
10044d5d0: f9000128    	str	x8, [x9]
10044d5d4: 910963e8    	add	x8, sp, #0x258
10044d5d8: f849f108    	ldur	x8, [x8, #0x9f]
10044d5dc: f8007128    	stur	x8, [x9, #0x7]
10044d5e0: d10343a2    	sub	x2, x29, #0xd0
10044d5e4: f94087e0    	ldr	x0, [sp, #0x108]
10044d5e8: f9408fe1    	ldr	x1, [sp, #0x118]
10044d5ec: 9400140b    	bl	0x100452618 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044d5f0: 14000019    	b	0x10044d654 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56c0>
10044d5f4: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d5f8: f9400102    	ldr	x2, [x8]
10044d5fc: d10343a0    	sub	x0, x29, #0xd0
10044d600: aa1403e1    	mov	x1, x20
10044d604: f94087e3    	ldr	x3, [sp, #0x108]
10044d608: f9408fe4    	ldr	x4, [sp, #0x118]
10044d60c: 94001476    	bl	0x1004527e4 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044d610: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044d614: f85303a0    	ldur	x0, [x29, #-0xd0]
10044d618: 7100091f    	cmp	w8, #0x2
10044d61c: 54006a80    	b.eq	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d620: 92401c08    	and	x8, x0, #0xff
10044d624: f100291f    	cmp	x8, #0xa
10044d628: 54000061    	b.ne	0x10044d634 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56a0>
10044d62c: 52800488    	mov	w8, #0x24               ; =36
10044d630: 1400000b    	b	0x10044d65c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56c8>
10044d634: f85383a8    	ldur	x8, [x29, #-0xc8]
10044d638: a93123a0    	stp	x0, x8, [x29, #-0xf0]
10044d63c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d640: f9400102    	ldr	x2, [x8]
10044d644: d103c3a3    	sub	x3, x29, #0xf0
10044d648: f94087e0    	ldr	x0, [sp, #0x108]
10044d64c: f9408fe1    	ldr	x1, [sp, #0x118]
10044d650: 94001410    	bl	0x100452690 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044d654: b50068c0    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d658: 52800928    	mov	w8, #0x49               ; =73
10044d65c: 52800009    	mov	w9, #0x0                ; =0
10044d660: 5280002a    	mov	w10, #0x1               ; =1
10044d664: 12001d0b    	and	w11, w8, #0xff
10044d668: 7101257f    	cmp	w11, #0x49
10044d66c: 54009c21    	b.ne	0x10044e9f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a5c>
10044d670: 52800028    	mov	w8, #0x1                ; =1
10044d674: 6a53711f    	tst	w8, w19, lsr #28
10044d678: 52800048    	mov	w8, #0x2                ; =2
10044d67c: 9a880508    	cinc	x8, x8, ne
10044d680: f94093e9    	ldr	x9, [sp, #0x120]
10044d684: 8b294116    	add	x22, x8, w9, uxtw
10044d688: eb1c02df    	cmp	x22, x28
10044d68c: 5400af22    	b.hs	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044d690: b8767b09    	ldr	w9, [x24, x22, lsl #2]
10044d694: 17fffd50    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044d698: 52800008    	mov	w8, #0x0                ; =0
10044d69c: 1a8a0129    	csel	w9, w9, w10, eq
10044d6a0: b81343a9    	stur	w9, [x29, #-0xcc]
10044d6a4: b81303a8    	stur	w8, [x29, #-0xd0]
10044d6a8: d10343a3    	sub	x3, x29, #0xd0
10044d6ac: f94087e0    	ldr	x0, [sp, #0x108]
10044d6b0: f9408fe1    	ldr	x1, [sp, #0x118]
10044d6b4: aa1303e2    	mov	x2, x19
10044d6b8: 94001b41    	bl	0x1004543bc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots27commit_number_local_discard17hecbdd4c596cb35adE>
10044d6bc: b5006580    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d6c0: f94083e8    	ldr	x8, [sp, #0x100]
10044d6c4: 7211011f    	tst	w8, #0x8000
10044d6c8: 52800048    	mov	w8, #0x2                ; =2
10044d6cc: 1a880508    	cinc	w8, w8, ne
10044d6d0: 0b140109    	add	w9, w8, w20
10044d6d4: 17fffd40    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044d6d8: d10343a0    	sub	x0, x29, #0xd0
10044d6dc: aa1403e1    	mov	x1, x20
10044d6e0: 97ff7296    	bl	0x10042a138 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044d6e4: 385303b6    	ldurb	w22, [x29, #-0xd0]
10044d6e8: 71002adf    	cmp	w22, #0xa
10044d6ec: 910963e9    	add	x9, sp, #0x258
10044d6f0: f94077e8    	ldr	x8, [sp, #0xe8]
10044d6f4: 54ffc321    	b.ne	0x10044cf58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4fc4>
10044d6f8: f85383a0    	ldur	x0, [x29, #-0xc8]
10044d6fc: b4ffa6a0    	cbz	x0, 0x10044cbd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c3c>
10044d700: 1400031b    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044d704: bd400541    	ldr	s1, [x10, #0x4]
10044d708: 0f20a421    	sshll.2d	v1, v1, #0x0
10044d70c: 5e61d821    	scvtf	d1, d1
10044d710: 5311692a    	ubfx	w10, w9, #17, #10
10044d714: 7102415f    	cmp	w10, #0x90
10044d718: 5400010c    	b.gt	0x10044d738 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57a4>
10044d71c: 71023d5f    	cmp	w10, #0x8f
10044d720: 54000720    	b.eq	0x10044d804 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5870>
10044d724: 7102415f    	cmp	w10, #0x90
10044d728: 54000621    	b.ne	0x10044d7ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5858>
10044d72c: 1e612000    	fcmp	d0, d1
10044d730: 1a9f87ea    	cset	w10, ls
10044d734: 14000094    	b	0x10044d984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59f0>
10044d738: 7102455f    	cmp	w10, #0x91
10044d73c: 540006a0    	b.eq	0x10044d810 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x587c>
10044d740: 7102495f    	cmp	w10, #0x92
10044d744: 54000541    	b.ne	0x10044d7ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5858>
10044d748: 1e612000    	fcmp	d0, d1
10044d74c: 1a9fb7ea    	cset	w10, ge
10044d750: 1400008d    	b	0x10044d984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59f0>
10044d754: bd400521    	ldr	s1, [x9, #0x4]
10044d758: 0f20a421    	sshll.2d	v1, v1, #0x0
10044d75c: 5e61d821    	scvtf	d1, d1
10044d760: 1e612000    	fcmp	d0, d1
10044d764: 1a9f57e9    	cset	w9, mi
10044d768: 7205019f    	tst	w12, #0x8000000
10044d76c: 1a9f17ea    	cset	w10, eq
10044d770: 4a090149    	eor	w9, w10, w9
10044d774: 36001109    	tbz	w9, #0x0, 0x10044d994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5a00>
10044d778: 52800029    	mov	w9, #0x1                ; =1
10044d77c: 6a53713f    	tst	w9, w19, lsr #28
10044d780: 52800049    	mov	w9, #0x2                ; =2
10044d784: 9a890529    	cinc	x9, x9, ne
10044d788: 8b080136    	add	x22, x9, x8
10044d78c: eb1c02df    	cmp	x22, x28
10044d790: 54fff803    	b.lo	0x10044d690 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56fc>
10044d794: 14000537    	b	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044d798: b94103e9    	ldr	w9, [sp, #0x100]
10044d79c: b94043e8    	ldr	w8, [sp, #0x40]
10044d7a0: 292623a9    	stp	w9, w8, [x29, #-0xd0]
10044d7a4: f81383b4    	stur	x20, [x29, #-0xc8]
10044d7a8: 14000035    	b	0x10044d87c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58e8>
10044d7ac: b94043e8    	ldr	w8, [sp, #0x40]
10044d7b0: 53107d03    	lsr	w3, w8, #16
10044d7b4: 721f391f    	tst	w8, #0xfffe
10044d7b8: 54000560    	b.eq	0x10044d864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58d0>
10044d7bc: d10343a0    	sub	x0, x29, #0xd0
10044d7c0: f94087e1    	ldr	x1, [sp, #0x108]
10044d7c4: f9408fe2    	ldr	x2, [sp, #0x118]
10044d7c8: 94002604    	bl	0x100456fd8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots19immediate_parameter17h006e24dbcb03b212E>
10044d7cc: 1400002a    	b	0x10044d874 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58e0>
10044d7d0: f85403a9    	ldur	x9, [x29, #-0xc0]
10044d7d4: 71000d1f    	cmp	w8, #0x3
10044d7d8: 540035a0    	b.eq	0x10044de8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ef8>
10044d7dc: 36001988    	tbz	w8, #0x0, 0x10044db0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b78>
10044d7e0: f81383a9    	stur	x9, [x29, #-0xc8]
10044d7e4: 52800088    	mov	w8, #0x4                ; =4
10044d7e8: 140000cc    	b	0x10044db18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b84>
10044d7ec: 51022d4b    	sub	w11, w10, #0x8b
10044d7f0: 7100057f    	cmp	w11, #0x1
10044d7f4: 54000be8    	b.hi	0x10044d970 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59dc>
10044d7f8: 1e612000    	fcmp	d0, d1
10044d7fc: 1a9f17ea    	cset	w10, eq
10044d800: 14000061    	b	0x10044d984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59f0>
10044d804: 1e612000    	fcmp	d0, d1
10044d808: 1a9f57ea    	cset	w10, mi
10044d80c: 1400005e    	b	0x10044d984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59f0>
10044d810: 1e612000    	fcmp	d0, d1
10044d814: 1a9fd7ea    	cset	w10, gt
10044d818: 1400005b    	b	0x10044d984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59f0>
10044d81c: 9403130d    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044d820: 17fffceb    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044d824: b853c3a9    	ldur	w9, [x29, #-0xc4]
10044d828: fc5403ab    	ldur	d11, [x29, #-0xc0]
10044d82c: 7200011f    	tst	w8, #0x1
10044d830: 52800068    	mov	w8, #0x3                ; =3
10044d834: 1a880508    	cinc	w8, w8, ne
10044d838: 2904a7e8    	stp	w8, w9, [sp, #0x24]
10044d83c: f94087e8    	ldr	x8, [sp, #0x108]
10044d840: a940851b    	ldp	x27, x1, [x8, #0x8]
10044d844: aa1b03e0    	mov	x0, x27
10044d848: f90083e1    	str	x1, [sp, #0x100]
10044d84c: f9408fe2    	ldr	x2, [sp, #0x118]
10044d850: 97f4c1b7    	bl	0x10017df2c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044d854: aa0103f6    	mov	x22, x1
10044d858: 36000a20    	tbz	w0, #0x0, 0x10044d99c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5a08>
10044d85c: b4000b96    	cbz	x22, 0x10044d9cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5a38>
10044d860: 140004a3    	b	0x10044eaec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b58>
10044d864: d10343a0    	sub	x0, x29, #0xd0
10044d868: f94087e1    	ldr	x1, [sp, #0x108]
10044d86c: f9408fe2    	ldr	x2, [sp, #0x118]
10044d870: 940025b3    	bl	0x100456f3c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots15immediate_local17h49fd8b485127d92bE>
10044d874: b85303a8    	ldur	w8, [x29, #-0xd0]
10044d878: b90103e8    	str	w8, [sp, #0x100]
10044d87c: b94103e8    	ldr	w8, [sp, #0x100]
10044d880: 7100091f    	cmp	w8, #0x2
10044d884: 54ffc4c0    	b.eq	0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044d888: b85343a8    	ldur	w8, [x29, #-0xcc]
10044d88c: b90043e8    	str	w8, [sp, #0x40]
10044d890: fc5383ab    	ldur	d11, [x29, #-0xc8]
10044d894: 794072c8    	ldrh	w8, [x22, #0x38]
10044d898: 7100051f    	cmp	w8, #0x1
10044d89c: 1a9f97e4    	cset	w4, hi
10044d8a0: 794076c5    	ldrh	w5, [x22, #0x3a]
10044d8a4: d10343a0    	sub	x0, x29, #0xd0
10044d8a8: f94087e1    	ldr	x1, [sp, #0x108]
10044d8ac: f9408fe2    	ldr	x2, [sp, #0x118]
10044d8b0: b9403be3    	ldr	w3, [sp, #0x38]
10044d8b4: 940025f0    	bl	0x100457074 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots31admit_numeric_local_with_source17h7bd7fb1048a69868E>
10044d8b8: b85303b4    	ldur	w20, [x29, #-0xd0]
10044d8bc: 71000a9f    	cmp	w20, #0x2
10044d8c0: 54ffc2e0    	b.eq	0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044d8c4: 1e790143    	fcvtzu	w3, d10
10044d8c8: b85343a8    	ldur	w8, [x29, #-0xcc]
10044d8cc: b90033e8    	str	w8, [sp, #0x30]
10044d8d0: fc5383aa    	ldur	d10, [x29, #-0xc8]
10044d8d4: a9740ba8    	ldp	x8, x2, [x29, #-0xc0]
10044d8d8: f9001fe8    	str	x8, [sp, #0x38]
10044d8dc: f9407fe8    	ldr	x8, [sp, #0xf8]
10044d8e0: f9400101    	ldr	x1, [x8]
10044d8e4: d10343a0    	sub	x0, x29, #0xd0
10044d8e8: 94001fe5    	bl	0x10045587c <__ZN13quickjs_oxide6engine6object16ordinary_storage62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$24peek_dense_number_result17hacaa150a333ea219E>
10044d8ec: b85303a8    	ldur	w8, [x29, #-0xd0]
10044d8f0: 3707c168    	tbnz	w8, #0x0, 0x10044d11c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5188>
10044d8f4: b85383a8    	ldur	w8, [x29, #-0xc8]
10044d8f8: fc5403a0    	ldur	d0, [x29, #-0xc0]
10044d8fc: bc53c3a1    	ldur	s1, [x29, #-0xc4]
10044d900: 0f20a421    	sshll.2d	v1, v1, #0x0
10044d904: 5e61d821    	scvtf	d1, d1
10044d908: 7200011f    	tst	w8, #0x1
10044d90c: 1e611c00    	fcsel	d0, d0, d1, ne
10044d910: b94043e8    	ldr	w8, [sp, #0x40]
10044d914: 1e620101    	scvtf	d1, w8
10044d918: b94103e8    	ldr	w8, [sp, #0x100]
10044d91c: 7200011f    	tst	w8, #0x1
10044d920: 1e611d61    	fcsel	d1, d11, d1, ne
10044d924: 1e600820    	fmul	d0, d1, d0
10044d928: 1e780009    	fcvtzs	w9, d0
10044d92c: 1e620121    	scvtf	d1, w9
10044d930: 1e612000    	fcmp	d0, d1
10044d934: 54000121    	b.ne	0x10044d958 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59c4>
10044d938: 1e602008    	fcmp	d0, #0.0
10044d93c: 9e660008    	fmov	x8, d0
10044d940: fa400904    	ccmp	x8, #0x0, #0x4, eq
10044d944: 540000a1    	b.ne	0x10044d958 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59c4>
10044d948: 52800008    	mov	w8, #0x0                ; =0
10044d94c: b90013e9    	str	w9, [sp, #0x10]
10044d950: 1e604120    	fmov	d0, d9
10044d954: 14000002    	b	0x10044d95c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59c8>
10044d958: 52800028    	mov	w8, #0x1                ; =1
10044d95c: 34000b1b    	cbz	w27, 0x10044dabc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b28>
10044d960: b94013e9    	ldr	w9, [sp, #0x10]
10044d964: b90017e9    	str	w9, [sp, #0x14]
10044d968: 1e604008    	fmov	d8, d0
10044d96c: 14000081    	b	0x10044db70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bdc>
10044d970: 5102354a    	sub	w10, w10, #0x8d
10044d974: 7100055f    	cmp	w10, #0x1
10044d978: 5400b428    	b.hi	0x10044effc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7068>
10044d97c: 1e612000    	fcmp	d0, d1
10044d980: 1a9f07ea    	cset	w10, ne
10044d984: 7205013f    	tst	w9, #0x8000000
10044d988: 1a9f17e9    	cset	w9, eq
10044d98c: 4a0a0129    	eor	w9, w9, w10
10044d990: 3707ef49    	tbnz	w9, #0x0, 0x10044d778 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57e4>
10044d994: 91001289    	add	x9, x20, #0x4
10044d998: 17fffc8f    	b	0x10044cbd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c40>
10044d99c: f94083e8    	ldr	x8, [sp, #0x100]
10044d9a0: eb16011f    	cmp	x8, x22
10044d9a4: 5400b389    	b.ls	0x10044f014 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7080>
10044d9a8: 8b161368    	add	x8, x27, x22, lsl #4
10044d9ac: 2944a7ea    	ldp	w10, w9, [sp, #0x24]
10044d9b0: 3900010a    	strb	w10, [x8]
10044d9b4: b9000509    	str	w9, [x8, #0x4]
10044d9b8: fd00050b    	str	d11, [x8, #0x8]
10044d9bc: f9408fe9    	ldr	x9, [sp, #0x118]
10044d9c0: f9402128    	ldr	x8, [x9, #0x40]
10044d9c4: 91000508    	add	x8, x8, #0x1
10044d9c8: f9002128    	str	x8, [x9, #0x40]
10044d9cc: b9403be8    	ldr	w8, [sp, #0x38]
10044d9d0: 7100011f    	cmp	w8, #0x0
10044d9d4: 52800068    	mov	w8, #0x3                ; =3
10044d9d8: 1a880508    	cinc	w8, w8, ne
10044d9dc: 378002f4    	tbnz	w20, #0x10, 0x10044da38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5aa4>
10044d9e0: f94073e9    	ldr	x9, [sp, #0xe0]
10044d9e4: f9400129    	ldr	x9, [x9]
10044d9e8: f9407bea    	ldr	x10, [sp, #0xf0]
10044d9ec: f940014a    	ldr	x10, [x10]
10044d9f0: eb09014a    	subs	x10, x10, x9
10044d9f4: 9a8a33ea    	csel	x10, xzr, x10, lo
10044d9f8: f94023eb    	ldr	x11, [sp, #0x40]
10044d9fc: eb0b015f    	cmp	x10, x11
10044da00: 54009749    	b.ls	0x10044ece8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6d54>
10044da04: f94087ea    	ldr	x10, [sp, #0x108]
10044da08: f940094a    	ldr	x10, [x10, #0x10]
10044da0c: f94023eb    	ldr	x11, [sp, #0x40]
10044da10: 8b0b0136    	add	x22, x9, x11
10044da14: eb0a02df    	cmp	x22, x10
10044da18: 5400b0e2    	b.hs	0x10044f034 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x70a0>
10044da1c: f94087e9    	ldr	x9, [sp, #0x108]
10044da20: f9400529    	ldr	x9, [x9, #0x8]
10044da24: 8b161129    	add	x9, x9, x22, lsl #4
10044da28: 3940012a    	ldrb	w10, [x9]
10044da2c: 7100395f    	cmp	w10, #0xe
10044da30: 540002e1    	b.ne	0x10044da8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5af8>
10044da34: 140004c8    	b	0x10044ed54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6dc0>
10044da38: f9406be9    	ldr	x9, [sp, #0xd0]
10044da3c: f9400129    	ldr	x9, [x9]
10044da40: f94073ea    	ldr	x10, [sp, #0xe0]
10044da44: f940014a    	ldr	x10, [x10]
10044da48: eb09014a    	subs	x10, x10, x9
10044da4c: 9a8a33ea    	csel	x10, xzr, x10, lo
10044da50: f94023eb    	ldr	x11, [sp, #0x40]
10044da54: eb0b015f    	cmp	x10, x11
10044da58: 54009129    	b.ls	0x10044ec7c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ce8>
10044da5c: f94087ea    	ldr	x10, [sp, #0x108]
10044da60: f940094a    	ldr	x10, [x10, #0x10]
10044da64: f94023eb    	ldr	x11, [sp, #0x40]
10044da68: 8b0b0136    	add	x22, x9, x11
10044da6c: eb0a02df    	cmp	x22, x10
10044da70: 5400adc2    	b.hs	0x10044f028 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7094>
10044da74: f94087e9    	ldr	x9, [sp, #0x108]
10044da78: f9400529    	ldr	x9, [x9, #0x8]
10044da7c: 8b161129    	add	x9, x9, x22, lsl #4
10044da80: 3940012a    	ldrb	w10, [x9]
10044da84: 7100395f    	cmp	w10, #0xe
10044da88: 540099c0    	b.eq	0x10044edc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6e2c>
10044da8c: 39000128    	strb	w8, [x9]
10044da90: b94033e8    	ldr	w8, [sp, #0x30]
10044da94: b9000528    	str	w8, [x9, #0x4]
10044da98: fd00052a    	str	d10, [x9, #0x8]
10044da9c: 52800028    	mov	w8, #0x1                ; =1
10044daa0: 6a53711f    	tst	w8, w19, lsr #28
10044daa4: 52800048    	mov	w8, #0x2                ; =2
10044daa8: 9a880508    	cinc	x8, x8, ne
10044daac: 8b190116    	add	x22, x8, x25
10044dab0: eb1c02df    	cmp	x22, x28
10044dab4: 54ffdee3    	b.lo	0x10044d690 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56fc>
10044dab8: 1400046e    	b	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044dabc: 37000094    	tbnz	w20, #0x0, 0x10044dacc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b38>
10044dac0: 360004c8    	tbz	w8, #0x0, 0x10044db58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bc4>
10044dac4: b94033e9    	ldr	w9, [sp, #0x30]
10044dac8: 1e62012a    	scvtf	d10, w9
10044dacc: b94013e9    	ldr	w9, [sp, #0x10]
10044dad0: 1e620121    	scvtf	d1, w9
10044dad4: 7100011f    	cmp	w8, #0x0
10044dad8: 1e611c01    	fcsel	d1, d0, d1, ne
10044dadc: 1e612941    	fadd	d1, d10, d1
10044dae0: 1e780029    	fcvtzs	w9, d1
10044dae4: 1e620122    	scvtf	d2, w9
10044dae8: 1e622020    	fcmp	d1, d2
10044daec: 540000a1    	b.ne	0x10044db00 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b6c>
10044daf0: 1e602028    	fcmp	d1, #0.0
10044daf4: 9e660028    	fmov	x8, d1
10044daf8: fa400904    	ccmp	x8, #0x0, #0x4, eq
10044dafc: 54000360    	b.eq	0x10044db68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bd4>
10044db00: 52800028    	mov	w8, #0x1                ; =1
10044db04: 1e604028    	fmov	d8, d1
10044db08: 1400001a    	b	0x10044db70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bdc>
10044db0c: b853c3a8    	ldur	w8, [x29, #-0xc4]
10044db10: b81343a8    	stur	w8, [x29, #-0xcc]
10044db14: 52800068    	mov	w8, #0x3                ; =3
10044db18: 381303a8    	sturb	w8, [x29, #-0xd0]
10044db1c: d10343a2    	sub	x2, x29, #0xd0
10044db20: f94087e0    	ldr	x0, [sp, #0x108]
10044db24: f9408fe1    	ldr	x1, [sp, #0x118]
10044db28: 94001276    	bl	0x100452500 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044db2c: b5004200    	cbnz	x0, 0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044db30: 52800028    	mov	w8, #0x1                ; =1
10044db34: 6a53711f    	tst	w8, w19, lsr #28
10044db38: 52800048    	mov	w8, #0x2                ; =2
10044db3c: 9a880508    	cinc	x8, x8, ne
10044db40: 8b140116    	add	x22, x8, x20
10044db44: eb1c02df    	cmp	x22, x28
10044db48: 54ffda43    	b.lo	0x10044d690 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56fc>
10044db4c: 14000449    	b	0x10044ec70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cdc>
10044db50: 94031240    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044db54: 17fffc1e    	b	0x10044cbcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c38>
10044db58: b94013e9    	ldr	w9, [sp, #0x10]
10044db5c: b94033ea    	ldr	w10, [sp, #0x30]
10044db60: 2b090149    	adds	w9, w10, w9
10044db64: 54fffb06    	b.vs	0x10044dac4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b30>
10044db68: 52800008    	mov	w8, #0x0                ; =0
10044db6c: b90017e9    	str	w9, [sp, #0x14]
10044db70: 7100011f    	cmp	w8, #0x0
10044db74: 52800068    	mov	w8, #0x3                ; =3
10044db78: 1a880508    	cinc	w8, w8, ne
10044db7c: f9401fe9    	ldr	x9, [sp, #0x38]
10044db80: 39000128    	strb	w8, [x9]
10044db84: b94017e8    	ldr	w8, [sp, #0x14]
10044db88: b9000528    	str	w8, [x9, #0x4]
10044db8c: fd000528    	str	d8, [x9, #0x8]
10044db90: 17fffc51    	b	0x10044ccd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d40>
10044db94: d2800008    	mov	x8, #0x0                ; =0
10044db98: 17fffc36    	b	0x10044cc70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4cdc>
10044db9c: f0001394    	adrp	x20, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044dba0: 397eea9f    	ldrb	wzr, [x20, #0xfba]
10044dba4: 52800620    	mov	w0, #0x31               ; =49
10044dba8: 940327e2    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044dbac: b40082e0    	cbz	x0, 0x10044ec08 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c74>
10044dbb0: 52800f28    	mov	w8, #0x79               ; =121
10044dbb4: 3900c008    	strb	w8, [x0, #0x30]
10044dbb8: 90000ae8    	adrp	x8, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044dbbc: 911fed08    	add	x8, x8, #0x7fb
10044dbc0: ad400500    	ldp	q0, q1, [x8]
10044dbc4: ad000400    	stp	q0, q1, [x0]
10044dbc8: 3dc00900    	ldr	q0, [x8, #0x20]
10044dbcc: 3d800800    	str	q0, [x0, #0x20]
10044dbd0: 528000a8    	mov	w8, #0x5                ; =5
10044dbd4: 381783a8    	sturb	w8, [x29, #-0x88]
10044dbd8: 52800628    	mov	w8, #0x31               ; =49
10044dbdc: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044dbe0: f81703bf    	stur	xzr, [x29, #-0x90]
10044dbe4: f81583a8    	stur	x8, [x29, #-0xa8]
10044dbe8: f81303bf    	stur	xzr, [x29, #-0xd0]
10044dbec: 397eea9f    	ldrb	wzr, [x20, #0xfba]
10044dbf0: 52800a00    	mov	w0, #0x50               ; =80
10044dbf4: 940327cf    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044dbf8: b40024a0    	cbz	x0, 0x10044e08c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60f8>
10044dbfc: ad7a87a0    	ldp	q0, q1, [x29, #-0xb0]
10044dc00: ad010400    	stp	q0, q1, [x0, #0x20]
10044dc04: 3cd703a0    	ldur	q0, [x29, #-0x90]
10044dc08: 3d801000    	str	q0, [x0, #0x40]
10044dc0c: ad7983a1    	ldp	q1, q0, [x29, #-0xd0]
10044dc10: ad000001    	stp	q1, q0, [x0]
10044dc14: f90006a0    	str	x0, [x21, #0x8]
10044dc18: 52800928    	mov	w8, #0x49               ; =73
10044dc1c: f94093f4    	ldr	x20, [sp, #0x120]
10044dc20: 390002a8    	strb	w8, [x21]
10044dc24: f94097e8    	ldr	x8, [sp, #0x128]
10044dc28: a902d113    	stp	x19, x20, [x8, #0x28]
10044dc2c: 17ffe903    	b	0x100448038 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xa4>
10044dc30: f0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044dc34: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044dc38: 52800460    	mov	w0, #0x23               ; =35
10044dc3c: 940327bd    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044dc40: b4008020    	cbz	x0, 0x10044ec44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6cb0>
10044dc44: 528d8c28    	mov	w8, #0x6c61             ; =27745
10044dc48: 72ac8d28    	movk	w8, #0x6469, lsl #16
10044dc4c: b801f008    	stur	w8, [x0, #0x1f]
10044dc50: 90000ae8    	adrp	x8, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044dc54: 9120b108    	add	x8, x8, #0x82c
10044dc58: ad400500    	ldp	q0, q1, [x8]
10044dc5c: ad000400    	stp	q0, q1, [x0]
10044dc60: 528000a8    	mov	w8, #0x5                ; =5
10044dc64: 381783a8    	sturb	w8, [x29, #-0x88]
10044dc68: 52800468    	mov	w8, #0x23               ; =35
10044dc6c: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044dc70: f81703bf    	stur	xzr, [x29, #-0x90]
10044dc74: f81583a8    	stur	x8, [x29, #-0xa8]
10044dc78: f81303bf    	stur	xzr, [x29, #-0xd0]
10044dc7c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044dc80: 52800a00    	mov	w0, #0x50               ; =80
10044dc84: 940327ab    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044dc88: b4005e20    	cbz	x0, 0x10044e84c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68b8>
10044dc8c: ad7a87a0    	ldp	q0, q1, [x29, #-0xb0]
10044dc90: ad010400    	stp	q0, q1, [x0, #0x20]
10044dc94: 3cd703a0    	ldur	q0, [x29, #-0x90]
10044dc98: 3d801000    	str	q0, [x0, #0x40]
10044dc9c: ad7983a1    	ldp	q1, q0, [x29, #-0xd0]
10044dca0: ad000001    	stp	q1, q0, [x0]
10044dca4: f90006a0    	str	x0, [x21, #0x8]
10044dca8: 52800928    	mov	w8, #0x49               ; =73
10044dcac: f94093f3    	ldr	x19, [sp, #0x120]
10044dcb0: aa1303f4    	mov	x20, x19
10044dcb4: 17ffffdb    	b	0x10044dc20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c8c>
10044dcb8: f94093f3    	ldr	x19, [sp, #0x120]
10044dcbc: 940311b5    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044dcc0: 140001ab    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044dcc4: 121e7b68    	and	w8, w27, #0xfffffffd
10044dcc8: 7102311f    	cmp	w8, #0x8c
10044dccc: 54000161    	b.ne	0x10044dcf8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d64>
10044dcd0: 71023b7f    	cmp	w27, #0x8e
10044dcd4: 1a9f17e8    	cset	w8, eq
10044dcd8: 52800849    	mov	w9, #0x42               ; =66
10044dcdc: 140002d9    	b	0x10044e840 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68ac>
10044dce0: d0000ac0    	adrp	x0, 0x1005a7000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x34e90>
10044dce4: 910d0c00    	add	x0, x0, #0x343
10044dce8: b0001222    	adrp	x2, 0x100692000 <dyld_stub_binder+0x100692000>
10044dcec: 9136e042    	add	x2, x2, #0xdb8
10044dcf0: 52800481    	mov	w1, #0x24               ; =36
10044dcf4: 9402db1e    	bl	0x10050496c <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044dcf8: aa1b03e0    	mov	x0, x27
10044dcfc: 94001ca1    	bl	0x100454f80 <__ZN13quickjs_oxide6engine2vm7numeric9operation11NumericKind10for_opcode17h734c5b14b50f18b1E>
10044dd00: 12001c08    	and	w8, w0, #0xff
10044dd04: 7100651f    	cmp	w8, #0x19
10044dd08: 540059a1    	b.ne	0x10044e83c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68a8>
10044dd0c: 90000ae1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044dd10: 91223421    	add	x1, x1, #0x88d
10044dd14: f94093f3    	ldr	x19, [sp, #0x120]
10044dd18: 528000a0    	mov	w0, #0x5                ; =5
10044dd1c: 528003e2    	mov	w2, #0x1f               ; =31
10044dd20: 97f4a1d8    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044dd24: 14000192    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044dd28: 528007c8    	mov	w8, #0x3e               ; =62
10044dd2c: 14000192    	b	0x10044e374 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e0>
10044dd30: f9001719    	str	x25, [x24, #0x28]
10044dd34: 1400031d    	b	0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
10044dd38: d0000a80    	adrp	x0, 0x10059f000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x2ce90>
10044dd3c: 91328400    	add	x0, x0, #0xca1
10044dd40: b0001202    	adrp	x2, 0x10068e000 <dyld_stub_binder+0x10068e000>
10044dd44: 911d6042    	add	x2, x2, #0x758
10044dd48: 52800341    	mov	w1, #0x1a               ; =26
10044dd4c: 9402db08    	bl	0x10050496c <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044dd50: d0000ac0    	adrp	x0, 0x1005a7000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x34e90>
10044dd54: 910d0c00    	add	x0, x0, #0x343
10044dd58: b0001222    	adrp	x2, 0x100692000 <dyld_stub_binder+0x100692000>
10044dd5c: 913c0042    	add	x2, x2, #0xf00
10044dd60: 52800481    	mov	w1, #0x24               ; =36
10044dd64: 9402db02    	bl	0x10050496c <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044dd68: f94093f3    	ldr	x19, [sp, #0x120]
10044dd6c: 94031189    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044dd70: 1400035e    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044dd74: 7101cb7f    	cmp	w27, #0x72
10044dd78: 54001161    	b.ne	0x10044dfa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6010>
10044dd7c: 52800628    	mov	w8, #0x31               ; =49
10044dd80: 1400017d    	b	0x10044e374 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e0>
10044dd84: 52800096    	mov	w22, #0x4               ; =4
10044dd88: 7102bf7f    	cmp	w27, #0xaf
10044dd8c: 5400170c    	b.gt	0x10044e06c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60d8>
10044dd90: 7102bb7f    	cmp	w27, #0xae
10044dd94: 54003220    	b.eq	0x10044e3d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6444>
10044dd98: 7102bf7f    	cmp	w27, #0xaf
10044dd9c: f9408fe1    	ldr	x1, [sp, #0x118]
10044dda0: f94087e0    	ldr	x0, [sp, #0x108]
10044dda4: 54003241    	b.ne	0x10044e3ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6458>
10044dda8: 52800036    	mov	w22, #0x1               ; =1
10044ddac: 14000190    	b	0x10044e3ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6458>
10044ddb0: f85383a0    	ldur	x0, [x29, #-0xc8]
10044ddb4: 1400016e    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ddb8: 52800488    	mov	w8, #0x24               ; =36
10044ddbc: 390002a8    	strb	w8, [x21]
10044ddc0: f94083e8    	ldr	x8, [sp, #0x100]
10044ddc4: 790006a8    	strh	w8, [x21, #0x2]
10044ddc8: 0f000420    	movi.2s	v0, #0x1
10044ddcc: bd0006a0    	str	s0, [x21, #0x4]
10044ddd0: 1400016a    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044ddd4: 121f1b68    	and	w8, w27, #0xfe
10044ddd8: 7102e11f    	cmp	w8, #0xb8
10044dddc: 1a9f17e8    	cset	w8, eq
10044dde0: 7102db7f    	cmp	w27, #0xb6
10044dde4: 52801729    	mov	w9, #0xb9               ; =185
10044dde8: 7a491364    	ccmp	w27, w9, #0x4, ne
10044ddec: 528003c9    	mov	w9, #0x1e               ; =30
10044ddf0: 390002a9    	strb	w9, [x21]
10044ddf4: f94083e9    	ldr	x9, [sp, #0x100]
10044ddf8: 790006a9    	strh	w9, [x21, #0x2]
10044ddfc: 1a9f17e9    	cset	w9, eq
10044de00: 390012a8    	strb	w8, [x21, #0x4]
10044de04: 390016a9    	strb	w9, [x21, #0x5]
10044de08: 1400015c    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044de0c: 51002928    	sub	w8, w9, #0xa
10044de10: 7100151f    	cmp	w8, #0x5
10044de14: 54005c82    	b.hs	0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044de18: 9403115e    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044de1c: 14000333    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044de20: f0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044de24: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044de28: 52800454    	mov	w20, #0x22              ; =34
10044de2c: 52800440    	mov	w0, #0x22               ; =34
10044de30: 94032740    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044de34: b4008580    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044de38: 528e6c88    	mov	w8, #0x7364             ; =29540
10044de3c: 79004008    	strh	w8, [x0, #0x20]
10044de40: b0000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044de44: 91071508    	add	x8, x8, #0x1c5
10044de48: ad400500    	ldp	q0, q1, [x8]
10044de4c: ad000400    	stp	q0, q1, [x0]
10044de50: 528000a8    	mov	w8, #0x5                ; =5
10044de54: 381783a8    	sturb	w8, [x29, #-0x88]
10044de58: 52800448    	mov	w8, #0x22               ; =34
10044de5c: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044de60: f81703bf    	stur	xzr, [x29, #-0x90]
10044de64: f81583a8    	stur	x8, [x29, #-0xa8]
10044de68: f81303bf    	stur	xzr, [x29, #-0xd0]
10044de6c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044de70: 52800a00    	mov	w0, #0x50               ; =80
10044de74: 9403272f    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044de78: b5002580    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044de7c: 52800100    	mov	w0, #0x8                ; =8
10044de80: 52800a01    	mov	w1, #0x50               ; =80
10044de84: 9402d936    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044de88: 14000453    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044de8c: f90006a9    	str	x9, [x21, #0x8]
10044de90: 14000138    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044de94: 52800488    	mov	w8, #0x24               ; =36
10044de98: 390002a8    	strb	w8, [x21]
10044de9c: f94083e8    	ldr	x8, [sp, #0x100]
10044dea0: 790006a8    	strh	w8, [x21, #0x2]
10044dea4: 0f000440    	movi.2s	v0, #0x2
10044dea8: bd0006a0    	str	s0, [x21, #0x4]
10044deac: 14000133    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044deb0: f0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044deb4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044deb8: 528002b4    	mov	w20, #0x15              ; =21
10044debc: 528002a0    	mov	w0, #0x15               ; =21
10044dec0: 9403271c    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044dec4: b4008100    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044dec8: b0000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044decc: 9106c108    	add	x8, x8, #0x1b0
10044ded0: 3dc00100    	ldr	q0, [x8]
10044ded4: 3d800000    	str	q0, [x0]
10044ded8: f840d108    	ldur	x8, [x8, #0xd]
10044dedc: f800d008    	stur	x8, [x0, #0xd]
10044dee0: 528000a8    	mov	w8, #0x5                ; =5
10044dee4: 381783a8    	sturb	w8, [x29, #-0x88]
10044dee8: 528002a8    	mov	w8, #0x15               ; =21
10044deec: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044def0: f81703bf    	stur	xzr, [x29, #-0x90]
10044def4: f81583a8    	stur	x8, [x29, #-0xa8]
10044def8: f81303bf    	stur	xzr, [x29, #-0xd0]
10044defc: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044df00: 52800a00    	mov	w0, #0x50               ; =80
10044df04: 9403270b    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044df08: b5002100    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044df0c: 52800100    	mov	w0, #0x8                ; =8
10044df10: 52800a01    	mov	w1, #0x50               ; =80
10044df14: 9402d912    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044df18: 1400042f    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044df1c: f94093f3    	ldr	x19, [sp, #0x120]
10044df20: 9403111c    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044df24: 140002f1    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044df28: 528003e8    	mov	w8, #0x1f               ; =31
10044df2c: 390002a8    	strb	w8, [x21]
10044df30: b90006bf    	str	wzr, [x21, #0x4]
10044df34: 14000111    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044df38: f0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044df3c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044df40: 52800454    	mov	w20, #0x22              ; =34
10044df44: 52800440    	mov	w0, #0x22               ; =34
10044df48: 940326fa    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044df4c: b4007cc0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044df50: 528e6c88    	mov	w8, #0x7364             ; =29540
10044df54: 79004008    	strh	w8, [x0, #0x20]
10044df58: b0000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044df5c: 91071508    	add	x8, x8, #0x1c5
10044df60: ad400500    	ldp	q0, q1, [x8]
10044df64: ad000400    	stp	q0, q1, [x0]
10044df68: 528000a8    	mov	w8, #0x5                ; =5
10044df6c: 381783a8    	sturb	w8, [x29, #-0x88]
10044df70: 52800448    	mov	w8, #0x22               ; =34
10044df74: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044df78: f81703bf    	stur	xzr, [x29, #-0x90]
10044df7c: f81583a8    	stur	x8, [x29, #-0xa8]
10044df80: f81303bf    	stur	xzr, [x29, #-0xd0]
10044df84: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044df88: 52800a00    	mov	w0, #0x50               ; =80
10044df8c: 940326e9    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044df90: b5001cc0    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044df94: 52800100    	mov	w0, #0x8                ; =8
10044df98: 52800a01    	mov	w1, #0x50               ; =80
10044df9c: 9402d8f0    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044dfa0: 1400040d    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044dfa4: aa1b03e0    	mov	x0, x27
10044dfa8: 94001bf6    	bl	0x100454f80 <__ZN13quickjs_oxide6engine2vm7numeric9operation11NumericKind10for_opcode17h734c5b14b50f18b1E>
10044dfac: 12001c08    	and	w8, w0, #0xff
10044dfb0: 7100651f    	cmp	w8, #0x19
10044dfb4: 54004441    	b.ne	0x10044e83c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68a8>
10044dfb8: 90000ae1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044dfbc: 91223421    	add	x1, x1, #0x88d
10044dfc0: f94093f3    	ldr	x19, [sp, #0x120]
10044dfc4: 528000a0    	mov	w0, #0x5                ; =5
10044dfc8: 528003e2    	mov	w2, #0x1f               ; =31
10044dfcc: 97f4a12d    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044dfd0: 140000e7    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044dfd4: f0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044dfd8: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044dfdc: 528002b4    	mov	w20, #0x15              ; =21
10044dfe0: 528002a0    	mov	w0, #0x15               ; =21
10044dfe4: 940326d3    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044dfe8: b40077e0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044dfec: b0000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044dff0: 9106c108    	add	x8, x8, #0x1b0
10044dff4: 3dc00100    	ldr	q0, [x8]
10044dff8: 3d800000    	str	q0, [x0]
10044dffc: f840d108    	ldur	x8, [x8, #0xd]
10044e000: f800d008    	stur	x8, [x0, #0xd]
10044e004: 528000a8    	mov	w8, #0x5                ; =5
10044e008: 381783a8    	sturb	w8, [x29, #-0x88]
10044e00c: 528002a8    	mov	w8, #0x15               ; =21
10044e010: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044e014: f81703bf    	stur	xzr, [x29, #-0x90]
10044e018: f81583a8    	stur	x8, [x29, #-0xa8]
10044e01c: f81303bf    	stur	xzr, [x29, #-0xd0]
10044e020: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e024: 52800a00    	mov	w0, #0x50               ; =80
10044e028: 940326c2    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e02c: b50017e0    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044e030: 52800100    	mov	w0, #0x8                ; =8
10044e034: 52800a01    	mov	w1, #0x50               ; =80
10044e038: 9402d8c9    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e03c: 140003e6    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e040: 90000ae9    	adrp	x9, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e044: 911b0529    	add	x9, x9, #0x6c1
10044e048: 528006ca    	mov	w10, #0x36              ; =54
10044e04c: 140000c1    	b	0x10044e350 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63bc>
10044e050: 90000ae9    	adrp	x9, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e054: 911a4129    	add	x9, x9, #0x690
10044e058: 5280062a    	mov	w10, #0x31              ; =49
10044e05c: 140000bd    	b	0x10044e350 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63bc>
10044e060: f94093f3    	ldr	x19, [sp, #0x120]
10044e064: 940310cb    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e068: 140000c1    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e06c: 7102c37f    	cmp	w27, #0xb0
10044e070: 54001b80    	b.eq	0x10044e3e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x644c>
10044e074: 7102c77f    	cmp	w27, #0xb1
10044e078: f9408fe1    	ldr	x1, [sp, #0x118]
10044e07c: f94087e0    	ldr	x0, [sp, #0x108]
10044e080: 54001b61    	b.ne	0x10044e3ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6458>
10044e084: 52800076    	mov	w22, #0x3               ; =3
10044e088: 140000d9    	b	0x10044e3ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6458>
10044e08c: 52800100    	mov	w0, #0x8                ; =8
10044e090: 52800a01    	mov	w1, #0x50               ; =80
10044e094: 9402d8b2    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e098: 140003cf    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e09c: f85183a8    	ldur	x8, [x29, #-0xe8]
10044e0a0: f90006a8    	str	x8, [x21, #0x8]
10044e0a4: 140000b3    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e0a8: f94093f3    	ldr	x19, [sp, #0x120]
10044e0ac: 940310b9    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e0b0: 140000af    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e0b4: f85183b4    	ldur	x20, [x29, #-0xe8]
10044e0b8: f90006b4    	str	x20, [x21, #0x8]
10044e0bc: 140000ad    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e0c0: 7100191f    	cmp	w8, #0x6
10044e0c4: 54004701    	b.ne	0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044e0c8: f85383b4    	ldur	x20, [x29, #-0xc8]
10044e0cc: f9401688    	ldr	x8, [x20, #0x28]
10044e0d0: b4000068    	cbz	x8, 0x10044e0dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6148>
10044e0d4: f9401a80    	ldr	x0, [x20, #0x30]
10044e0d8: 9403267b    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e0dc: f9402280    	ldr	x0, [x20, #0x40]
10044e0e0: b4000040    	cbz	x0, 0x10044e0e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6154>
10044e0e4: 94032678    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e0e8: aa1403e0    	mov	x0, x20
10044e0ec: 94032676    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e0f0: 1400022d    	b	0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044e0f4: 52800348    	mov	w8, #0x1a               ; =26
10044e0f8: 390002a8    	strb	w8, [x21]
10044e0fc: 390012bf    	strb	wzr, [x21, #0x4]
10044e100: f94083e8    	ldr	x8, [sp, #0x100]
10044e104: b9000aa8    	str	w8, [x21, #0x8]
10044e108: 1400009c    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e10c: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044e110: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e114: 528004d4    	mov	w20, #0x26              ; =38
10044e118: 528004c0    	mov	w0, #0x26               ; =38
10044e11c: 94032685    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e120: b4006e20    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044e124: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e128: 91094508    	add	x8, x8, #0x251
10044e12c: ad400500    	ldp	q0, q1, [x8]
10044e130: ad000400    	stp	q0, q1, [x0]
10044e134: f841e108    	ldur	x8, [x8, #0x1e]
10044e138: f801e008    	stur	x8, [x0, #0x1e]
10044e13c: 528000a8    	mov	w8, #0x5                ; =5
10044e140: 381783a8    	sturb	w8, [x29, #-0x88]
10044e144: 528004c8    	mov	w8, #0x26               ; =38
10044e148: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044e14c: f81703bf    	stur	xzr, [x29, #-0x90]
10044e150: f81583a8    	stur	x8, [x29, #-0xa8]
10044e154: f81303bf    	stur	xzr, [x29, #-0xd0]
10044e158: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e15c: 52800a00    	mov	w0, #0x50               ; =80
10044e160: 94032674    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e164: b5000e20    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044e168: 52800100    	mov	w0, #0x8                ; =8
10044e16c: 52800a01    	mov	w1, #0x50               ; =80
10044e170: 9402d87b    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e174: 14000398    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e178: f94093f3    	ldr	x19, [sp, #0x120]
10044e17c: 94031085    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e180: 1400007b    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e184: f94093f3    	ldr	x19, [sp, #0x120]
10044e188: 94031082    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e18c: 14000078    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e190: f90006a8    	str	x8, [x21, #0x8]
10044e194: 14000077    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e198: 9403107e    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e19c: f90006a0    	str	x0, [x21, #0x8]
10044e1a0: 52800928    	mov	w8, #0x49               ; =73
10044e1a4: 390002a8    	strb	w8, [x21]
10044e1a8: 140000e8    	b	0x10044e548 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65b4>
10044e1ac: 7100291f    	cmp	w8, #0xa
10044e1b0: 54001860    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044e1b4: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e1b8: 91240421    	add	x1, x1, #0x901
10044e1bc: f94093f3    	ldr	x19, [sp, #0x120]
10044e1c0: 528000a0    	mov	w0, #0x5                ; =5
10044e1c4: 52800362    	mov	w2, #0x1b               ; =27
10044e1c8: 97f4a0ae    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e1cc: 14000068    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e1d0: 7100291f    	cmp	w8, #0xa
10044e1d4: 54001740    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044e1d8: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e1dc: 91231c21    	add	x1, x1, #0x8c7
10044e1e0: f94093f3    	ldr	x19, [sp, #0x120]
10044e1e4: 528000a0    	mov	w0, #0x5                ; =5
10044e1e8: 52800222    	mov	w2, #0x11               ; =17
10044e1ec: 97f4a0a5    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e1f0: 1400005f    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e1f4: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044e1f8: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e1fc: 52800334    	mov	w20, #0x19              ; =25
10044e200: 52800320    	mov	w0, #0x19               ; =25
10044e204: 9403264b    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e208: b40066e0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044e20c: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e210: 91082908    	add	x8, x8, #0x20a
10044e214: 3dc00100    	ldr	q0, [x8]
10044e218: 3d800000    	str	q0, [x0]
10044e21c: 3cc09100    	ldur	q0, [x8, #0x9]
10044e220: 3c809000    	stur	q0, [x0, #0x9]
10044e224: 528000a8    	mov	w8, #0x5                ; =5
10044e228: 381783a8    	sturb	w8, [x29, #-0x88]
10044e22c: 52800328    	mov	w8, #0x19               ; =25
10044e230: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044e234: f81703bf    	stur	xzr, [x29, #-0x90]
10044e238: f81583a8    	stur	x8, [x29, #-0xa8]
10044e23c: f81303bf    	stur	xzr, [x29, #-0xd0]
10044e240: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e244: 52800a00    	mov	w0, #0x50               ; =80
10044e248: 9403263a    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e24c: b50006e0    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044e250: 52800100    	mov	w0, #0x8                ; =8
10044e254: 52800a01    	mov	w1, #0x50               ; =80
10044e258: 9402d841    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e25c: 1400035e    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e260: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044e264: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e268: 528004d4    	mov	w20, #0x26              ; =38
10044e26c: 528004c0    	mov	w0, #0x26               ; =38
10044e270: 94032630    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e274: b4006380    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044e278: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e27c: 91094508    	add	x8, x8, #0x251
10044e280: ad400500    	ldp	q0, q1, [x8]
10044e284: ad000400    	stp	q0, q1, [x0]
10044e288: f841e108    	ldur	x8, [x8, #0x1e]
10044e28c: f801e008    	stur	x8, [x0, #0x1e]
10044e290: 528000a8    	mov	w8, #0x5                ; =5
10044e294: 381783a8    	sturb	w8, [x29, #-0x88]
10044e298: 528004c8    	mov	w8, #0x26               ; =38
10044e29c: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044e2a0: f81703bf    	stur	xzr, [x29, #-0x90]
10044e2a4: f81583a8    	stur	x8, [x29, #-0xa8]
10044e2a8: f81303bf    	stur	xzr, [x29, #-0xd0]
10044e2ac: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e2b0: 52800a00    	mov	w0, #0x50               ; =80
10044e2b4: 9403261f    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e2b8: b5000380    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044e2bc: 52800100    	mov	w0, #0x8                ; =8
10044e2c0: 52800a01    	mov	w1, #0x50               ; =80
10044e2c4: 9402d826    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e2c8: 14000343    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e2cc: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044e2d0: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e2d4: 52800334    	mov	w20, #0x19              ; =25
10044e2d8: 52800320    	mov	w0, #0x19               ; =25
10044e2dc: 94032615    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e2e0: b4006020    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044e2e4: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e2e8: 91082908    	add	x8, x8, #0x20a
10044e2ec: 3dc00100    	ldr	q0, [x8]
10044e2f0: 3d800000    	str	q0, [x0]
10044e2f4: 3cc09100    	ldur	q0, [x8, #0x9]
10044e2f8: 3c809000    	stur	q0, [x0, #0x9]
10044e2fc: 528000a8    	mov	w8, #0x5                ; =5
10044e300: 381783a8    	sturb	w8, [x29, #-0x88]
10044e304: 52800328    	mov	w8, #0x19               ; =25
10044e308: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044e30c: f81703bf    	stur	xzr, [x29, #-0x90]
10044e310: f81583a8    	stur	x8, [x29, #-0xa8]
10044e314: f81303bf    	stur	xzr, [x29, #-0xd0]
10044e318: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e31c: 52800a00    	mov	w0, #0x50               ; =80
10044e320: 94032604    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e324: b4004820    	cbz	x0, 0x10044ec28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c94>
10044e328: ad7a87a0    	ldp	q0, q1, [x29, #-0xb0]
10044e32c: ad010400    	stp	q0, q1, [x0, #0x20]
10044e330: 3cd703a0    	ldur	q0, [x29, #-0x90]
10044e334: 3d801000    	str	q0, [x0, #0x40]
10044e338: ad7983a1    	ldp	q1, q0, [x29, #-0xd0]
10044e33c: ad000001    	stp	q1, q0, [x0]
10044e340: 1400000b    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e344: 90000ae9    	adrp	x9, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e348: 91198929    	add	x9, x9, #0x662
10044e34c: 528005ca    	mov	w10, #0x2e              ; =46
10044e350: f900151f    	str	xzr, [x8, #0x28]
10044e354: 52800088    	mov	w8, #0x4                ; =4
10044e358: 381303a8    	sturb	w8, [x29, #-0xd0]
10044e35c: a933aba9    	stp	x9, x10, [x29, #-0xc8]
10044e360: d10343a0    	sub	x0, x29, #0xd0
10044e364: f94093f3    	ldr	x19, [sp, #0x120]
10044e368: 97f450fb    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e36c: f90006a0    	str	x0, [x21, #0x8]
10044e370: 52800928    	mov	w8, #0x49               ; =73
10044e374: 390002a8    	strb	w8, [x21]
10044e378: f94093f3    	ldr	x19, [sp, #0x120]
10044e37c: aa1303f4    	mov	x20, x19
10044e380: 17fffe29    	b	0x10044dc24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c90>
10044e384: 7200011f    	tst	w8, #0x1
10044e388: 52800468    	mov	w8, #0x23               ; =35
10044e38c: 1a881508    	cinc	w8, w8, eq
10044e390: 390002a8    	strb	w8, [x21]
10044e394: f94083e8    	ldr	x8, [sp, #0x100]
10044e398: 790006a8    	strh	w8, [x21, #0x2]
10044e39c: 14000119    	b	0x10044e800 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x686c>
10044e3a0: 94030ffc    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e3a4: 140001d1    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044e3a8: 94030ffa    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e3ac: 140001cf    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044e3b0: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e3b4: 91219c21    	add	x1, x1, #0x867
10044e3b8: f94093f3    	ldr	x19, [sp, #0x120]
10044e3bc: 528000a0    	mov	w0, #0x5                ; =5
10044e3c0: 528004c2    	mov	w2, #0x26               ; =38
10044e3c4: 97f4a02f    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e3c8: 17ffffe9    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e3cc: f94093f3    	ldr	x19, [sp, #0x120]
10044e3d0: 94030ff0    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e3d4: 17ffffe6    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e3d8: 52800016    	mov	w22, #0x0               ; =0
10044e3dc: 1400000b    	b	0x10044e408 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6474>
10044e3e0: 52800056    	mov	w22, #0x2               ; =2
10044e3e4: f9408fe1    	ldr	x1, [sp, #0x118]
10044e3e8: f94087e0    	ldr	x0, [sp, #0x108]
10044e3ec: f94093f3    	ldr	x19, [sp, #0x120]
10044e3f0: d2800002    	mov	x2, #0x0                ; =0
10044e3f4: 97ff859a    	bl	0x10042fa5c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
10044e3f8: 36000060    	tbz	w0, #0x0, 0x10044e404 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6470>
10044e3fc: f90006a1    	str	x1, [x21, #0x8]
10044e400: 17ffffdc    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e404: f940abf4    	ldr	x20, [sp, #0x150]
10044e408: 528008e8    	mov	w8, #0x47               ; =71
10044e40c: 390002a8    	strb	w8, [x21]
10044e410: 390006b6    	strb	w22, [x21, #0x1]
10044e414: f94093f3    	ldr	x19, [sp, #0x120]
10044e418: 17fffe03    	b	0x10044dc24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c90>
10044e41c: 390502ff    	strb	wzr, [x23, #0x140]
10044e420: 14000017    	b	0x10044e47c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64e8>
10044e424: d10343a0    	sub	x0, x29, #0xd0
10044e428: a951cfe2    	ldp	x2, x19, [sp, #0x118]
10044e42c: f94087e1    	ldr	x1, [sp, #0x108]
10044e430: 94001621    	bl	0x100453cb4 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
10044e434: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044e438: 7100291f    	cmp	w8, #0xa
10044e43c: 54000400    	b.eq	0x10044e4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6528>
10044e440: b85343a9    	ldur	w9, [x29, #-0xcc]
10044e444: 910963eb    	add	x11, sp, #0x258
10044e448: b80fb169    	stur	w9, [x11, #0xfb]
10044e44c: d10343a9    	sub	x9, x29, #0xd0
10044e450: b8401129    	ldur	w9, [x9, #0x1]
10044e454: b81103a9    	stur	w9, [x29, #-0xf0]
10044e458: f85383a9    	ldur	x9, [x29, #-0xc8]
10044e45c: 390502e8    	strb	w8, [x23, #0x140]
10044e460: 910506e8    	add	x8, x23, #0x141
10044e464: b85103aa    	ldur	w10, [x29, #-0xf0]
10044e468: b900010a    	str	w10, [x8]
10044e46c: b84fb168    	ldur	w8, [x11, #0xfb]
10044e470: b90146e8    	str	w8, [x23, #0x144]
10044e474: f900a6e9    	str	x9, [x23, #0x148]
10044e478: f940abf4    	ldr	x20, [sp, #0x150]
10044e47c: 528008c8    	mov	w8, #0x46               ; =70
10044e480: 390002a8    	strb	w8, [x21]
10044e484: f94093f3    	ldr	x19, [sp, #0x120]
10044e488: 17fffde7    	b	0x10044dc24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c90>
10044e48c: 528007a8    	mov	w8, #0x3d               ; =61
10044e490: 17ffffb9    	b	0x10044e374 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e0>
10044e494: f9408be8    	ldr	x8, [sp, #0x110]
10044e498: f9400108    	ldr	x8, [x8]
10044e49c: 39442902    	ldrb	w2, [x8, #0x10a]
10044e4a0: d10343a0    	sub	x0, x29, #0xd0
10044e4a4: 9104c3e1    	add	x1, sp, #0x130
10044e4a8: f94093f3    	ldr	x19, [sp, #0x120]
10044e4ac: 94000b0f    	bl	0x1004510e8 <__ZN13quickjs_oxide6engine2vm7execute15deferred_action17h2233d5b0a1374355E>
10044e4b0: 385303a8    	ldurb	w8, [x29, #-0xd0]
10044e4b4: 7101291f    	cmp	w8, #0x4a
10044e4b8: 540012c1    	b.ne	0x10044e710 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x677c>
10044e4bc: f85383a8    	ldur	x8, [x29, #-0xc8]
10044e4c0: f90006a8    	str	x8, [x21, #0x8]
10044e4c4: 17ffffab    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e4c8: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e4cc: 9122b021    	add	x1, x1, #0x8ac
10044e4d0: f94093f3    	ldr	x19, [sp, #0x120]
10044e4d4: 528000a0    	mov	w0, #0x5                ; =5
10044e4d8: 52800362    	mov	w2, #0x1b               ; =27
10044e4dc: 97f49fe9    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e4e0: 17ffffa3    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e4e4: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e4e8: 91247021    	add	x1, x1, #0x91c
10044e4ec: f94093f3    	ldr	x19, [sp, #0x120]
10044e4f0: 528000a0    	mov	w0, #0x5                ; =5
10044e4f4: 52800382    	mov	w2, #0x1c               ; =28
10044e4f8: 97f49fe2    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e4fc: 17ffff9c    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e500: 52800228    	mov	w8, #0x11               ; =17
10044e504: 14000096    	b	0x10044e75c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67c8>
10044e508: b4000140    	cbz	x0, 0x10044e530 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x659c>
10044e50c: f9401688    	ldr	x8, [x20, #0x28]
10044e510: b4000068    	cbz	x8, 0x10044e51c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6588>
10044e514: f9401a80    	ldr	x0, [x20, #0x30]
10044e518: 9403256b    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e51c: f9402280    	ldr	x0, [x20, #0x40]
10044e520: b4000040    	cbz	x0, 0x10044e528 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6594>
10044e524: 94032568    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e528: aa1403e0    	mov	x0, x20
10044e52c: 94032566    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e530: 52800033    	mov	w19, #0x1               ; =1
10044e534: 52800408    	mov	w8, #0x20               ; =32
10044e538: 390002a8    	strb	w8, [x21]
10044e53c: f94083e8    	ldr	x8, [sp, #0x100]
10044e540: b90006a8    	str	w8, [x21, #0x4]
10044e544: 390022b3    	strb	w19, [x21, #0x8]
10044e548: b85403a8    	ldur	w8, [x29, #-0xc0]
10044e54c: 7100091f    	cmp	w8, #0x2
10044e550: 54fff140    	b.eq	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e554: f85303a0    	ldur	x0, [x29, #-0xd0]
10044e558: f9400008    	ldr	x8, [x0]
10044e55c: f1000508    	subs	x8, x8, #0x1
10044e560: f9000008    	str	x8, [x0]
10044e564: 54fff0a1    	b.ne	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e568: f94093f3    	ldr	x19, [sp, #0x120]
10044e56c: 97efd9ec    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044e570: 17ffff82    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e574: 52800348    	mov	w8, #0x1a               ; =26
10044e578: 390002a8    	strb	w8, [x21]
10044e57c: 52800088    	mov	w8, #0x4                ; =4
10044e580: 390012a8    	strb	w8, [x21, #0x4]
10044e584: 17ffff7d    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e588: 3dc0bfe0    	ldr	q0, [sp, #0x2f0]
10044e58c: 3c9103a0    	stur	q0, [x29, #-0xf0]
10044e590: f94183e8    	ldr	x8, [sp, #0x300]
10044e594: f81203a8    	stur	x8, [x29, #-0xe0]
10044e598: d10343a0    	sub	x0, x29, #0xd0
10044e59c: d103c3a1    	sub	x1, x29, #0xf0
10044e5a0: f94093f3    	ldr	x19, [sp, #0x120]
10044e5a4: 97f9f509    	bl	0x1002cb9c8 <__ZN49_$LT$T$u20$as$u20$alloc..string..SpecToString$GT$14spec_to_string17h32513371fca93ff9E>
10044e5a8: d10343a1    	sub	x1, x29, #0xd0
10044e5ac: f94093f3    	ldr	x19, [sp, #0x120]
10044e5b0: 528000a0    	mov	w0, #0x5                ; =5
10044e5b4: 97f9f533    	bl	0x1002cba80 <__ZN13quickjs_oxide6engine3api5error5Error3new17h4cec04b22d647e9eE>
10044e5b8: 17ffff6d    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e5bc: 52800708    	mov	w8, #0x38               ; =56
10044e5c0: 14000067    	b	0x10044e75c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67c8>
10044e5c4: 721f195f    	tst	w10, #0xfe
10044e5c8: 1a9f17e8    	cset	w8, eq
10044e5cc: 71009b7f    	cmp	w27, #0x26
10044e5d0: 52800589    	mov	w9, #0x2c               ; =44
10044e5d4: 5280048a    	mov	w10, #0x24              ; =36
10044e5d8: 390002aa    	strb	w10, [x21]
10044e5dc: 7a491364    	ccmp	w27, w9, #0x4, ne
10044e5e0: f94083e9    	ldr	x9, [sp, #0x100]
10044e5e4: 790006a9    	strh	w9, [x21, #0x2]
10044e5e8: 52802029    	mov	w9, #0x101              ; =257
10044e5ec: 79000aa9    	strh	w9, [x21, #0x4]
10044e5f0: 1a9f17e9    	cset	w9, eq
10044e5f4: 39001aa8    	strb	w8, [x21, #0x6]
10044e5f8: 39001ea9    	strb	w9, [x21, #0x7]
10044e5fc: 17ffff5f    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e600: 52800468    	mov	w8, #0x23               ; =35
10044e604: 14000056    	b	0x10044e75c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67c8>
10044e608: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e60c: 91231c21    	add	x1, x1, #0x8c7
10044e610: f94093f3    	ldr	x19, [sp, #0x120]
10044e614: 528000a0    	mov	w0, #0x5                ; =5
10044e618: 52800222    	mov	w2, #0x11               ; =17
10044e61c: 97f49f99    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e620: 17ffff53    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e624: 52800488    	mov	w8, #0x24               ; =36
10044e628: 390002a8    	strb	w8, [x21]
10044e62c: f94083e8    	ldr	x8, [sp, #0x100]
10044e630: 790006a8    	strh	w8, [x21, #0x2]
10044e634: d0000d28    	adrp	x8, 0x1005f4000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x81e90>
10044e638: fd47bd00    	ldr	d0, [x8, #0xf78]
10044e63c: bd0006a0    	str	s0, [x21, #0x4]
10044e640: 17ffff4e    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e644: 910bc3e0    	add	x0, sp, #0x2f0
10044e648: 1400000f    	b	0x10044e684 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66f0>
10044e64c: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e650: 91236021    	add	x1, x1, #0x8d8
10044e654: f94093f3    	ldr	x19, [sp, #0x120]
10044e658: 528000a0    	mov	w0, #0x5                ; =5
10044e65c: 52800522    	mov	w2, #0x29               ; =41
10044e660: 97f49f88    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e664: 17ffff42    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e668: f94093f3    	ldr	x19, [sp, #0x120]
10044e66c: 94030f49    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e670: 17ffff3f    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e674: f94093f3    	ldr	x19, [sp, #0x120]
10044e678: 94030f46    	bl	0x100512390 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044e67c: 17ffff3c    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e680: d103c3a0    	sub	x0, x29, #0xf0
10044e684: 97f45034    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e688: 14000118    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044e68c: 121e7a88    	and	w8, w20, #0xfffffffd
10044e690: 7102311f    	cmp	w8, #0x8c
10044e694: 54000bc1    	b.ne	0x10044e80c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6878>
10044e698: 71023a9f    	cmp	w20, #0x8e
10044e69c: 17fffd8e    	b	0x10044dcd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d40>
10044e6a0: f90006bb    	str	x27, [x21, #0x8]
10044e6a4: 17ffff33    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e6a8: 910823e0    	add	x0, sp, #0x208
10044e6ac: f94093f3    	ldr	x19, [sp, #0x120]
10044e6b0: 97f45029    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e6b4: 17ffff2e    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e6b8: 52800488    	mov	w8, #0x24               ; =36
10044e6bc: 390002a8    	strb	w8, [x21]
10044e6c0: 790006b4    	strh	w20, [x21, #0x2]
10044e6c4: 17fffdc1    	b	0x10044ddc8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e34>
10044e6c8: 910a03e0    	add	x0, sp, #0x280
10044e6cc: 14000002    	b	0x10044e6d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6740>
10044e6d0: 910a83e0    	add	x0, sp, #0x2a0
10044e6d4: f94093f3    	ldr	x19, [sp, #0x120]
10044e6d8: 97f4501f    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e6dc: 17ffff24    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e6e0: b5001648    	cbnz	x8, 0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
10044e6e4: 910c43e0    	add	x0, sp, #0x310
10044e6e8: 97ef8c18    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044e6ec: 140000af    	b	0x10044e9a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a14>
10044e6f0: 9108e3e0    	add	x0, sp, #0x238
10044e6f4: f94093f3    	ldr	x19, [sp, #0x120]
10044e6f8: 97f45017    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e6fc: 17ffff1c    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e700: b5001528    	cbnz	x8, 0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044e704: 910c43e0    	add	x0, sp, #0x310
10044e708: 97ef8c10    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044e70c: 140000a6    	b	0x10044e9a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a10>
10044e710: d10343a9    	sub	x9, x29, #0xd0
10044e714: b8401129    	ldur	w9, [x9, #0x1]
10044e718: b9025be9    	str	w9, [sp, #0x258]
10044e71c: b85343a9    	ldur	w9, [x29, #-0xcc]
10044e720: 910963ea    	add	x10, sp, #0x258
10044e724: b8003149    	stur	w9, [x10, #0x3]
10044e728: 7101251f    	cmp	w8, #0x49
10044e72c: 54000c61    	b.ne	0x10044e8b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6924>
10044e730: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e734: 91270421    	add	x1, x1, #0x9c1
10044e738: f94093f3    	ldr	x19, [sp, #0x120]
10044e73c: 528000a0    	mov	w0, #0x5                ; =5
10044e740: 52800522    	mov	w2, #0x29               ; =41
10044e744: 97f49f4f    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e748: 17ffff09    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e74c: f85383b6    	ldur	x22, [x29, #-0xc8]
10044e750: f90006b6    	str	x22, [x21, #0x8]
10044e754: 17ffff07    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044e758: 52800728    	mov	w8, #0x39               ; =57
10044e75c: 390002a8    	strb	w8, [x21]
10044e760: f94083e8    	ldr	x8, [sp, #0x100]
10044e764: 790006a8    	strh	w8, [x21, #0x2]
10044e768: 17ffff04    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e76c: d103c3a0    	sub	x0, x29, #0xf0
10044e770: 97f44ff9    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e774: 140000dd    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044e778: 52800488    	mov	w8, #0x24               ; =36
10044e77c: 390002a8    	strb	w8, [x21]
10044e780: 7100bf7f    	cmp	w27, #0x2f
10044e784: 1a9f17e8    	cset	w8, eq
10044e788: f94083e9    	ldr	x9, [sp, #0x100]
10044e78c: 790006a9    	strh	w9, [x21, #0x2]
10044e790: 52802049    	mov	w9, #0x102              ; =258
10044e794: 79000aa9    	strh	w9, [x21, #0x4]
10044e798: 39001abf    	strb	wzr, [x21, #0x6]
10044e79c: 39001ea8    	strb	w8, [x21, #0x7]
10044e7a0: 17fffef6    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e7a4: 90000ae1    	adrp	x1, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e7a8: 91071421    	add	x1, x1, #0x1c5
10044e7ac: 52800442    	mov	w2, #0x22               ; =34
10044e7b0: 14000007    	b	0x10044e7cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6838>
10044e7b4: d103c3a0    	sub	x0, x29, #0xf0
10044e7b8: 97f44fe7    	bl	0x100162754 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044e7bc: 17fffe78    	b	0x10044e19c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6208>
10044e7c0: 90000ae1    	adrp	x1, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044e7c4: 9106c021    	add	x1, x1, #0x1b0
10044e7c8: 528002a2    	mov	w2, #0x15               ; =21
10044e7cc: f94093f3    	ldr	x19, [sp, #0x120]
10044e7d0: 528000a0    	mov	w0, #0x5                ; =5
10044e7d4: 97f49f2b    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e7d8: 17fffee5    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e7dc: 52800488    	mov	w8, #0x24               ; =36
10044e7e0: 390002a8    	strb	w8, [x21]
10044e7e4: 790006b3    	strh	w19, [x21, #0x2]
10044e7e8: 17fffd78    	b	0x10044ddc8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e34>
10044e7ec: 7200011f    	tst	w8, #0x1
10044e7f0: 52800468    	mov	w8, #0x23               ; =35
10044e7f4: 1a881508    	cinc	w8, w8, eq
10044e7f8: 390002a8    	strb	w8, [x21]
10044e7fc: 790006b3    	strh	w19, [x21, #0x2]
10044e800: 0f008420    	movi.4h	v0, #0x1
10044e804: bd0006a0    	str	s0, [x21, #0x4]
10044e808: 17fffedc    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e80c: aa1403e0    	mov	x0, x20
10044e810: 940019dc    	bl	0x100454f80 <__ZN13quickjs_oxide6engine2vm7numeric9operation11NumericKind10for_opcode17h734c5b14b50f18b1E>
10044e814: 12001c08    	and	w8, w0, #0xff
10044e818: 7100651f    	cmp	w8, #0x19
10044e81c: 54000101    	b.ne	0x10044e83c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68a8>
10044e820: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e824: 9124e021    	add	x1, x1, #0x938
10044e828: f94093f3    	ldr	x19, [sp, #0x120]
10044e82c: 528000a0    	mov	w0, #0x5                ; =5
10044e830: 52800362    	mov	w2, #0x1b               ; =27
10044e834: 97f49f13    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044e838: 17fffecd    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e83c: 52800869    	mov	w9, #0x43               ; =67
10044e840: 390002a9    	strb	w9, [x21]
10044e844: 390006a8    	strb	w8, [x21, #0x1]
10044e848: 17fffecc    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e84c: 52800100    	mov	w0, #0x8                ; =8
10044e850: 52800a01    	mov	w1, #0x50               ; =80
10044e854: 9402d6c2    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e858: 140001df    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e85c: 71025b7f    	cmp	w27, #0x96
10044e860: 52800348    	mov	w8, #0x1a               ; =26
10044e864: 390002a8    	strb	w8, [x21]
10044e868: 52800208    	mov	w8, #0x10               ; =16
10044e86c: 390012a8    	strb	w8, [x21, #0x4]
10044e870: 1a9f17e8    	cset	w8, eq
10044e874: 390016a8    	strb	w8, [x21, #0x5]
10044e878: f94083e8    	ldr	x8, [sp, #0x100]
10044e87c: b9000aa8    	str	w8, [x21, #0x8]
10044e880: 17fffebe    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e884: f94093f3    	ldr	x19, [sp, #0x120]
10044e888: 94030ef2    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044e88c: 17fffeb8    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e890: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044e894: 912ee042    	add	x2, x2, #0xbb8
10044e898: 1400004d    	b	0x10044e9cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a38>
10044e89c: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044e8a0: 912ee042    	add	x2, x2, #0xbb8
10044e8a4: f94093f3    	ldr	x19, [sp, #0x120]
10044e8a8: aa0103e0    	mov	x0, x1
10044e8ac: aa1c03e1    	mov	x1, x28
10044e8b0: 9402d762    	bl	0x100504638 <__ZN4core5slice5index24slice_end_index_len_fail17h658aaf9fdfc67c91E>
10044e8b4: 140001c8    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e8b8: f85383a9    	ldur	x9, [x29, #-0xc8]
10044e8bc: b9425bea    	ldr	w10, [sp, #0x258]
10044e8c0: b80012aa    	stur	w10, [x21, #0x1]
10044e8c4: 910963ea    	add	x10, sp, #0x258
10044e8c8: b840314a    	ldur	w10, [x10, #0x3]
10044e8cc: b90006aa    	str	w10, [x21, #0x4]
10044e8d0: 390002a8    	strb	w8, [x21]
10044e8d4: f90006a9    	str	x9, [x21, #0x8]
10044e8d8: 17fffea8    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e8dc: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044e8e0: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e8e4: 52800474    	mov	w20, #0x23              ; =35
10044e8e8: 52800460    	mov	w0, #0x23               ; =35
10044e8ec: 94032491    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e8f0: b4002fa0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044e8f4: 528d2e68    	mov	w8, #0x6973             ; =26995
10044e8f8: 72acedc8    	movk	w8, #0x676e, lsl #16
10044e8fc: b801f008    	stur	w8, [x0, #0x1f]
10044e900: f0000ac8    	adrp	x8, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044e904: 9125ed08    	add	x8, x8, #0x97b
10044e908: ad400500    	ldp	q0, q1, [x8]
10044e90c: ad000400    	stp	q0, q1, [x0]
10044e910: 528000a8    	mov	w8, #0x5                ; =5
10044e914: 381783a8    	sturb	w8, [x29, #-0x88]
10044e918: 52800468    	mov	w8, #0x23               ; =35
10044e91c: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044e920: f81703bf    	stur	xzr, [x29, #-0x90]
10044e924: f81583a8    	stur	x8, [x29, #-0xa8]
10044e928: f81303bf    	stur	xzr, [x29, #-0xd0]
10044e92c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044e930: 52800a00    	mov	w0, #0x50               ; =80
10044e934: 9403247f    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044e938: b5ff9aa0    	cbnz	x0, 0x10044dc8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5cf8>
10044e93c: 52800100    	mov	w0, #0x8                ; =8
10044e940: 52800a01    	mov	w1, #0x50               ; =80
10044e944: 9402d686    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044e948: 140001a3    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e94c: aa1403e1    	mov	x1, x20
10044e950: 97f1c661    	bl	0x1000c02d4 <__ZN4core3ptr132drop_in_place$LT$core..result..Result$LT$core..convert..Infallible$C$quickjs_oxide..engine..api..runtime_error..RuntimeError$GT$$GT$17h392d3b5fe08b5889E>
10044e954: 14000010    	b	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044e958: f94093f3    	ldr	x19, [sp, #0x120]
10044e95c: 94030ebd    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044e960: 14000062    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044e964: 7100191f    	cmp	w8, #0x6
10044e968: 54000161    	b.ne	0x10044e994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a00>
10044e96c: f85383b4    	ldur	x20, [x29, #-0xc8]
10044e970: f9401688    	ldr	x8, [x20, #0x28]
10044e974: b4000068    	cbz	x8, 0x10044e980 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x69ec>
10044e978: f9401a80    	ldr	x0, [x20, #0x30]
10044e97c: 94032452    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e980: f9402280    	ldr	x0, [x20, #0x40]
10044e984: b4000040    	cbz	x0, 0x10044e98c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x69f8>
10044e988: 9403244f    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e98c: aa1403e0    	mov	x0, x20
10044e990: 9403244d    	bl	0x100517ac4 <dyld_stub_binder+0x100517ac4>
10044e994: f94083f3    	ldr	x19, [sp, #0x100]
10044e998: f9401668    	ldr	x8, [x19, #0x28]
10044e99c: 91000508    	add	x8, x8, #0x1
10044e9a0: f9001668    	str	x8, [x19, #0x28]
10044e9a4: 52800013    	mov	w19, #0x0               ; =0
10044e9a8: 71032f7f    	cmp	w27, #0xcb
10044e9ac: 52800428    	mov	w8, #0x21               ; =33
10044e9b0: 390002a8    	strb	w8, [x21]
10044e9b4: 1a9f07e8    	cset	w8, ne
10044e9b8: 390006a8    	strb	w8, [x21, #0x1]
10044e9bc: 39000ab3    	strb	w19, [x21, #0x2]
10044e9c0: 17fffe6e    	b	0x10044e378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e4>
10044e9c4: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044e9c8: 91254042    	add	x2, x2, #0x950
10044e9cc: f94093f3    	ldr	x19, [sp, #0x120]
10044e9d0: aa1603e0    	mov	x0, x22
10044e9d4: 9402d78f    	bl	0x100504810 <__ZN4core5slice5index22slice_index_order_fail17h1d9efe3670e787d1E>
10044e9d8: 1400017f    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044e9dc: 52800668    	mov	w8, #0x33               ; =51
10044e9e0: 17fffe65    	b	0x10044e374 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63e0>
10044e9e4: f94093f3    	ldr	x19, [sp, #0x120]
10044e9e8: 94030e9a    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044e9ec: 17fffe60    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044e9f0: 390002a8    	strb	w8, [x21]
10044e9f4: 790006b4    	strh	w20, [x21, #0x2]
10044e9f8: 390012aa    	strb	w10, [x21, #0x4]
10044e9fc: 390016bf    	strb	wzr, [x21, #0x5]
10044ea00: 39001aa9    	strb	w9, [x21, #0x6]
10044ea04: f94093f3    	ldr	x19, [sp, #0x120]
10044ea08: aa1303f4    	mov	x20, x19
10044ea0c: 39001ebf    	strb	wzr, [x21, #0x7]
10044ea10: 17fffc85    	b	0x10044dc24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c90>
10044ea14: f94093f3    	ldr	x19, [sp, #0x120]
10044ea18: 94030e8e    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea1c: 17fffe54    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ea20: f94093f3    	ldr	x19, [sp, #0x120]
10044ea24: 94030e8b    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea28: 17fffe51    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ea2c: f94093f3    	ldr	x19, [sp, #0x120]
10044ea30: 94030e88    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea34: 17fffe4e    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ea38: 94030e86    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea3c: 17fffdd8    	b	0x10044e19c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6208>
10044ea40: 90001222    	adrp	x2, 0x100692000 <dyld_stub_binder+0x100692000>
10044ea44: 9136e042    	add	x2, x2, #0xdb8
10044ea48: 52800481    	mov	w1, #0x24               ; =36
10044ea4c: b0000ac0    	adrp	x0, 0x1005a7000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x34e90>
10044ea50: 910d0c00    	add	x0, x0, #0x343
10044ea54: f94093f3    	ldr	x19, [sp, #0x120]
10044ea58: 9402d7c5    	bl	0x10050496c <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044ea5c: 1400015e    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ea60: f94093f3    	ldr	x19, [sp, #0x120]
10044ea64: 94030e7b    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea68: 14000020    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044ea6c: d0001220    	adrp	x0, 0x100694000 <dyld_stub_binder+0x100694000>
10044ea70: 91058000    	add	x0, x0, #0x160
10044ea74: f94093f3    	ldr	x19, [sp, #0x120]
10044ea78: 9402d784    	bl	0x100504888 <__ZN4core4cell22panic_already_borrowed17ha05dfdc27881e579E>
10044ea7c: 14000156    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ea80: 94030e74    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea84: 14000019    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044ea88: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044ea8c: 91254042    	add	x2, x2, #0x950
10044ea90: aa0903fc    	mov	x28, x9
10044ea94: 17ffff84    	b	0x10044e8a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6910>
10044ea98: 94030e6e    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ea9c: 14000013    	b	0x10044eae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b54>
10044eaa0: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eaa4: 91254042    	add	x2, x2, #0x950
10044eaa8: aa0b03fc    	mov	x28, x11
10044eaac: 17ffff7e    	b	0x10044e8a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6910>
10044eab0: f94093f3    	ldr	x19, [sp, #0x120]
10044eab4: 94030e67    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044eab8: 17fffe2d    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044eabc: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eac0: 91254042    	add	x2, x2, #0x950
10044eac4: aa0a03fc    	mov	x28, x10
10044eac8: 17ffff77    	b	0x10044e8a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6910>
10044eacc: f94093f3    	ldr	x19, [sp, #0x120]
10044ead0: 94030e60    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044ead4: 17fffe26    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ead8: f94093f3    	ldr	x19, [sp, #0x120]
10044eadc: 94030e5d    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044eae0: 17fffe23    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044eae4: 94030e5b    	bl	0x100512450 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044eae8: aa0003f6    	mov	x22, x0
10044eaec: f90006b6    	str	x22, [x21, #0x8]
10044eaf0: 17fffe20    	b	0x10044e370 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63dc>
10044eaf4: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eaf8: 91312042    	add	x2, x2, #0xc48
10044eafc: 17ffffb4    	b	0x10044e9cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a38>
10044eb00: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eb04: 91312042    	add	x2, x2, #0xc48
10044eb08: 17ffff67    	b	0x10044e8a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6910>
10044eb0c: d00011e8    	adrp	x8, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044eb10: 91398108    	add	x8, x8, #0xe60
10044eb14: f81303a8    	stur	x8, [x29, #-0xd0]
10044eb18: d00011e1    	adrp	x1, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044eb1c: 9139c021    	add	x1, x1, #0xe70
10044eb20: 14000020    	b	0x10044eba0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c0c>
10044eb24: d0001200    	adrp	x0, 0x100690000 <dyld_stub_binder+0x100690000>
10044eb28: 91268000    	add	x0, x0, #0x9a0
10044eb2c: f94093f3    	ldr	x19, [sp, #0x120]
10044eb30: 9402d767    	bl	0x1005048cc <__ZN4core4cell30panic_already_mutably_borrowed17h8ed64df0236bfa1cE>
10044eb34: 14000128    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044eb38: d00011e8    	adrp	x8, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044eb3c: 91398108    	add	x8, x8, #0xe60
10044eb40: f81303a8    	stur	x8, [x29, #-0xd0]
10044eb44: d00011e1    	adrp	x1, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044eb48: 9139c021    	add	x1, x1, #0xe70
10044eb4c: 14000021    	b	0x10044ebd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6c3c>
10044eb50: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eb54: 91254042    	add	x2, x2, #0x950
10044eb58: aa0803fc    	mov	x28, x8
10044eb5c: 17ffff52    	b	0x10044e8a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6910>
10044eb60: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eb64: 91254042    	add	x2, x2, #0x950
10044eb68: aa0b03f6    	mov	x22, x11
10044eb6c: 17ffff98    	b	0x10044e9cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a38>
10044eb70: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eb74: 91254042    	add	x2, x2, #0x950
10044eb78: aa0903f6    	mov	x22, x9
10044eb7c: 17ffff94    	b	0x10044e9cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6a38>
10044eb80: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eb84: 91254042    	add	x2, x2, #0x950
10044eb88: 17ffff47    	b	0x10044e8a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6910>
10044eb8c: d0001208    	adrp	x8, 0x100690000 <dyld_stub_binder+0x100690000>
10044eb90: 912ce108    	add	x8, x8, #0xb38
10044eb94: f81303a8    	stur	x8, [x29, #-0xd0]
10044eb98: d0001201    	adrp	x1, 0x100690000 <dyld_stub_binder+0x100690000>
10044eb9c: 912d2021    	add	x1, x1, #0xb48
10044eba0: d10343a0    	sub	x0, x29, #0xd0
10044eba4: 52800028    	mov	w8, #0x1                ; =1
10044eba8: 910b43e9    	add	x9, sp, #0x2d0
10044ebac: a900a408    	stp	x8, x9, [x0, #0x8]
10044ebb0: a901fc1f    	stp	xzr, xzr, [x0, #0x18]
10044ebb4: 9402d682    	bl	0x1005045bc <__ZN4core9panicking9panic_fmt17heec96bfc27e6c546E>
10044ebb8: 14000107    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ebbc: d0001208    	adrp	x8, 0x100690000 <dyld_stub_binder+0x100690000>
10044ebc0: 912ce108    	add	x8, x8, #0xb38
10044ebc4: f81303a8    	stur	x8, [x29, #-0xd0]
10044ebc8: d0001201    	adrp	x1, 0x100690000 <dyld_stub_binder+0x100690000>
10044ebcc: 912d2021    	add	x1, x1, #0xb48
10044ebd0: d10343a0    	sub	x0, x29, #0xd0
10044ebd4: 52800028    	mov	w8, #0x1                ; =1
10044ebd8: 910b43e9    	add	x9, sp, #0x2d0
10044ebdc: a900a408    	stp	x8, x9, [x0, #0x8]
10044ebe0: a901fc1f    	stp	xzr, xzr, [x0, #0x18]
10044ebe4: 9402d676    	bl	0x1005045bc <__ZN4core9panicking9panic_fmt17heec96bfc27e6c546E>
10044ebe8: 140000fb    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ebec: f0000ac1    	adrp	x1, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044ebf0: 91267821    	add	x1, x1, #0x99e
10044ebf4: f94093f3    	ldr	x19, [sp, #0x120]
10044ebf8: 528000a0    	mov	w0, #0x5                ; =5
10044ebfc: 52800462    	mov	w2, #0x23               ; =35
10044ec00: 97f49e20    	bl	0x100176480 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044ec04: 17fffdda    	b	0x10044e36c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044ec08: 52800020    	mov	w0, #0x1                ; =1
10044ec0c: 52800621    	mov	w1, #0x31               ; =49
10044ec10: 9402d5d3    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ec14: 140000f0    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ec18: 52800100    	mov	w0, #0x8                ; =8
10044ec1c: 52800a01    	mov	w1, #0x50               ; =80
10044ec20: 9402d5cf    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ec24: 140000ec    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ec28: 52800100    	mov	w0, #0x8                ; =8
10044ec2c: 52800a01    	mov	w1, #0x50               ; =80
10044ec30: 9402d5cb    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ec34: 140000e8    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ec38: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ec3c: 912e8108    	add	x8, x8, #0xba0
10044ec40: 140000cc    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ec44: f94093f3    	ldr	x19, [sp, #0x120]
10044ec48: 52800020    	mov	w0, #0x1                ; =1
10044ec4c: 52800461    	mov	w1, #0x23               ; =35
10044ec50: 9402d5c3    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ec54: 140000e0    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ec58: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ec5c: 912f4108    	add	x8, x8, #0xbd0
10044ec60: 140000c4    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ec64: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ec68: 912fa108    	add	x8, x8, #0xbe8
10044ec6c: 140000c1    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ec70: 900011c8    	adrp	x8, 0x100686000 <dyld_stub_binder+0x100686000>
10044ec74: 91358108    	add	x8, x8, #0xd60
10044ec78: 140000be    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ec7c: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044ec80: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ec84: 528004d4    	mov	w20, #0x26              ; =38
10044ec88: 528004c0    	mov	w0, #0x26               ; =38
10044ec8c: 940323a9    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044ec90: b40012a0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044ec94: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044ec98: 91094508    	add	x8, x8, #0x251
10044ec9c: ad400500    	ldp	q0, q1, [x8]
10044eca0: ad000400    	stp	q0, q1, [x0]
10044eca4: f841e108    	ldur	x8, [x8, #0x1e]
10044eca8: f801e008    	stur	x8, [x0, #0x1e]
10044ecac: 528000a8    	mov	w8, #0x5                ; =5
10044ecb0: 381783a8    	sturb	w8, [x29, #-0x88]
10044ecb4: 528004c8    	mov	w8, #0x26               ; =38
10044ecb8: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044ecbc: f81703bf    	stur	xzr, [x29, #-0x90]
10044ecc0: f81583a8    	stur	x8, [x29, #-0xa8]
10044ecc4: f81303bf    	stur	xzr, [x29, #-0xd0]
10044ecc8: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044eccc: 52800a00    	mov	w0, #0x50               ; =80
10044ecd0: 94032398    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044ecd4: b5ffb2a0    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044ecd8: 52800100    	mov	w0, #0x8                ; =8
10044ecdc: 52800a01    	mov	w1, #0x50               ; =80
10044ece0: 9402d59f    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ece4: 140000bc    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ece8: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044ecec: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ecf0: 52800454    	mov	w20, #0x22              ; =34
10044ecf4: 52800440    	mov	w0, #0x22               ; =34
10044ecf8: 9403238e    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044ecfc: b4000f40    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044ed00: 528e6c88    	mov	w8, #0x7364             ; =29540
10044ed04: 79004008    	strh	w8, [x0, #0x20]
10044ed08: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044ed0c: 91071508    	add	x8, x8, #0x1c5
10044ed10: ad400500    	ldp	q0, q1, [x8]
10044ed14: ad000400    	stp	q0, q1, [x0]
10044ed18: 528000a8    	mov	w8, #0x5                ; =5
10044ed1c: 381783a8    	sturb	w8, [x29, #-0x88]
10044ed20: 52800448    	mov	w8, #0x22               ; =34
10044ed24: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044ed28: f81703bf    	stur	xzr, [x29, #-0x90]
10044ed2c: f81583a8    	stur	x8, [x29, #-0xa8]
10044ed30: f81303bf    	stur	xzr, [x29, #-0xd0]
10044ed34: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ed38: 52800a00    	mov	w0, #0x50               ; =80
10044ed3c: 9403237d    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044ed40: b5ffaf40    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044ed44: 52800100    	mov	w0, #0x8                ; =8
10044ed48: 52800a01    	mov	w1, #0x50               ; =80
10044ed4c: 9402d584    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ed50: 140000a1    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ed54: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044ed58: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ed5c: 528002b4    	mov	w20, #0x15              ; =21
10044ed60: 528002a0    	mov	w0, #0x15               ; =21
10044ed64: 94032373    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044ed68: b4000be0    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044ed6c: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044ed70: 9106c108    	add	x8, x8, #0x1b0
10044ed74: 3dc00100    	ldr	q0, [x8]
10044ed78: 3d800000    	str	q0, [x0]
10044ed7c: f840d108    	ldur	x8, [x8, #0xd]
10044ed80: f800d008    	stur	x8, [x0, #0xd]
10044ed84: 528000a8    	mov	w8, #0x5                ; =5
10044ed88: 381783a8    	sturb	w8, [x29, #-0x88]
10044ed8c: 528002a8    	mov	w8, #0x15               ; =21
10044ed90: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044ed94: f81703bf    	stur	xzr, [x29, #-0x90]
10044ed98: f81583a8    	stur	x8, [x29, #-0xa8]
10044ed9c: f81303bf    	stur	xzr, [x29, #-0xd0]
10044eda0: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044eda4: 52800a00    	mov	w0, #0x50               ; =80
10044eda8: 94032362    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044edac: b5ffabe0    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044edb0: 52800100    	mov	w0, #0x8                ; =8
10044edb4: 52800a01    	mov	w1, #0x50               ; =80
10044edb8: 9402d569    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044edbc: 14000086    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044edc0: d0001393    	adrp	x19, 0x1006c0000 <dyld_stub_binder+0x1006c0000>
10044edc4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044edc8: 52800334    	mov	w20, #0x19              ; =25
10044edcc: 52800320    	mov	w0, #0x19               ; =25
10044edd0: 94032358    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044edd4: b4000880    	cbz	x0, 0x10044eee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f50>
10044edd8: 90000ae8    	adrp	x8, 0x1005aa000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37e90>
10044eddc: 91082908    	add	x8, x8, #0x20a
10044ede0: 3dc00100    	ldr	q0, [x8]
10044ede4: 3d800000    	str	q0, [x0]
10044ede8: 3cc09100    	ldur	q0, [x8, #0x9]
10044edec: 3c809000    	stur	q0, [x0, #0x9]
10044edf0: 528000a8    	mov	w8, #0x5                ; =5
10044edf4: 381783a8    	sturb	w8, [x29, #-0x88]
10044edf8: 52800328    	mov	w8, #0x19               ; =25
10044edfc: a93623a0    	stp	x0, x8, [x29, #-0xa0]
10044ee00: f81703bf    	stur	xzr, [x29, #-0x90]
10044ee04: f81583a8    	stur	x8, [x29, #-0xa8]
10044ee08: f81303bf    	stur	xzr, [x29, #-0xd0]
10044ee0c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ee10: 52800a00    	mov	w0, #0x50               ; =80
10044ee14: 94032347    	bl	0x100517b30 <dyld_stub_binder+0x100517b30>
10044ee18: b5ffa880    	cbnz	x0, 0x10044e328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6394>
10044ee1c: 52800100    	mov	w0, #0x8                ; =8
10044ee20: 52800a01    	mov	w1, #0x50               ; =80
10044ee24: 9402d54e    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ee28: 1400006b    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ee2c: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ee30: 9135c108    	add	x8, x8, #0xd70
10044ee34: 1400004f    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ee38: b0001220    	adrp	x0, 0x100693000 <dyld_stub_binder+0x100693000>
10044ee3c: 911d6000    	add	x0, x0, #0x758
10044ee40: f94093f3    	ldr	x19, [sp, #0x120]
10044ee44: 9402d676    	bl	0x10050481c <__ZN4core6option13unwrap_failed17h292f3acdd436a6d6E>
10044ee48: 14000063    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ee4c: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ee50: 91272108    	add	x8, x8, #0x9c8
10044ee54: 14000047    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ee58: 900011c9    	adrp	x9, 0x100686000 <dyld_stub_binder+0x100686000>
10044ee5c: 91358129    	add	x9, x9, #0xd60
10044ee60: f90027e9    	str	x9, [sp, #0x48]
10044ee64: aa0803f6    	mov	x22, x8
10044ee68: 14000043    	b	0x10044ef74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fe0>
10044ee6c: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ee70: 91386108    	add	x8, x8, #0xe18
10044ee74: 1400003f    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044ee78: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044ee7c: 911ca042    	add	x2, x2, #0x728
10044ee80: 14000003    	b	0x10044ee8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ef8>
10044ee84: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044ee88: 911d0042    	add	x2, x2, #0x740
10044ee8c: 52800501    	mov	w1, #0x28               ; =40
10044ee90: f0000ac0    	adrp	x0, 0x1005a9000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36e90>
10044ee94: 91254c00    	add	x0, x0, #0x953
10044ee98: 17fffeef    	b	0x10044ea54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ac0>
10044ee9c: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eea0: 912e8042    	add	x2, x2, #0xba0
10044eea4: aa0803f4    	mov	x20, x8
10044eea8: 14000016    	b	0x10044ef00 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f6c>
10044eeac: d00011e2    	adrp	x2, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044eeb0: 913a8042    	add	x2, x2, #0xea0
10044eeb4: aa1403e0    	mov	x0, x20
10044eeb8: f9400fe1    	ldr	x1, [sp, #0x18]
10044eebc: 9402d5ac    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044eec0: 14000045    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044eec4: d00011e2    	adrp	x2, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044eec8: 913a8042    	add	x2, x2, #0xea0
10044eecc: aa1603e1    	mov	x1, x22
10044eed0: 9402d5a7    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044eed4: 14000040    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044eed8: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044eedc: 911c4108    	add	x8, x8, #0x710
10044eee0: 14000024    	b	0x10044ef70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fdc>
10044eee4: f94093f3    	ldr	x19, [sp, #0x120]
10044eee8: 52800020    	mov	w0, #0x1                ; =1
10044eeec: aa1403e1    	mov	x1, x20
10044eef0: 9402d51b    	bl	0x10050435c <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044eef4: 14000038    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044eef8: b0001222    	adrp	x2, 0x100693000 <dyld_stub_binder+0x100693000>
10044eefc: 91272042    	add	x2, x2, #0x9c8
10044ef00: aa1403e0    	mov	x0, x20
10044ef04: aa1903e1    	mov	x1, x25
10044ef08: 9402d599    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044ef0c: 14000032    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ef10: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef14: 9132e108    	add	x8, x8, #0xcb8
10044ef18: f81303a8    	stur	x8, [x29, #-0xd0]
10044ef1c: b0001221    	adrp	x1, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef20: 91332021    	add	x1, x1, #0xcc8
10044ef24: 14000006    	b	0x10044ef3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fa8>
10044ef28: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef2c: 91324108    	add	x8, x8, #0xc90
10044ef30: f81303a8    	stur	x8, [x29, #-0xd0]
10044ef34: b0001221    	adrp	x1, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef38: 91328021    	add	x1, x1, #0xca0
10044ef3c: d10343a0    	sub	x0, x29, #0xd0
10044ef40: 52800028    	mov	w8, #0x1                ; =1
10044ef44: 910b43e9    	add	x9, sp, #0x2d0
10044ef48: a900a408    	stp	x8, x9, [x0, #0x8]
10044ef4c: a901fc1f    	stp	xzr, xzr, [x0, #0x18]
10044ef50: f94093f3    	ldr	x19, [sp, #0x120]
10044ef54: 9402d59a    	bl	0x1005045bc <__ZN4core9panicking9panic_fmt17heec96bfc27e6c546E>
10044ef58: 1400001f    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ef5c: b0001229    	adrp	x9, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef60: 91318129    	add	x9, x9, #0xc60
10044ef64: 17ffffbf    	b	0x10044ee60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6ecc>
10044ef68: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef6c: 9131e108    	add	x8, x8, #0xc78
10044ef70: f90027e8    	str	x8, [sp, #0x48]
10044ef74: f94093f3    	ldr	x19, [sp, #0x120]
10044ef78: aa1603e0    	mov	x0, x22
10044ef7c: aa1c03e1    	mov	x1, x28
10044ef80: f94027e2    	ldr	x2, [sp, #0x48]
10044ef84: 9402d57a    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044ef88: 14000013    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044ef8c: b0001220    	adrp	x0, 0x100693000 <dyld_stub_binder+0x100693000>
10044ef90: 9126c000    	add	x0, x0, #0x9b0
10044ef94: 17fffee6    	b	0x10044eb2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b98>
10044ef98: d00011e2    	adrp	x2, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044ef9c: 91392042    	add	x2, x2, #0xe48
10044efa0: 9402d573    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044efa4: 1400000c    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044efa8: b0001220    	adrp	x0, 0x100693000 <dyld_stub_binder+0x100693000>
10044efac: 91338000    	add	x0, x0, #0xce0
10044efb0: 17fffedf    	b	0x10044eb2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6b98>
10044efb4: d00011e2    	adrp	x2, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044efb8: 91392042    	add	x2, x2, #0xe48
10044efbc: 9402d56c    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044efc0: 14000005    	b	0x10044efd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7040>
10044efc4: 900011c2    	adrp	x2, 0x100686000 <dyld_stub_binder+0x100686000>
10044efc8: 91358042    	add	x2, x2, #0xd60
10044efcc: aa1c03e1    	mov	x1, x28
10044efd0: 9402d567    	bl	0x10050456c <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044efd4: d4200020    	brk	#0x1
10044efd8: f9000fe1    	str	x1, [sp, #0x18]
10044efdc: d0001202    	adrp	x2, 0x100690000 <dyld_stub_binder+0x100690000>
10044efe0: 912de042    	add	x2, x2, #0xb78
10044efe4: 17ffffb4    	b	0x10044eeb4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6f20>
10044efe8: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044efec: 91272108    	add	x8, x8, #0x9c8
10044eff0: f90027e8    	str	x8, [sp, #0x48]
10044eff4: aa0003f6    	mov	x22, x0
10044eff8: 17ffffdf    	b	0x10044ef74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fe0>
10044effc: b0001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044f000: 911e6108    	add	x8, x8, #0x798
10044f004: f81303a8    	stur	x8, [x29, #-0xd0]
10044f008: 90001221    	adrp	x1, 0x100693000 <dyld_stub_binder+0x100693000>
10044f00c: 911ea021    	add	x1, x1, #0x7a8
10044f010: 17ffffcb    	b	0x10044ef3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fa8>
10044f014: 90001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044f018: 91272108    	add	x8, x8, #0x9c8
10044f01c: f90027e8    	str	x8, [sp, #0x48]
10044f020: f94083fc    	ldr	x28, [sp, #0x100]
10044f024: 17ffffd4    	b	0x10044ef74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fe0>
10044f028: 90001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044f02c: 91386108    	add	x8, x8, #0xe18
10044f030: 14000003    	b	0x10044f03c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x70a8>
10044f034: 90001228    	adrp	x8, 0x100693000 <dyld_stub_binder+0x100693000>
10044f038: 9135c108    	add	x8, x8, #0xd70
10044f03c: f90027e8    	str	x8, [sp, #0x48]
10044f040: aa0a03fc    	mov	x28, x10
10044f044: 17ffffcc    	b	0x10044ef74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6fe0>
10044f048: 14000071    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f04c: 14000070    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f050: 1400006f    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f054: 1400006e    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f058: aa0003f5    	mov	x21, x0
10044f05c: b85403a8    	ldur	w8, [x29, #-0xc0]
10044f060: 7100091f    	cmp	w8, #0x2
10044f064: 540008e0    	b.eq	0x10044f180 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71ec>
10044f068: f85303a0    	ldur	x0, [x29, #-0xd0]
10044f06c: f9400008    	ldr	x8, [x0]
10044f070: f1000508    	subs	x8, x8, #0x1
10044f074: f9000008    	str	x8, [x0]
10044f078: 54000841    	b.ne	0x10044f180 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71ec>
10044f07c: 97efd728    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044f080: f94093f3    	ldr	x19, [sp, #0x120]
10044f084: 14000070    	b	0x10044f244 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x72b0>
10044f088: 9402d681    	bl	0x100504a8c <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044f08c: f94093f3    	ldr	x19, [sp, #0x120]
10044f090: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f094: a902a513    	stp	x19, x9, [x8, #0x28]
10044f098: 9403222e    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f09c: f94093f3    	ldr	x19, [sp, #0x120]
10044f0a0: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f0a4: a902a513    	stp	x19, x9, [x8, #0x28]
10044f0a8: 9403222a    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f0ac: aa0003f5    	mov	x21, x0
10044f0b0: f9400288    	ldr	x8, [x20]
10044f0b4: f1000508    	subs	x8, x8, #0x1
10044f0b8: f9000288    	str	x8, [x20]
10044f0bc: 54000621    	b.ne	0x10044f180 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71ec>
10044f0c0: 910c43e0    	add	x0, sp, #0x310
10044f0c4: 97ef89a1    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044f0c8: f94093f3    	ldr	x19, [sp, #0x120]
10044f0cc: 1400005e    	b	0x10044f244 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x72b0>
10044f0d0: 9402d66f    	bl	0x100504a8c <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044f0d4: aa0003f5    	mov	x21, x0
10044f0d8: f9400388    	ldr	x8, [x28]
10044f0dc: f1000508    	subs	x8, x8, #0x1
10044f0e0: f9000388    	str	x8, [x28]
10044f0e4: 540004e1    	b.ne	0x10044f180 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71ec>
10044f0e8: 910c43e0    	add	x0, sp, #0x310
10044f0ec: 97ef8997    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044f0f0: f94093f3    	ldr	x19, [sp, #0x120]
10044f0f4: 14000054    	b	0x10044f244 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x72b0>
10044f0f8: 9402d665    	bl	0x100504a8c <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044f0fc: f94093f3    	ldr	x19, [sp, #0x120]
10044f100: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f104: a902a513    	stp	x19, x9, [x8, #0x28]
10044f108: 94032212    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f10c: f9401708    	ldr	x8, [x24, #0x28]
10044f110: d1000508    	sub	x8, x8, #0x1
10044f114: f9001708    	str	x8, [x24, #0x28]
10044f118: f94093f3    	ldr	x19, [sp, #0x120]
10044f11c: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f120: a902a513    	stp	x19, x9, [x8, #0x28]
10044f124: 9403220b    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f128: 14000001    	b	0x10044f12c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7198>
10044f12c: f94083f3    	ldr	x19, [sp, #0x100]
10044f130: f9401668    	ldr	x8, [x19, #0x28]
10044f134: 91000508    	add	x8, x8, #0x1
10044f138: f9001668    	str	x8, [x19, #0x28]
10044f13c: f94093f3    	ldr	x19, [sp, #0x120]
10044f140: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f144: a902a513    	stp	x19, x9, [x8, #0x28]
10044f148: 94032202    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f14c: 14000001    	b	0x10044f150 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71bc>
10044f150: aa0003f5    	mov	x21, x0
10044f154: b85403a8    	ldur	w8, [x29, #-0xc0]
10044f158: 7100091f    	cmp	w8, #0x2
10044f15c: 54000120    	b.eq	0x10044f180 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71ec>
10044f160: f85303a0    	ldur	x0, [x29, #-0xd0]
10044f164: f9400008    	ldr	x8, [x0]
10044f168: f1000508    	subs	x8, x8, #0x1
10044f16c: f9000008    	str	x8, [x0]
10044f170: 54000081    	b.ne	0x10044f180 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x71ec>
10044f174: 97efd6ea    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044f178: f94093f3    	ldr	x19, [sp, #0x120]
10044f17c: 14000032    	b	0x10044f244 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x72b0>
10044f180: f94093f3    	ldr	x19, [sp, #0x120]
10044f184: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f188: a902a513    	stp	x19, x9, [x8, #0x28]
10044f18c: aa1503e0    	mov	x0, x21
10044f190: 940321f0    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f194: 9402d63e    	bl	0x100504a8c <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044f198: 1400001d    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f19c: 1400001c    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1a0: 1400001b    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1a4: 1400001a    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1a8: 14000019    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1ac: 14000018    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1b0: 14000017    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1b4: 14000016    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1b8: 14000015    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1bc: 14000014    	b	0x10044f20c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x7278>
10044f1c0: f94093f3    	ldr	x19, [sp, #0x120]
10044f1c4: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f1c8: a902a513    	stp	x19, x9, [x8, #0x28]
10044f1cc: 940321e1    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f1d0: f94083f3    	ldr	x19, [sp, #0x100]
10044f1d4: f9401668    	ldr	x8, [x19, #0x28]
10044f1d8: d1000508    	sub	x8, x8, #0x1
10044f1dc: f9001668    	str	x8, [x19, #0x28]
10044f1e0: f94093f3    	ldr	x19, [sp, #0x120]
10044f1e4: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f1e8: a902a513    	stp	x19, x9, [x8, #0x28]
10044f1ec: 940321d9    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f1f0: f94016c8    	ldr	x8, [x22, #0x28]
10044f1f4: d1000508    	sub	x8, x8, #0x1
10044f1f8: f90016c8    	str	x8, [x22, #0x28]
10044f1fc: f94093f3    	ldr	x19, [sp, #0x120]
10044f200: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f204: a902a513    	stp	x19, x9, [x8, #0x28]
10044f208: 940321d2    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f20c: aa0003f5    	mov	x21, x0
10044f210: d10343a0    	sub	x0, x29, #0xd0
10044f214: 97f1044a    	bl	0x10009033c <__ZN4core3ptr65drop_in_place$LT$quickjs_oxide..engine..api..error..ErrorData$GT$17hdbf522962b5e705eE>
10044f218: f94093f3    	ldr	x19, [sp, #0x120]
10044f21c: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f220: a902a513    	stp	x19, x9, [x8, #0x28]
10044f224: aa1503e0    	mov	x0, x21
10044f228: 940321ca    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f22c: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f230: a902a513    	stp	x19, x9, [x8, #0x28]
10044f234: 940321c7    	bl	0x100517950 <dyld_stub_binder+0x100517950>
10044f238: aa0003f5    	mov	x21, x0
10044f23c: d10343a0    	sub	x0, x29, #0xd0
10044f240: 97f1043f    	bl	0x10009033c <__ZN4core3ptr65drop_in_place$LT$quickjs_oxide..engine..api..error..ErrorData$GT$17hdbf522962b5e705eE>
10044f244: a95223e9    	ldp	x9, x8, [sp, #0x120]
10044f248: a902a513    	stp	x19, x9, [x8, #0x28]
10044f24c: aa1503e0    	mov	x0, x21
10044f250: 940321c0    	bl	0x100517950 <dyld_stub_binder+0x100517950>
