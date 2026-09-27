
/private/tmp/oxide-m2-pr53-plain/release/qjs:	file format mach-o arm64

Disassembly of section __TEXT,__text:

00000001004452f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E>:
1004452f4: 6db923e9    	stp	d9, d8, [sp, #-0x70]!
1004452f8: a9016ffc    	stp	x28, x27, [sp, #0x10]
1004452fc: a90267fa    	stp	x26, x25, [sp, #0x20]
100445300: a9035ff8    	stp	x24, x23, [sp, #0x30]
100445304: a90457f6    	stp	x22, x21, [sp, #0x40]
100445308: a9054ff4    	stp	x20, x19, [sp, #0x50]
10044530c: a9067bfd    	stp	x29, x30, [sp, #0x60]
100445310: 910183fd    	add	x29, sp, #0x60
100445314: d10f03ff    	sub	sp, sp, #0x3c0
100445318: aa0103f7    	mov	x23, x1
10044531c: aa0003f5    	mov	x21, x0
100445320: a9508420    	ldp	x0, x1, [x1, #0x108]
100445324: 97f3c9f8    	bl	0x100137b04 <__ZN13quickjs_oxide6engine2vm5frame10FrameStore11current_mut17h3f2d85e78871d7baE>
100445328: 36000060    	tbz	w0, #0x0, 0x100445334 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40>
10044532c: f90006a1    	str	x1, [x21, #0x8]
100445330: 14000017    	b	0x10044538c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x98>
100445334: f940003a    	ldr	x26, [x1]
100445338: b9413348    	ldr	w8, [x26, #0x130]
10044533c: 7100091f    	cmp	w8, #0x2
100445340: 5402a0e0    	b.eq	0x10044a75c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5468>
100445344: b9400348    	ldr	w8, [x26]
100445348: 3602a0a8    	tbz	w8, #0x0, 0x10044a75c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5468>
10044534c: aa1a03f8    	mov	x24, x26
100445350: f8408f08    	ldr	x8, [x24, #0x8]!
100445354: b402a328    	cbz	x8, 0x10044a7b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54c4>
100445358: f940a348    	ldr	x8, [x26, #0x140]
10044535c: b402a3a8    	cbz	x8, 0x10044a7d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54dc>
100445360: aa0103f3    	mov	x19, x1
100445364: 91050354    	add	x20, x26, #0x140
100445368: 910082f6    	add	x22, x23, #0x20
10044536c: aa1603e0    	mov	x0, x22
100445370: aa1403e1    	mov	x1, x20
100445374: 97f4bd85    	bl	0x100174988 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore13check_current17h13a1812719f68e40E>
100445378: f100001f    	cmp	x0, #0x0
10044537c: 9a9f02d0    	csel	x16, x22, xzr, eq
100445380: 9a80028f    	csel	x15, x20, x0, eq
100445384: b40001a0    	cbz	x0, 0x1004453b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xc4>
100445388: f90006a0    	str	x0, [x21, #0x8]
10044538c: 52800928    	mov	w8, #0x49               ; =73
100445390: 390002a8    	strb	w8, [x21]
100445394: 910f03ff    	add	sp, sp, #0x3c0
100445398: a9467bfd    	ldp	x29, x30, [sp, #0x60]
10044539c: a9454ff4    	ldp	x20, x19, [sp, #0x50]
1004453a0: a94457f6    	ldp	x22, x21, [sp, #0x40]
1004453a4: a9435ff8    	ldp	x24, x23, [sp, #0x30]
1004453a8: a94267fa    	ldp	x26, x25, [sp, #0x20]
1004453ac: a9416ffc    	ldp	x28, x27, [sp, #0x10]
1004453b0: 6cc723e9    	ldp	d9, d8, [sp], #0x70
1004453b4: d65f03c0    	ret
1004453b8: f9008ff3    	str	x19, [sp, #0x118]
1004453bc: aa1303f1    	mov	x17, x19
1004453c0: a9c2e633    	ldp	x19, x25, [x17, #0x28]!
1004453c4: d360ff28    	lsr	x8, x25, #32
1004453c8: b5029288    	cbnz	x8, 0x10044a618 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5324>
1004453cc: 91044348    	add	x8, x26, #0x110
1004453d0: f90087e8    	str	x8, [sp, #0x108]
1004453d4: f9400108    	ldr	x8, [x8]
1004453d8: f9403509    	ldr	x9, [x8, #0x68]
1004453dc: b40291e9    	cbz	x9, 0x10044a618 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5324>
1004453e0: f940310a    	ldr	x10, [x8, #0x60]
1004453e4: 9100414a    	add	x10, x10, #0x10
1004453e8: f100053f    	cmp	x9, #0x1
1004453ec: 54000061    	b.ne	0x1004453f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x104>
1004453f0: d280000b    	mov	x11, #0x0               ; =0
1004453f4: 1400000a    	b	0x10044541c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x128>
1004453f8: d280000b    	mov	x11, #0x0               ; =0
1004453fc: d341fd2c    	lsr	x12, x9, #1
100445400: 8b0b018d    	add	x13, x12, x11
100445404: b86d794e    	ldr	w14, [x10, x13, lsl #2]
100445408: 6b1901df    	cmp	w14, w25
10044540c: 9a8d816b    	csel	x11, x11, x13, hi
100445410: cb0c0129    	sub	x9, x9, x12
100445414: f100053f    	cmp	x9, #0x1
100445418: 54ffff28    	b.hi	0x1004453fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x108>
10044541c: b86b7949    	ldr	w9, [x10, x11, lsl #2]
100445420: 6b19013f    	cmp	w9, w25
100445424: 54028fa1    	b.ne	0x10044a618 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5324>
100445428: f9006bf1    	str	x17, [sp, #0xd0]
10044542c: f9402d1c    	ldr	x28, [x8, #0x58]
100445430: eb19039f    	cmp	x28, x25
100445434: 540293e9    	b.ls	0x10044a6b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53bc>
100445438: f90083f0    	str	x16, [sp, #0x100]
10044543c: 9100c1e9    	add	x9, x15, #0x30
100445440: a90ee3e9    	stp	x9, x24, [sp, #0xe8]
100445444: 910081ea    	add	x10, x15, #0x20
100445448: f9008bef    	str	x15, [sp, #0x110]
10044544c: 9100a1eb    	add	x11, x15, #0x28
100445450: 910983e9    	add	x9, sp, #0x260
100445454: b2400129    	orr	x9, x9, #0x1
100445458: f9004fe9    	str	x9, [sp, #0x98]
10044545c: 910603e9    	add	x9, sp, #0x180
100445460: b2400129    	orr	x9, x9, #0x1
100445464: f9004be9    	str	x9, [sp, #0x90]
100445468: d10383a9    	sub	x9, x29, #0xe0
10044546c: b2400129    	orr	x9, x9, #0x1
100445470: a90c2be9    	stp	x9, x10, [sp, #0xc0]
100445474: 910663e9    	add	x9, sp, #0x198
100445478: b240012a    	orr	x10, x9, #0x1
10044547c: d10303a9    	sub	x9, x29, #0xc0
100445480: b2400129    	orr	x9, x9, #0x1
100445484: a90da7eb    	stp	x11, x9, [sp, #0xd8]
100445488: 910b43e9    	add	x9, sp, #0x2d0
10044548c: b240012b    	orr	x11, x9, #0x1
100445490: d103c3a9    	sub	x9, x29, #0xf0
100445494: b2400129    	orr	x9, x9, #0x1
100445498: a907afe9    	stp	x9, x11, [sp, #0x78]
10044549c: 910b83e9    	add	x9, sp, #0x2e0
1004454a0: b2400129    	orr	x9, x9, #0x1
1004454a4: f90047e9    	str	x9, [sp, #0x88]
1004454a8: 910863e9    	add	x9, sp, #0x218
1004454ac: b2400129    	orr	x9, x9, #0x1
1004454b0: a905a7ea    	stp	x10, x9, [sp, #0x58]
1004454b4: 9107a3e9    	add	x9, sp, #0x1e8
1004454b8: b240012a    	orr	x10, x9, #0x1
1004454bc: 9106e3e9    	add	x9, sp, #0x1b8
1004454c0: b2400129    	orr	x9, x9, #0x1
1004454c4: a90aabe9    	stp	x9, x10, [sp, #0xa8]
1004454c8: 910723e9    	add	x9, sp, #0x1c8
1004454cc: b240012a    	orr	x10, x9, #0x1
1004454d0: 910763e9    	add	x9, sp, #0x1d8
1004454d4: b2400129    	orr	x9, x9, #0x1
1004454d8: a906abe9    	stp	x9, x10, [sp, #0x68]
1004454dc: 9106a3e9    	add	x9, sp, #0x1a8
1004454e0: b2400129    	orr	x9, x9, #0x1
1004454e4: f9005fe9    	str	x9, [sp, #0xb8]
1004454e8: 9105a3e9    	add	x9, sp, #0x168
1004454ec: b2400129    	orr	x9, x9, #0x1
1004454f0: f90053e9    	str	x9, [sp, #0xa0]
1004454f4: 910523e9    	add	x9, sp, #0x148
1004454f8: b240012a    	orr	x10, x9, #0x1
1004454fc: 910563e9    	add	x9, sp, #0x158
100445500: b2400129    	orr	x9, x9, #0x1
100445504: a904abe9    	stp	x9, x10, [sp, #0x48]
100445508: b00011e9    	adrp	x9, 0x100682000 <dyld_stub_binder+0x100682000>
10044550c: 9134c129    	add	x9, x9, #0xd30
100445510: f90023e9    	str	x9, [sp, #0x40]
100445514: aa1903e9    	mov	x9, x25
100445518: f9402908    	ldr	x8, [x8, #0x50]
10044551c: 91004118    	add	x24, x8, #0x10
100445520: b8697b13    	ldr	w19, [x24, x9, lsl #2]
100445524: 53106668    	ubfx	w8, w19, #16, #10
100445528: 7103911f    	cmp	w8, #0xe4
10044552c: 54028c22    	b.hs	0x10044a6b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53bc>
100445530: 90000a09    	adrp	x9, 0x100585000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16ce8>
100445534: 912b8129    	add	x9, x9, #0xae0
100445538: 7868593b    	ldrh	w27, [x9, w8, uxtw #1]
10044553c: 531d7a68    	ubfx	w8, w19, #29, #2
100445540: 0b080329    	add	w9, w25, w8
100445544: 11000534    	add	w20, w9, #0x1
100445548: 531c7e69    	lsr	w9, w19, #28
10044554c: a91273f8    	stp	x24, x28, [sp, #0x120]
100445550: 53127e6a    	lsr	w10, w19, #18
100445554: 1218054a    	and	w10, w10, #0x300
100445558: b90133f3    	str	w19, [sp, #0x130]
10044555c: b90137f9    	str	w25, [sp, #0x134]
100445560: 331c726a    	bfxil	w10, w19, #28, #1
100445564: b9013bf4    	str	w20, [sp, #0x138]
100445568: 79027bfb    	strh	w27, [sp, #0x13c]
10044556c: 79027fea    	strh	w10, [sp, #0x13e]
100445570: f900a3f4    	str	x20, [sp, #0x140]
100445574: 360000e9    	tbz	w9, #0x0, 0x100445590 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x29c>
100445578: 2a1903e9    	mov	w9, w25
10044557c: 91000536    	add	x22, x9, #0x1
100445580: eb1c02df    	cmp	x22, x28
100445584: 540321a2    	b.hs	0x10044b9b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c4>
100445588: b8767b09    	ldr	w9, [x24, x22, lsl #2]
10044558c: 14000002    	b	0x100445594 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2a0>
100445590: 12003e69    	and	w9, w19, #0xffff
100445594: f9007fe9    	str	x9, [sp, #0xf8]
100445598: 71038f7f    	cmp	w27, #0xe3
10044559c: 5402cc48    	b.hi	0x10044af24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c30>
1004455a0: d000090b    	adrp	x11, 0x100567000 <dyld_stub_binder+0x100567000>
1004455a4: 912e316b    	add	x11, x11, #0xb8c
1004455a8: 10000089    	adr	x9, 0x1004455b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2c4>
1004455ac: 787b796a    	ldrh	w10, [x11, x27, lsl #1]
1004455b0: 8b0a0929    	add	x9, x9, x10, lsl #2
1004455b4: d61f0120    	br	x9
1004455b8: f9408be8    	ldr	x8, [sp, #0x110]
1004455bc: f9402113    	ldr	x19, [x8, #0x40]
1004455c0: f1000a7f    	cmp	x19, #0x2
1004455c4: 5402eac3    	b.lo	0x10044b31c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6028>
1004455c8: f94077e8    	ldr	x8, [sp, #0xe8]
1004455cc: f9400108    	ldr	x8, [x8]
1004455d0: 8b130101    	add	x1, x8, x19
1004455d4: d1000836    	sub	x22, x1, #0x2
1004455d8: eb16003f    	cmp	x1, x22
1004455dc: f94083e8    	ldr	x8, [sp, #0x100]
1004455e0: 5402ea43    	b.lo	0x10044b328 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6034>
1004455e4: f940091c    	ldr	x28, [x8, #0x10]
1004455e8: eb1c003f    	cmp	x1, x28
1004455ec: 5402ea48    	b.hi	0x10044b334 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6040>
1004455f0: f9400514    	ldr	x20, [x8, #0x8]
1004455f4: 8b161298    	add	x24, x20, x22, lsl #4
1004455f8: 39400308    	ldrb	w8, [x24]
1004455fc: 7100391f    	cmp	w8, #0xe
100445600: 540289a0    	b.eq	0x10044a734 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5440>
100445604: 7100251f    	cmp	w8, #0x9
100445608: 54028968    	b.hi	0x10044a734 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5440>
10044560c: 3940430a    	ldrb	w10, [x24, #0x10]
100445610: 51002949    	sub	w9, w10, #0xa
100445614: 7100153f    	cmp	w9, #0x5
100445618: 540288e3    	b.lo	0x10044a734 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5440>
10044561c: 7100111f    	cmp	w8, #0x4
100445620: 54000180    	b.eq	0x100445650 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x35c>
100445624: 71000d1f    	cmp	w8, #0x3
100445628: 540288c1    	b.ne	0x10044a740 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x544c>
10044562c: b9400708    	ldr	w8, [x24, #0x4]
100445630: 71000d5f    	cmp	w10, #0x3
100445634: 54000840    	b.eq	0x10044573c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x448>
100445638: 7100115f    	cmp	w10, #0x4
10044563c: 54028821    	b.ne	0x10044a740 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x544c>
100445640: 5280000a    	mov	w10, #0x0               ; =0
100445644: f9400f0d    	ldr	x13, [x24, #0x18]
100445648: 5280002b    	mov	w11, #0x1               ; =1
10044564c: 1400003f    	b	0x100445748 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x454>
100445650: f9400709    	ldr	x9, [x24, #0x8]
100445654: 71000d5f    	cmp	w10, #0x3
100445658: 540006a0    	b.eq	0x10044572c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x438>
10044565c: 7100115f    	cmp	w10, #0x4
100445660: 54028701    	b.ne	0x10044a740 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x544c>
100445664: f9400f0d    	ldr	x13, [x24, #0x18]
100445668: 5280002a    	mov	w10, #0x1               ; =1
10044566c: 5280002b    	mov	w11, #0x1               ; =1
100445670: 14000036    	b	0x100445748 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x454>
100445674: f9408be2    	ldr	x2, [sp, #0x110]
100445678: f940204c    	ldr	x12, [x2, #0x40]
10044567c: b402eaac    	cbz	x12, 0x10044b3d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60dc>
100445680: f94083f1    	ldr	x17, [sp, #0x100]
100445684: f9400a3c    	ldr	x28, [x17, #0x10]
100445688: f94077e8    	ldr	x8, [sp, #0xe8]
10044568c: f940010e    	ldr	x14, [x8]
100445690: d1000588    	sub	x8, x12, #0x1
100445694: 8b0801d6    	add	x22, x14, x8
100445698: eb1c02df    	cmp	x22, x28
10044569c: 54030cc2    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
1004456a0: f9400634    	ldr	x20, [x17, #0x8]
1004456a4: 8b161289    	add	x9, x20, x22, lsl #4
1004456a8: 3940012a    	ldrb	w10, [x9]
1004456ac: 5100294b    	sub	w11, w10, #0xa
1004456b0: 7100117f    	cmp	w11, #0x4
1004456b4: 540289a9    	b.ls	0x10044a7e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54f4>
1004456b8: 7100115f    	cmp	w10, #0x4
1004456bc: 54000bc0    	b.eq	0x100445834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x540>
1004456c0: 71000d5f    	cmp	w10, #0x3
1004456c4: 54028981    	b.ne	0x10044a7f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5500>
1004456c8: 5280000a    	mov	w10, #0x0               ; =0
1004456cc: b940052b    	ldr	w11, [x9, #0x4]
1004456d0: 1400005b    	b	0x10044583c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x548>
1004456d4: 52800028    	mov	w8, #0x1                ; =1
1004456d8: 6a53711f    	tst	w8, w19, lsr #28
1004456dc: 9a880508    	cinc	x8, x8, ne
1004456e0: 2a1903f4    	mov	w20, w25
1004456e4: 8b140116    	add	x22, x8, x20
1004456e8: eb1c02df    	cmp	x22, x28
1004456ec: 54030bc2    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
1004456f0: 11009f6a    	add	w10, w27, #0x27
1004456f4: 12001d4c    	and	w12, w10, #0xff
1004456f8: b8767b00    	ldr	w0, [x24, x22, lsl #2]
1004456fc: 1100cb68    	add	w8, w27, #0x32
100445700: 12001d09    	and	w9, w8, #0xff
100445704: 7100313f    	cmp	w9, #0xc
100445708: 1a9f97e9    	cset	w9, hi
10044570c: f9408beb    	ldr	x11, [sp, #0x110]
100445710: f940216b    	ldr	x11, [x11, #0x40]
100445714: 71000d9f    	cmp	w12, #0x3
100445718: 54001742    	b.hs	0x100445a00 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x70c>
10044571c: b100117f    	cmn	x11, #0x4
100445720: 540205c8    	b.hi	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100445724: 91000d6b    	add	x11, x11, #0x3
100445728: 140000b9    	b	0x100445a0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x718>
10044572c: 5280000b    	mov	w11, #0x0               ; =0
100445730: b940170c    	ldr	w12, [x24, #0x14]
100445734: 5280002a    	mov	w10, #0x1               ; =1
100445738: 14000004    	b	0x100445748 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x454>
10044573c: 5280000a    	mov	w10, #0x0               ; =0
100445740: 5280000b    	mov	w11, #0x0               ; =0
100445744: b940170c    	ldr	w12, [x24, #0x14]
100445748: 292823aa    	stp	w10, w8, [x29, #-0xc0]
10044574c: f81483a9    	stur	x9, [x29, #-0xb8]
100445750: 292a33ab    	stp	w11, w12, [x29, #-0xb0]
100445754: f81583ad    	stur	x13, [x29, #-0xa8]
100445758: d10383a0    	sub	x0, x29, #0xe0
10044575c: d10303a2    	sub	x2, x29, #0xc0
100445760: d10303a8    	sub	x8, x29, #0xc0
100445764: 91004103    	add	x3, x8, #0x10
100445768: aa1b03e1    	mov	x1, x27
10044576c: 94002948    	bl	0x10044fc8c <__ZN13quickjs_oxide6engine2vm7execute20binary_number_result17h75ea38afe7117b04E>
100445770: eb1c02df    	cmp	x22, x28
100445774: 54030842    	b.hs	0x10044b87c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6588>
100445778: 3cd203a0    	ldur	q0, [x29, #-0xe0]
10044577c: 3d800300    	str	q0, [x24]
100445780: 910006d6    	add	x22, x22, #0x1
100445784: eb1c02df    	cmp	x22, x28
100445788: 54030742    	b.hs	0x10044b870 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x657c>
10044578c: d37ceec8    	lsl	x8, x22, #4
100445790: 528001c9    	mov	w9, #0xe                ; =14
100445794: 38286a89    	strb	w9, [x20, x8]
100445798: d1000668    	sub	x8, x19, #0x1
10044579c: f9408be9    	ldr	x9, [sp, #0x110]
1004457a0: f9002128    	str	x8, [x9, #0x40]
1004457a4: 1400125a    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
1004457a8: f9407fe8    	ldr	x8, [sp, #0xf8]
1004457ac: 12003d13    	and	w19, w8, #0xffff
1004457b0: f9406fe8    	ldr	x8, [sp, #0xd8]
1004457b4: f9400109    	ldr	x9, [x8]
1004457b8: f94077e8    	ldr	x8, [sp, #0xe8]
1004457bc: f9400108    	ldr	x8, [x8]
1004457c0: eb09010a    	subs	x10, x8, x9
1004457c4: 9a8a33ea    	csel	x10, xzr, x10, lo
1004457c8: eb13015f    	cmp	x10, x19
1004457cc: 540285e9    	b.ls	0x10044a888 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5594>
1004457d0: f94083f0    	ldr	x16, [sp, #0x100]
1004457d4: f9400a1c    	ldr	x28, [x16, #0x10]
1004457d8: 8b130136    	add	x22, x9, x19
1004457dc: eb1c02df    	cmp	x22, x28
1004457e0: 54030542    	b.hs	0x10044b888 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6594>
1004457e4: f9400609    	ldr	x9, [x16, #0x8]
1004457e8: 8b16112a    	add	x10, x9, x22, lsl #4
1004457ec: 3940014c    	ldrb	w12, [x10]
1004457f0: 7100399f    	cmp	w12, #0xe
1004457f4: f9408bef    	ldr	x15, [sp, #0x110]
1004457f8: f9407bf1    	ldr	x17, [sp, #0xf0]
1004457fc: 540287c0    	b.eq	0x10044a8f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5600>
100445800: 5100af6b    	sub	w11, w27, #0x2b
100445804: 5100298d    	sub	w13, w12, #0xa
100445808: d100258e    	sub	x14, x12, #0x9
10044580c: 710011bf    	cmp	w13, #0x4
100445810: 9a9f31cd    	csel	x13, x14, xzr, lo
100445814: f10009bf    	cmp	x13, #0x2
100445818: 5400d32c    	b.gt	0x10044727c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1f88>
10044581c: d10005ad    	sub	x13, x13, #0x1
100445820: f10009bf    	cmp	x13, #0x2
100445824: 910923ee    	add	x14, sp, #0x248
100445828: 5400de42    	b.hs	0x1004473f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x20fc>
10044582c: 5280008c    	mov	w12, #0x4               ; =4
100445830: 140006f3    	b	0x1004473fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2108>
100445834: fd400528    	ldr	d8, [x9, #0x8]
100445838: 5280002a    	mov	w10, #0x1               ; =1
10044583c: 5101d76d    	sub	w13, w27, #0x75
100445840: 12001daf    	and	w15, w13, #0xff
100445844: 710005ff    	cmp	w15, #0x1
100445848: 540000e8    	b.hi	0x100445864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x570>
10044584c: b100059f    	cmn	x12, #0x1
100445850: 54027d20    	b.eq	0x10044a7f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5500>
100445854: f9401c4f    	ldr	x15, [x2, #0x38]
100445858: cb0e01ee    	sub	x14, x15, x14
10044585c: eb0e019f    	cmp	x12, x14
100445860: 54027ca2    	b.hs	0x10044a7f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5500>
100445864: 7101c77f    	cmp	w27, #0x71
100445868: 54005960    	b.eq	0x100446394 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x10a0>
10044586c: 7101cb7f    	cmp	w27, #0x72
100445870: 540058e0    	b.eq	0x10044638c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1098>
100445874: 1e620160    	scvtf	d0, w11
100445878: 7101df7f    	cmp	w27, #0x77
10044587c: 54005941    	b.ne	0x1004463a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x10b0>
100445880: 7100015f    	cmp	w10, #0x0
100445884: 1e601d00    	fcsel	d0, d8, d0, ne
100445888: 9e66000a    	fmov	x10, d0
10044588c: 9240f94a    	and	x10, x10, #0x7fffffffffffffff
100445890: d2effe0b    	mov	x11, #0x7ff0000000000000 ; =9218868437227405312
100445894: eb0b015f    	cmp	x10, x11
100445898: fa4bd144    	ccmp	x10, x11, #0x4, le
10044589c: fa401944    	ccmp	x10, #0x0, #0x4, ne
1004458a0: 5400f281    	b.ne	0x1004476f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x23fc>
1004458a4: 5280000a    	mov	w10, #0x0               ; =0
1004458a8: 12800013    	mov	w19, #-0x1              ; =-1
1004458ac: 140007aa    	b	0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
1004458b0: 71015b7f    	cmp	w27, #0x56
1004458b4: 52800068    	mov	w8, #0x3                ; =3
1004458b8: 9a880508    	cinc	x8, x8, ne
1004458bc: 7101577f    	cmp	w27, #0x55
1004458c0: 52800049    	mov	w9, #0x2                ; =2
1004458c4: 9a880125    	csel	x5, x9, x8, eq
1004458c8: f94083e8    	ldr	x8, [sp, #0x100]
1004458cc: a9408500    	ldp	x0, x1, [x8, #0x8]
1004458d0: f9408be8    	ldr	x8, [sp, #0x110]
1004458d4: f9401902    	ldr	x2, [x8, #0x30]
1004458d8: f9402103    	ldr	x3, [x8, #0x40]
1004458dc: 52800024    	mov	w4, #0x1                ; =1
1004458e0: 52800006    	mov	w6, #0x0                ; =0
1004458e4: 94002897    	bl	0x10044fb40 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23rotate_operands_current17ha464970b12b6798cE>
1004458e8: 14001208    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004458ec: f9407be8    	ldr	x8, [sp, #0xf0]
1004458f0: f9400114    	ldr	x20, [x8]
1004458f4: 7101bf7f    	cmp	w27, #0x6f
1004458f8: 54006f00    	b.eq	0x1004466d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13e4>
1004458fc: 7101c37f    	cmp	w27, #0x70
100445900: f9408be1    	ldr	x1, [sp, #0x110]
100445904: f94083e0    	ldr	x0, [sp, #0x100]
100445908: 54006f61    	b.ne	0x1004466f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1400>
10044590c: 52800023    	mov	w3, #0x1                ; =1
100445910: 14000375    	b	0x1004466e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x13f0>
100445914: 71014b7f    	cmp	w27, #0x52
100445918: 52800068    	mov	w8, #0x3                ; =3
10044591c: 9a880508    	cinc	x8, x8, ne
100445920: 7101477f    	cmp	w27, #0x51
100445924: 52800049    	mov	w9, #0x2                ; =2
100445928: 9a880124    	csel	x4, x9, x8, eq
10044592c: d1000489    	sub	x9, x4, #0x1
100445930: f9408be1    	ldr	x1, [sp, #0x110]
100445934: f9402028    	ldr	x8, [x1, #0x40]
100445938: eb09011f    	cmp	x8, x9
10044593c: 5402d9a9    	b.ls	0x10044b470 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x617c>
100445940: f94083e0    	ldr	x0, [sp, #0x100]
100445944: f940081c    	ldr	x28, [x0, #0x10]
100445948: a94eabe9    	ldp	x9, x10, [sp, #0xe8]
10044594c: f9400129    	ldr	x9, [x9]
100445950: cb040108    	sub	x8, x8, x4
100445954: 8b080136    	add	x22, x9, x8
100445958: eb1c02df    	cmp	x22, x28
10044595c: 5402f6c2    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100445960: f9400408    	ldr	x8, [x0, #0x8]
100445964: d37ceec9    	lsl	x9, x22, #4
100445968: 38696908    	ldrb	w8, [x8, x9]
10044596c: 51002908    	sub	w8, w8, #0xa
100445970: 7100111f    	cmp	w8, #0x4
100445974: 540286c9    	b.ls	0x10044aa4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5758>
100445978: f9400142    	ldr	x2, [x10]
10044597c: d2800003    	mov	x3, #0x0                ; =0
100445980: 940027c7    	bl	0x10044f89c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots11insert_copy17hb51f0c3ae63d8d95E>
100445984: 140011e1    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100445988: 7103377f    	cmp	w27, #0xcd
10044598c: 1a9f17e9    	cset	w9, eq
100445990: 71032f7f    	cmp	w27, #0xcb
100445994: 54005460    	b.eq	0x100446420 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x112c>
100445998: f9408be8    	ldr	x8, [sp, #0x110]
10044599c: f9402108    	ldr	x8, [x8, #0x40]
1004459a0: f94083ea    	ldr	x10, [sp, #0x100]
1004459a4: f9407bec    	ldr	x12, [sp, #0xf0]
1004459a8: b402d968    	cbz	x8, 0x10044b4d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61e0>
1004459ac: b900fbe9    	str	w9, [sp, #0xf8]
1004459b0: f940095c    	ldr	x28, [x10, #0x10]
1004459b4: f94077e9    	ldr	x9, [sp, #0xe8]
1004459b8: f9400129    	ldr	x9, [x9]
1004459bc: 8b090109    	add	x9, x8, x9
1004459c0: d1000536    	sub	x22, x9, #0x1
1004459c4: eb1c02df    	cmp	x22, x28
1004459c8: 5402f362    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
1004459cc: f9400549    	ldr	x9, [x10, #0x8]
1004459d0: 8b161134    	add	x20, x9, x22, lsl #4
1004459d4: 3940028a    	ldrb	w10, [x20]
1004459d8: 5100294b    	sub	w11, w10, #0xa
1004459dc: 7100117f    	cmp	w11, #0x4
1004459e0: 5402a0c9    	b.ls	0x10044adf8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b04>
1004459e4: 7100155f    	cmp	w10, #0x5
1004459e8: 5400f8c0    	b.eq	0x100447900 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x260c>
1004459ec: 71000d5f    	cmp	w10, #0x3
1004459f0: 5402c741    	b.ne	0x10044b2d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe4>
1004459f4: b9400694    	ldr	w20, [x20, #0x4]
1004459f8: 36f94094    	tbz	w20, #0x1f, 0x100448208 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2f14>
1004459fc: 14001637    	b	0x10044b2d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe4>
100445a00: b1000d7f    	cmn	x11, #0x3
100445a04: 5401eea8    	b.hi	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100445a08: 9100096b    	add	x11, x11, #0x2
100445a0c: f9408bed    	ldr	x13, [sp, #0x110]
100445a10: a94331ad    	ldp	x13, x12, [x13, #0x30]
100445a14: cb0d018c    	sub	x12, x12, x13
100445a18: eb0c017f    	cmp	x11, x12
100445a1c: 5401ede8    	b.hi	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100445a20: 7210001f    	tst	w0, #0x10000
100445a24: f94067eb    	ldr	x11, [sp, #0xc8]
100445a28: f9406fed    	ldr	x13, [sp, #0xd8]
100445a2c: 9a8b01ab    	csel	x11, x13, x11, eq
100445a30: f94077ec    	ldr	x12, [sp, #0xe8]
100445a34: 9a8d018c    	csel	x12, x12, x13, eq
100445a38: f9400181    	ldr	x1, [x12]
100445a3c: f9400176    	ldr	x22, [x11]
100445a40: eb16002c    	subs	x12, x1, x22
100445a44: 5402cfe3    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100445a48: f94083eb    	ldr	x11, [sp, #0x100]
100445a4c: f940096b    	ldr	x11, [x11, #0x10]
100445a50: eb0b003f    	cmp	x1, x11
100445a54: 5402d5a8    	b.hi	0x10044b508 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6214>
100445a58: 92403c04    	and	x4, x0, #0xffff
100445a5c: eb04019f    	cmp	x12, x4
100445a60: 5401ebc9    	b.ls	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100445a64: f94083ec    	ldr	x12, [sp, #0x100]
100445a68: f940058c    	ldr	x12, [x12, #0x8]
100445a6c: 8b16118d    	add	x13, x12, x22, lsl #4
100445a70: 8b0411ad    	add	x13, x13, x4, lsl #4
100445a74: 394001ae    	ldrb	w14, [x13]
100445a78: 710025df    	cmp	w14, #0x9
100445a7c: 5401eae8    	b.hi	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100445a80: 71000ddf    	cmp	w14, #0x3
100445a84: 54019680    	b.eq	0x100448d54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3a60>
100445a88: 710011df    	cmp	w14, #0x4
100445a8c: 5401ea61    	b.ne	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100445a90: fd4005a0    	ldr	d0, [x13, #0x8]
100445a94: 720f001f    	tst	w0, #0x20000
100445a98: 1e7e1001    	fmov	d1, #-1.00000000
100445a9c: 1e6e1002    	fmov	d2, #1.00000000
100445aa0: 1e610c41    	fcsel	d1, d2, d1, eq
100445aa4: 1e602828    	fadd	d8, d1, d0
100445aa8: 5280002e    	mov	w14, #0x1               ; =1
100445aac: 52800031    	mov	w17, #0x1               ; =1
100445ab0: 14000f22    	b	0x100449738 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4444>
100445ab4: 52800028    	mov	w8, #0x1                ; =1
100445ab8: 6a53711f    	tst	w8, w19, lsr #28
100445abc: 9a880509    	cinc	x9, x8, ne
100445ac0: 2a1903e8    	mov	w8, w25
100445ac4: 8b080136    	add	x22, x9, x8
100445ac8: eb1c02df    	cmp	x22, x28
100445acc: 5402ecc2    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100445ad0: f9408be9    	ldr	x9, [sp, #0x110]
100445ad4: f9402129    	ldr	x9, [x9, #0x40]
100445ad8: b1000d3f    	cmn	x9, #0x3
100445adc: 54020fe8    	b.hi	0x100449cd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e4>
100445ae0: 91000929    	add	x9, x9, #0x2
100445ae4: f9408beb    	ldr	x11, [sp, #0x110]
100445ae8: a943296b    	ldp	x11, x10, [x11, #0x30]
100445aec: cb0b014a    	sub	x10, x10, x11
100445af0: eb0a013f    	cmp	x9, x10
100445af4: 54020f28    	b.hi	0x100449cd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e4>
100445af8: 7103537f    	cmp	w27, #0xd4
100445afc: f94067e9    	ldr	x9, [sp, #0xc8]
100445b00: f9406feb    	ldr	x11, [sp, #0xd8]
100445b04: 9a890169    	csel	x9, x11, x9, eq
100445b08: f94077ea    	ldr	x10, [sp, #0xe8]
100445b0c: 9a8b014a    	csel	x10, x10, x11, eq
100445b10: f9400141    	ldr	x1, [x10]
100445b14: f9400129    	ldr	x9, [x9]
100445b18: eb09002c    	subs	x12, x1, x9
100445b1c: 5402d5e3    	b.lo	0x10044b5d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62e4>
100445b20: f94083eb    	ldr	x11, [sp, #0x100]
100445b24: f940096a    	ldr	x10, [x11, #0x10]
100445b28: eb0a003f    	cmp	x1, x10
100445b2c: 5402cfc8    	b.hi	0x10044b524 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6230>
100445b30: f9407fed    	ldr	x13, [sp, #0xf8]
100445b34: 12003dad    	and	w13, w13, #0xffff
100445b38: f940056b    	ldr	x11, [x11, #0x8]
100445b3c: 2f00e400    	movi	d0, #0000000000000000
100445b40: eb0d019f    	cmp	x12, x13
100445b44: 5400d4a9    	b.ls	0x1004475d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22e4>
100445b48: 8b091169    	add	x9, x11, x9, lsl #4
100445b4c: 8b0d1129    	add	x9, x9, x13, lsl #4
100445b50: 3940012c    	ldrb	w12, [x9]
100445b54: 7100259f    	cmp	w12, #0x9
100445b58: 5400d408    	b.hi	0x1004475d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22e4>
100445b5c: 71000d9f    	cmp	w12, #0x3
100445b60: 54020a60    	b.eq	0x100449cac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49b8>
100445b64: 7100119f    	cmp	w12, #0x4
100445b68: 5400d381    	b.ne	0x1004475d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22e4>
100445b6c: 5280000c    	mov	w12, #0x0               ; =0
100445b70: fd400520    	ldr	d0, [x9, #0x8]
100445b74: 1400069a    	b	0x1004475dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22e8>
100445b78: 52800048    	mov	w8, #0x2                ; =2
100445b7c: b81503a8    	stur	w8, [x29, #-0xb0]
100445b80: f94083e8    	ldr	x8, [sp, #0x100]
100445b84: a940851c    	ldp	x28, x1, [x8, #0x8]
100445b88: 71032b7f    	cmp	w27, #0xca
100445b8c: 1a9f17e8    	cset	w8, eq
100445b90: b9003be8    	str	w8, [sp, #0x38]
100445b94: f9408bf6    	ldr	x22, [sp, #0x110]
100445b98: 540122e1    	b.ne	0x100447ff4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2d00>
100445b9c: aa1c03e0    	mov	x0, x28
100445ba0: aa0103f3    	mov	x19, x1
100445ba4: aa1603e2    	mov	x2, x22
100445ba8: 97f4e0dd    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
100445bac: aa0103f4    	mov	x20, x1
100445bb0: 37029f00    	tbnz	w0, #0x0, 0x10044af90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c9c>
100445bb4: b40121c0    	cbz	x0, 0x100447fec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2cf8>
100445bb8: f9401688    	ldr	x8, [x20, #0x28]
100445bbc: f9408bf6    	ldr	x22, [sp, #0x110]
100445bc0: b4000068    	cbz	x8, 0x100445bcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x8d8>
100445bc4: f9401a80    	ldr	x0, [x20, #0x30]
100445bc8: 94033837    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
100445bcc: f9402280    	ldr	x0, [x20, #0x40]
100445bd0: b4000040    	cbz	x0, 0x100445bd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x8e4>
100445bd4: 94033834    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
100445bd8: aa1403e0    	mov	x0, x20
100445bdc: 94033832    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
100445be0: 14000904    	b	0x100447ff0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2cfc>
100445be4: f9407fe9    	ldr	x9, [sp, #0xf8]
100445be8: 12003133    	and	w19, w9, #0x1fff
100445bec: f9408bed    	ldr	x13, [sp, #0x110]
100445bf0: f94021a8    	ldr	x8, [x13, #0x40]
100445bf4: 37704fe9    	tbnz	w9, #0xe, 0x1004465f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x12fc>
100445bf8: b100051f    	cmn	x8, #0x1
100445bfc: f94083e0    	ldr	x0, [sp, #0x100]
100445c00: 540053e0    	b.eq	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
100445c04: a94325a1    	ldp	x1, x9, [x13, #0x30]
100445c08: cb010129    	sub	x9, x9, x1
100445c0c: eb09011f    	cmp	x8, x9
100445c10: 54005003    	b.lo	0x100446610 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x131c>
100445c14: 1400029a    	b	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
100445c18: f9408bec    	ldr	x12, [sp, #0x110]
100445c1c: f9402188    	ldr	x8, [x12, #0x40]
100445c20: b402c3a8    	cbz	x8, 0x10044b494 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61a0>
100445c24: f94083eb    	ldr	x11, [sp, #0x100]
100445c28: f940097c    	ldr	x28, [x11, #0x10]
100445c2c: f94077e9    	ldr	x9, [sp, #0xe8]
100445c30: f940012a    	ldr	x10, [x9]
100445c34: d1000509    	sub	x9, x8, #0x1
100445c38: 8b0a0136    	add	x22, x9, x10
100445c3c: eb1c02df    	cmp	x22, x28
100445c40: 5402dfa2    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100445c44: f9400568    	ldr	x8, [x11, #0x8]
100445c48: 8b16110a    	add	x10, x8, x22, lsl #4
100445c4c: 39400148    	ldrb	w8, [x10]
100445c50: 5100290b    	sub	w11, w8, #0xa
100445c54: 7100117f    	cmp	w11, #0x4
100445c58: 54027b49    	b.ls	0x10044abc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58cc>
100445c5c: 7100150b    	subs	w11, w8, #0x5
100445c60: 7a472904    	ccmp	w8, #0x7, #0x4, hs
100445c64: 5402b461    	b.ne	0x10044b2f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ffc>
100445c68: f9002189    	str	x9, [x12, #0x40]
100445c6c: 3940054c    	ldrb	w12, [x10, #0x1]
100445c70: b940054d    	ldr	w13, [x10, #0x4]
100445c74: f9400549    	ldr	x9, [x10, #0x8]
100445c78: 528001ce    	mov	w14, #0xe               ; =14
100445c7c: 3900014e    	strb	w14, [x10]
100445c80: 71000d1f    	cmp	w8, #0x3
100445c84: 5400af2c    	b.gt	0x100447268 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1f74>
100445c88: 7100091f    	cmp	w8, #0x2
100445c8c: 54010842    	b.hs	0x100447d94 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2aa0>
100445c90: 71025b7f    	cmp	w27, #0x96
100445c94: 540223c0    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100445c98: 14000af6    	b	0x100448870 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x357c>
100445c9c: f9408be8    	ldr	x8, [sp, #0x110]
100445ca0: f9402108    	ldr	x8, [x8, #0x40]
100445ca4: b100051f    	cmn	x8, #0x1
100445ca8: 540009a0    	b.eq	0x100445ddc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xae8>
100445cac: f9408bea    	ldr	x10, [sp, #0x110]
100445cb0: a943254a    	ldp	x10, x9, [x10, #0x30]
100445cb4: cb0a0129    	sub	x9, x9, x10
100445cb8: eb09011f    	cmp	x8, x9
100445cbc: 54000902    	b.hs	0x100445ddc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xae8>
100445cc0: 71034b7f    	cmp	w27, #0xd2
100445cc4: f94067e8    	ldr	x8, [sp, #0xc8]
100445cc8: f9406fea    	ldr	x10, [sp, #0xd8]
100445ccc: 9a880148    	csel	x8, x10, x8, eq
100445cd0: f94077e9    	ldr	x9, [sp, #0xe8]
100445cd4: 9a8a0129    	csel	x9, x9, x10, eq
100445cd8: f9400121    	ldr	x1, [x9]
100445cdc: f9400116    	ldr	x22, [x8]
100445ce0: eb160028    	subs	x8, x1, x22
100445ce4: 5402bae3    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100445ce8: f94083e9    	ldr	x9, [sp, #0x100]
100445cec: f9400929    	ldr	x9, [x9, #0x10]
100445cf0: eb09003f    	cmp	x1, x9
100445cf4: 5402bf48    	b.hi	0x10044b4dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61e8>
100445cf8: f9407fe9    	ldr	x9, [sp, #0xf8]
100445cfc: 12003d29    	and	w9, w9, #0xffff
100445d00: eb09011f    	cmp	x8, x9
100445d04: 540006c9    	b.ls	0x100445ddc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xae8>
100445d08: f94083e8    	ldr	x8, [sp, #0x100]
100445d0c: f9400508    	ldr	x8, [x8, #0x8]
100445d10: 8b161108    	add	x8, x8, x22, lsl #4
100445d14: 8b091102    	add	x2, x8, x9, lsl #4
100445d18: 39400048    	ldrb	w8, [x2]
100445d1c: 7100251f    	cmp	w8, #0x9
100445d20: 540005e1    	b.ne	0x100445ddc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xae8>
100445d24: 52800048    	mov	w8, #0x2                ; =2
100445d28: b81503a8    	stur	w8, [x29, #-0xb0]
100445d2c: 52800028    	mov	w8, #0x1                ; =1
100445d30: 6a53711f    	tst	w8, w19, lsr #28
100445d34: 9a880508    	cinc	x8, x8, ne
100445d38: 2a1903f6    	mov	w22, w25
100445d3c: 8b160100    	add	x0, x8, x22
100445d40: eb1c001f    	cmp	x0, x28
100445d44: 5402e722    	b.hs	0x10044ba28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6734>
100445d48: b8607b05    	ldr	w5, [x24, x0, lsl #2]
100445d4c: f9407be8    	ldr	x8, [sp, #0xf0]
100445d50: f9400101    	ldr	x1, [x8]
100445d54: d10383a0    	sub	x0, x29, #0xe0
100445d58: d10303a7    	sub	x7, x29, #0xc0
100445d5c: f94087e3    	ldr	x3, [sp, #0x108]
100445d60: aa1403e4    	mov	x4, x20
100445d64: 52800026    	mov	w6, #0x1                ; =1
100445d68: 94002b95    	bl	0x100450bbc <__ZN13quickjs_oxide6engine6object16ordinary_storage2ic62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$21property_ic_read_fast17h51aad55151f8a4d2E>
100445d6c: 385203b4    	ldurb	w20, [x29, #-0xe0]
100445d70: f94063e9    	ldr	x9, [sp, #0xc0]
100445d74: b9400128    	ldr	w8, [x9]
100445d78: b902e3e8    	str	w8, [sp, #0x2e0]
100445d7c: b8403128    	ldur	w8, [x9, #0x3]
100445d80: 910923e9    	add	x9, sp, #0x248
100445d84: b809b128    	stur	w8, [x9, #0x9b]
100445d88: f85283a8    	ldur	x8, [x29, #-0xd8]
100445d8c: f9001fe8    	str	x8, [sp, #0x38]
100445d90: b85503a8    	ldur	w8, [x29, #-0xb0]
100445d94: 7100091f    	cmp	w8, #0x2
100445d98: 540000e0    	b.eq	0x100445db4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xac0>
100445d9c: f85403a0    	ldur	x0, [x29, #-0xc0]
100445da0: f9400008    	ldr	x8, [x0]
100445da4: f1000508    	subs	x8, x8, #0x1
100445da8: f9000008    	str	x8, [x0]
100445dac: 54000041    	b.ne	0x100445db4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xac0>
100445db0: 97effbdb    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
100445db4: 71002e9f    	cmp	w20, #0xb
100445db8: 5402ae60    	b.eq	0x10044b384 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6090>
100445dbc: b942e3e8    	ldr	w8, [sp, #0x2e0]
100445dc0: b90193e8    	str	w8, [sp, #0x190]
100445dc4: 910923e8    	add	x8, sp, #0x248
100445dc8: b849b108    	ldur	w8, [x8, #0x9b]
100445dcc: 910253e9    	add	x9, sp, #0x94
100445dd0: b80ff128    	stur	w8, [x9, #0xff]
100445dd4: 71002a9f    	cmp	w20, #0xa
100445dd8: 540191c1    	b.ne	0x100449010 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d1c>
100445ddc: 71034b7f    	cmp	w27, #0xd2
100445de0: 5400ade1    	b.ne	0x10044739c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x20a8>
100445de4: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100445de8: f9408be1    	ldr	x1, [sp, #0x110]
100445dec: 52800002    	mov	w2, #0x0                ; =0
100445df0: 9400246e    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100445df4: f9408be4    	ldr	x4, [sp, #0x110]
100445df8: f94083e3    	ldr	x3, [sp, #0x100]
100445dfc: 910923eb    	add	x11, sp, #0x248
100445e00: b40172a0    	cbz	x0, 0x100448c54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3960>
100445e04: 39400008    	ldrb	w8, [x0]
100445e08: 71001d1f    	cmp	w8, #0x7
100445e0c: 54017248    	b.hi	0x100448c54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3960>
100445e10: 52800029    	mov	w9, #0x1                ; =1
100445e14: 1ac82129    	lsl	w9, w9, w8
100445e18: 5280138a    	mov	w10, #0x9c              ; =156
100445e1c: 6a0a013f    	tst	w9, w10
100445e20: 54017000    	b.eq	0x100448c20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x392c>
100445e24: f8401009    	ldur	x9, [x0, #0x1]
100445e28: f90173e9    	str	x9, [sp, #0x2e0]
100445e2c: f9400409    	ldr	x9, [x0, #0x8]
100445e30: f809f169    	stur	x9, [x11, #0x9f]
100445e34: 14000b7d    	b	0x100448c28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3934>
100445e38: 7101b77f    	cmp	w27, #0x6d
100445e3c: 1a9f17e9    	cset	w9, eq
100445e40: f9408be8    	ldr	x8, [sp, #0x110]
100445e44: f9402108    	ldr	x8, [x8, #0x40]
100445e48: eb09011f    	cmp	x8, x9
100445e4c: 5402b1e9    	b.ls	0x10044b488 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6194>
100445e50: f94083ec    	ldr	x12, [sp, #0x100]
100445e54: f940099c    	ldr	x28, [x12, #0x10]
100445e58: f94077ea    	ldr	x10, [sp, #0xe8]
100445e5c: f940014a    	ldr	x10, [x10]
100445e60: aa2903e9    	mvn	x9, x9
100445e64: 8b0a010b    	add	x11, x8, x10
100445e68: 8b090176    	add	x22, x11, x9
100445e6c: eb1c02df    	cmp	x22, x28
100445e70: 5402ce22    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100445e74: f9400589    	ldr	x9, [x12, #0x8]
100445e78: d37ceecb    	lsl	x11, x22, #4
100445e7c: 386b692b    	ldrb	w11, [x9, x11]
100445e80: 5100296c    	sub	w12, w11, #0xa
100445e84: 7100119f    	cmp	w12, #0x4
100445e88: 54026a29    	b.ls	0x10044abcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58d8>
100445e8c: 7100217f    	cmp	w11, #0x8
100445e90: 540037c2    	b.hs	0x100446588 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1294>
100445e94: 528013ec    	mov	w12, #0x9f              ; =159
100445e98: 1acb258b    	lsr	w11, w12, w11
100445e9c: 3600376b    	tbz	w11, #0x0, 0x100446588 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1294>
100445ea0: 5280002c    	mov	w12, #0x1               ; =1
100445ea4: 140001bd    	b	0x100446598 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x12a4>
100445ea8: f9407fe8    	ldr	x8, [sp, #0xf8]
100445eac: 12003d13    	and	w19, w8, #0xffff
100445eb0: f94067e8    	ldr	x8, [sp, #0xc8]
100445eb4: f9400108    	ldr	x8, [x8]
100445eb8: f9406fe9    	ldr	x9, [sp, #0xd8]
100445ebc: f9400129    	ldr	x9, [x9]
100445ec0: eb080129    	subs	x9, x9, x8
100445ec4: 9a8933e9    	csel	x9, xzr, x9, lo
100445ec8: eb13013f    	cmp	x9, x19
100445ecc: 54026449    	b.ls	0x10044ab54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5860>
100445ed0: f94083ec    	ldr	x12, [sp, #0x100]
100445ed4: f940099c    	ldr	x28, [x12, #0x10]
100445ed8: 8b130116    	add	x22, x8, x19
100445edc: eb1c02df    	cmp	x22, x28
100445ee0: f9408be2    	ldr	x2, [sp, #0x110]
100445ee4: f9407bed    	ldr	x13, [sp, #0xf0]
100445ee8: 5402ce62    	b.hs	0x10044b8b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65c0>
100445eec: f9400580    	ldr	x0, [x12, #0x8]
100445ef0: d37ceec8    	lsl	x8, x22, #4
100445ef4: 38686808    	ldrb	w8, [x0, x8]
100445ef8: 7100391f    	cmp	w8, #0xe
100445efc: 54026aa0    	b.eq	0x10044ac50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x595c>
100445f00: 51002909    	sub	w9, w8, #0xa
100445f04: d100250a    	sub	x10, x8, #0x9
100445f08: 7100113f    	cmp	w9, #0x4
100445f0c: 9a9f3149    	csel	x9, x10, xzr, lo
100445f10: d100052a    	sub	x10, x9, #0x1
100445f14: f1000d5f    	cmp	x10, #0x3
100445f18: 910923ee    	add	x14, sp, #0x248
100445f1c: 5400f562    	b.hs	0x100447dc8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2ad4>
100445f20: f9408fe8    	ldr	x8, [sp, #0x118]
100445f24: f9401108    	ldr	x8, [x8, #0x20]
100445f28: b4024448    	cbz	x8, 0x10044a7b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54bc>
100445f2c: 7100bf7f    	cmp	w27, #0x2f
100445f30: 5400b241    	b.ne	0x100447578 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2284>
100445f34: f9402048    	ldr	x8, [x2, #0x40]
100445f38: b402afe8    	cbz	x8, 0x10044b534 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6240>
100445f3c: f940099c    	ldr	x28, [x12, #0x10]
100445f40: f94077e9    	ldr	x9, [sp, #0xe8]
100445f44: f9400129    	ldr	x9, [x9]
100445f48: 8b090108    	add	x8, x8, x9
100445f4c: d1000516    	sub	x22, x8, #0x1
100445f50: eb1c02df    	cmp	x22, x28
100445f54: 5402c702    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100445f58: f9400588    	ldr	x8, [x12, #0x8]
100445f5c: 8b161102    	add	x2, x8, x22, lsl #4
100445f60: 39400048    	ldrb	w8, [x2]
100445f64: 51002909    	sub	w9, w8, #0xa
100445f68: 7100113f    	cmp	w9, #0x4
100445f6c: 54028e29    	b.ls	0x10044b130 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e3c>
100445f70: 71001d1f    	cmp	w8, #0x7
100445f74: 540188c8    	b.hi	0x10044908c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d98>
100445f78: 52800029    	mov	w9, #0x1                ; =1
100445f7c: 1ac82129    	lsl	w9, w9, w8
100445f80: 5280138a    	mov	w10, #0x9c              ; =156
100445f84: 6a0a013f    	tst	w9, w10
100445f88: 54012be0    	b.eq	0x100448504 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3210>
100445f8c: f8401049    	ldur	x9, [x2, #0x1]
100445f90: f81203a9    	stur	x9, [x29, #-0xe0]
100445f94: f9400449    	ldr	x9, [x2, #0x8]
100445f98: f80ff1c9    	stur	x9, [x14, #0xff]
100445f9c: 1400095c    	b	0x10044850c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3218>
100445fa0: 52800028    	mov	w8, #0x1                ; =1
100445fa4: 6a53711f    	tst	w8, w19, lsr #28
100445fa8: 9a880508    	cinc	x8, x8, ne
100445fac: 2a1903f4    	mov	w20, w25
100445fb0: 8b140116    	add	x22, x8, x20
100445fb4: eb1c02df    	cmp	x22, x28
100445fb8: 5402c562    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100445fbc: f9408be8    	ldr	x8, [sp, #0x110]
100445fc0: f9402108    	ldr	x8, [x8, #0x40]
100445fc4: b1000d1f    	cmn	x8, #0x3
100445fc8: 54001b48    	b.hi	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
100445fcc: 91000908    	add	x8, x8, #0x2
100445fd0: f9408bea    	ldr	x10, [sp, #0x110]
100445fd4: a943254a    	ldp	x10, x9, [x10, #0x30]
100445fd8: cb0a0129    	sub	x9, x9, x10
100445fdc: eb09011f    	cmp	x8, x9
100445fe0: 54001a88    	b.hi	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
100445fe4: b8767b09    	ldr	w9, [x24, x22, lsl #2]
100445fe8: 7210013f    	tst	w9, #0x10000
100445fec: f94067e8    	ldr	x8, [sp, #0xc8]
100445ff0: f9406feb    	ldr	x11, [sp, #0xd8]
100445ff4: 9a880168    	csel	x8, x11, x8, eq
100445ff8: f94077ea    	ldr	x10, [sp, #0xe8]
100445ffc: 9a8b014a    	csel	x10, x10, x11, eq
100446000: f9400141    	ldr	x1, [x10]
100446004: f9400116    	ldr	x22, [x8]
100446008: eb16002a    	subs	x10, x1, x22
10044600c: 5402a1a3    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100446010: f94083e8    	ldr	x8, [sp, #0x100]
100446014: f9400908    	ldr	x8, [x8, #0x10]
100446018: eb08003f    	cmp	x1, x8
10044601c: 5402ace8    	b.hi	0x10044b5b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62c4>
100446020: 92403d2b    	and	x11, x9, #0xffff
100446024: eb0b015f    	cmp	x10, x11
100446028: 54001849    	b.ls	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
10044602c: f94083e9    	ldr	x9, [sp, #0x100]
100446030: f9400529    	ldr	x9, [x9, #0x8]
100446034: 8b16112a    	add	x10, x9, x22, lsl #4
100446038: 8b0b114a    	add	x10, x10, x11, lsl #4
10044603c: 3940014b    	ldrb	w11, [x10]
100446040: 71000d7f    	cmp	w11, #0x3
100446044: 54001761    	b.ne	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
100446048: b9400543    	ldr	w3, [x10, #0x4]
10044604c: 37f81723    	tbnz	w3, #0x1f, 0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
100446050: 7103437f    	cmp	w27, #0xd0
100446054: f94067ea    	ldr	x10, [sp, #0xc8]
100446058: f9406fec    	ldr	x12, [sp, #0xd8]
10044605c: 9a8a018a    	csel	x10, x12, x10, eq
100446060: f94077eb    	ldr	x11, [sp, #0xe8]
100446064: 9a8c016b    	csel	x11, x11, x12, eq
100446068: f9400161    	ldr	x1, [x11]
10044606c: f9400156    	ldr	x22, [x10]
100446070: eb16002a    	subs	x10, x1, x22
100446074: 54029e63    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100446078: eb08003f    	cmp	x1, x8
10044607c: 5402a9e8    	b.hi	0x10044b5b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62c4>
100446080: f9407fe8    	ldr	x8, [sp, #0xf8]
100446084: 12003d08    	and	w8, w8, #0xffff
100446088: eb08015f    	cmp	x10, x8
10044608c: 54001529    	b.ls	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
100446090: 8b161129    	add	x9, x9, x22, lsl #4
100446094: 8b081122    	add	x2, x9, x8, lsl #4
100446098: 39400048    	ldrb	w8, [x2]
10044609c: 7100251f    	cmp	w8, #0x9
1004460a0: 54001488    	b.hi	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
1004460a4: f9407be8    	ldr	x8, [sp, #0xf0]
1004460a8: f9400101    	ldr	x1, [x8]
1004460ac: d10303a0    	sub	x0, x29, #0xc0
1004460b0: 9400318d    	bl	0x1004526e4 <__ZN13quickjs_oxide6engine6object16ordinary_storage62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$17peek_dense_number17h2449f7698d3d5da4E>
1004460b4: b85403a8    	ldur	w8, [x29, #-0xc0]
1004460b8: 7100091f    	cmp	w8, #0x2
1004460bc: 540013a0    	b.eq	0x100446330 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x103c>
1004460c0: f85483a9    	ldur	x9, [x29, #-0xb8]
1004460c4: 71000d1f    	cmp	w8, #0x3
1004460c8: 54025420    	b.eq	0x10044ab4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5858>
1004460cc: 36021fc8    	tbz	w8, #0x0, 0x10044a4c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x51d0>
1004460d0: f81483a9    	stur	x9, [x29, #-0xb8]
1004460d4: 52800088    	mov	w8, #0x4                ; =4
1004460d8: 140010fe    	b	0x10044a4d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x51dc>
1004460dc: 52800028    	mov	w8, #0x1                ; =1
1004460e0: 6a53711f    	tst	w8, w19, lsr #28
1004460e4: 9a880509    	cinc	x9, x8, ne
1004460e8: 2a1903e8    	mov	w8, w25
1004460ec: 8b080136    	add	x22, x9, x8
1004460f0: eb1c02df    	cmp	x22, x28
1004460f4: 5402bb82    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
1004460f8: f9408be9    	ldr	x9, [sp, #0x110]
1004460fc: f9402129    	ldr	x9, [x9, #0x40]
100446100: b1000d3f    	cmn	x9, #0x3
100446104: 5401ede8    	b.hi	0x100449ec0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bcc>
100446108: 91000929    	add	x9, x9, #0x2
10044610c: f9408beb    	ldr	x11, [sp, #0x110]
100446110: a943296b    	ldp	x11, x10, [x11, #0x30]
100446114: cb0b014a    	sub	x10, x10, x11
100446118: eb0a013f    	cmp	x9, x10
10044611c: 5401ed28    	b.hi	0x100449ec0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bcc>
100446120: 71038b7f    	cmp	w27, #0xe2
100446124: f94067e9    	ldr	x9, [sp, #0xc8]
100446128: f9406feb    	ldr	x11, [sp, #0xd8]
10044612c: 9a890169    	csel	x9, x11, x9, eq
100446130: f94077ea    	ldr	x10, [sp, #0xe8]
100446134: 9a8b014a    	csel	x10, x10, x11, eq
100446138: f9400141    	ldr	x1, [x10]
10044613c: f940012b    	ldr	x11, [x9]
100446140: eb0b002c    	subs	x12, x1, x11
100446144: 5402a423    	b.lo	0x10044b5c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62d4>
100446148: f94083ea    	ldr	x10, [sp, #0x100]
10044614c: f9400949    	ldr	x9, [x10, #0x10]
100446150: eb09003f    	cmp	x1, x9
100446154: 54029c48    	b.hi	0x10044b4dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61e8>
100446158: f9407fed    	ldr	x13, [sp, #0xf8]
10044615c: 12003dad    	and	w13, w13, #0xffff
100446160: f940054a    	ldr	x10, [x10, #0x8]
100446164: 2f00e400    	movi	d0, #0000000000000000
100446168: eb0d019f    	cmp	x12, x13
10044616c: 5400a709    	b.ls	0x10044764c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2358>
100446170: 8b0b114b    	add	x11, x10, x11, lsl #4
100446174: 8b0d116c    	add	x12, x11, x13, lsl #4
100446178: 3940018b    	ldrb	w11, [x12]
10044617c: 7100257f    	cmp	w11, #0x9
100446180: 5400a668    	b.hi	0x10044764c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2358>
100446184: 71000d7f    	cmp	w11, #0x3
100446188: 5401d9c0    	b.eq	0x100449cc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49cc>
10044618c: 7100117f    	cmp	w11, #0x4
100446190: 5400a5e1    	b.ne	0x10044764c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2358>
100446194: 5280000b    	mov	w11, #0x0               ; =0
100446198: fd400580    	ldr	d0, [x12, #0x8]
10044619c: 1400052d    	b	0x100447650 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x235c>
1004461a0: 52800028    	mov	w8, #0x1                ; =1
1004461a4: 6a53711f    	tst	w8, w19, lsr #28
1004461a8: 9a88050a    	cinc	x10, x8, ne
1004461ac: 2a1903e9    	mov	w9, w25
1004461b0: 8b090156    	add	x22, x10, x9
1004461b4: eb1c02df    	cmp	x22, x28
1004461b8: 5402b562    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
1004461bc: 6a53711f    	tst	w8, w19, lsr #28
1004461c0: 52800048    	mov	w8, #0x2                ; =2
1004461c4: 9a880508    	cinc	x8, x8, ne
1004461c8: 8b090108    	add	x8, x8, x9
1004461cc: eb1c011f    	cmp	x8, x28
1004461d0: 5402b682    	b.hs	0x10044b8a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65ac>
1004461d4: b8687b08    	ldr	w8, [x24, x8, lsl #2]
1004461d8: 92402509    	and	x9, x8, #0x3ff
1004461dc: 71038d3f    	cmp	w9, #0xe3
1004461e0: 5402b828    	b.hi	0x10044b8e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65f0>
1004461e4: b8767b07    	ldr	w7, [x24, x22, lsl #2]
1004461e8: f00009ea    	adrp	x10, 0x100585000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16ce8>
1004461ec: 912b814a    	add	x10, x10, #0xae0
1004461f0: 78697949    	ldrh	w9, [x10, x9, lsl #1]
1004461f4: 53107cea    	lsr	w10, w7, #16
1004461f8: 71037b7f    	cmp	w27, #0xde
1004461fc: 1a9f07e4    	cset	w4, ne
100446200: 530a2906    	ubfx	w6, w8, #10, #1
100446204: 530b3108    	ubfx	w8, w8, #11, #2
100446208: a94f17eb    	ldp	x11, x5, [sp, #0xf0]
10044620c: f9400163    	ldr	x3, [x11]
100446210: 79000fe9    	strh	w9, [sp, #0x6]
100446214: 79000bea    	strh	w10, [sp, #0x4]
100446218: 910943e0    	add	x0, sp, #0x250
10044621c: b90003e8    	str	w8, [sp]
100446220: f94083e1    	ldr	x1, [sp, #0x100]
100446224: f9408be2    	ldr	x2, [sp, #0x110]
100446228: 94003352    	bl	0x100452f70 <__ZN13quickjs_oxide6engine2vm7execute22try_dense_index_binary17h96cf7ae8d1c87a34E>
10044622c: b94253e8    	ldr	w8, [sp, #0x250]
100446230: 7100091f    	cmp	w8, #0x2
100446234: 54007bc0    	b.eq	0x1004471ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1eb8>
100446238: f9412fe9    	ldr	x9, [sp, #0x258]
10044623c: 71000d1f    	cmp	w8, #0x3
100446240: 54024860    	b.eq	0x10044ab4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5858>
100446244: f9408be1    	ldr	x1, [sp, #0x110]
100446248: f94083e0    	ldr	x0, [sp, #0x100]
10044624c: 3600d0e8    	tbz	w8, #0x0, 0x100447c68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2974>
100446250: f81483a9    	stur	x9, [x29, #-0xb8]
100446254: 52800088    	mov	w8, #0x4                ; =4
100446258: 14000687    	b	0x100447c74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2980>
10044625c: 52800028    	mov	w8, #0x1                ; =1
100446260: 6a53711f    	tst	w8, w19, lsr #28
100446264: 9a88050a    	cinc	x10, x8, ne
100446268: 2a1903e9    	mov	w9, w25
10044626c: 8b090156    	add	x22, x10, x9
100446270: eb1c02df    	cmp	x22, x28
100446274: 5402af82    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100446278: 6a53711f    	tst	w8, w19, lsr #28
10044627c: 52800048    	mov	w8, #0x2                ; =2
100446280: 9a880508    	cinc	x8, x8, ne
100446284: 8b090108    	add	x8, x8, x9
100446288: eb1c011f    	cmp	x8, x28
10044628c: 5402b0a2    	b.hs	0x10044b8a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65ac>
100446290: b8687b08    	ldr	w8, [x24, x8, lsl #2]
100446294: 92402509    	and	x9, x8, #0x3ff
100446298: 71038d3f    	cmp	w9, #0xe3
10044629c: 5402b1e8    	b.hi	0x10044b8d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65e4>
1004462a0: b8767b07    	ldr	w7, [x24, x22, lsl #2]
1004462a4: f00009ea    	adrp	x10, 0x100585000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16ce8>
1004462a8: 912b814a    	add	x10, x10, #0xae0
1004462ac: 78697949    	ldrh	w9, [x10, x9, lsl #1]
1004462b0: 53107cea    	lsr	w10, w7, #16
1004462b4: 7103737f    	cmp	w27, #0xdc
1004462b8: 1a9f07e4    	cset	w4, ne
1004462bc: 530a2906    	ubfx	w6, w8, #10, #1
1004462c0: 530b3108    	ubfx	w8, w8, #11, #2
1004462c4: a94f17eb    	ldp	x11, x5, [sp, #0xf0]
1004462c8: f9400163    	ldr	x3, [x11]
1004462cc: 79000fe9    	strh	w9, [sp, #0x6]
1004462d0: 79000bea    	strh	w10, [sp, #0x4]
1004462d4: 910983e0    	add	x0, sp, #0x260
1004462d8: b90003e8    	str	w8, [sp]
1004462dc: f94083e1    	ldr	x1, [sp, #0x100]
1004462e0: f9408be2    	ldr	x2, [sp, #0x110]
1004462e4: 9400327e    	bl	0x100452cdc <__ZN13quickjs_oxide6engine2vm7execute21try_dense_read_binary17h0e2ade1b8cce7e9cE>
1004462e8: 394983e9    	ldrb	w9, [sp, #0x260]
1004462ec: 7100293f    	cmp	w9, #0xa
1004462f0: 540078c0    	b.eq	0x100447208 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1f14>
1004462f4: f94137e8    	ldr	x8, [sp, #0x268]
1004462f8: 71002d3f    	cmp	w9, #0xb
1004462fc: f9408be1    	ldr	x1, [sp, #0x110]
100446300: f94083e0    	ldr	x0, [sp, #0x100]
100446304: 54026260    	b.eq	0x10044af50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c5c>
100446308: 390603e9    	strb	w9, [sp, #0x180]
10044630c: a9492beb    	ldp	x11, x10, [sp, #0x90]
100446310: b9400149    	ldr	w9, [x10]
100446314: b9000169    	str	w9, [x11]
100446318: b8403149    	ldur	w9, [x10, #0x3]
10044631c: b8003169    	stur	w9, [x11, #0x3]
100446320: f900c7e8    	str	x8, [sp, #0x188]
100446324: 910603e2    	add	x2, sp, #0x180
100446328: 94002302    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044632c: 14000655    	b	0x100447c80 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x298c>
100446330: 7103437f    	cmp	w27, #0xd0
100446334: 54007121    	b.ne	0x100447158 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1e64>
100446338: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
10044633c: f9408be1    	ldr	x1, [sp, #0x110]
100446340: 52800002    	mov	w2, #0x0                ; =0
100446344: 94002319    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100446348: f9408be4    	ldr	x4, [sp, #0x110]
10044634c: f94083e3    	ldr	x3, [sp, #0x100]
100446350: 910923eb    	add	x11, sp, #0x248
100446354: b4013e60    	cbz	x0, 0x100448b20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x382c>
100446358: 39400008    	ldrb	w8, [x0]
10044635c: 71001d1f    	cmp	w8, #0x7
100446360: 54013e08    	b.hi	0x100448b20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x382c>
100446364: 52800029    	mov	w9, #0x1                ; =1
100446368: 1ac82129    	lsl	w9, w9, w8
10044636c: 5280138a    	mov	w10, #0x9c              ; =156
100446370: 6a0a013f    	tst	w9, w10
100446374: 54013bc0    	b.eq	0x100448aec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37f8>
100446378: f8401009    	ldur	x9, [x0, #0x1]
10044637c: f90173e9    	str	x9, [sp, #0x2e0]
100446380: f9400409    	ldr	x9, [x0, #0x8]
100446384: f809f169    	stur	x9, [x11, #0x9f]
100446388: 140009db    	b	0x100448af4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3800>
10044638c: aa0b03f3    	mov	x19, x11
100446390: 140004f1    	b	0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
100446394: 3600996a    	tbz	w10, #0x0, 0x1004476c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x23cc>
100446398: 1e614108    	fneg	d8, d8
10044639c: 5280002a    	mov	w10, #0x1               ; =1
1004463a0: 140004ed    	b	0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
1004463a4: 5101cf6c    	sub	w12, w27, #0x73
1004463a8: 121e798c    	and	w12, w12, #0xfffffffd
1004463ac: 7100056e    	subs	w14, w11, #0x1
1004463b0: 1a9f77ef    	cset	w15, vs
1004463b4: 3100056b    	adds	w11, w11, #0x1
1004463b8: 1a9f77f0    	cset	w16, vs
1004463bc: 72001d9f    	tst	w12, #0xff
1004463c0: 1a8f020f    	csel	w15, w16, w15, eq
1004463c4: 1e7e1001    	fmov	d1, #-1.00000000
1004463c8: 1e6e1002    	fmov	d2, #1.00000000
1004463cc: 1e610c43    	fcsel	d3, d2, d1, eq
1004463d0: 1e602860    	fadd	d0, d3, d0
1004463d4: 52800030    	mov	w16, #0x1               ; =1
1004463d8: 72001d9f    	tst	w12, #0xff
1004463dc: 1a8e016b    	csel	w11, w11, w14, eq
1004463e0: 720001ff    	tst	w15, #0x1
1004463e4: 1e601c00    	fcsel	d0, d0, d0, ne
1004463e8: 1a8b110b    	csel	w11, w8, w11, ne
1004463ec: 1a9f120e    	csel	w14, w16, wzr, ne
1004463f0: 72001d9f    	tst	w12, #0xff
1004463f4: 1e610c41    	fcsel	d1, d2, d1, eq
1004463f8: 1e682821    	fadd	d1, d1, d8
1004463fc: 5280002c    	mov	w12, #0x1               ; =1
100446400: 7100015f    	cmp	w10, #0x0
100446404: 1a880173    	csel	w19, w11, w8, eq
100446408: 1e610c08    	fcsel	d8, d0, d1, eq
10044640c: 1a8c01ca    	csel	w10, w14, w12, eq
100446410: 12001dab    	and	w11, w13, #0xff
100446414: 7100057f    	cmp	w11, #0x1
100446418: 540099e8    	b.hi	0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
10044641c: 140004d2    	b	0x100447764 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2470>
100446420: f9408be8    	ldr	x8, [sp, #0x110]
100446424: f9402108    	ldr	x8, [x8, #0x40]
100446428: f100050b    	subs	x11, x8, #0x1
10044642c: f94083ea    	ldr	x10, [sp, #0x100]
100446430: 540288e9    	b.ls	0x10044b54c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6258>
100446434: f94077e9    	ldr	x9, [sp, #0xe8]
100446438: f9400129    	ldr	x9, [x9]
10044643c: 8b080121    	add	x1, x9, x8
100446440: d1000836    	sub	x22, x1, #0x2
100446444: eb16003f    	cmp	x1, x22
100446448: 540288a3    	b.lo	0x10044b55c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6268>
10044644c: f90017eb    	str	x11, [sp, #0x28]
100446450: f940095c    	ldr	x28, [x10, #0x10]
100446454: eb1c003f    	cmp	x1, x28
100446458: 54028888    	b.hi	0x10044b568 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6274>
10044645c: f9400548    	ldr	x8, [x10, #0x8]
100446460: f90013e8    	str	x8, [sp, #0x20]
100446464: 8b16110a    	add	x10, x8, x22, lsl #4
100446468: 39400148    	ldrb	w8, [x10]
10044646c: 7100391f    	cmp	w8, #0xe
100446470: 54022080    	b.eq	0x10044a880 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x558c>
100446474: 7100251f    	cmp	w8, #0x9
100446478: 54022048    	b.hi	0x10044a880 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x558c>
10044647c: 38410149    	ldurb	w9, [x10, #0x10]
100446480: 71000d3f    	cmp	w9, #0x3
100446484: f9001fea    	str	x10, [sp, #0x38]
100446488: 5400a440    	b.eq	0x100447910 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x261c>
10044648c: 7100153f    	cmp	w9, #0x5
100446490: 54021f21    	b.ne	0x10044a874 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5580>
100446494: f9407be8    	ldr	x8, [sp, #0xf0]
100446498: f9400101    	ldr	x1, [x8]
10044649c: d10303a0    	sub	x0, x29, #0xc0
1004464a0: 91004142    	add	x2, x10, #0x10
1004464a4: f9007fe1    	str	x1, [sp, #0xf8]
1004464a8: 94002ee0    	bl	0x100452028 <__ZN13quickjs_oxide6engine4heap14slot_ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$36slot_value_release_readiness_jsvalue17h2038e983b6e2b9bfE>
1004464ac: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004464b0: 71002d1f    	cmp	w8, #0xb
1004464b4: 54024081    	b.ne	0x10044acc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59d0>
1004464b8: 385413a9    	ldurb	w9, [x29, #-0xbf]
1004464bc: 35024049    	cbnz	w9, 0x10044acc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59d0>
1004464c0: f9407fea    	ldr	x10, [sp, #0xf8]
1004464c4: f9401548    	ldr	x8, [x10, #0x28]
1004464c8: b27ff7e9    	mov	x9, #0x7ffffffffffffffe ; =9223372036854775806
1004464cc: eb09011f    	cmp	x8, x9
1004464d0: f9401fe9    	ldr	x9, [sp, #0x38]
1004464d4: 5402a9c8    	b.hi	0x10044ba0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6718>
1004464d8: 91000508    	add	x8, x8, #0x1
1004464dc: f9001548    	str	x8, [x10, #0x28]
1004464e0: b9401520    	ldr	w0, [x9, #0x14]
1004464e4: f9409941    	ldr	x1, [x10, #0x130]
1004464e8: eb00003f    	cmp	x1, x0
1004464ec: 5402a969    	b.ls	0x10044ba18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6724>
1004464f0: f9409548    	ldr	x8, [x10, #0x128]
1004464f4: 52800309    	mov	w9, #0x18               ; =24
1004464f8: 9ba92008    	umaddl	x8, w0, w9, x8
1004464fc: f9400109    	ldr	x9, [x8]
100446500: 927f052a    	and	x10, x9, #0x6
100446504: f100095f    	cmp	x10, #0x2
100446508: 540284c0    	b.eq	0x10044b5a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62ac>
10044650c: b940150a    	ldr	w10, [x8, #0x14]
100446510: 3402848a    	cbz	w10, 0x10044b5a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62ac>
100446514: f100113f    	cmp	x9, #0x4
100446518: 54029661    	b.ne	0x10044b7e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64f0>
10044651c: f9400509    	ldr	x9, [x8, #0x8]
100446520: f940012a    	ldr	x10, [x9]
100446524: b100054a    	adds	x10, x10, #0x1
100446528: f900012a    	str	x10, [x9]
10044652c: 5402a962    	b.hs	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
100446530: f9400514    	ldr	x20, [x8, #0x8]
100446534: f90183f4    	str	x20, [sp, #0x300]
100446538: f9407fe9    	ldr	x9, [sp, #0xf8]
10044653c: f9401528    	ldr	x8, [x9, #0x28]
100446540: d1000508    	sub	x8, x8, #0x1
100446544: f9001528    	str	x8, [x9, #0x28]
100446548: aa1403e0    	mov	x0, x20
10044654c: 97f103d0    	bl	0x10008748c <__ZN13quickjs_oxide6engine4atom29parse_canonical_u32_js_string17hf7015ef7cc8bce8eE>
100446550: b90033e1    	str	w1, [sp, #0x30]
100446554: f9400288    	ldr	x8, [x20]
100446558: d1000508    	sub	x8, x8, #0x1
10044655c: f9000288    	str	x8, [x20]
100446560: 360260e0    	tbz	w0, #0x0, 0x10044b17c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e88>
100446564: b94033e9    	ldr	w9, [sp, #0x30]
100446568: 3100053f    	cmn	w9, #0x1
10044656c: 54026080    	b.eq	0x10044b17c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e88>
100446570: b5000068    	cbnz	x8, 0x10044657c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1288>
100446574: 910c03e0    	add	x0, sp, #0x300
100446578: 97efac74    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044657c: f9401fe8    	ldr	x8, [sp, #0x38]
100446580: 39400108    	ldrb	w8, [x8]
100446584: 140004e9    	b	0x100447928 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2634>
100446588: f9408feb    	ldr	x11, [sp, #0x118]
10044658c: f940116b    	ldr	x11, [x11, #0x20]
100446590: b402110b    	cbz	x11, 0x10044a7b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54bc>
100446594: 5280000c    	mov	w12, #0x0               ; =0
100446598: 7101b37f    	cmp	w27, #0x6c
10044659c: 540067a1    	b.ne	0x100447290 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1f9c>
1004465a0: d100050b    	sub	x11, x8, #0x1
1004465a4: 8b0b0156    	add	x22, x10, x11
1004465a8: eb1c02df    	cmp	x22, x28
1004465ac: 54029442    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
1004465b0: 8b161128    	add	x8, x9, x22, lsl #4
1004465b4: 39400118    	ldrb	w24, [x8]
1004465b8: 51002b09    	sub	w9, w24, #0xa
1004465bc: 7100153f    	cmp	w9, #0x5
1004465c0: 54021d03    	b.lo	0x10044a960 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x566c>
1004465c4: f9408be9    	ldr	x9, [sp, #0x110]
1004465c8: f900212b    	str	x11, [x9, #0x40]
1004465cc: b8401109    	ldur	w9, [x8, #0x1]
1004465d0: b902e3e9    	str	w9, [sp, #0x2e0]
1004465d4: b9400509    	ldr	w9, [x8, #0x4]
1004465d8: 910923ea    	add	x10, sp, #0x248
1004465dc: b809b149    	stur	w9, [x10, #0x9b]
1004465e0: f9400513    	ldr	x19, [x8, #0x8]
1004465e4: 528001c9    	mov	w9, #0xe                ; =14
1004465e8: 39000109    	strb	w9, [x8]
1004465ec: 14000667    	b	0x100447f88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2c94>
1004465f0: b1000d1f    	cmn	x8, #0x3
1004465f4: f94083e0    	ldr	x0, [sp, #0x100]
1004465f8: 54000428    	b.hi	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
1004465fc: 91000908    	add	x8, x8, #0x2
100446600: a94325a1    	ldp	x1, x9, [x13, #0x30]
100446604: cb010129    	sub	x9, x9, x1
100446608: eb09011f    	cmp	x8, x9
10044660c: 54000388    	b.hi	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
100446610: f9406fe8    	ldr	x8, [sp, #0xd8]
100446614: f9400116    	ldr	x22, [x8]
100446618: eb160028    	subs	x8, x1, x22
10044661c: 54027123    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100446620: f940081c    	ldr	x28, [x0, #0x10]
100446624: eb1c003f    	cmp	x1, x28
100446628: 54027e08    	b.hi	0x10044b5e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62f4>
10044662c: eb13011f    	cmp	x8, x19
100446630: 54000269    	b.ls	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
100446634: f9400408    	ldr	x8, [x0, #0x8]
100446638: 8b161108    	add	x8, x8, x22, lsl #4
10044663c: 8b131108    	add	x8, x8, x19, lsl #4
100446640: 39400109    	ldrb	w9, [x8]
100446644: 7100253f    	cmp	w9, #0x9
100446648: 540001a8    	b.hi	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
10044664c: 71000d3f    	cmp	w9, #0x3
100446650: 5401b0c0    	b.eq	0x100449c68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4974>
100446654: 7100113f    	cmp	w9, #0x4
100446658: 54000121    	b.ne	0x10044667c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1388>
10044665c: fd400500    	ldr	d0, [x8, #0x8]
100446660: f9407fe8    	ldr	x8, [sp, #0xf8]
100446664: 7213011f    	tst	w8, #0x2000
100446668: 1e6e1001    	fmov	d1, #1.00000000
10044666c: 1e7e1002    	fmov	d2, #-1.00000000
100446670: 1e610c41    	fcsel	d1, d2, d1, eq
100446674: 1e602820    	fadd	d0, d1, d0
100446678: 14000d8a    	b	0x100449ca0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49ac>
10044667c: 7103637f    	cmp	w27, #0xd8
100446680: 54007521    	b.ne	0x100447524 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2230>
100446684: aa0d03e1    	mov	x1, x13
100446688: 52800002    	mov	w2, #0x0                ; =0
10044668c: aa1303e3    	mov	x3, x19
100446690: 94002246    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100446694: f9408be4    	ldr	x4, [sp, #0x110]
100446698: f94083e3    	ldr	x3, [sp, #0x100]
10044669c: 910923eb    	add	x11, sp, #0x248
1004466a0: b4013960    	cbz	x0, 0x100448dcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ad8>
1004466a4: 39400008    	ldrb	w8, [x0]
1004466a8: 71001d1f    	cmp	w8, #0x7
1004466ac: 54013908    	b.hi	0x100448dcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ad8>
1004466b0: 52800029    	mov	w9, #0x1                ; =1
1004466b4: 1ac82129    	lsl	w9, w9, w8
1004466b8: 5280138a    	mov	w10, #0x9c              ; =156
1004466bc: 6a0a013f    	tst	w9, w10
1004466c0: 540136c0    	b.eq	0x100448d98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3aa4>
1004466c4: f8401009    	ldur	x9, [x0, #0x1]
1004466c8: f90173e9    	str	x9, [sp, #0x2e0]
1004466cc: f9400409    	ldr	x9, [x0, #0x8]
1004466d0: f809f169    	stur	x9, [x11, #0x9f]
1004466d4: 140009b3    	b	0x100448da0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3aac>
1004466d8: d2800003    	mov	x3, #0x0                ; =0
1004466dc: f9408be1    	ldr	x1, [sp, #0x110]
1004466e0: f94083e0    	ldr	x0, [sp, #0x100]
1004466e4: aa1403e2    	mov	x2, x20
1004466e8: aa0303e4    	mov	x4, x3
1004466ec: 9400246c    	bl	0x10044f89c <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots11insert_copy17hb51f0c3ae63d8d95E>
1004466f0: 14000e86    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004466f4: f940202a    	ldr	x10, [x1, #0x40]
1004466f8: f1000d5f    	cmp	x10, #0x3
1004466fc: 5401f863    	b.lo	0x10044a608 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5314>
100446700: f940081c    	ldr	x28, [x0, #0x10]
100446704: f9401828    	ldr	x8, [x1, #0x30]
100446708: d1000d18    	sub	x24, x8, #0x3
10044670c: 8b0a0316    	add	x22, x24, x10
100446710: eb1c02df    	cmp	x22, x28
100446714: 54028902    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100446718: f940040b    	ldr	x11, [x0, #0x8]
10044671c: 8b161162    	add	x2, x11, x22, lsl #4
100446720: 3940005b    	ldrb	w27, [x2]
100446724: 51002b69    	sub	w9, w27, #0xa
100446728: 7100113f    	cmp	w9, #0x4
10044672c: 54009d48    	b.hi	0x100447ad4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x27e0>
100446730: 94031f90    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
100446734: 14000e75    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446738: d10303a0    	sub	x0, x29, #0xc0
10044673c: a94f8be1    	ldp	x1, x2, [sp, #0xf8]
100446740: f9408be3    	ldr	x3, [sp, #0x110]
100446744: 940028d8    	bl	0x100450aa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h4adb145aa1c64863E>
100446748: b85403a8    	ldur	w8, [x29, #-0xc0]
10044674c: 7100091f    	cmp	w8, #0x2
100446750: 5400a0e0    	b.eq	0x100447b6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2878>
100446754: f85483a9    	ldur	x9, [x29, #-0xb8]
100446758: 71000d1f    	cmp	w8, #0x3
10044675c: f9408be1    	ldr	x1, [sp, #0x110]
100446760: f94083e0    	ldr	x0, [sp, #0x100]
100446764: 54021f40    	b.eq	0x10044ab4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5858>
100446768: 36010548    	tbz	w8, #0x0, 0x100448810 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x351c>
10044676c: f81483a9    	stur	x9, [x29, #-0xb8]
100446770: 52800088    	mov	w8, #0x4                ; =4
100446774: 1400082a    	b	0x10044881c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3528>
100446778: 37fa3f14    	tbnz	w20, #0x1f, 0x10044af58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c64>
10044677c: b81443b4    	stur	w20, [x29, #-0xbc]
100446780: 52800068    	mov	w8, #0x3                ; =3
100446784: 381403a8    	sturb	w8, [x29, #-0xc0]
100446788: d10303a2    	sub	x2, x29, #0xc0
10044678c: f94083e0    	ldr	x0, [sp, #0x100]
100446790: f9408be1    	ldr	x1, [sp, #0x110]
100446794: 940021e7    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446798: b40106c0    	cbz	x0, 0x100448870 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x357c>
10044679c: 140011a3    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
1004467a0: f94087e8    	ldr	x8, [sp, #0x108]
1004467a4: f9400109    	ldr	x9, [x8]
1004467a8: f9403928    	ldr	x8, [x9, #0x70]
1004467ac: b4022168    	cbz	x8, 0x10044abd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58e4>
1004467b0: f9403d2a    	ldr	x10, [x9, #0x78]
1004467b4: f9407fe9    	ldr	x9, [sp, #0xf8]
1004467b8: 2a0903e9    	mov	w9, w9
1004467bc: eb09015f    	cmp	x10, x9
1004467c0: 540220c9    	b.ls	0x10044abd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x58e4>
1004467c4: 8b091914    	add	x20, x8, x9, lsl #6
1004467c8: f9407be8    	ldr	x8, [sp, #0xf0]
1004467cc: f9400102    	ldr	x2, [x8]
1004467d0: d10303a0    	sub	x0, x29, #0xc0
1004467d4: 91004281    	add	x1, x20, #0x10
1004467d8: f94083e3    	ldr	x3, [sp, #0x100]
1004467dc: f9408be4    	ldr	x4, [sp, #0x110]
1004467e0: 94002d04    	bl	0x100451bf0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h6c72f3f518c9b7b0E>
1004467e4: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004467e8: 7100051f    	cmp	w8, #0x1
1004467ec: 54023b00    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
1004467f0: 385413a8    	ldurb	w8, [x29, #-0xbf]
1004467f4: 3600b768    	tbz	w8, #0x0, 0x100447ee0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2bec>
1004467f8: 52800028    	mov	w8, #0x1                ; =1
1004467fc: 6a53711f    	tst	w8, w19, lsr #28
100446800: 14000cd4    	b	0x100449b50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x485c>
100446804: d10303a0    	sub	x0, x29, #0xc0
100446808: f94083e1    	ldr	x1, [sp, #0x100]
10044680c: f9408be2    	ldr	x2, [sp, #0x110]
100446810: 94002834    	bl	0x1004508e0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
100446814: 385403a8    	ldurb	w8, [x29, #-0xc0]
100446818: 71000d1f    	cmp	w8, #0x3
10044681c: 5401c780    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100446820: 14001103    	b	0x10044ac2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5938>
100446824: f9407fe8    	ldr	x8, [sp, #0xf8]
100446828: 7902ffe8    	strh	w8, [sp, #0x17e]
10044682c: 52800028    	mov	w8, #0x1                ; =1
100446830: 6a53711f    	tst	w8, w19, lsr #28
100446834: 9a88050a    	cinc	x10, x8, ne
100446838: 2a1903e9    	mov	w9, w25
10044683c: 8b090156    	add	x22, x10, x9
100446840: eb1c02df    	cmp	x22, x28
100446844: 54028102    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100446848: b8767b0a    	ldr	w10, [x24, x22, lsl #2]
10044684c: 7905a3ea    	strh	w10, [sp, #0x2d0]
100446850: 53107d4a    	lsr	w10, w10, #16
100446854: 781103aa    	sturh	w10, [x29, #-0xf0]
100446858: 6a53711f    	tst	w8, w19, lsr #28
10044685c: 52800048    	mov	w8, #0x2                ; =2
100446860: 9a880508    	cinc	x8, x8, ne
100446864: 8b090116    	add	x22, x8, x9
100446868: eb1c02df    	cmp	x22, x28
10044686c: 54027fc2    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100446870: d37ef6c8    	lsl	x8, x22, #2
100446874: 78e86b08    	ldrsh	w8, [x24, x8]
100446878: b90303e8    	str	w8, [sp, #0x300]
10044687c: 9105fbe8    	add	x8, sp, #0x17e
100446880: d103c3a9    	sub	x9, x29, #0xf0
100446884: a93427a8    	stp	x8, x9, [x29, #-0xc0]
100446888: 910c03e8    	add	x8, sp, #0x300
10044688c: 910b43e9    	add	x9, sp, #0x2d0
100446890: a93527a8    	stp	x8, x9, [x29, #-0xb0]
100446894: f9407be8    	ldr	x8, [sp, #0xf0]
100446898: f81603a8    	stur	x8, [x29, #-0xa0]
10044689c: d10383a0    	sub	x0, x29, #0xe0
1004468a0: d10303a1    	sub	x1, x29, #0xc0
1004468a4: f94083e2    	ldr	x2, [sp, #0x100]
1004468a8: f9408be3    	ldr	x3, [sp, #0x110]
1004468ac: 94003284    	bl	0x1004532bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h1c17d9bf303133e4E>
1004468b0: 385203a8    	ldurb	w8, [x29, #-0xe0]
1004468b4: 7100051f    	cmp	w8, #0x1
1004468b8: 54021260    	b.eq	0x10044ab04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5810>
1004468bc: 385213a8    	ldurb	w8, [x29, #-0xdf]
1004468c0: 3600a3c8    	tbz	w8, #0x0, 0x100447d38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2a44>
1004468c4: 11002299    	add	w25, w20, #0x8
1004468c8: 14000e12    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
1004468cc: f9407fe8    	ldr	x8, [sp, #0xf8]
1004468d0: 7902fbe8    	strh	w8, [sp, #0x17c]
1004468d4: 52800028    	mov	w8, #0x1                ; =1
1004468d8: 6a53711f    	tst	w8, w19, lsr #28
1004468dc: 9a880508    	cinc	x8, x8, ne
1004468e0: 8b394116    	add	x22, x8, w25, uxtw
1004468e4: eb1c02df    	cmp	x22, x28
1004468e8: 54027be2    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
1004468ec: b8767b08    	ldr	w8, [x24, x22, lsl #2]
1004468f0: 781103a8    	sturh	w8, [x29, #-0xf0]
1004468f4: 53107d08    	lsr	w8, w8, #16
1004468f8: b90303e8    	str	w8, [sp, #0x300]
1004468fc: 9105f3e8    	add	x8, sp, #0x17c
100446900: d103c3a9    	sub	x9, x29, #0xf0
100446904: a93427a8    	stp	x8, x9, [x29, #-0xc0]
100446908: f9407be9    	ldr	x9, [sp, #0xf0]
10044690c: a95023e2    	ldp	x2, x8, [sp, #0x100]
100446910: a93523a9    	stp	x9, x8, [x29, #-0xb0]
100446914: 910503e8    	add	x8, sp, #0x140
100446918: 910c03e9    	add	x9, sp, #0x300
10044691c: a93627a8    	stp	x8, x9, [x29, #-0xa0]
100446920: d10383a0    	sub	x0, x29, #0xe0
100446924: d10303a1    	sub	x1, x29, #0xc0
100446928: f9408be3    	ldr	x3, [sp, #0x110]
10044692c: 9400331c    	bl	0x10045359c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h8223cc06df0542f6E>
100446930: 385203a8    	ldurb	w8, [x29, #-0xe0]
100446934: 7100051f    	cmp	w8, #0x1
100446938: 54020e60    	b.eq	0x10044ab04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5810>
10044693c: 385213a8    	ldurb	w8, [x29, #-0xdf]
100446940: 36009ce8    	tbz	w8, #0x0, 0x100447cdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x29e8>
100446944: 11001699    	add	w25, w20, #0x5
100446948: 14000df2    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
10044694c: d10303a0    	sub	x0, x29, #0xc0
100446950: a94f8be1    	ldp	x1, x2, [sp, #0xf8]
100446954: f9408be3    	ldr	x3, [sp, #0x110]
100446958: 94002352    	bl	0x10044f6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h093a11e358292234E>
10044695c: 385403a8    	ldurb	w8, [x29, #-0xc0]
100446960: 7100051f    	cmp	w8, #0x1
100446964: 54022f40    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
100446968: 385413a8    	ldurb	w8, [x29, #-0xbf]
10044696c: 7100091f    	cmp	w8, #0x2
100446970: 54023720    	b.eq	0x10044b054 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d60>
100446974: f9407349    	ldr	x9, [x26, #0xe0]
100446978: f9407fe8    	ldr	x8, [sp, #0xf8]
10044697c: 92403d08    	and	x8, x8, #0xffff
100446980: eb08013f    	cmp	x9, x8
100446984: 5401bc49    	b.ls	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100446988: f9406f49    	ldr	x9, [x26, #0xd8]
10044698c: 3828693f    	strb	wzr, [x9, x8]
100446990: 14000ddf    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100446994: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100446998: f9408be1    	ldr	x1, [sp, #0x110]
10044699c: 52800002    	mov	w2, #0x0                ; =0
1004469a0: 94002182    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004469a4: f9408be4    	ldr	x4, [sp, #0x110]
1004469a8: f94083e3    	ldr	x3, [sp, #0x100]
1004469ac: 910923eb    	add	x11, sp, #0x248
1004469b0: b400fda0    	cbz	x0, 0x100448964 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3670>
1004469b4: 39400008    	ldrb	w8, [x0]
1004469b8: 71001d1f    	cmp	w8, #0x7
1004469bc: 5400fd48    	b.hi	0x100448964 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3670>
1004469c0: 52800029    	mov	w9, #0x1                ; =1
1004469c4: 1ac82129    	lsl	w9, w9, w8
1004469c8: 5280138a    	mov	w10, #0x9c              ; =156
1004469cc: 6a0a013f    	tst	w9, w10
1004469d0: 5400fb00    	b.eq	0x100448930 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x363c>
1004469d4: f8401009    	ldur	x9, [x0, #0x1]
1004469d8: f90173e9    	str	x9, [sp, #0x2e0]
1004469dc: f9400409    	ldr	x9, [x0, #0x8]
1004469e0: f809f169    	stur	x9, [x11, #0x9f]
1004469e4: 140007d5    	b	0x100448938 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3644>
1004469e8: f9408fe8    	ldr	x8, [sp, #0x118]
1004469ec: f9400508    	ldr	x8, [x8, #0x8]
1004469f0: b1000513    	adds	x19, x8, #0x1
1004469f4: 5401fc82    	b.hs	0x10044a984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5690>
1004469f8: f94083e0    	ldr	x0, [sp, #0x100]
1004469fc: f9408be1    	ldr	x1, [sp, #0x110]
100446a00: 52800022    	mov	w2, #0x1                ; =1
100446a04: 97ff98ee    	bl	0x10042cdbc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
100446a08: aa0103f4    	mov	x20, x1
100446a0c: 37023a40    	tbnz	w0, #0x0, 0x10044b154 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e60>
100446a10: 39400288    	ldrb	w8, [x20]
100446a14: 71000d1f    	cmp	w8, #0x3
100446a18: f9408be1    	ldr	x1, [sp, #0x110]
100446a1c: f94083e0    	ldr	x0, [sp, #0x100]
100446a20: 5401fb21    	b.ne	0x10044a984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5690>
100446a24: b9400696    	ldr	w22, [x20, #0x4]
100446a28: 37f9faf6    	tbnz	w22, #0x1f, 0x10044a984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5690>
100446a2c: 52800042    	mov	w2, #0x2                ; =2
100446a30: 97ff98e3    	bl	0x10042cdbc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
100446a34: aa0103f4    	mov	x20, x1
100446a38: 370238e0    	tbnz	w0, #0x0, 0x10044b154 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e60>
100446a3c: f94083e0    	ldr	x0, [sp, #0x100]
100446a40: f9408be1    	ldr	x1, [sp, #0x110]
100446a44: d2800002    	mov	x2, #0x0                ; =0
100446a48: 97ff98dd    	bl	0x10042cdbc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
100446a4c: aa0103fb    	mov	x27, x1
100446a50: 370235e0    	tbnz	w0, #0x0, 0x10044b10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e18>
100446a54: f9407be8    	ldr	x8, [sp, #0xf0]
100446a58: f9400101    	ldr	x1, [x8]
100446a5c: 9109c3e0    	add	x0, sp, #0x270
100446a60: aa1403e2    	mov	x2, x20
100446a64: aa1603e3    	mov	x3, x22
100446a68: aa1b03e4    	mov	x4, x27
100446a6c: 94002ad2    	bl	0x1004515b4 <__ZN13quickjs_oxide6engine6object16ordinary_storage2ic62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$28try_dense_array_write_scalar17h2b1073b890e82808E>
100446a70: 3949c3e8    	ldrb	w8, [sp, #0x270]
100446a74: 71002d1f    	cmp	w8, #0xb
100446a78: 54023621    	b.ne	0x10044b13c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e48>
100446a7c: 3949c7e8    	ldrb	w8, [sp, #0x271]
100446a80: f9408be3    	ldr	x3, [sp, #0x110]
100446a84: f94083e9    	ldr	x9, [sp, #0x100]
100446a88: 37012268    	tbnz	w8, #0x0, 0x100448ed4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3be0>
100446a8c: 39400368    	ldrb	w8, [x27]
100446a90: 7100111f    	cmp	w8, #0x4
100446a94: 540120e0    	b.eq	0x100448eb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3bbc>
100446a98: 71000d1f    	cmp	w8, #0x3
100446a9c: 5401f741    	b.ne	0x10044a984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5690>
100446aa0: bd400760    	ldr	s0, [x27, #0x4]
100446aa4: 0f20a400    	sshll.2d	v0, v0, #0x0
100446aa8: 5e61d800    	scvtf	d0, d0
100446aac: 14000902    	b	0x100448eb4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3bc0>
100446ab0: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100446ab4: f9408be1    	ldr	x1, [sp, #0x110]
100446ab8: 52800022    	mov	w2, #0x1                ; =1
100446abc: 9400213b    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100446ac0: f9408be4    	ldr	x4, [sp, #0x110]
100446ac4: f94083e3    	ldr	x3, [sp, #0x100]
100446ac8: 910923eb    	add	x11, sp, #0x248
100446acc: b400f900    	cbz	x0, 0x1004489ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36f8>
100446ad0: 39400008    	ldrb	w8, [x0]
100446ad4: 71001d1f    	cmp	w8, #0x7
100446ad8: 5400f8a8    	b.hi	0x1004489ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36f8>
100446adc: 52800029    	mov	w9, #0x1                ; =1
100446ae0: 1ac82129    	lsl	w9, w9, w8
100446ae4: 5280138a    	mov	w10, #0x9c              ; =156
100446ae8: 6a0a013f    	tst	w9, w10
100446aec: 5400f660    	b.eq	0x1004489b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36c4>
100446af0: f8401009    	ldur	x9, [x0, #0x1]
100446af4: f90173e9    	str	x9, [sp, #0x2e0]
100446af8: f9400409    	ldr	x9, [x0, #0x8]
100446afc: f809f169    	stur	x9, [x11, #0x9f]
100446b00: 140007b0    	b	0x1004489c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36cc>
100446b04: f94083e0    	ldr	x0, [sp, #0x100]
100446b08: f9408be1    	ldr	x1, [sp, #0x110]
100446b0c: d2800002    	mov	x2, #0x0                ; =0
100446b10: 97ff98ab    	bl	0x10042cdbc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
100446b14: 37021fa0    	tbnz	w0, #0x0, 0x10044af08 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c14>
100446b18: 39400028    	ldrb	w8, [x1]
100446b1c: 7100151f    	cmp	w8, #0x5
100446b20: f9408be3    	ldr	x3, [sp, #0x110]
100446b24: f94083e2    	ldr	x2, [sp, #0x100]
100446b28: 5400d5c2    	b.hs	0x1004485e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x32ec>
100446b2c: d10303a0    	sub	x0, x29, #0xc0
100446b30: aa0203e1    	mov	x1, x2
100446b34: aa0303e2    	mov	x2, x3
100446b38: 9400276a    	bl	0x1004508e0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
100446b3c: 385403a8    	ldurb	w8, [x29, #-0xc0]
100446b40: 7100291f    	cmp	w8, #0xa
100446b44: 5401e760    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100446b48: f94073eb    	ldr	x11, [sp, #0xe0]
100446b4c: b9400169    	ldr	w9, [x11]
100446b50: f94063ea    	ldr	x10, [sp, #0xc0]
100446b54: b9000149    	str	w9, [x10]
100446b58: b8403169    	ldur	w9, [x11, #0x3]
100446b5c: b8003149    	stur	w9, [x10, #0x3]
100446b60: f85483a9    	ldur	x9, [x29, #-0xb8]
100446b64: 381203a8    	sturb	w8, [x29, #-0xe0]
100446b68: f81283a9    	stur	x9, [x29, #-0xd8]
100446b6c: d10383a0    	sub	x0, x29, #0xe0
100446b70: 94002bfa    	bl	0x100451b58 <__ZN13quickjs_oxide6engine5value8js_value7JsValue20to_boolean_primitive17h5fda868505f7d3b0E>
100446b74: 52000008    	eor	w8, w0, #0x1
100446b78: 381413a8    	sturb	w8, [x29, #-0xbf]
100446b7c: 52800048    	mov	w8, #0x2                ; =2
100446b80: 381403a8    	sturb	w8, [x29, #-0xc0]
100446b84: d10303a2    	sub	x2, x29, #0xc0
100446b88: f94083e0    	ldr	x0, [sp, #0x100]
100446b8c: f9408be1    	ldr	x1, [sp, #0x110]
100446b90: 940020e8    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446b94: 14000d5d    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446b98: f9407749    	ldr	x9, [x26, #0xe8]
100446b9c: b40000a9    	cbz	x9, 0x100446bb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x18bc>
100446ba0: aa0903e2    	mov	x2, x9
100446ba4: 38440c48    	ldrb	w8, [x2, #0x40]!
100446ba8: 7100291f    	cmp	w8, #0xa
100446bac: 5400c741    	b.ne	0x100448494 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x31a0>
100446bb0: a950b3e8    	ldp	x8, x12, [sp, #0x108]
100446bb4: f9400108    	ldr	x8, [x8]
100446bb8: 3944290a    	ldrb	w10, [x8, #0x10a]
100446bbc: f8490349    	ldur	x9, [x26, #0x90]
100446bc0: f94083e0    	ldr	x0, [sp, #0x100]
100446bc4: f9407bed    	ldr	x13, [sp, #0xf0]
100446bc8: 910923ee    	add	x14, sp, #0x248
100446bcc: f100093f    	cmp	x9, #0x2
100446bd0: 3600c1ea    	tbz	w10, #0x0, 0x10044840c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3118>
100446bd4: 540246a0    	b.eq	0x10044b4a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61b4>
100446bd8: 384a8348    	ldurb	w8, [x26, #0xa8]
100446bdc: f94001a1    	ldr	x1, [x13]
100446be0: 71001d1f    	cmp	w8, #0x7
100446be4: 540124c8    	b.hi	0x10044907c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d88>
100446be8: 52800029    	mov	w9, #0x1                ; =1
100446bec: 1ac82129    	lsl	w9, w9, w8
100446bf0: 5280138a    	mov	w10, #0x9c              ; =156
100446bf4: 6a0a013f    	tst	w9, w10
100446bf8: 54012060    	b.eq	0x100449004 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d10>
100446bfc: f84a9349    	ldur	x9, [x26, #0xa9]
100446c00: f81203a9    	stur	x9, [x29, #-0xe0]
100446c04: f84b0349    	ldur	x9, [x26, #0xb0]
100446c08: 14000632    	b	0x1004484d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x31dc>
100446c0c: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100446c10: f9408be1    	ldr	x1, [sp, #0x110]
100446c14: 52800002    	mov	w2, #0x0                ; =0
100446c18: 940020e4    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100446c1c: f9408be4    	ldr	x4, [sp, #0x110]
100446c20: f94083e3    	ldr	x3, [sp, #0x100]
100446c24: 910923eb    	add	x11, sp, #0x248
100446c28: b400f380    	cbz	x0, 0x100448a98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37a4>
100446c2c: 39400008    	ldrb	w8, [x0]
100446c30: 71001d1f    	cmp	w8, #0x7
100446c34: 5400f328    	b.hi	0x100448a98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37a4>
100446c38: 52800029    	mov	w9, #0x1                ; =1
100446c3c: 1ac82129    	lsl	w9, w9, w8
100446c40: 5280138a    	mov	w10, #0x9c              ; =156
100446c44: 6a0a013f    	tst	w9, w10
100446c48: 5400f0e0    	b.eq	0x100448a64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3770>
100446c4c: f8401009    	ldur	x9, [x0, #0x1]
100446c50: f90173e9    	str	x9, [sp, #0x2e0]
100446c54: f9400409    	ldr	x9, [x0, #0x8]
100446c58: f809f169    	stur	x9, [x11, #0x9f]
100446c5c: 14000784    	b	0x100448a6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3778>
100446c60: 52800028    	mov	w8, #0x1                ; =1
100446c64: 381403a8    	sturb	w8, [x29, #-0xc0]
100446c68: d10303a2    	sub	x2, x29, #0xc0
100446c6c: f94083e0    	ldr	x0, [sp, #0x100]
100446c70: f9408be1    	ldr	x1, [sp, #0x110]
100446c74: 940020af    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446c78: 14000d24    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446c7c: f94083e8    	ldr	x8, [sp, #0x100]
100446c80: a9408500    	ldp	x0, x1, [x8, #0x8]
100446c84: f9408be8    	ldr	x8, [sp, #0x110]
100446c88: f9401902    	ldr	x2, [x8, #0x30]
100446c8c: f9402103    	ldr	x3, [x8, #0x40]
100446c90: d2800004    	mov	x4, #0x0                ; =0
100446c94: 52800085    	mov	w5, #0x4                ; =4
100446c98: 52800026    	mov	w6, #0x1                ; =1
100446c9c: 940023a9    	bl	0x10044fb40 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23rotate_operands_current17ha464970b12b6798cE>
100446ca0: 14000d1a    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446ca4: b9400348    	ldr	w8, [x26]
100446ca8: 36024008    	tbz	w8, #0x0, 0x10044b4a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61b4>
100446cac: 29425b54    	ldp	w20, w22, [x26, #0x10]
100446cb0: f9400741    	ldr	x1, [x26, #0x8]
100446cb4: 910b83e0    	add	x0, sp, #0x2e0
100446cb8: aa1403e2    	mov	x2, x20
100446cbc: aa1603e3    	mov	x3, x22
100446cc0: 97f08783    	bl	0x100068acc <__ZN13quickjs_oxide6engine4heap9ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$20retain_object_handle17ha34447950ade3eebE>
100446cc4: 394b83e8    	ldrb	w8, [sp, #0x2e0]
100446cc8: 7100191f    	cmp	w8, #0x6
100446ccc: 54021aa1    	b.ne	0x10044b020 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d2c>
100446cd0: 2928dbb4    	stp	w20, w22, [x29, #-0xbc]
100446cd4: 52800128    	mov	w8, #0x9                ; =9
100446cd8: 381403a8    	sturb	w8, [x29, #-0xc0]
100446cdc: f9407be8    	ldr	x8, [sp, #0xf0]
100446ce0: f9400102    	ldr	x2, [x8]
100446ce4: d10303a3    	sub	x3, x29, #0xc0
100446ce8: f94083e0    	ldr	x0, [sp, #0x100]
100446cec: f9408be1    	ldr	x1, [sp, #0x110]
100446cf0: 940020f4    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100446cf4: 14000d05    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446cf8: 52802048    	mov	w8, #0x102              ; =258
100446cfc: 781403a8    	sturh	w8, [x29, #-0xc0]
100446d00: d10303a2    	sub	x2, x29, #0xc0
100446d04: f94083e0    	ldr	x0, [sp, #0x100]
100446d08: f9408be1    	ldr	x1, [sp, #0x110]
100446d0c: 94002089    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446d10: 14000cfe    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446d14: 381403bf    	sturb	wzr, [x29, #-0xc0]
100446d18: d10303a2    	sub	x2, x29, #0xc0
100446d1c: f94083e0    	ldr	x0, [sp, #0x100]
100446d20: f9408be1    	ldr	x1, [sp, #0x110]
100446d24: 94002083    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446d28: 14000cf8    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446d2c: d10303a0    	sub	x0, x29, #0xc0
100446d30: a94f8be1    	ldp	x1, x2, [sp, #0xf8]
100446d34: f9408be3    	ldr	x3, [sp, #0x110]
100446d38: 9400225a    	bl	0x10044f6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h093a11e358292234E>
100446d3c: 385403a8    	ldurb	w8, [x29, #-0xc0]
100446d40: 7100051f    	cmp	w8, #0x1
100446d44: 54021040    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
100446d48: 385413a8    	ldurb	w8, [x29, #-0xbf]
100446d4c: 71000d1f    	cmp	w8, #0x3
100446d50: 540085a0    	b.eq	0x100447e04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2b10>
100446d54: 7100091f    	cmp	w8, #0x2
100446d58: f94083ea    	ldr	x10, [sp, #0x100]
100446d5c: f9407bec    	ldr	x12, [sp, #0xf0]
100446d60: 54022380    	b.eq	0x10044b1d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5edc>
100446d64: f9408fe8    	ldr	x8, [sp, #0x118]
100446d68: f9401108    	ldr	x8, [x8, #0x20]
100446d6c: b401d228    	cbz	x8, 0x10044a7b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54bc>
100446d70: f9407fe8    	ldr	x8, [sp, #0xf8]
100446d74: 12003d13    	and	w19, w8, #0xffff
100446d78: f9406fe8    	ldr	x8, [sp, #0xd8]
100446d7c: f9400108    	ldr	x8, [x8]
100446d80: f94077e9    	ldr	x9, [sp, #0xe8]
100446d84: f9400129    	ldr	x9, [x9]
100446d88: eb080129    	subs	x9, x9, x8
100446d8c: 9a8933e9    	csel	x9, xzr, x9, lo
100446d90: eb13013f    	cmp	x9, x19
100446d94: 54022489    	b.ls	0x10044b224 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f30>
100446d98: f940095c    	ldr	x28, [x10, #0x10]
100446d9c: 8b130116    	add	x22, x8, x19
100446da0: eb1c02df    	cmp	x22, x28
100446da4: 54025722    	b.hs	0x10044b888 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6594>
100446da8: f9400548    	ldr	x8, [x10, #0x8]
100446dac: 8b161108    	add	x8, x8, x22, lsl #4
100446db0: 39400109    	ldrb	w9, [x8]
100446db4: 7100393f    	cmp	w9, #0xe
100446db8: 540224e0    	b.eq	0x10044b254 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f60>
100446dbc: b840110a    	ldur	w10, [x8, #0x1]
100446dc0: f94037eb    	ldr	x11, [sp, #0x68]
100446dc4: b900016a    	str	w10, [x11]
100446dc8: b940050a    	ldr	w10, [x8, #0x4]
100446dcc: b800316a    	stur	w10, [x11, #0x3]
100446dd0: f940050a    	ldr	x10, [x8, #0x8]
100446dd4: 5280018b    	mov	w11, #0xc               ; =12
100446dd8: 3900010b    	strb	w11, [x8]
100446ddc: 390763e9    	strb	w9, [sp, #0x1d8]
100446de0: f900f3ea    	str	x10, [sp, #0x1e0]
100446de4: f9408fe8    	ldr	x8, [sp, #0x118]
100446de8: f9401103    	ldr	x3, [x8, #0x20]
100446dec: f9400182    	ldr	x2, [x12]
100446df0: f9406be0    	ldr	x0, [sp, #0xd0]
100446df4: aa1903e1    	mov	x1, x25
100446df8: 940028f6    	bl	0x1004511d0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
100446dfc: b5020160    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100446e00: f9407be8    	ldr	x8, [sp, #0xf0]
100446e04: f9400100    	ldr	x0, [x8]
100446e08: 910763e1    	add	x1, sp, #0x1d8
100446e0c: 97f46e00    	bl	0x10016260c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
100446e10: b4007fe0    	cbz	x0, 0x100447e0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2b18>
100446e14: 14001005    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100446e18: a94f83e8    	ldp	x8, x0, [sp, #0xf8]
100446e1c: 2a0803e8    	mov	w8, w8
100446e20: a95087e9    	ldp	x9, x1, [sp, #0x108]
100446e24: f9400129    	ldr	x9, [x9]
100446e28: f940452a    	ldr	x10, [x9, #0x88]
100446e2c: eb08015f    	cmp	x10, x8
100446e30: 5401e809    	b.ls	0x10044ab30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
100446e34: f9404129    	ldr	x9, [x9, #0x80]
100446e38: 5280030a    	mov	w10, #0x18              ; =24
100446e3c: 9baa2508    	umaddl	x8, w8, w10, x9
100446e40: b8410d09    	ldr	w9, [x8, #0x10]!
100446e44: 3501e769    	cbnz	w9, 0x10044ab30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
100446e48: 39402109    	ldrb	w9, [x8, #0x8]
100446e4c: 71000d3f    	cmp	w9, #0x3
100446e50: 54007eac    	b.gt	0x100447e24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2b30>
100446e54: 7100053f    	cmp	w9, #0x1
100446e58: 5400d14c    	b.gt	0x100448880 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x358c>
100446e5c: 340112e9    	cbz	w9, 0x1004490b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3dc4>
100446e60: 7100053f    	cmp	w9, #0x1
100446e64: 5401e661    	b.ne	0x10044ab30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
100446e68: 52800028    	mov	w8, #0x1                ; =1
100446e6c: 381403a8    	sturb	w8, [x29, #-0xc0]
100446e70: d10303a2    	sub	x2, x29, #0xc0
100446e74: 9400202f    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446e78: 14000ca4    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446e7c: a94f83ea    	ldp	x10, x0, [sp, #0xf8]
100446e80: 13003d49    	sxth	w9, w10
100446e84: 7100011f    	cmp	w8, #0x0
100446e88: 1a8a0128    	csel	w8, w9, w10, eq
100446e8c: b81443a8    	stur	w8, [x29, #-0xbc]
100446e90: 52800068    	mov	w8, #0x3                ; =3
100446e94: 381403a8    	sturb	w8, [x29, #-0xc0]
100446e98: d10303a2    	sub	x2, x29, #0xc0
100446e9c: f9408be1    	ldr	x1, [sp, #0x110]
100446ea0: 94002024    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446ea4: 14000c99    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446ea8: f8490348    	ldur	x8, [x26, #0x90]
100446eac: f100091f    	cmp	x8, #0x2
100446eb0: 54022fc0    	b.eq	0x10044b4a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61b4>
100446eb4: f9407be8    	ldr	x8, [sp, #0xf0]
100446eb8: f9400102    	ldr	x2, [x8]
100446ebc: 384b8348    	ldurb	w8, [x26, #0xb8]
100446ec0: 71001d1f    	cmp	w8, #0x7
100446ec4: f9408be1    	ldr	x1, [sp, #0x110]
100446ec8: f94083e0    	ldr	x0, [sp, #0x100]
100446ecc: 910923eb    	add	x11, sp, #0x248
100446ed0: 540106e8    	b.hi	0x100448fac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cb8>
100446ed4: 52800029    	mov	w9, #0x1                ; =1
100446ed8: 1ac82129    	lsl	w9, w9, w8
100446edc: 5280138a    	mov	w10, #0x9c              ; =156
100446ee0: 6a0a013f    	tst	w9, w10
100446ee4: 5400a6e0    	b.eq	0x1004483c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30cc>
100446ee8: f84b9349    	ldur	x9, [x26, #0xb9]
100446eec: f81203a9    	stur	x9, [x29, #-0xe0]
100446ef0: f84c0349    	ldur	x9, [x26, #0xc0]
100446ef4: f80ff169    	stur	x9, [x11, #0xff]
100446ef8: 14000534    	b	0x1004483c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30d4>
100446efc: 52800048    	mov	w8, #0x2                ; =2
100446f00: 781403a8    	sturh	w8, [x29, #-0xc0]
100446f04: d10303a2    	sub	x2, x29, #0xc0
100446f08: f94083e0    	ldr	x0, [sp, #0x100]
100446f0c: f9408be1    	ldr	x1, [sp, #0x110]
100446f10: 94002008    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100446f14: 14000c7d    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100446f18: f94087e8    	ldr	x8, [sp, #0x108]
100446f1c: f9400108    	ldr	x8, [x8]
100446f20: f940551c    	ldr	x28, [x8, #0xa8]
100446f24: f9407fe9    	ldr	x9, [sp, #0xf8]
100446f28: 12003d36    	and	w22, w9, #0xffff
100446f2c: eb16039f    	cmp	x28, x22
100446f30: 54025069    	b.ls	0x10044b93c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6648>
100446f34: f9405108    	ldr	x8, [x8, #0xa0]
100446f38: 8b161508    	add	x8, x8, x22, lsl #5
100446f3c: 3940ad13    	ldrb	w19, [x8, #0x2b]
100446f40: 71001a7f    	cmp	w19, #0x6
100446f44: f9408be3    	ldr	x3, [sp, #0x110]
100446f48: f94083e2    	ldr	x2, [sp, #0x100]
100446f4c: 540205a0    	b.eq	0x10044b000 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d0c>
100446f50: 3940a114    	ldrb	w20, [x8, #0x28]
100446f54: d10303a0    	sub	x0, x29, #0xc0
100446f58: f9407fe1    	ldr	x1, [sp, #0xf8]
100446f5c: 940021d1    	bl	0x10044f6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h093a11e358292234E>
100446f60: 385403a8    	ldurb	w8, [x29, #-0xc0]
100446f64: 7100051f    	cmp	w8, #0x1
100446f68: 5401ff20    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
100446f6c: 385413a8    	ldurb	w8, [x29, #-0xbf]
100446f70: 7100027f    	cmp	w19, #0x0
100446f74: 7a420900    	ccmp	w8, #0x2, #0x0, eq
100446f78: f9408be3    	ldr	x3, [sp, #0x110]
100446f7c: f94083e2    	ldr	x2, [sp, #0x100]
100446f80: 54020a00    	b.eq	0x10044b0c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5dcc>
100446f84: 3601f614    	tbz	w20, #0x0, 0x10044ae44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b50>
100446f88: 71000d1f    	cmp	w8, #0x3
100446f8c: 5401f5c8    	b.hi	0x10044ae44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b50>
100446f90: 7100091f    	cmp	w8, #0x2
100446f94: 5401f580    	b.eq	0x10044ae44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b50>
100446f98: f9408fe8    	ldr	x8, [sp, #0x118]
100446f9c: f9401108    	ldr	x8, [x8, #0x20]
100446fa0: b401c088    	cbz	x8, 0x10044a7b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54bc>
100446fa4: d10303a0    	sub	x0, x29, #0xc0
100446fa8: f9407fe1    	ldr	x1, [sp, #0xf8]
100446fac: 940028c8    	bl	0x1004512cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h4f3d28bcddbdd275E>
100446fb0: 385403a8    	ldurb	w8, [x29, #-0xc0]
100446fb4: 7100391f    	cmp	w8, #0xe
100446fb8: 5401fca0    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
100446fbc: f94073ea    	ldr	x10, [sp, #0xe0]
100446fc0: b9400149    	ldr	w9, [x10]
100446fc4: f9403beb    	ldr	x11, [sp, #0x70]
100446fc8: b9000169    	str	w9, [x11]
100446fcc: b8403149    	ldur	w9, [x10, #0x3]
100446fd0: b8003169    	stur	w9, [x11, #0x3]
100446fd4: f85483a9    	ldur	x9, [x29, #-0xb8]
100446fd8: 390723e8    	strb	w8, [sp, #0x1c8]
100446fdc: f900ebe9    	str	x9, [sp, #0x1d0]
100446fe0: f9408fe8    	ldr	x8, [sp, #0x118]
100446fe4: f9401103    	ldr	x3, [x8, #0x20]
100446fe8: f9407be8    	ldr	x8, [sp, #0xf0]
100446fec: f9400102    	ldr	x2, [x8]
100446ff0: f9406be0    	ldr	x0, [sp, #0xd0]
100446ff4: aa1903e1    	mov	x1, x25
100446ff8: 94002876    	bl	0x1004511d0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
100446ffc: b501f160    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100447000: f9407be8    	ldr	x8, [sp, #0xf0]
100447004: f9400100    	ldr	x0, [x8]
100447008: 910723e1    	add	x1, sp, #0x1c8
10044700c: 97f46d80    	bl	0x10016260c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
100447010: 14000c3e    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100447014: f94083e8    	ldr	x8, [sp, #0x100]
100447018: a9408500    	ldp	x0, x1, [x8, #0x8]
10044701c: f9408be8    	ldr	x8, [sp, #0x110]
100447020: f9401902    	ldr	x2, [x8, #0x30]
100447024: f9402103    	ldr	x3, [x8, #0x40]
100447028: d2800004    	mov	x4, #0x0                ; =0
10044702c: 52800045    	mov	w5, #0x2                ; =2
100447030: 52800006    	mov	w6, #0x0                ; =0
100447034: 940022c3    	bl	0x10044fb40 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23rotate_operands_current17ha464970b12b6798cE>
100447038: 14000c34    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044703c: f8490348    	ldur	x8, [x26, #0x90]
100447040: f100091f    	cmp	x8, #0x2
100447044: 54022320    	b.eq	0x10044b4a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61b4>
100447048: 384b8348    	ldurb	w8, [x26, #0xb8]
10044704c: 35018608    	cbnz	w8, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447050: 14000fee    	b	0x10044b008 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d14>
100447054: d10303a0    	sub	x0, x29, #0xc0
100447058: a94f8be1    	ldp	x1, x2, [sp, #0xf8]
10044705c: f9408be3    	ldr	x3, [sp, #0x110]
100447060: 9400264b    	bl	0x10045098c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h6225c2cdb0985a83E>
100447064: b85403a8    	ldur	w8, [x29, #-0xc0]
100447068: 7100091f    	cmp	w8, #0x2
10044706c: 54005aa0    	b.eq	0x100447bc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x28cc>
100447070: f85483a9    	ldur	x9, [x29, #-0xb8]
100447074: 71000d1f    	cmp	w8, #0x3
100447078: f9408be1    	ldr	x1, [sp, #0x110]
10044707c: f94083e0    	ldr	x0, [sp, #0x100]
100447080: 5401d660    	b.eq	0x10044ab4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5858>
100447084: 3600bd48    	tbz	w8, #0x0, 0x10044882c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3538>
100447088: f81483a9    	stur	x9, [x29, #-0xb8]
10044708c: 52800088    	mov	w8, #0x4                ; =4
100447090: 140005ea    	b	0x100448838 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3544>
100447094: d10303a0    	sub	x0, x29, #0xc0
100447098: f94083e1    	ldr	x1, [sp, #0x100]
10044709c: f9408be2    	ldr	x2, [sp, #0x110]
1004470a0: 94002610    	bl	0x1004508e0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
1004470a4: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004470a8: 71000d1f    	cmp	w8, #0x3
1004470ac: 5401dae1    	b.ne	0x10044ac08 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5914>
1004470b0: b85443a1    	ldur	w1, [x29, #-0xbc]
1004470b4: 37f9ff81    	tbnz	w1, #0x1f, 0x10044b0a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5db0>
1004470b8: f900a3e1    	str	x1, [sp, #0x140]
1004470bc: f94087e8    	ldr	x8, [sp, #0x108]
1004470c0: f9400108    	ldr	x8, [x8]
1004470c4: 91014100    	add	x0, x8, #0x50
1004470c8: 97f13f2d    	bl	0x100096d7c <__ZN13quickjs_oxide6engine4code4exec8ExecCode14opcode_at_exec17hced033b0ccca821cE>
1004470cc: 12003c08    	and	w8, w0, #0xffff
1004470d0: 7103911f    	cmp	w8, #0xe4
1004470d4: 540181c1    	b.ne	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
1004470d8: 14001003    	b	0x10044b0e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5df0>
1004470dc: f9407fe8    	ldr	x8, [sp, #0xf8]
1004470e0: 12002508    	and	w8, w8, #0x3ff
1004470e4: 7103911f    	cmp	w8, #0xe4
1004470e8: 5401f462    	b.hs	0x10044af74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c80>
1004470ec: d00009e9    	adrp	x9, 0x100585000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x16ce8>
1004470f0: 912b8129    	add	x9, x9, #0xae0
1004470f4: 78685934    	ldrh	w20, [x9, w8, uxtw #1]
1004470f8: f94083e8    	ldr	x8, [sp, #0x100]
1004470fc: a9408901    	ldp	x1, x2, [x8, #0x8]
100447100: d10303a0    	sub	x0, x29, #0xc0
100447104: f9408be3    	ldr	x3, [sp, #0x110]
100447108: aa1403e4    	mov	x4, x20
10044710c: 94002ddb    	bl	0x100452878 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore26number_pair_branch_current17h41b56ddccc2127ddE>
100447110: 385403a8    	ldurb	w8, [x29, #-0xc0]
100447114: 7100051f    	cmp	w8, #0x1
100447118: 5401f1a0    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044711c: 385413a8    	ldurb	w8, [x29, #-0xbf]
100447120: 7100091f    	cmp	w8, #0x2
100447124: 54006c61    	b.ne	0x100447eb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2bbc>
100447128: f94083e8    	ldr	x8, [sp, #0x100]
10044712c: a9408901    	ldp	x1, x2, [x8, #0x8]
100447130: d10303a0    	sub	x0, x29, #0xc0
100447134: f9408be3    	ldr	x3, [sp, #0x110]
100447138: aa1403e4    	mov	x4, x20
10044713c: 94002e71    	bl	0x100452b00 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore21binary_number_current17h3cfb32a46b661496E>
100447140: 385403a8    	ldurb	w8, [x29, #-0xc0]
100447144: 7100051f    	cmp	w8, #0x1
100447148: 5401f020    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044714c: 385413a8    	ldurb	w8, [x29, #-0xbf]
100447150: 37017de8    	tbnz	w8, #0x0, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447154: 1400103b    	b	0x10044b240 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f4c>
100447158: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
10044715c: f9408be1    	ldr	x1, [sp, #0x110]
100447160: 52800022    	mov	w2, #0x1                ; =1
100447164: 94001f91    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447168: f9408be4    	ldr	x4, [sp, #0x110]
10044716c: f94083e3    	ldr	x3, [sp, #0x100]
100447170: 910923eb    	add	x11, sp, #0x248
100447174: b400d1a0    	cbz	x0, 0x100448ba8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x38b4>
100447178: 39400008    	ldrb	w8, [x0]
10044717c: 71001d1f    	cmp	w8, #0x7
100447180: 5400d148    	b.hi	0x100448ba8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x38b4>
100447184: 52800029    	mov	w9, #0x1                ; =1
100447188: 1ac82129    	lsl	w9, w9, w8
10044718c: 5280138a    	mov	w10, #0x9c              ; =156
100447190: 6a0a013f    	tst	w9, w10
100447194: 5400cf00    	b.eq	0x100448b74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3880>
100447198: f8401009    	ldur	x9, [x0, #0x1]
10044719c: f90173e9    	str	x9, [sp, #0x2e0]
1004471a0: f9400409    	ldr	x9, [x0, #0x8]
1004471a4: f809f169    	stur	x9, [x11, #0x9f]
1004471a8: 14000675    	b	0x100448b7c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3888>
1004471ac: 71037b7f    	cmp	w27, #0xde
1004471b0: 54005320    	b.eq	0x100447c14 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2920>
1004471b4: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
1004471b8: f9408be1    	ldr	x1, [sp, #0x110]
1004471bc: 52800022    	mov	w2, #0x1                ; =1
1004471c0: 94001f7a    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004471c4: f9408be4    	ldr	x4, [sp, #0x110]
1004471c8: f94083e3    	ldr	x3, [sp, #0x100]
1004471cc: b4010fe0    	cbz	x0, 0x1004493c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40d4>
1004471d0: 39400008    	ldrb	w8, [x0]
1004471d4: 71001d1f    	cmp	w8, #0x7
1004471d8: 54010f88    	b.hi	0x1004493c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40d4>
1004471dc: 52800029    	mov	w9, #0x1                ; =1
1004471e0: 1ac82129    	lsl	w9, w9, w8
1004471e4: 5280138a    	mov	w10, #0x9c              ; =156
1004471e8: 6a0a013f    	tst	w9, w10
1004471ec: 910923ea    	add	x10, sp, #0x248
1004471f0: 54010d00    	b.eq	0x100449390 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x409c>
1004471f4: f8401009    	ldur	x9, [x0, #0x1]
1004471f8: f90173e9    	str	x9, [sp, #0x2e0]
1004471fc: f9400409    	ldr	x9, [x0, #0x8]
100447200: f809f149    	stur	x9, [x10, #0x9f]
100447204: 14000865    	b	0x100449398 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40a4>
100447208: 7103737f    	cmp	w27, #0xdc
10044720c: f9408be1    	ldr	x1, [sp, #0x110]
100447210: f94083e0    	ldr	x0, [sp, #0x100]
100447214: 540053c0    	b.eq	0x100447c8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2998>
100447218: 52800022    	mov	w2, #0x1                ; =1
10044721c: f9407fe3    	ldr	x3, [sp, #0xf8]
100447220: 94001f62    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447224: f9408be4    	ldr	x4, [sp, #0x110]
100447228: f94083e3    	ldr	x3, [sp, #0x100]
10044722c: b40116c0    	cbz	x0, 0x100449504 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4210>
100447230: 39400008    	ldrb	w8, [x0]
100447234: 71001d1f    	cmp	w8, #0x7
100447238: 54011668    	b.hi	0x100449504 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4210>
10044723c: 52800029    	mov	w9, #0x1                ; =1
100447240: 1ac82129    	lsl	w9, w9, w8
100447244: 5280138a    	mov	w10, #0x9c              ; =156
100447248: 6a0a013f    	tst	w9, w10
10044724c: 910923ea    	add	x10, sp, #0x248
100447250: 540113e0    	b.eq	0x1004494cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x41d8>
100447254: f8401009    	ldur	x9, [x0, #0x1]
100447258: f90173e9    	str	x9, [sp, #0x2e0]
10044725c: f9400409    	ldr	x9, [x0, #0x8]
100447260: f809f149    	stur	x9, [x10, #0x9f]
100447264: 1400089c    	b	0x1004494d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x41e0>
100447268: 7100097f    	cmp	w11, #0x2
10044726c: 540059e2    	b.hs	0x100447da8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2ab4>
100447270: 71025b7f    	cmp	w27, #0x96
100447274: 540174c1    	b.ne	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447278: 1400057e    	b	0x100448870 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x357c>
10044727c: f1000dbf    	cmp	x13, #0x3
100447280: 910923ee    	add	x14, sp, #0x248
100447284: 5401eec1    	b.ne	0x10044b05c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d68>
100447288: 5280006c    	mov	w12, #0x3               ; =3
10044728c: 1400005c    	b	0x1004473fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2108>
100447290: f100091f    	cmp	x8, #0x2
100447294: 540211a3    	b.lo	0x10044b4c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61d4>
100447298: 8b0a010b    	add	x11, x8, x10
10044729c: d1000976    	sub	x22, x11, #0x2
1004472a0: eb1c02df    	cmp	x22, x28
1004472a4: 54022c82    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
1004472a8: d37ceecb    	lsl	x11, x22, #4
1004472ac: 386b692b    	ldrb	w11, [x9, x11]
1004472b0: 5100296b    	sub	w11, w11, #0xa
1004472b4: 7100157f    	cmp	w11, #0x5
1004472b8: 5401b543    	b.lo	0x10044a960 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x566c>
1004472bc: d1000508    	sub	x8, x8, #0x1
1004472c0: 8b080156    	add	x22, x10, x8
1004472c4: eb1c02df    	cmp	x22, x28
1004472c8: 54022b62    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
1004472cc: 8b161129    	add	x9, x9, x22, lsl #4
1004472d0: 3940013b    	ldrb	w27, [x9]
1004472d4: 51002b6a    	sub	w10, w27, #0xa
1004472d8: 7100155f    	cmp	w10, #0x5
1004472dc: 5401b423    	b.lo	0x10044a960 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x566c>
1004472e0: f9408bea    	ldr	x10, [sp, #0x110]
1004472e4: f9002148    	str	x8, [x10, #0x40]
1004472e8: b840112a    	ldur	w10, [x9, #0x1]
1004472ec: b81203aa    	stur	w10, [x29, #-0xe0]
1004472f0: b940052a    	ldr	w10, [x9, #0x4]
1004472f4: 910923eb    	add	x11, sp, #0x248
1004472f8: b80fb16a    	stur	w10, [x11, #0xfb]
1004472fc: f940052d    	ldr	x13, [x9, #0x8]
100447300: 528001ca    	mov	w10, #0xe               ; =14
100447304: 3900012a    	strb	w10, [x9]
100447308: f94083eb    	ldr	x11, [sp, #0x100]
10044730c: b4020de8    	cbz	x8, 0x10044b4c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61d4>
100447310: f940097c    	ldr	x28, [x11, #0x10]
100447314: f94077e9    	ldr	x9, [sp, #0xe8]
100447318: f940012a    	ldr	x10, [x9]
10044731c: d1000509    	sub	x9, x8, #0x1
100447320: 8b090156    	add	x22, x10, x9
100447324: eb1c02df    	cmp	x22, x28
100447328: 54022862    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
10044732c: f9001fed    	str	x13, [sp, #0x38]
100447330: f9400568    	ldr	x8, [x11, #0x8]
100447334: 8b161108    	add	x8, x8, x22, lsl #4
100447338: 39400118    	ldrb	w24, [x8]
10044733c: 51002b0a    	sub	w10, w24, #0xa
100447340: 7100115f    	cmp	w10, #0x4
100447344: 5401b0e9    	b.ls	0x10044a960 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x566c>
100447348: b900fbec    	str	w12, [sp, #0xf8]
10044734c: f9408be2    	ldr	x2, [sp, #0x110]
100447350: f9002049    	str	x9, [x2, #0x40]
100447354: b8401109    	ldur	w9, [x8, #0x1]
100447358: b81403a9    	stur	w9, [x29, #-0xc0]
10044735c: b9400509    	ldr	w9, [x8, #0x4]
100447360: d10303aa    	sub	x10, x29, #0xc0
100447364: b8003149    	stur	w9, [x10, #0x3]
100447368: f9400513    	ldr	x19, [x8, #0x8]
10044736c: 528001c9    	mov	w9, #0xe                ; =14
100447370: 39000109    	strb	w9, [x8]
100447374: f94083e8    	ldr	x8, [sp, #0x100]
100447378: a940f114    	ldp	x20, x28, [x8, #0x8]
10044737c: aa1403e0    	mov	x0, x20
100447380: aa1c03e1    	mov	x1, x28
100447384: 97f4dae6    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
100447388: aa0103f6    	mov	x22, x1
10044738c: 36005d40    	tbz	w0, #0x0, 0x100447f34 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2c40>
100447390: 910923ea    	add	x10, sp, #0x248
100447394: b4005ef6    	cbz	x22, 0x100447f70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2c7c>
100447398: 1400106f    	b	0x10044b554 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6260>
10044739c: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
1004473a0: f9408be1    	ldr	x1, [sp, #0x110]
1004473a4: 52800022    	mov	w2, #0x1                ; =1
1004473a8: 94001f00    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004473ac: f9408be4    	ldr	x4, [sp, #0x110]
1004473b0: f94083e3    	ldr	x3, [sp, #0x100]
1004473b4: 910923eb    	add	x11, sp, #0x248
1004473b8: b400c920    	cbz	x0, 0x100448cdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39e8>
1004473bc: 39400008    	ldrb	w8, [x0]
1004473c0: 71001d1f    	cmp	w8, #0x7
1004473c4: 5400c8c8    	b.hi	0x100448cdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39e8>
1004473c8: 52800029    	mov	w9, #0x1                ; =1
1004473cc: 1ac82129    	lsl	w9, w9, w8
1004473d0: 5280138a    	mov	w10, #0x9c              ; =156
1004473d4: 6a0a013f    	tst	w9, w10
1004473d8: 5400c680    	b.eq	0x100448ca8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39b4>
1004473dc: f8401009    	ldur	x9, [x0, #0x1]
1004473e0: f90173e9    	str	x9, [sp, #0x2e0]
1004473e4: f9400409    	ldr	x9, [x0, #0x8]
1004473e8: f809f169    	stur	x9, [x11, #0x9f]
1004473ec: 14000631    	b	0x100448cb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39bc>
1004473f0: 51000d8c    	sub	w12, w12, #0x3
1004473f4: 7100099f    	cmp	w12, #0x2
1004473f8: 1a9f27ec    	cset	w12, lo
1004473fc: 12001d6b    	and	w11, w11, #0xff
100447400: 7100097f    	cmp	w11, #0x2
100447404: 540000c2    	b.hs	0x10044741c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2128>
100447408: 7100059f    	cmp	w12, #0x1
10044740c: 540000c0    	b.eq	0x100447424 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2130>
100447410: 71000d9f    	cmp	w12, #0x3
100447414: 54000241    	b.ne	0x10044745c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2168>
100447418: 14000f21    	b	0x10044b09c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5da8>
10044741c: 7100059f    	cmp	w12, #0x1
100447420: 540001e1    	b.ne	0x10044745c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2168>
100447424: f94021eb    	ldr	x11, [x15, #0x40]
100447428: b40001ab    	cbz	x11, 0x10044745c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2168>
10044742c: d100056b    	sub	x11, x11, #0x1
100447430: 8b0b010c    	add	x12, x8, x11
100447434: eb1c019f    	cmp	x12, x28
100447438: 54000122    	b.hs	0x10044745c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2168>
10044743c: 8b0c112c    	add	x12, x9, x12, lsl #4
100447440: 3940018d    	ldrb	w13, [x12]
100447444: 710011bf    	cmp	w13, #0x4
100447448: 5400d9a0    	b.eq	0x100448f7c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c88>
10044744c: 71000dbf    	cmp	w13, #0x3
100447450: 54000061    	b.ne	0x10044745c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2168>
100447454: b9400588    	ldr	w8, [x12, #0x4]
100447458: 140006ca    	b	0x100448f80 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3c8c>
10044745c: f9408fea    	ldr	x10, [sp, #0x118]
100447460: f940114a    	ldr	x10, [x10, #0x20]
100447464: b4019a6a    	cbz	x10, 0x10044a7b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54bc>
100447468: 71009b7f    	cmp	w27, #0x26
10044746c: 54000060    	b.eq	0x100447478 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2184>
100447470: 7100b37f    	cmp	w27, #0x2c
100447474: 54000301    	b.ne	0x1004474d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x21e0>
100447478: f94021ea    	ldr	x10, [x15, #0x40]
10044747c: b402000a    	cbz	x10, 0x10044b47c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6188>
100447480: 8b080148    	add	x8, x10, x8
100447484: d1000516    	sub	x22, x8, #0x1
100447488: eb1c02df    	cmp	x22, x28
10044748c: 54021d42    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100447490: 8b161122    	add	x2, x9, x22, lsl #4
100447494: 39400048    	ldrb	w8, [x2]
100447498: 51002909    	sub	w9, w8, #0xa
10044749c: 7100113f    	cmp	w9, #0x4
1004474a0: 5401b429    	b.ls	0x10044ab24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5830>
1004474a4: 71001d1f    	cmp	w8, #0x7
1004474a8: 54008188    	b.hi	0x1004484d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x31e4>
1004474ac: 52800029    	mov	w9, #0x1                ; =1
1004474b0: 1ac82129    	lsl	w9, w9, w8
1004474b4: 5280138a    	mov	w10, #0x9c              ; =156
1004474b8: 6a0a013f    	tst	w9, w10
1004474bc: 540017c0    	b.eq	0x1004477b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24c0>
1004474c0: f8401049    	ldur	x9, [x2, #0x1]
1004474c4: f81203a9    	stur	x9, [x29, #-0xe0]
1004474c8: f9400449    	ldr	x9, [x2, #0x8]
1004474cc: f80ff1c9    	stur	x9, [x14, #0xff]
1004474d0: 140000bb    	b	0x1004477bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24c8>
1004474d4: f94021ea    	ldr	x10, [x15, #0x40]
1004474d8: b402020a    	cbz	x10, 0x10044b518 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6224>
1004474dc: d100054b    	sub	x11, x10, #0x1
1004474e0: 8b0b0116    	add	x22, x8, x11
1004474e4: eb1c02df    	cmp	x22, x28
1004474e8: 54021a62    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
1004474ec: 8b16112a    	add	x10, x9, x22, lsl #4
1004474f0: 39400148    	ldrb	w8, [x10]
1004474f4: 51002909    	sub	w9, w8, #0xa
1004474f8: 7100113f    	cmp	w9, #0x4
1004474fc: 5401cb29    	b.ls	0x10044ae60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b6c>
100447500: f90021eb    	str	x11, [x15, #0x40]
100447504: b8401149    	ldur	w9, [x10, #0x1]
100447508: b902e3e9    	str	w9, [sp, #0x2e0]
10044750c: b9400549    	ldr	w9, [x10, #0x4]
100447510: b809b1c9    	stur	w9, [x14, #0x9b]
100447514: f9400549    	ldr	x9, [x10, #0x8]
100447518: 528001cb    	mov	w11, #0xe               ; =14
10044751c: 3900014b    	strb	w11, [x10]
100447520: 140000b1    	b	0x1004477e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24f0>
100447524: aa0d03e1    	mov	x1, x13
100447528: 52800002    	mov	w2, #0x0                ; =0
10044752c: aa1303e3    	mov	x3, x19
100447530: 94001e9e    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447534: f9408be4    	ldr	x4, [sp, #0x110]
100447538: f94083e3    	ldr	x3, [sp, #0x100]
10044753c: 910923eb    	add	x11, sp, #0x248
100447540: b400c8c0    	cbz	x0, 0x100448e58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b64>
100447544: 39400008    	ldrb	w8, [x0]
100447548: 71001d1f    	cmp	w8, #0x7
10044754c: 5400c868    	b.hi	0x100448e58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b64>
100447550: 52800029    	mov	w9, #0x1                ; =1
100447554: 1ac82129    	lsl	w9, w9, w8
100447558: 5280138a    	mov	w10, #0x9c              ; =156
10044755c: 6a0a013f    	tst	w9, w10
100447560: 5400c620    	b.eq	0x100448e24 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b30>
100447564: f8401009    	ldur	x9, [x0, #0x1]
100447568: f90173e9    	str	x9, [sp, #0x2e0]
10044756c: f9400409    	ldr	x9, [x0, #0x8]
100447570: f809f169    	stur	x9, [x11, #0x9f]
100447574: 1400062e    	b	0x100448e2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b38>
100447578: f9402048    	ldr	x8, [x2, #0x40]
10044757c: b401fe28    	cbz	x8, 0x10044b540 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x624c>
100447580: f940099c    	ldr	x28, [x12, #0x10]
100447584: f94077e9    	ldr	x9, [sp, #0xe8]
100447588: f940012a    	ldr	x10, [x9]
10044758c: d1000509    	sub	x9, x8, #0x1
100447590: 8b090156    	add	x22, x10, x9
100447594: eb1c02df    	cmp	x22, x28
100447598: 540214e2    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
10044759c: f9400588    	ldr	x8, [x12, #0x8]
1004475a0: 8b16110a    	add	x10, x8, x22, lsl #4
1004475a4: 39400148    	ldrb	w8, [x10]
1004475a8: 5100290b    	sub	w11, w8, #0xa
1004475ac: 7100117f    	cmp	w11, #0x4
1004475b0: 5401dba9    	b.ls	0x10044b124 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e30>
1004475b4: f9002049    	str	x9, [x2, #0x40]
1004475b8: b8401149    	ldur	w9, [x10, #0x1]
1004475bc: b902e3e9    	str	w9, [sp, #0x2e0]
1004475c0: b9400549    	ldr	w9, [x10, #0x4]
1004475c4: b809b1c9    	stur	w9, [x14, #0x9b]
1004475c8: f9400549    	ldr	x9, [x10, #0x8]
1004475cc: 528001cb    	mov	w11, #0xe               ; =14
1004475d0: 3900014b    	strb	w11, [x10]
1004475d4: 140003d8    	b	0x100448534 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3240>
1004475d8: 5280002c    	mov	w12, #0x1               ; =1
1004475dc: b8767b09    	ldr	w9, [x24, x22, lsl #2]
1004475e0: 7210013f    	tst	w9, #0x10000
1004475e4: f94067ed    	ldr	x13, [sp, #0xc8]
1004475e8: f9406fef    	ldr	x15, [sp, #0xd8]
1004475ec: 9a8d01ed    	csel	x13, x15, x13, eq
1004475f0: f94077ee    	ldr	x14, [sp, #0xe8]
1004475f4: 9a8f01ce    	csel	x14, x14, x15, eq
1004475f8: f94001c1    	ldr	x1, [x14]
1004475fc: f94001b6    	ldr	x22, [x13]
100447600: eb16002d    	subs	x13, x1, x22
100447604: 5401f1e3    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100447608: eb0a003f    	cmp	x1, x10
10044760c: 5401f8c8    	b.hi	0x10044b524 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6230>
100447610: 92403d2a    	and	x10, x9, #0xffff
100447614: eb0a01bf    	cmp	x13, x10
100447618: 54013609    	b.ls	0x100449cd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e4>
10044761c: 8b16116b    	add	x11, x11, x22, lsl #4
100447620: 8b0a116a    	add	x10, x11, x10, lsl #4
100447624: 3940014b    	ldrb	w11, [x10]
100447628: 7100297f    	cmp	w11, #0xa
10044762c: 54013562    	b.hs	0x100449cd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e4>
100447630: 71000d7f    	cmp	w11, #0x3
100447634: 54013500    	b.eq	0x100449cd4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e0>
100447638: 7100117f    	cmp	w11, #0x4
10044763c: 540134e1    	b.ne	0x100449cd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e4>
100447640: 370134cc    	tbnz	w12, #0x0, 0x100449cd8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x49e4>
100447644: fd400541    	ldr	d1, [x10, #0x8]
100447648: 14000acb    	b	0x10044a174 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e80>
10044764c: 5280002b    	mov	w11, #0x1               ; =1
100447650: b8767b0c    	ldr	w12, [x24, x22, lsl #2]
100447654: 7210019f    	tst	w12, #0x10000
100447658: f94067ed    	ldr	x13, [sp, #0xc8]
10044765c: f9406fef    	ldr	x15, [sp, #0xd8]
100447660: 9a8d01ed    	csel	x13, x15, x13, eq
100447664: f94077ee    	ldr	x14, [sp, #0xe8]
100447668: 9a8f01ce    	csel	x14, x14, x15, eq
10044766c: f94001c1    	ldr	x1, [x14]
100447670: f94001b6    	ldr	x22, [x13]
100447674: eb16002d    	subs	x13, x1, x22
100447678: 5401ee43    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
10044767c: eb09003f    	cmp	x1, x9
100447680: 5401f2e8    	b.hi	0x10044b4dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61e8>
100447684: 92403d89    	and	x9, x12, #0xffff
100447688: eb0901bf    	cmp	x13, x9
10044768c: 540141a9    	b.ls	0x100449ec0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bcc>
100447690: 8b16114a    	add	x10, x10, x22, lsl #4
100447694: 8b091149    	add	x9, x10, x9, lsl #4
100447698: 3940012a    	ldrb	w10, [x9]
10044769c: 7100295f    	cmp	w10, #0xa
1004476a0: 54014102    	b.hs	0x100449ec0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bcc>
1004476a4: 71000d5f    	cmp	w10, #0x3
1004476a8: 540140a0    	b.eq	0x100449ebc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bc8>
1004476ac: 7100115f    	cmp	w10, #0x4
1004476b0: 54014081    	b.ne	0x100449ec0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bcc>
1004476b4: 3701406b    	tbnz	w11, #0x0, 0x100449ec0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4bcc>
1004476b8: fd400521    	ldr	d1, [x9, #0x8]
1004476bc: 14000ac2    	b	0x10044a1c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ed0>
1004476c0: 1e620160    	scvtf	d0, w11
1004476c4: 1e614008    	fneg	d8, d0
1004476c8: 1e780113    	fcvtzs	w19, d8
1004476cc: 1e620260    	scvtf	d0, w19
1004476d0: 1e602100    	fcmp	d8, d0
1004476d4: 540000a1    	b.ne	0x1004476e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x23f4>
1004476d8: 5280000a    	mov	w10, #0x0               ; =0
1004476dc: 350003cb    	cbnz	w11, 0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
1004476e0: 9e66010b    	fmov	x11, d8
1004476e4: b6f8038b    	tbz	x11, #0x3f, 0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
1004476e8: 5280002a    	mov	w10, #0x1               ; =1
1004476ec: 1400001a    	b	0x100447754 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2460>
1004476f0: 5280000a    	mov	w10, #0x0               ; =0
1004476f4: 1e65c000    	frintz	d0, d0
1004476f8: d2e7be0b    	mov	x11, #0x3df0000000000000 ; =4463067230724161536
1004476fc: 9e670161    	fmov	d1, x11
100447700: 1e610801    	fmul	d1, d0, d1
100447704: 1e65c021    	frintz	d1, d1
100447708: d2f83e0b    	mov	x11, #-0x3e10000000000000 ; =-4472074429978902528
10044770c: 9e670162    	fmov	d2, x11
100447710: 1f420021    	fmadd	d1, d1, d2, d0
100447714: 6f07e7e3    	movi.2d	v3, #0xffffffffffffffff
100447718: 6ee0f863    	fneg.2d	v3, v3
10044771c: 6ea31c20    	bit.16b	v0, v1, v3
100447720: d2e83e0b    	mov	x11, #0x41f0000000000000 ; =4751297606875873280
100447724: 9e670161    	fmov	d1, x11
100447728: 1e612801    	fadd	d1, d0, d1
10044772c: 1e602008    	fcmp	d0, #0.0
100447730: 1e604c20    	fcsel	d0, d1, d0, mi
100447734: 1e622801    	fadd	d1, d0, d2
100447738: 1e78002b    	fcvtzs	w11, d1
10044773c: d2e83c0c    	mov	x12, #0x41e0000000000000 ; =4746794007248502784
100447740: 9e670181    	fmov	d1, x12
100447744: 1e78000c    	fcvtzs	w12, d0
100447748: 1e612000    	fcmp	d0, d1
10044774c: 1a8bb18b    	csel	w11, w12, w11, lt
100447750: 2a2b03f3    	mvn	w19, w11
100447754: f9002048    	str	x8, [x2, #0x40]
100447758: 528001c8    	mov	w8, #0xe                ; =14
10044775c: 39000128    	strb	w8, [x9]
100447760: a940f234    	ldp	x20, x28, [x17, #0x8]
100447764: 7100015f    	cmp	w10, #0x0
100447768: 52800068    	mov	w8, #0x3                ; =3
10044776c: 1a880518    	cinc	w24, w8, ne
100447770: aa1403e0    	mov	x0, x20
100447774: aa1c03e1    	mov	x1, x28
100447778: 97f4d9e9    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044777c: aa0103f6    	mov	x22, x1
100447780: 36000060    	tbz	w0, #0x0, 0x10044778c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2498>
100447784: b4014c56    	cbz	x22, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447788: 14000f73    	b	0x10044b554 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6260>
10044778c: eb16039f    	cmp	x28, x22
100447790: 54020829    	b.ls	0x10044b894 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65a0>
100447794: 8b161288    	add	x8, x20, x22, lsl #4
100447798: 39000118    	strb	w24, [x8]
10044779c: b9000513    	str	w19, [x8, #0x4]
1004477a0: fd000508    	str	d8, [x8, #0x8]
1004477a4: f9408be9    	ldr	x9, [sp, #0x110]
1004477a8: f9402128    	ldr	x8, [x9, #0x40]
1004477ac: 91000508    	add	x8, x8, #0x1
1004477b0: 17fff7fc    	b	0x1004457a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ac>
1004477b4: 7200053f    	tst	w9, #0x3
1004477b8: 54006900    	b.eq	0x1004484d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x31e4>
1004477bc: f85203a9    	ldur	x9, [x29, #-0xe0]
1004477c0: f94073ea    	ldr	x10, [sp, #0xe0]
1004477c4: f9000149    	str	x9, [x10]
1004477c8: f84ff1c9    	ldur	x9, [x14, #0xff]
1004477cc: f8007149    	stur	x9, [x10, #0x7]
1004477d0: b9400149    	ldr	w9, [x10]
1004477d4: b902e3e9    	str	w9, [sp, #0x2e0]
1004477d8: b8403149    	ldur	w9, [x10, #0x3]
1004477dc: b809b1c9    	stur	w9, [x14, #0x9b]
1004477e0: f85483a9    	ldur	x9, [x29, #-0xb8]
1004477e4: f9406fea    	ldr	x10, [sp, #0xd8]
1004477e8: f940014a    	ldr	x10, [x10]
1004477ec: f94077eb    	ldr	x11, [sp, #0xe8]
1004477f0: f940016b    	ldr	x11, [x11]
1004477f4: eb0a016b    	subs	x11, x11, x10
1004477f8: 9a8b33eb    	csel	x11, xzr, x11, lo
1004477fc: eb13017f    	cmp	x11, x19
100447800: 54018f09    	b.ls	0x10044a9e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56ec>
100447804: f9400a1c    	ldr	x28, [x16, #0x10]
100447808: 8b130156    	add	x22, x10, x19
10044780c: eb1c02df    	cmp	x22, x28
100447810: 540203c2    	b.hs	0x10044b888 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6594>
100447814: f940060a    	ldr	x10, [x16, #0x8]
100447818: 8b16114a    	add	x10, x10, x22, lsl #4
10044781c: 3940014b    	ldrb	w11, [x10]
100447820: 7100397f    	cmp	w11, #0xe
100447824: 540191a0    	b.eq	0x10044aa58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5764>
100447828: b840114c    	ldur	w12, [x10, #0x1]
10044782c: f9405fed    	ldr	x13, [sp, #0xb8]
100447830: b90001ac    	str	w12, [x13]
100447834: b940054c    	ldr	w12, [x10, #0x4]
100447838: b80031ac    	stur	w12, [x13, #0x3]
10044783c: f940054c    	ldr	x12, [x10, #0x8]
100447840: 39000148    	strb	w8, [x10]
100447844: b942e3e8    	ldr	w8, [sp, #0x2e0]
100447848: b8001148    	stur	w8, [x10, #0x1]
10044784c: b849b1c8    	ldur	w8, [x14, #0x9b]
100447850: b9000548    	str	w8, [x10, #0x4]
100447854: f9000549    	str	x9, [x10, #0x8]
100447858: 3906a3eb    	strb	w11, [sp, #0x1a8]
10044785c: f900dbec    	str	x12, [sp, #0x1b0]
100447860: f9408fea    	ldr	x10, [sp, #0x118]
100447864: f9401149    	ldr	x9, [x10, #0x20]
100447868: f9400228    	ldr	x8, [x17]
10044786c: f9001559    	str	x25, [x10, #0x28]
100447870: f940150a    	ldr	x10, [x8, #0x28]
100447874: b501e3ca    	cbnz	x10, 0x10044b4ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61f8>
100447878: 9280000a    	mov	x10, #-0x1              ; =-1
10044787c: f900150a    	str	x10, [x8, #0x28]
100447880: 3947210a    	ldrb	w10, [x8, #0x1c8]
100447884: 7100095f    	cmp	w10, #0x2
100447888: 540000e1    	b.ne	0x1004478a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b0>
10044788c: f940e10a    	ldr	x10, [x8, #0x1c0]
100447890: b401ab8a    	cbz	x10, 0x10044ae00 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b0c>
100447894: f940dd0b    	ldr	x11, [x8, #0x1b8]
100447898: 8b0a196a    	add	x10, x11, x10, lsl #6
10044789c: d101014a    	sub	x10, x10, #0x40
1004478a0: 14000002    	b	0x1004478a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25b4>
1004478a4: 9107210a    	add	x10, x8, #0x1c8
1004478a8: f940114b    	ldr	x11, [x10, #0x20]
1004478ac: eb09017f    	cmp	x11, x9
1004478b0: 540190a1    	b.ne	0x10044aac4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57d0>
1004478b4: 39400149    	ldrb	w9, [x10]
1004478b8: 370190e9    	tbnz	w9, #0x0, 0x10044aad4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57e0>
1004478bc: b9401149    	ldr	w9, [x10, #0x10]
1004478c0: 7100053f    	cmp	w9, #0x1
1004478c4: 540000c1    	b.ne	0x1004478dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25e8>
1004478c8: f9400d49    	ldr	x9, [x10, #0x18]
1004478cc: eb19013f    	cmp	x9, x25
1004478d0: 54000061    	b.ne	0x1004478dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25e8>
1004478d4: d2800009    	mov	x9, #0x0                ; =0
1004478d8: 14000005    	b	0x1004478ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x25f8>
1004478dc: 52800029    	mov	w9, #0x1                ; =1
1004478e0: a9016549    	stp	x9, x25, [x10, #0x10]
1004478e4: f9401509    	ldr	x9, [x8, #0x28]
1004478e8: 91000529    	add	x9, x9, #0x1
1004478ec: f9001509    	str	x9, [x8, #0x28]
1004478f0: f9400220    	ldr	x0, [x17]
1004478f4: 9106a3e1    	add	x1, sp, #0x1a8
1004478f8: 97f46b45    	bl	0x10016260c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
1004478fc: 14000a03    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100447900: 7103377f    	cmp	w27, #0xcd
100447904: 540040e1    	b.ne	0x100448120 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2e2c>
100447908: f9400196    	ldr	x22, [x12]
10044790c: 1400020f    	b	0x100448148 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2e54>
100447910: b9401549    	ldr	w9, [x10, #0x14]
100447914: b90033e9    	str	w9, [sp, #0x30]
100447918: 37f9d829    	tbnz	w9, #0x1f, 0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
10044791c: f9407be9    	ldr	x9, [sp, #0xf0]
100447920: f9400129    	ldr	x9, [x9]
100447924: f9007fe9    	str	x9, [sp, #0xf8]
100447928: 7100251f    	cmp	w8, #0x9
10044792c: f9407fe1    	ldr	x1, [sp, #0xf8]
100447930: 5401d761    	b.ne	0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
100447934: d10303a0    	sub	x0, x29, #0xc0
100447938: f9401fe2    	ldr	x2, [sp, #0x38]
10044793c: 940029bb    	bl	0x100452028 <__ZN13quickjs_oxide6engine4heap14slot_ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$36slot_value_release_readiness_jsvalue17h2038e983b6e2b9bfE>
100447940: 385403a8    	ldurb	w8, [x29, #-0xc0]
100447944: 71002d1f    	cmp	w8, #0xb
100447948: 54019be1    	b.ne	0x10044acc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59d0>
10044794c: 385413a9    	ldurb	w9, [x29, #-0xbf]
100447950: 35019ba9    	cbnz	w9, 0x10044acc4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59d0>
100447954: f9407fe9    	ldr	x9, [sp, #0xf8]
100447958: f9401528    	ldr	x8, [x9, #0x28]
10044795c: b501d608    	cbnz	x8, 0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
100447960: 92800008    	mov	x8, #-0x1               ; =-1
100447964: f9001528    	str	x8, [x9, #0x28]
100447968: f9401fe8    	ldr	x8, [sp, #0x38]
10044796c: 2940a10a    	ldp	w10, w8, [x8, #0x4]
100447970: a94fcd21    	ldp	x1, x19, [x9, #0xf8]
100447974: 29032be8    	stp	w8, w10, [sp, #0x18]
100447978: b9030be8    	str	w8, [sp, #0x308]
10044797c: b90307ea    	str	w10, [sp, #0x304]
100447980: b90303ff    	str	wzr, [sp, #0x300]
100447984: d10303a0    	sub	x0, x29, #0xc0
100447988: 910c03e3    	add	x3, sp, #0x300
10044798c: aa0103f8    	mov	x24, x1
100447990: aa1303e2    	mov	x2, x19
100447994: 97efa9f8    	bl	0x100032174 <__ZN13quickjs_oxide6engine4heap14object_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$22validate_slot_identity17h9c24141d9f2cc0c4E>
100447998: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044799c: 7100191f    	cmp	w8, #0x6
1004479a0: 5401d361    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
1004479a4: f85483b4    	ldur	x20, [x29, #-0xb8]
1004479a8: f9000bf3    	str	x19, [sp, #0x10]
1004479ac: eb13029f    	cmp	x20, x19
1004479b0: 5401fb02    	b.hs	0x10044b910 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x661c>
1004479b4: 52802308    	mov	w8, #0x118              ; =280
1004479b8: 9b086293    	madd	x19, x20, x8, x24
1004479bc: 39400268    	ldrb	w8, [x19]
1004479c0: 7100051f    	cmp	w8, #0x1
1004479c4: 5401d241    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
1004479c8: aa1303e2    	mov	x2, x19
1004479cc: f8408c48    	ldr	x8, [x2, #0x8]!
1004479d0: f100051f    	cmp	x8, #0x1
1004479d4: 5401d1c8    	b.hi	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
1004479d8: aa1803e1    	mov	x1, x24
1004479dc: 3943b669    	ldrb	w9, [x19, #0xed]
1004479e0: 39416268    	ldrb	w8, [x19, #0x58]
1004479e4: 71000d3f    	cmp	w9, #0x3
1004479e8: 540002a1    	b.ne	0x100447a3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2748>
1004479ec: 7100091f    	cmp	w8, #0x2
1004479f0: 54000261    	b.ne	0x100447a3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2748>
1004479f4: f9403268    	ldr	x8, [x19, #0x60]
1004479f8: d2f00009    	mov	x9, #-0x8000000000000000 ; =-9223372036854775808
1004479fc: eb09011f    	cmp	x8, x9
100447a00: 54006da1    	b.ne	0x1004487b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x34c0>
100447a04: d10303a0    	sub	x0, x29, #0xc0
100447a08: f9407fe8    	ldr	x8, [sp, #0xf8]
100447a0c: 9100c101    	add	x1, x8, #0x30
100447a10: b94033e3    	ldr	w3, [sp, #0x30]
100447a14: 94002a4b    	bl	0x100452340 <__ZN13quickjs_oxide6engine6object16ordinary_storage29materialized_array_own_number17h87c9af0567eda137E>
100447a18: b85403a8    	ldur	w8, [x29, #-0xc0]
100447a1c: 7100091f    	cmp	w8, #0x2
100447a20: 5401cf60    	b.eq	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100447a24: 3600ade8    	tbz	w8, #0x0, 0x100448fe0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cec>
100447a28: fc5483a0    	ldur	d0, [x29, #-0xb8]
100447a2c: 52800088    	mov	w8, #0x4                ; =4
100447a30: 390b03e8    	strb	w8, [sp, #0x2c0]
100447a34: fd0167e0    	str	d0, [sp, #0x2c8]
100447a38: 1400056e    	b	0x100448ff0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cfc>
100447a3c: 71000d1f    	cmp	w8, #0x3
100447a40: 540063e1    	b.ne	0x1004486bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x33c8>
100447a44: b94033e8    	ldr	w8, [sp, #0x30]
100447a48: 37f9ce28    	tbnz	w8, #0x1f, 0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100447a4c: d10303a0    	sub	x0, x29, #0xc0
100447a50: b94033e8    	ldr	w8, [sp, #0x30]
100447a54: 32010105    	orr	w5, w8, #0x80000000
100447a58: f9400be2    	ldr	x2, [sp, #0x10]
100447a5c: 29430fe4    	ldp	w4, w3, [sp, #0x18]
100447a60: 97f090b2    	bl	0x10006bd28 <__ZN13quickjs_oxide6engine6object16ordinary_storage6locate17heb1b0d901cfe2fdaE>
100447a64: 385403a0    	ldurb	w0, [x29, #-0xc0]
100447a68: f85483b4    	ldur	x20, [x29, #-0xb8]
100447a6c: 71002c1f    	cmp	w0, #0xb
100447a70: 5401c841    	b.ne	0x10044b378 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6084>
100447a74: 385503a8    	ldurb	w8, [x29, #-0xb0]
100447a78: 121f1908    	and	w8, w8, #0xfe
100447a7c: 7100091f    	cmp	w8, #0x2
100447a80: 5401cc60    	b.eq	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100447a84: 91008260    	add	x0, x19, #0x20
100447a88: 97f091ef    	bl	0x10006c244 <__ZN94_$LT$quickjs_oxide..engine..heap..object_records..Slots$u20$as$u20$core..ops..deref..Deref$GT$5deref17ha37b741159f3cf0eE>
100447a8c: eb01029f    	cmp	x20, x1
100447a90: 5401fd62    	b.hs	0x10044ba3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6748>
100447a94: 52800308    	mov	w8, #0x18               ; =24
100447a98: 9b080288    	madd	x8, x20, x8, x0
100447a9c: b9400109    	ldr	w9, [x8]
100447aa0: 3400b3e9    	cbz	w9, 0x10044911c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3e28>
100447aa4: 7100053f    	cmp	w9, #0x1
100447aa8: 5401cb21    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100447aac: 29409103    	ldp	w3, w4, [x8, #0x4]
100447ab0: f9407fe8    	ldr	x8, [sp, #0xf8]
100447ab4: a94f8901    	ldp	x1, x2, [x8, #0xf8]
100447ab8: d10303a0    	sub	x0, x29, #0xc0
100447abc: 97f12c91    	bl	0x100092d00 <__ZN13quickjs_oxide6engine4heap15binding_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$7var_ref17hdb209f11d90c7491E>
100447ac0: 385403a8    	ldurb	w8, [x29, #-0xc0]
100447ac4: 7100191f    	cmp	w8, #0x6
100447ac8: 5401ca21    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100447acc: f85483a1    	ldur	x1, [x29, #-0xb8]
100447ad0: 14000594    	b	0x100449120 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3e2c>
100447ad4: f9408be9    	ldr	x9, [sp, #0x110]
100447ad8: f9401d29    	ldr	x9, [x9, #0x38]
100447adc: eb080128    	subs	x8, x9, x8
100447ae0: 9a8833e8    	csel	x8, xzr, x8, lo
100447ae4: cb0a0108    	sub	x8, x8, x10
100447ae8: f100091f    	cmp	x8, #0x2
100447aec: 54006748    	b.hi	0x1004487d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x34e0>
100447af0: b00013b3    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
100447af4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
100447af8: 52800574    	mov	w20, #0x2b              ; =43
100447afc: 52800560    	mov	w0, #0x2b               ; =43
100447b00: 94033084    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
100447b04: b401f6c0    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
100447b08: f0000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
100447b0c: 91066d08    	add	x8, x8, #0x19b
100447b10: ad400500    	ldp	q0, q1, [x8]
100447b14: ad000400    	stp	q0, q1, [x0]
100447b18: 3cc1b100    	ldur	q0, [x8, #0x1b]
100447b1c: 3c81b000    	stur	q0, [x0, #0x1b]
100447b20: 528000a8    	mov	w8, #0x5                ; =5
100447b24: 381883a8    	sturb	w8, [x29, #-0x78]
100447b28: 52800568    	mov	w8, #0x2b               ; =43
100447b2c: a93723a0    	stp	x0, x8, [x29, #-0x90]
100447b30: f81803bf    	stur	xzr, [x29, #-0x80]
100447b34: f81683a8    	stur	x8, [x29, #-0x98]
100447b38: f81403bf    	stur	xzr, [x29, #-0xc0]
100447b3c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
100447b40: 52800a00    	mov	w0, #0x50               ; =80
100447b44: 94033073    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
100447b48: b401e6e0    	cbz	x0, 0x10044b824 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6530>
100447b4c: ad7b07a0    	ldp	q0, q1, [x29, #-0xa0]
100447b50: ad010400    	stp	q0, q1, [x0, #0x20]
100447b54: 3cd803a0    	ldur	q0, [x29, #-0x80]
100447b58: 3d801000    	str	q0, [x0, #0x40]
100447b5c: ad7a03a1    	ldp	q1, q0, [x29, #-0xc0]
100447b60: ad000001    	stp	q1, q0, [x0]
100447b64: b4012d40    	cbz	x0, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447b68: 14000cb0    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100447b6c: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100447b70: f9408be1    	ldr	x1, [sp, #0x110]
100447b74: 52800022    	mov	w2, #0x1                ; =1
100447b78: 94001d0c    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447b7c: f9408be4    	ldr	x4, [sp, #0x110]
100447b80: f94083e3    	ldr	x3, [sp, #0x100]
100447b84: b400b840    	cbz	x0, 0x10044928c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f98>
100447b88: 39400008    	ldrb	w8, [x0]
100447b8c: 71001d1f    	cmp	w8, #0x7
100447b90: 5400b7e8    	b.hi	0x10044928c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f98>
100447b94: 52800029    	mov	w9, #0x1                ; =1
100447b98: 1ac82129    	lsl	w9, w9, w8
100447b9c: 5280138a    	mov	w10, #0x9c              ; =156
100447ba0: 6a0a013f    	tst	w9, w10
100447ba4: 910923ea    	add	x10, sp, #0x248
100447ba8: 5400b560    	b.eq	0x100449254 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f60>
100447bac: f8401009    	ldur	x9, [x0, #0x1]
100447bb0: f90173e9    	str	x9, [sp, #0x2e0]
100447bb4: f9400409    	ldr	x9, [x0, #0x8]
100447bb8: f809f149    	stur	x9, [x10, #0x9f]
100447bbc: 140005a8    	b	0x10044925c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f68>
100447bc0: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100447bc4: f9408be1    	ldr	x1, [sp, #0x110]
100447bc8: 52800002    	mov	w2, #0x0                ; =0
100447bcc: 94001cf7    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447bd0: f9408be4    	ldr	x4, [sp, #0x110]
100447bd4: f94083e3    	ldr	x3, [sp, #0x100]
100447bd8: b400bb20    	cbz	x0, 0x10044933c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4048>
100447bdc: 39400008    	ldrb	w8, [x0]
100447be0: 71001d1f    	cmp	w8, #0x7
100447be4: 5400bac8    	b.hi	0x10044933c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4048>
100447be8: 52800029    	mov	w9, #0x1                ; =1
100447bec: 1ac82129    	lsl	w9, w9, w8
100447bf0: 5280138a    	mov	w10, #0x9c              ; =156
100447bf4: 6a0a013f    	tst	w9, w10
100447bf8: 910923ea    	add	x10, sp, #0x248
100447bfc: 5400b840    	b.eq	0x100449304 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4010>
100447c00: f8401009    	ldur	x9, [x0, #0x1]
100447c04: f90173e9    	str	x9, [sp, #0x2e0]
100447c08: f9400409    	ldr	x9, [x0, #0x8]
100447c0c: f809f149    	stur	x9, [x10, #0x9f]
100447c10: 140005bf    	b	0x10044930c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4018>
100447c14: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100447c18: f9408be1    	ldr	x1, [sp, #0x110]
100447c1c: 52800002    	mov	w2, #0x0                ; =0
100447c20: 94001ce2    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447c24: f9408be4    	ldr	x4, [sp, #0x110]
100447c28: f94083e3    	ldr	x3, [sp, #0x100]
100447c2c: b400c260    	cbz	x0, 0x100449478 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4184>
100447c30: 39400008    	ldrb	w8, [x0]
100447c34: 71001d1f    	cmp	w8, #0x7
100447c38: 5400c208    	b.hi	0x100449478 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4184>
100447c3c: 52800029    	mov	w9, #0x1                ; =1
100447c40: 1ac82129    	lsl	w9, w9, w8
100447c44: 5280138a    	mov	w10, #0x9c              ; =156
100447c48: 6a0a013f    	tst	w9, w10
100447c4c: 910923ea    	add	x10, sp, #0x248
100447c50: 5400bf80    	b.eq	0x100449440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x414c>
100447c54: f8401009    	ldur	x9, [x0, #0x1]
100447c58: f90173e9    	str	x9, [sp, #0x2e0]
100447c5c: f9400409    	ldr	x9, [x0, #0x8]
100447c60: f809f149    	stur	x9, [x10, #0x9f]
100447c64: 140005f9    	b	0x100449448 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4154>
100447c68: b94257e8    	ldr	w8, [sp, #0x254]
100447c6c: b81443a8    	stur	w8, [x29, #-0xbc]
100447c70: 52800068    	mov	w8, #0x3                ; =3
100447c74: 381403a8    	sturb	w8, [x29, #-0xc0]
100447c78: d10303a2    	sub	x2, x29, #0xc0
100447c7c: 94001cad    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100447c80: b5018d40    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100447c84: 11001299    	add	w25, w20, #0x4
100447c88: 14000922    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
100447c8c: 52800002    	mov	w2, #0x0                ; =0
100447c90: f9407fe3    	ldr	x3, [sp, #0xf8]
100447c94: 94001cc5    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447c98: f9408be4    	ldr	x4, [sp, #0x110]
100447c9c: f94083e3    	ldr	x3, [sp, #0x100]
100447ca0: b400c8a0    	cbz	x0, 0x1004495b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x42c0>
100447ca4: 39400008    	ldrb	w8, [x0]
100447ca8: 71001d1f    	cmp	w8, #0x7
100447cac: 5400c848    	b.hi	0x1004495b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x42c0>
100447cb0: 52800029    	mov	w9, #0x1                ; =1
100447cb4: 1ac82129    	lsl	w9, w9, w8
100447cb8: 5280138a    	mov	w10, #0x9c              ; =156
100447cbc: 6a0a013f    	tst	w9, w10
100447cc0: 910923ea    	add	x10, sp, #0x248
100447cc4: 5400c5c0    	b.eq	0x10044957c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4288>
100447cc8: f8401009    	ldur	x9, [x0, #0x1]
100447ccc: f90173e9    	str	x9, [sp, #0x2e0]
100447cd0: f9400409    	ldr	x9, [x0, #0x8]
100447cd4: f809f149    	stur	x9, [x10, #0x9f]
100447cd8: 1400062b    	b	0x100449584 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4290>
100447cdc: 7942fbf4    	ldrh	w20, [sp, #0x17c]
100447ce0: f94083e0    	ldr	x0, [sp, #0x100]
100447ce4: f9408be1    	ldr	x1, [sp, #0x110]
100447ce8: 52800002    	mov	w2, #0x0                ; =0
100447cec: aa1403e3    	mov	x3, x20
100447cf0: 94001cae    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447cf4: f9408be4    	ldr	x4, [sp, #0x110]
100447cf8: f94083e3    	ldr	x3, [sp, #0x100]
100447cfc: b400ca20    	cbz	x0, 0x100449640 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x434c>
100447d00: 39400008    	ldrb	w8, [x0]
100447d04: 71001d1f    	cmp	w8, #0x7
100447d08: 5400c9c8    	b.hi	0x100449640 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x434c>
100447d0c: 52800029    	mov	w9, #0x1                ; =1
100447d10: 1ac82129    	lsl	w9, w9, w8
100447d14: 5280138a    	mov	w10, #0x9c              ; =156
100447d18: 6a0a013f    	tst	w9, w10
100447d1c: 910923ea    	add	x10, sp, #0x248
100447d20: 5400c740    	b.eq	0x100449608 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4314>
100447d24: f8401009    	ldur	x9, [x0, #0x1]
100447d28: f90173e9    	str	x9, [sp, #0x2e0]
100447d2c: f9400409    	ldr	x9, [x0, #0x8]
100447d30: f809f149    	stur	x9, [x10, #0x9f]
100447d34: 14000637    	b	0x100449610 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x431c>
100447d38: 7942fff4    	ldrh	w20, [sp, #0x17e]
100447d3c: f94083e0    	ldr	x0, [sp, #0x100]
100447d40: f9408be1    	ldr	x1, [sp, #0x110]
100447d44: 52800002    	mov	w2, #0x0                ; =0
100447d48: aa1403e3    	mov	x3, x20
100447d4c: 94001c97    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447d50: f9408be4    	ldr	x4, [sp, #0x110]
100447d54: f94083e3    	ldr	x3, [sp, #0x100]
100447d58: b400cbc0    	cbz	x0, 0x1004496d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43dc>
100447d5c: 39400008    	ldrb	w8, [x0]
100447d60: 71001d1f    	cmp	w8, #0x7
100447d64: 5400cb68    	b.hi	0x1004496d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43dc>
100447d68: 52800029    	mov	w9, #0x1                ; =1
100447d6c: 1ac82129    	lsl	w9, w9, w8
100447d70: 5280138a    	mov	w10, #0x9c              ; =156
100447d74: 6a0a013f    	tst	w9, w10
100447d78: 910923ea    	add	x10, sp, #0x248
100447d7c: 5400c8e0    	b.eq	0x100449698 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43a4>
100447d80: f8401009    	ldur	x9, [x0, #0x1]
100447d84: f90173e9    	str	x9, [sp, #0x2e0]
100447d88: f9400409    	ldr	x9, [x0, #0x8]
100447d8c: f809f149    	stur	x9, [x10, #0x9f]
100447d90: 14000644    	b	0x1004496a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43ac>
100447d94: 710001bf    	cmp	w13, #0x0
100447d98: 1a9f07e9    	cset	w9, ne
100447d9c: 7100091f    	cmp	w8, #0x2
100447da0: 1a890188    	csel	w8, w12, w9, eq
100447da4: 140002ae    	b	0x10044885c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3568>
100447da8: 7100111f    	cmp	w8, #0x4
100447dac: 54005541    	b.ne	0x100448854 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3560>
100447db0: 9e670120    	fmov	d0, x9
100447db4: 1e602008    	fcmp	d0, #0.0
100447db8: 54fef6c0    	b.eq	0x100445c90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x99c>
100447dbc: 1e602000    	fcmp	d0, d0
100447dc0: 1a9f67e8    	cset	w8, vc
100447dc4: 140002a6    	b	0x10044885c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3568>
100447dc8: b501a169    	cbnz	x9, 0x10044b1f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f00>
100447dcc: 51000d08    	sub	w8, w8, #0x3
100447dd0: 7100091f    	cmp	w8, #0x2
100447dd4: 54ff0a62    	b.hs	0x100445f20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xc2c>
100447dd8: 7100bf7f    	cmp	w27, #0x2f
100447ddc: 1a9f17e4    	cset	w4, eq
100447de0: aa1c03e1    	mov	x1, x28
100447de4: f9407fe3    	ldr	x3, [sp, #0xf8]
100447de8: 940025cd    	bl	0x10045151c <__ZN13quickjs_oxide6engine2vm5stack6number61_$LT$impl$u20$quickjs_oxide..engine..vm..stack..SlotStore$GT$35store_proven_number_operand_current17h5730d3b1bb9520a0E>
100447dec: f9408be2    	ldr	x2, [sp, #0x110]
100447df0: f94083ec    	ldr	x12, [sp, #0x100]
100447df4: f9407bed    	ldr	x13, [sp, #0xf0]
100447df8: 910923ee    	add	x14, sp, #0x248
100447dfc: 35011880    	cbnz	w0, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447e00: 17fff848    	b	0x100445f20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xc2c>
100447e04: f9407fe8    	ldr	x8, [sp, #0xf8]
100447e08: 12003d13    	and	w19, w8, #0xffff
100447e0c: f9407348    	ldr	x8, [x26, #0xe0]
100447e10: eb13011f    	cmp	x8, x19
100447e14: 540117c9    	b.ls	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447e18: f9406f48    	ldr	x8, [x26, #0xd8]
100447e1c: 3833691f    	strb	wzr, [x8, x19]
100447e20: 140008bb    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447e24: 7100153f    	cmp	w9, #0x5
100447e28: 5400542c    	b.gt	0x1004488ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x35b8>
100447e2c: 7100113f    	cmp	w9, #0x4
100447e30: 540094c0    	b.eq	0x1004490c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3dd4>
100447e34: 7100153f    	cmp	w9, #0x5
100447e38: 540167c1    	b.ne	0x10044ab30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
100447e3c: fc40c100    	ldur	d0, [x8, #0xc]
100447e40: d10303a8    	sub	x8, x29, #0xc0
100447e44: fc004100    	stur	d0, [x8, #0x4]
100447e48: 528000c8    	mov	w8, #0x6                ; =6
100447e4c: 381403a8    	sturb	w8, [x29, #-0xc0]
100447e50: f9407be8    	ldr	x8, [sp, #0xf0]
100447e54: f9400101    	ldr	x1, [x8]
100447e58: d10383a0    	sub	x0, x29, #0xe0
100447e5c: d10303a2    	sub	x2, x29, #0xc0
100447e60: 97ff7d8e    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
100447e64: 385203a8    	ldurb	w8, [x29, #-0xe0]
100447e68: 7100291f    	cmp	w8, #0xa
100447e6c: 540164c0    	b.eq	0x10044ab04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5810>
100447e70: f94063ea    	ldr	x10, [sp, #0xc0]
100447e74: b9400149    	ldr	w9, [x10]
100447e78: f94027eb    	ldr	x11, [sp, #0x48]
100447e7c: b9000169    	str	w9, [x11]
100447e80: b8403149    	ldur	w9, [x10, #0x3]
100447e84: b8003169    	stur	w9, [x11, #0x3]
100447e88: f85283a9    	ldur	x9, [x29, #-0xd8]
100447e8c: 390563e8    	strb	w8, [sp, #0x158]
100447e90: f900b3e9    	str	x9, [sp, #0x160]
100447e94: f9407be8    	ldr	x8, [sp, #0xf0]
100447e98: f9400102    	ldr	x2, [x8]
100447e9c: 910563e3    	add	x3, sp, #0x158
100447ea0: f94083e0    	ldr	x0, [sp, #0x100]
100447ea4: f9408be1    	ldr	x1, [sp, #0x110]
100447ea8: 94001c86    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100447eac: 14000897    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100447eb0: f9407fe9    	ldr	x9, [sp, #0xf8]
100447eb4: 7216013f    	tst	w9, #0x400
100447eb8: 1a9f17e9    	cset	w9, eq
100447ebc: 4a080128    	eor	w8, w9, w8
100447ec0: 36004c48    	tbz	w8, #0x0, 0x100448848 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3554>
100447ec4: 52800028    	mov	w8, #0x1                ; =1
100447ec8: 6a53711f    	tst	w8, w19, lsr #28
100447ecc: 9a880508    	cinc	x8, x8, ne
100447ed0: 8b394116    	add	x22, x8, w25, uxtw
100447ed4: eb1c02df    	cmp	x22, x28
100447ed8: 5400e443    	b.lo	0x100449b60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x486c>
100447edc: 14000e62    	b	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100447ee0: 7844c294    	ldurh	w20, [x20, #0x4c]
100447ee4: f94083e0    	ldr	x0, [sp, #0x100]
100447ee8: f9408be1    	ldr	x1, [sp, #0x110]
100447eec: 52800002    	mov	w2, #0x0                ; =0
100447ef0: aa1403e3    	mov	x3, x20
100447ef4: 94001c2d    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100447ef8: b400df80    	cbz	x0, 0x100449ae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47f4>
100447efc: 39400008    	ldrb	w8, [x0]
100447f00: 71001d1f    	cmp	w8, #0x7
100447f04: 5400df28    	b.hi	0x100449ae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47f4>
100447f08: 52800029    	mov	w9, #0x1                ; =1
100447f0c: 1ac82129    	lsl	w9, w9, w8
100447f10: 5280138a    	mov	w10, #0x9c              ; =156
100447f14: 6a0a013f    	tst	w9, w10
100447f18: 5400dcc0    	b.eq	0x100449ab0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47bc>
100447f1c: f8401009    	ldur	x9, [x0, #0x1]
100447f20: f90173e9    	str	x9, [sp, #0x2e0]
100447f24: f9400409    	ldr	x9, [x0, #0x8]
100447f28: 910923ea    	add	x10, sp, #0x248
100447f2c: f809f149    	stur	x9, [x10, #0x9f]
100447f30: 140006e2    	b	0x100449ab8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47c4>
100447f34: eb16039f    	cmp	x28, x22
100447f38: 5401cae9    	b.ls	0x10044b894 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65a0>
100447f3c: 8b161288    	add	x8, x20, x22, lsl #4
100447f40: 3900011b    	strb	w27, [x8]
100447f44: b85203a9    	ldur	w9, [x29, #-0xe0]
100447f48: b8001109    	stur	w9, [x8, #0x1]
100447f4c: 910923ea    	add	x10, sp, #0x248
100447f50: b84fb149    	ldur	w9, [x10, #0xfb]
100447f54: b9000509    	str	w9, [x8, #0x4]
100447f58: f9401fe9    	ldr	x9, [sp, #0x38]
100447f5c: f9000509    	str	x9, [x8, #0x8]
100447f60: f9408be9    	ldr	x9, [sp, #0x110]
100447f64: f9402128    	ldr	x8, [x9, #0x40]
100447f68: 91000508    	add	x8, x8, #0x1
100447f6c: f9002128    	str	x8, [x9, #0x40]
100447f70: b85403a8    	ldur	w8, [x29, #-0xc0]
100447f74: b902e3e8    	str	w8, [sp, #0x2e0]
100447f78: d10303a8    	sub	x8, x29, #0xc0
100447f7c: b8403108    	ldur	w8, [x8, #0x3]
100447f80: b809b148    	stur	w8, [x10, #0x9b]
100447f84: b940fbec    	ldr	w12, [sp, #0xf8]
100447f88: b942e3e8    	ldr	w8, [sp, #0x2e0]
100447f8c: f9405be9    	ldr	x9, [sp, #0xb0]
100447f90: b9000128    	str	w8, [x9]
100447f94: b849b148    	ldur	w8, [x10, #0x9b]
100447f98: b8003128    	stur	w8, [x9, #0x3]
100447f9c: 3907a3f8    	strb	w24, [sp, #0x1e8]
100447fa0: f900fbf3    	str	x19, [sp, #0x1f0]
100447fa4: 37010b4c    	tbnz	w12, #0x0, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447fa8: f9408fe8    	ldr	x8, [sp, #0x118]
100447fac: f9401103    	ldr	x3, [x8, #0x20]
100447fb0: f9407be8    	ldr	x8, [sp, #0xf0]
100447fb4: f9400102    	ldr	x2, [x8]
100447fb8: f9406be0    	ldr	x0, [sp, #0xd0]
100447fbc: aa1903e1    	mov	x1, x25
100447fc0: 94002484    	bl	0x1004511d0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
100447fc4: b5017320    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100447fc8: f9407be8    	ldr	x8, [sp, #0xf0]
100447fcc: f9400101    	ldr	x1, [x8]
100447fd0: 9107e3e0    	add	x0, sp, #0x1f8
100447fd4: 9107a3e2    	add	x2, sp, #0x1e8
100447fd8: 97f03c65    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
100447fdc: 3947e3e8    	ldrb	w8, [sp, #0x1f8]
100447fe0: 71002d1f    	cmp	w8, #0xb
100447fe4: 54010940    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100447fe8: 14000c4b    	b	0x10044b114 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e20>
100447fec: f9408bf6    	ldr	x22, [sp, #0x110]
100447ff0: aa1303e1    	mov	x1, x19
100447ff4: f94022d8    	ldr	x24, [x22, #0x40]
100447ff8: b401a558    	cbz	x24, 0x10044b4a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61ac>
100447ffc: f94077e8    	ldr	x8, [sp, #0xe8]
100448000: f9400108    	ldr	x8, [x8]
100448004: 8b080308    	add	x8, x24, x8
100448008: d1000508    	sub	x8, x8, #0x1
10044800c: eb01011f    	cmp	x8, x1
100448010: 5401c762    	b.hs	0x10044b8fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6608>
100448014: 8b081396    	add	x22, x28, x8, lsl #4
100448018: 394002c8    	ldrb	w8, [x22]
10044801c: 51002908    	sub	w8, w8, #0xa
100448020: 7100111f    	cmp	w8, #0x4
100448024: 54015e89    	b.ls	0x10044abf4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5900>
100448028: aa0103f3    	mov	x19, x1
10044802c: 71032b7f    	cmp	w27, #0xca
100448030: 1a9f17e6    	cset	w6, eq
100448034: a94f17e8    	ldp	x8, x5, [sp, #0xf0]
100448038: f9400101    	ldr	x1, [x8]
10044803c: 910c03e0    	add	x0, sp, #0x300
100448040: d10303a7    	sub	x7, x29, #0xc0
100448044: aa1603e2    	mov	x2, x22
100448048: f94087e3    	ldr	x3, [sp, #0x108]
10044804c: aa1903e4    	mov	x4, x25
100448050: 940022db    	bl	0x100450bbc <__ZN13quickjs_oxide6engine6object16ordinary_storage2ic62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$21property_ic_read_fast17h51aad55151f8a4d2E>
100448054: 394c03e8    	ldrb	w8, [sp, #0x300]
100448058: 7100291f    	cmp	w8, #0xa
10044805c: 54016300    	b.eq	0x10044acbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59c8>
100448060: 3dc0c3e0    	ldr	q0, [sp, #0x300]
100448064: 3c9103a0    	stur	q0, [x29, #-0xf0]
100448068: 71032b7f    	cmp	w27, #0xca
10044806c: 54000121    	b.ne	0x100448090 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2d9c>
100448070: eb13029f    	cmp	x20, x19
100448074: 5401c902    	b.hs	0x10044b994 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66a0>
100448078: 3cd103a0    	ldur	q0, [x29, #-0xf0]
10044807c: 3cb47b80    	str	q0, [x28, x20, lsl #4]
100448080: 91000708    	add	x8, x24, #0x1
100448084: f9408be9    	ldr	x9, [sp, #0x110]
100448088: f9002128    	str	x8, [x9, #0x40]
10044808c: 1400001b    	b	0x1004480f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2e04>
100448090: 394002c8    	ldrb	w8, [x22]
100448094: f84012c9    	ldur	x9, [x22, #0x1]
100448098: f90183e9    	str	x9, [sp, #0x300]
10044809c: f94006c9    	ldr	x9, [x22, #0x8]
1004480a0: 910923ea    	add	x10, sp, #0x248
1004480a4: f80bf149    	stur	x9, [x10, #0xbf]
1004480a8: 3cd103a0    	ldur	q0, [x29, #-0xf0]
1004480ac: 3d8002c0    	str	q0, [x22]
1004480b0: 51002909    	sub	w9, w8, #0xa
1004480b4: 7100113f    	cmp	w9, #0x4
1004480b8: 54000209    	b.ls	0x1004480f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2e04>
1004480bc: 390b83e8    	strb	w8, [sp, #0x2e0]
1004480c0: f94183e8    	ldr	x8, [sp, #0x300]
1004480c4: f94047e9    	ldr	x9, [sp, #0x88]
1004480c8: f9000128    	str	x8, [x9]
1004480cc: 910923e8    	add	x8, sp, #0x248
1004480d0: f84bf108    	ldur	x8, [x8, #0xbf]
1004480d4: f8007128    	stur	x8, [x9, #0x7]
1004480d8: f9407be8    	ldr	x8, [sp, #0xf0]
1004480dc: f9400101    	ldr	x1, [x8]
1004480e0: d10383a0    	sub	x0, x29, #0xe0
1004480e4: 910b83e2    	add	x2, sp, #0x2e0
1004480e8: 97f03c21    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
1004480ec: 385203a8    	ldurb	w8, [x29, #-0xe0]
1004480f0: 71002d1f    	cmp	w8, #0xb
1004480f4: 54018a01    	b.ne	0x10044b234 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f40>
1004480f8: b85503a8    	ldur	w8, [x29, #-0xb0]
1004480fc: 7100091f    	cmp	w8, #0x2
100448100: 54010060    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448104: f85403a0    	ldur	x0, [x29, #-0xc0]
100448108: f9400008    	ldr	x8, [x0]
10044810c: f1000508    	subs	x8, x8, #0x1
100448110: f9000008    	str	x8, [x0]
100448114: 5400ffc1    	b.ne	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448118: 97eff301    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044811c: 140007fc    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448120: f9400196    	ldr	x22, [x12]
100448124: d10303a0    	sub	x0, x29, #0xc0
100448128: aa1603e1    	mov	x1, x22
10044812c: aa1403e2    	mov	x2, x20
100448130: 940027be    	bl	0x100452028 <__ZN13quickjs_oxide6engine4heap14slot_ownership62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$36slot_value_release_readiness_jsvalue17h2038e983b6e2b9bfE>
100448134: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448138: 71002d1f    	cmp	w8, #0xb
10044813c: 54018b41    	b.ne	0x10044b2a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fb0>
100448140: 385413a9    	ldurb	w9, [x29, #-0xbf]
100448144: 35018b09    	cbnz	w9, 0x10044b2a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fb0>
100448148: f94016c8    	ldr	x8, [x22, #0x28]
10044814c: 92f00009    	mov	x9, #0x7fffffffffffffff ; =9223372036854775807
100448150: eb09011f    	cmp	x8, x9
100448154: 5401c4e2    	b.hs	0x10044b9f0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66fc>
100448158: 91000508    	add	x8, x8, #0x1
10044815c: f90016c8    	str	x8, [x22, #0x28]
100448160: b9400680    	ldr	w0, [x20, #0x4]
100448164: f9409ac1    	ldr	x1, [x22, #0x130]
100448168: eb00003f    	cmp	x1, x0
10044816c: 5401c489    	b.ls	0x10044b9fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6708>
100448170: f94096c8    	ldr	x8, [x22, #0x128]
100448174: 52800309    	mov	w9, #0x18               ; =24
100448178: 9ba92008    	umaddl	x8, w0, w9, x8
10044817c: f9400109    	ldr	x9, [x8]
100448180: 927f052a    	and	x10, x9, #0x6
100448184: f100095f    	cmp	x10, #0x2
100448188: 54019f60    	b.eq	0x10044b574 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6280>
10044818c: b940150a    	ldr	w10, [x8, #0x14]
100448190: 34019f2a    	cbz	w10, 0x10044b574 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6280>
100448194: f100113f    	cmp	x9, #0x4
100448198: 5401b0e1    	b.ne	0x10044b7b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64c0>
10044819c: f9400509    	ldr	x9, [x8, #0x8]
1004481a0: f940012a    	ldr	x10, [x9]
1004481a4: b100054a    	adds	x10, x10, #0x1
1004481a8: f900012a    	str	x10, [x9]
1004481ac: 5401c562    	b.hs	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
1004481b0: f940051c    	ldr	x28, [x8, #0x8]
1004481b4: f90183fc    	str	x28, [sp, #0x300]
1004481b8: f94016c8    	ldr	x8, [x22, #0x28]
1004481bc: d1000508    	sub	x8, x8, #0x1
1004481c0: f90016c8    	str	x8, [x22, #0x28]
1004481c4: aa1c03e0    	mov	x0, x28
1004481c8: 97f0fcb1    	bl	0x10008748c <__ZN13quickjs_oxide6engine4atom29parse_canonical_u32_js_string17hf7015ef7cc8bce8eE>
1004481cc: f9400388    	ldr	x8, [x28]
1004481d0: d1000508    	sub	x8, x8, #0x1
1004481d4: f9000388    	str	x8, [x28]
1004481d8: 36017c20    	tbz	w0, #0x0, 0x10044b15c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e68>
1004481dc: aa0103f4    	mov	x20, x1
1004481e0: 3100043f    	cmn	w1, #0x1
1004481e4: 54017bc0    	b.eq	0x10044b15c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e68>
1004481e8: b5000068    	cbnz	x8, 0x1004481f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2f00>
1004481ec: 910c03e0    	add	x0, sp, #0x300
1004481f0: 97efa556    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
1004481f4: f94083e8    	ldr	x8, [sp, #0x100]
1004481f8: a940f109    	ldp	x9, x28, [x8, #0x8]
1004481fc: f9408be8    	ldr	x8, [sp, #0x110]
100448200: f9402108    	ldr	x8, [x8, #0x40]
100448204: f9407bec    	ldr	x12, [sp, #0xf0]
100448208: f100091f    	cmp	x8, #0x2
10044820c: 540197a3    	b.lo	0x10044b500 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x620c>
100448210: f94077ea    	ldr	x10, [sp, #0xe8]
100448214: f940014a    	ldr	x10, [x10]
100448218: 8b0a0108    	add	x8, x8, x10
10044821c: d1000916    	sub	x22, x8, #0x2
100448220: eb1c02df    	cmp	x22, x28
100448224: 5401b082    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100448228: 8b161128    	add	x8, x9, x22, lsl #4
10044822c: 39400109    	ldrb	w9, [x8]
100448230: 5100292a    	sub	w10, w9, #0xa
100448234: 7100115f    	cmp	w10, #0x4
100448238: 54016029    	b.ls	0x10044ae3c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b48>
10044823c: 7100253f    	cmp	w9, #0x9
100448240: 540184c1    	b.ne	0x10044b2d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe4>
100448244: f9400198    	ldr	x24, [x12]
100448248: f9401713    	ldr	x19, [x24, #0x28]
10044824c: 92f00009    	mov	x9, #0x7fffffffffffffff ; =9223372036854775807
100448250: eb09027f    	cmp	x19, x9
100448254: 540199c2    	b.hs	0x10044b58c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6298>
100448258: 91000669    	add	x9, x19, #0x1
10044825c: f9001709    	str	x9, [x24, #0x28]
100448260: a94fdb1c    	ldp	x28, x22, [x24, #0xf8]
100448264: fc404100    	ldur	d0, [x8, #0x4]
100448268: 910923e8    	add	x8, sp, #0x248
10044826c: fc08c100    	stur	d0, [x8, #0x8c]
100448270: b902d3ff    	str	wzr, [sp, #0x2d0]
100448274: d10303a0    	sub	x0, x29, #0xc0
100448278: 910b43e3    	add	x3, sp, #0x2d0
10044827c: aa1c03e1    	mov	x1, x28
100448280: aa1603e2    	mov	x2, x22
100448284: 97efa7bc    	bl	0x100032174 <__ZN13quickjs_oxide6engine4heap14object_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$22validate_slot_identity17h9c24141d9f2cc0c4E>
100448288: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044828c: 7100191f    	cmp	w8, #0x6
100448290: 540128a1    	b.ne	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
100448294: f85483a0    	ldur	x0, [x29, #-0xb8]
100448298: eb16001f    	cmp	x0, x22
10044829c: 5401b462    	b.hs	0x10044b928 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6634>
1004482a0: 52802308    	mov	w8, #0x118              ; =280
1004482a4: 9b087008    	madd	x8, x0, x8, x28
1004482a8: 39400109    	ldrb	w9, [x8]
1004482ac: 7100053f    	cmp	w9, #0x1
1004482b0: 540127a1    	b.ne	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
1004482b4: f9400509    	ldr	x9, [x8, #0x8]
1004482b8: f100053f    	cmp	x9, #0x1
1004482bc: 54012748    	b.hi	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
1004482c0: 3943b509    	ldrb	w9, [x8, #0xed]
1004482c4: 71000d3f    	cmp	w9, #0x3
1004482c8: 540126e1    	b.ne	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
1004482cc: 39416109    	ldrb	w9, [x8, #0x58]
1004482d0: 7100093f    	cmp	w9, #0x2
1004482d4: 54012681    	b.ne	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
1004482d8: f9403109    	ldr	x9, [x8, #0x60]
1004482dc: d2f0000a    	mov	x10, #-0x8000000000000000 ; =-9223372036854775808
1004482e0: eb0a013f    	cmp	x9, x10
1004482e4: 54012600    	b.eq	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
1004482e8: f940390a    	ldr	x10, [x8, #0x70]
1004482ec: 2a1403e9    	mov	w9, w20
1004482f0: eb09015f    	cmp	x10, x9
1004482f4: 54012589    	b.ls	0x10044a7a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54b0>
1004482f8: f9403508    	ldr	x8, [x8, #0x68]
1004482fc: 8b091101    	add	x1, x8, x9, lsl #4
100448300: 910c03e0    	add	x0, sp, #0x300
100448304: 940028cb    	bl	0x100452630 <__ZN13quickjs_oxide6engine6object16ordinary_storage23immediate_value_jsvalue17hc5220bf7094f18e2E>
100448308: f9001713    	str	x19, [x24, #0x28]
10044830c: 394c03e8    	ldrb	w8, [sp, #0x300]
100448310: 7100291f    	cmp	w8, #0xa
100448314: 54017e20    	b.eq	0x10044b2d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe4>
100448318: 3dc0c3e0    	ldr	q0, [sp, #0x300]
10044831c: 3c9103a0    	stur	q0, [x29, #-0xf0]
100448320: 7103377f    	cmp	w27, #0xcd
100448324: f94083e8    	ldr	x8, [sp, #0x100]
100448328: 54000340    	b.eq	0x100448390 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x309c>
10044832c: a9408901    	ldp	x1, x2, [x8, #0x8]
100448330: d10303a0    	sub	x0, x29, #0xc0
100448334: f9408be3    	ldr	x3, [sp, #0x110]
100448338: 97ff950a    	bl	0x10042d760 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
10044833c: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448340: 7100291f    	cmp	w8, #0xa
100448344: 54017420    	b.eq	0x10044b1c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ed4>
100448348: f94073ea    	ldr	x10, [sp, #0xe0]
10044834c: b9400149    	ldr	w9, [x10]
100448350: f94047eb    	ldr	x11, [sp, #0x88]
100448354: b9000169    	str	w9, [x11]
100448358: b8403149    	ldur	w9, [x10, #0x3]
10044835c: b8003169    	stur	w9, [x11, #0x3]
100448360: f85483a9    	ldur	x9, [x29, #-0xb8]
100448364: 390b83e8    	strb	w8, [sp, #0x2e0]
100448368: f90177e9    	str	x9, [sp, #0x2e8]
10044836c: f9407be8    	ldr	x8, [sp, #0xf0]
100448370: f9400101    	ldr	x1, [x8]
100448374: d10383a0    	sub	x0, x29, #0xe0
100448378: 910b83e2    	add	x2, sp, #0x2e0
10044837c: 97f03b7c    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
100448380: 385203a8    	ldurb	w8, [x29, #-0xe0]
100448384: 71002d1f    	cmp	w8, #0xb
100448388: f94083e8    	ldr	x8, [sp, #0x100]
10044838c: 540172e1    	b.ne	0x10044b1e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ef4>
100448390: a940f114    	ldp	x20, x28, [x8, #0x8]
100448394: aa1403e0    	mov	x0, x20
100448398: aa1c03e1    	mov	x1, x28
10044839c: f9408be2    	ldr	x2, [sp, #0x110]
1004483a0: 97f4d6df    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
1004483a4: aa0103f6    	mov	x22, x1
1004483a8: 37079ee0    	tbnz	w0, #0x0, 0x100447784 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x2490>
1004483ac: eb16039f    	cmp	x28, x22
1004483b0: 5401a729    	b.ls	0x10044b894 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65a0>
1004483b4: 3cd103a0    	ldur	q0, [x29, #-0xf0]
1004483b8: 3cb67a80    	str	q0, [x20, x22, lsl #4]
1004483bc: 17fffcfa    	b	0x1004477a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24b0>
1004483c0: 7200053f    	tst	w9, #0x3
1004483c4: 54005f40    	b.eq	0x100448fac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cb8>
1004483c8: 381403a8    	sturb	w8, [x29, #-0xc0]
1004483cc: f85203a9    	ldur	x9, [x29, #-0xe0]
1004483d0: f94073ea    	ldr	x10, [sp, #0xe0]
1004483d4: f9000149    	str	x9, [x10]
1004483d8: f84ff169    	ldur	x9, [x11, #0xff]
1004483dc: f8007149    	stur	x9, [x10, #0x7]
1004483e0: b9400149    	ldr	w9, [x10]
1004483e4: f94053eb    	ldr	x11, [sp, #0xa0]
1004483e8: b9000169    	str	w9, [x11]
1004483ec: b8403149    	ldur	w9, [x10, #0x3]
1004483f0: b8003169    	stur	w9, [x11, #0x3]
1004483f4: f85483a9    	ldur	x9, [x29, #-0xb8]
1004483f8: 3905a3e8    	strb	w8, [sp, #0x168]
1004483fc: f900bbe9    	str	x9, [sp, #0x170]
100448400: 9105a3e3    	add	x3, sp, #0x168
100448404: 94001b2f    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448408: 14000740    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044840c: 540184e0    	b.eq	0x10044b4a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61b4>
100448410: 384a8349    	ldurb	w9, [x26, #0xa8]
100448414: 7100093f    	cmp	w9, #0x2
100448418: 540062c2    	b.hs	0x100449070 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d7c>
10044841c: 295a0d02    	ldp	w2, w3, [x8, #0xd0]
100448420: f94001a1    	ldr	x1, [x13]
100448424: 91024340    	add	x0, x26, #0x90
100448428: 97ffdfee    	bl	0x1004403e0 <__ZN13quickjs_oxide6engine2vm8protocol9CallInput13callee_global17hccaf868ce3e72756E>
10044842c: 370156e0    	tbnz	w0, #0x0, 0x10044af08 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c14>
100448430: fd400420    	ldr	d0, [x1, #0x8]
100448434: d10303a8    	sub	x8, x29, #0xc0
100448438: fc004100    	stur	d0, [x8, #0x4]
10044843c: 52800128    	mov	w8, #0x9                ; =9
100448440: 381403a8    	sturb	w8, [x29, #-0xc0]
100448444: f9407be8    	ldr	x8, [sp, #0xf0]
100448448: f9400101    	ldr	x1, [x8]
10044844c: d10383a0    	sub	x0, x29, #0xe0
100448450: d10303a2    	sub	x2, x29, #0xc0
100448454: 97ff7c11    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
100448458: 385203a8    	ldurb	w8, [x29, #-0xe0]
10044845c: 7100291f    	cmp	w8, #0xa
100448460: 54013520    	b.eq	0x10044ab04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5810>
100448464: f94063ea    	ldr	x10, [sp, #0xc0]
100448468: b9400149    	ldr	w9, [x10]
10044846c: b902e3e9    	str	w9, [sp, #0x2e0]
100448470: b8403149    	ldur	w9, [x10, #0x3]
100448474: 910923ee    	add	x14, sp, #0x248
100448478: b809b1c9    	stur	w9, [x14, #0x9b]
10044847c: f85283a9    	ldur	x9, [x29, #-0xd8]
100448480: f9408bec    	ldr	x12, [sp, #0x110]
100448484: f94083e0    	ldr	x0, [sp, #0x100]
100448488: f9407bed    	ldr	x13, [sp, #0xf0]
10044848c: f94073ea    	ldr	x10, [sp, #0xe0]
100448490: 14000714    	b	0x10044a0e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4dec>
100448494: f9407bed    	ldr	x13, [sp, #0xf0]
100448498: f94001a1    	ldr	x1, [x13]
10044849c: 71001d1f    	cmp	w8, #0x7
1004484a0: f9408bec    	ldr	x12, [sp, #0x110]
1004484a4: f94083e0    	ldr	x0, [sp, #0x100]
1004484a8: 910923ee    	add	x14, sp, #0x248
1004484ac: 5400dfc8    	b.hi	0x10044a0a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4db0>
1004484b0: 5280002a    	mov	w10, #0x1               ; =1
1004484b4: 1ac8214a    	lsl	w10, w10, w8
1004484b8: 5280138b    	mov	w11, #0x9c              ; =156
1004484bc: 6a0b015f    	tst	w10, w11
1004484c0: 54006b80    	b.eq	0x100449230 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f3c>
1004484c4: f844112a    	ldur	x10, [x9, #0x41]
1004484c8: f81203aa    	stur	x10, [x29, #-0xe0]
1004484cc: f9402529    	ldr	x9, [x9, #0x48]
1004484d0: f80ff1c9    	stur	x9, [x14, #0xff]
1004484d4: 14000359    	b	0x100449238 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f44>
1004484d8: f9400221    	ldr	x1, [x17]
1004484dc: d10303a0    	sub	x0, x29, #0xc0
1004484e0: 97ff7bee    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
1004484e4: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004484e8: 7100291f    	cmp	w8, #0xa
1004484ec: f94083f0    	ldr	x16, [sp, #0x100]
1004484f0: f9407bf1    	ldr	x17, [sp, #0xf0]
1004484f4: 910923ee    	add	x14, sp, #0x248
1004484f8: f94073ea    	ldr	x10, [sp, #0xe0]
1004484fc: 54ff96a1    	b.ne	0x1004477d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x24dc>
100448500: 140008cc    	b	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100448504: 7200053f    	tst	w9, #0x3
100448508: 54005c20    	b.eq	0x10044908c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d98>
10044850c: f85203a9    	ldur	x9, [x29, #-0xe0]
100448510: f94073ea    	ldr	x10, [sp, #0xe0]
100448514: f9000149    	str	x9, [x10]
100448518: f84ff1c9    	ldur	x9, [x14, #0xff]
10044851c: f8007149    	stur	x9, [x10, #0x7]
100448520: b9400149    	ldr	w9, [x10]
100448524: b902e3e9    	str	w9, [sp, #0x2e0]
100448528: b8403149    	ldur	w9, [x10, #0x3]
10044852c: b809b1c9    	stur	w9, [x14, #0x9b]
100448530: f85483a9    	ldur	x9, [x29, #-0xb8]
100448534: f94067ea    	ldr	x10, [sp, #0xc8]
100448538: f940014a    	ldr	x10, [x10]
10044853c: f9406feb    	ldr	x11, [sp, #0xd8]
100448540: f940016b    	ldr	x11, [x11]
100448544: eb0a016b    	subs	x11, x11, x10
100448548: 9a8b33eb    	csel	x11, xzr, x11, lo
10044854c: eb13017f    	cmp	x11, x19
100448550: 54013d49    	b.ls	0x10044acf8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5a04>
100448554: f940099c    	ldr	x28, [x12, #0x10]
100448558: 8b130156    	add	x22, x10, x19
10044855c: eb1c02df    	cmp	x22, x28
100448560: 54019aa2    	b.hs	0x10044b8b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65c0>
100448564: f940058a    	ldr	x10, [x12, #0x8]
100448568: 8b16114a    	add	x10, x10, x22, lsl #4
10044856c: 3940014b    	ldrb	w11, [x10]
100448570: 7100397f    	cmp	w11, #0xe
100448574: 54013f80    	b.eq	0x10044ad64 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5a70>
100448578: b840114c    	ldur	w12, [x10, #0x1]
10044857c: f94057ef    	ldr	x15, [sp, #0xa8]
100448580: b90001ec    	str	w12, [x15]
100448584: b940054c    	ldr	w12, [x10, #0x4]
100448588: b80031ec    	stur	w12, [x15, #0x3]
10044858c: f940054c    	ldr	x12, [x10, #0x8]
100448590: 39000148    	strb	w8, [x10]
100448594: b942e3e8    	ldr	w8, [sp, #0x2e0]
100448598: b8001148    	stur	w8, [x10, #0x1]
10044859c: b849b1c8    	ldur	w8, [x14, #0x9b]
1004485a0: b9000548    	str	w8, [x10, #0x4]
1004485a4: f9000549    	str	x9, [x10, #0x8]
1004485a8: 3906e3eb    	strb	w11, [sp, #0x1b8]
1004485ac: f900e3ec    	str	x12, [sp, #0x1c0]
1004485b0: f9408fe8    	ldr	x8, [sp, #0x118]
1004485b4: f9401103    	ldr	x3, [x8, #0x20]
1004485b8: f94001a2    	ldr	x2, [x13]
1004485bc: f9406be0    	ldr	x0, [sp, #0xd0]
1004485c0: aa1903e1    	mov	x1, x25
1004485c4: 94002303    	bl	0x1004511d0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
1004485c8: b5014300    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
1004485cc: f9407be8    	ldr	x8, [sp, #0xf0]
1004485d0: f9400100    	ldr	x0, [x8]
1004485d4: 9106e3e1    	add	x1, sp, #0x1b8
1004485d8: 97f4680d    	bl	0x10016260c <__ZN13quickjs_oxide6engine2vm8bindings21release_frame_binding17h9e71af6262778e6eE>
1004485dc: 140006cb    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004485e0: 71001d1f    	cmp	w8, #0x7
1004485e4: 54ff2a40    	b.eq	0x100446b2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x1838>
1004485e8: f9407be8    	ldr	x8, [sp, #0xf0]
1004485ec: f9400101    	ldr	x1, [x8]
1004485f0: d10303a0    	sub	x0, x29, #0xc0
1004485f4: 9400252c    	bl	0x100451aa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame28_$u7b$$u7b$closure$u7d$$u7d$17h0708239d021321f4E>
1004485f8: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004485fc: 7100051f    	cmp	w8, #0x1
100448600: 54014a60    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
100448604: a95123e2    	ldp	x2, x8, [sp, #0x110]
100448608: f9401108    	ldr	x8, [x8, #0x20]
10044860c: f94083e1    	ldr	x1, [sp, #0x100]
100448610: b4010d08    	cbz	x8, 0x10044a7b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x54bc>
100448614: 385413b3    	ldurb	w19, [x29, #-0xbf]
100448618: d10303a0    	sub	x0, x29, #0xc0
10044861c: 940020b1    	bl	0x1004508e0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
100448620: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448624: 7100291f    	cmp	w8, #0xa
100448628: 54014920    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044862c: f94073ea    	ldr	x10, [sp, #0xe0]
100448630: b9400149    	ldr	w9, [x10]
100448634: f94033eb    	ldr	x11, [sp, #0x60]
100448638: b9000169    	str	w9, [x11]
10044863c: b8403149    	ldur	w9, [x10, #0x3]
100448640: b8003169    	stur	w9, [x11, #0x3]
100448644: f85483a9    	ldur	x9, [x29, #-0xb8]
100448648: 390863e8    	strb	w8, [sp, #0x218]
10044864c: f90113e9    	str	x9, [sp, #0x220]
100448650: 52800028    	mov	w8, #0x1                ; =1
100448654: 0a330108    	bic	w8, w8, w19
100448658: 381413a8    	sturb	w8, [x29, #-0xbf]
10044865c: 52800048    	mov	w8, #0x2                ; =2
100448660: 381403a8    	sturb	w8, [x29, #-0xc0]
100448664: d10303a2    	sub	x2, x29, #0xc0
100448668: f94083e0    	ldr	x0, [sp, #0x100]
10044866c: f9408be1    	ldr	x1, [sp, #0x110]
100448670: 94001a30    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100448674: b5013da0    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448678: f9408fe8    	ldr	x8, [sp, #0x118]
10044867c: f9401103    	ldr	x3, [x8, #0x20]
100448680: f9407be8    	ldr	x8, [sp, #0xf0]
100448684: f9400102    	ldr	x2, [x8]
100448688: f9406be0    	ldr	x0, [sp, #0xd0]
10044868c: aa1903e1    	mov	x1, x25
100448690: 940022d0    	bl	0x1004511d0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor13publish_fault17h938918c69d2697abE>
100448694: b5013ca0    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448698: f9407be8    	ldr	x8, [sp, #0xf0]
10044869c: f9400101    	ldr	x1, [x8]
1004486a0: 9108a3e0    	add	x0, sp, #0x228
1004486a4: 910863e2    	add	x2, sp, #0x218
1004486a8: 97f03ab1    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
1004486ac: 3948a3e8    	ldrb	w8, [sp, #0x228]
1004486b0: 71002d1f    	cmp	w8, #0xb
1004486b4: 5400d2c0    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
1004486b8: 14000b68    	b	0x10044b458 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6164>
1004486bc: 294323e9    	ldp	w9, w8, [sp, #0x18]
1004486c0: 292127a8    	stp	w8, w9, [x29, #-0xf8]
1004486c4: b81043bf    	stur	wzr, [x29, #-0xfc]
1004486c8: d10303a0    	sub	x0, x29, #0xc0
1004486cc: d103f3a3    	sub	x3, x29, #0xfc
1004486d0: f9400bf3    	ldr	x19, [sp, #0x10]
1004486d4: aa1303e2    	mov	x2, x19
1004486d8: 97efa6a7    	bl	0x100032174 <__ZN13quickjs_oxide6engine4heap14object_storage51_$LT$impl$u20$quickjs_oxide..engine..heap..Heap$GT$22validate_slot_identity17h9c24141d9f2cc0c4E>
1004486dc: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004486e0: 7100191f    	cmp	w8, #0x6
1004486e4: 54016941    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
1004486e8: f85483b4    	ldur	x20, [x29, #-0xb8]
1004486ec: eb13029f    	cmp	x20, x19
1004486f0: 54019102    	b.hs	0x10044b910 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x661c>
1004486f4: 52802308    	mov	w8, #0x118              ; =280
1004486f8: 9b086288    	madd	x8, x20, x8, x24
1004486fc: 39400109    	ldrb	w9, [x8]
100448700: 7100053f    	cmp	w9, #0x1
100448704: 54016841    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100448708: f9400509    	ldr	x9, [x8, #0x8]
10044870c: f100053f    	cmp	x9, #0x1
100448710: 540167e8    	b.hi	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100448714: 39416109    	ldrb	w9, [x8, #0x58]
100448718: 7100753f    	cmp	w9, #0x1d
10044871c: 54016781    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100448720: b9406d09    	ldr	w9, [x8, #0x6c]
100448724: 3941c114    	ldrb	w20, [x8, #0x70]
100448728: 3cc5c100    	ldur	q0, [x8, #0x5c]
10044872c: 3d80c3e0    	str	q0, [sp, #0x300]
100448730: b90313e9    	str	w9, [sp, #0x310]
100448734: 390c53f4    	strb	w20, [sp, #0x314]
100448738: 51001e88    	sub	w8, w20, #0x7
10044873c: 7100091f    	cmp	w8, #0x2
100448740: 54016663    	b.lo	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
100448744: b94033e8    	ldr	w8, [sp, #0x30]
100448748: 2a0803e4    	mov	w4, w8
10044874c: d10303a0    	sub	x0, x29, #0xc0
100448750: 910c03e3    	add	x3, sp, #0x300
100448754: aa1803e1    	mov	x1, x24
100448758: aa1303e2    	mov	x2, x19
10044875c: d2800005    	mov	x5, #0x0                ; =0
100448760: 97f0ff6a    	bl	0x100088508 <__ZN13quickjs_oxide6engine8builtins12array_buffer11typed_array33ordinary_typed_array_word_in_heap17h3136afd441ee1654E>
100448764: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448768: 71002d1f    	cmp	w8, #0xb
10044876c: 54016381    	b.ne	0x10044b3dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60e8>
100448770: 385413a8    	ldurb	w8, [x29, #-0xbf]
100448774: 7100091f    	cmp	w8, #0x2
100448778: 540164a1    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
10044877c: d10303a8    	sub	x8, x29, #0xc0
100448780: f8402102    	ldur	x2, [x8, #0x2]
100448784: d10303a0    	sub	x0, x29, #0xc0
100448788: aa1403e1    	mov	x1, x20
10044878c: 97f10183    	bl	0x100088d98 <__ZN13quickjs_oxide6engine8builtins12array_buffer11typed_array25typed_array_decode_number17hf2a2b840b4fe4c9bE>
100448790: fc5483a0    	ldur	d0, [x29, #-0xb8]
100448794: 296827a8    	ldp	w8, w9, [x29, #-0xc0]
100448798: 7100011f    	cmp	w8, #0x0
10044879c: 52800068    	mov	w8, #0x3                ; =3
1004487a0: 1a880508    	cinc	w8, w8, ne
1004487a4: 390b03e8    	strb	w8, [sp, #0x2c0]
1004487a8: b902c7e9    	str	w9, [sp, #0x2c4]
1004487ac: fd0167e0    	str	d0, [sp, #0x2c8]
1004487b0: 14000210    	b	0x100448ff0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3cfc>
1004487b4: f9403a69    	ldr	x9, [x19, #0x70]
1004487b8: b94033e8    	ldr	w8, [sp, #0x30]
1004487bc: 2a0803e8    	mov	w8, w8
1004487c0: eb08013f    	cmp	x9, x8
1004487c4: 54016249    	b.ls	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
1004487c8: f9403669    	ldr	x9, [x19, #0x68]
1004487cc: 8b081121    	add	x1, x9, x8, lsl #4
1004487d0: 14000254    	b	0x100449120 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3e2c>
1004487d4: 71001f7f    	cmp	w27, #0x7
1004487d8: f9007feb    	str	x11, [sp, #0xf8]
1004487dc: f9001fea    	str	x10, [sp, #0x38]
1004487e0: 5400d0e8    	b.hi	0x10044a1fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f08>
1004487e4: 52800028    	mov	w8, #0x1                ; =1
1004487e8: 1adb2108    	lsl	w8, w8, w27
1004487ec: 52801389    	mov	w9, #0x9c               ; =156
1004487f0: 6a09011f    	tst	w8, w9
1004487f4: 54009ba0    	b.eq	0x100449b68 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4874>
1004487f8: f8401048    	ldur	x8, [x2, #0x1]
1004487fc: f81203a8    	stur	x8, [x29, #-0xe0]
100448800: f9400448    	ldr	x8, [x2, #0x8]
100448804: 910923e9    	add	x9, sp, #0x248
100448808: f80ff128    	stur	x8, [x9, #0xff]
10044880c: 140004d9    	b	0x100449b70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x487c>
100448810: b85443a8    	ldur	w8, [x29, #-0xbc]
100448814: b81443a8    	stur	w8, [x29, #-0xbc]
100448818: 52800068    	mov	w8, #0x3                ; =3
10044881c: 381403a8    	sturb	w8, [x29, #-0xc0]
100448820: d10303a2    	sub	x2, x29, #0xc0
100448824: 940019c3    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100448828: 14000007    	b	0x100448844 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3550>
10044882c: b85443a8    	ldur	w8, [x29, #-0xbc]
100448830: b81443a8    	stur	w8, [x29, #-0xbc]
100448834: 52800068    	mov	w8, #0x3                ; =3
100448838: 381403a8    	sturb	w8, [x29, #-0xc0]
10044883c: d10303a2    	sub	x2, x29, #0xc0
100448840: 940019bc    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100448844: b5012f20    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448848: f940a3e8    	ldr	x8, [sp, #0x140]
10044884c: 91000919    	add	x25, x8, #0x2
100448850: 14000630    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
100448854: f100013f    	cmp	x9, #0x0
100448858: 1a9f07e8    	cset	w8, ne
10044885c: 71025b7f    	cmp	w27, #0x96
100448860: 1a9f07e9    	cset	w9, ne
100448864: 12000108    	and	w8, w8, #0x1
100448868: 6b08013f    	cmp	w9, w8
10044886c: 5400c500    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448870: f9407fe8    	ldr	x8, [sp, #0xf8]
100448874: 2a0803e8    	mov	w8, w8
100448878: f900a3e8    	str	x8, [sp, #0x140]
10044887c: 14000624    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448880: 7100093f    	cmp	w9, #0x2
100448884: 54004300    	b.eq	0x1004490e4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3df0>
100448888: 71000d3f    	cmp	w9, #0x3
10044888c: 54011521    	b.ne	0x10044ab30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
100448890: b9400d08    	ldr	w8, [x8, #0xc]
100448894: b81443a8    	stur	w8, [x29, #-0xbc]
100448898: 52800068    	mov	w8, #0x3                ; =3
10044889c: 381403a8    	sturb	w8, [x29, #-0xc0]
1004488a0: d10303a2    	sub	x2, x29, #0xc0
1004488a4: 940019a3    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
1004488a8: 14000618    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004488ac: 7100193f    	cmp	w9, #0x6
1004488b0: 54004280    	b.eq	0x100449100 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3e0c>
1004488b4: 71001d3f    	cmp	w9, #0x7
1004488b8: 540113c1    	b.ne	0x10044ab30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x583c>
1004488bc: fc40c100    	ldur	d0, [x8, #0xc]
1004488c0: d10303a8    	sub	x8, x29, #0xc0
1004488c4: fc004100    	stur	d0, [x8, #0x4]
1004488c8: 528000a8    	mov	w8, #0x5                ; =5
1004488cc: 381403a8    	sturb	w8, [x29, #-0xc0]
1004488d0: f9407be8    	ldr	x8, [sp, #0xf0]
1004488d4: f9400101    	ldr	x1, [x8]
1004488d8: d10383a0    	sub	x0, x29, #0xe0
1004488dc: d10303a2    	sub	x2, x29, #0xc0
1004488e0: 97ff7aee    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
1004488e4: 385203a8    	ldurb	w8, [x29, #-0xe0]
1004488e8: 7100291f    	cmp	w8, #0xa
1004488ec: 540110c0    	b.eq	0x10044ab04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5810>
1004488f0: f94063ea    	ldr	x10, [sp, #0xc0]
1004488f4: b9400149    	ldr	w9, [x10]
1004488f8: f9402beb    	ldr	x11, [sp, #0x50]
1004488fc: b9000169    	str	w9, [x11]
100448900: b8403149    	ldur	w9, [x10, #0x3]
100448904: b8003169    	stur	w9, [x11, #0x3]
100448908: f85283a9    	ldur	x9, [x29, #-0xd8]
10044890c: 390523e8    	strb	w8, [sp, #0x148]
100448910: f900abe9    	str	x9, [sp, #0x150]
100448914: f9407be8    	ldr	x8, [sp, #0xf0]
100448918: f9400102    	ldr	x2, [x8]
10044891c: 910523e3    	add	x3, sp, #0x148
100448920: f94083e0    	ldr	x0, [sp, #0x100]
100448924: f9408be1    	ldr	x1, [sp, #0x110]
100448928: 940019e6    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044892c: 140005f7    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448930: 7200053f    	tst	w9, #0x3
100448934: 54000180    	b.eq	0x100448964 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3670>
100448938: 381403a8    	sturb	w8, [x29, #-0xc0]
10044893c: f94173e8    	ldr	x8, [sp, #0x2e0]
100448940: f94073e9    	ldr	x9, [sp, #0xe0]
100448944: f9000128    	str	x8, [x9]
100448948: f849f168    	ldur	x8, [x11, #0x9f]
10044894c: f8007128    	stur	x8, [x9, #0x7]
100448950: d10303a2    	sub	x2, x29, #0xc0
100448954: aa0303e0    	mov	x0, x3
100448958: aa0403e1    	mov	x1, x4
10044895c: 940019bb    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448960: 140005ea    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448964: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100448968: f9400102    	ldr	x2, [x8]
10044896c: d10303a0    	sub	x0, x29, #0xc0
100448970: 94001a29    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100448974: 385503a8    	ldurb	w8, [x29, #-0xb0]
100448978: f85403a0    	ldur	x0, [x29, #-0xc0]
10044897c: 7100091f    	cmp	w8, #0x2
100448980: 54012540    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448984: 92401c09    	and	x9, x0, #0xff
100448988: f100293f    	cmp	x9, #0xa
10044898c: f9408be1    	ldr	x1, [sp, #0x110]
100448990: f94083e9    	ldr	x9, [sp, #0x100]
100448994: f9407bea    	ldr	x10, [sp, #0xf0]
100448998: 54012220    	b.eq	0x10044addc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ae8>
10044899c: f85483a8    	ldur	x8, [x29, #-0xb8]
1004489a0: a93223a0    	stp	x0, x8, [x29, #-0xe0]
1004489a4: f9400142    	ldr	x2, [x10]
1004489a8: d10383a3    	sub	x3, x29, #0xe0
1004489ac: aa0903e0    	mov	x0, x9
1004489b0: 940019c4    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
1004489b4: 140005d5    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004489b8: 7200053f    	tst	w9, #0x3
1004489bc: 54000180    	b.eq	0x1004489ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x36f8>
1004489c0: 381403a8    	sturb	w8, [x29, #-0xc0]
1004489c4: f94173e8    	ldr	x8, [sp, #0x2e0]
1004489c8: f94073e9    	ldr	x9, [sp, #0xe0]
1004489cc: f9000128    	str	x8, [x9]
1004489d0: f849f168    	ldur	x8, [x11, #0x9f]
1004489d4: f8007128    	stur	x8, [x9, #0x7]
1004489d8: d10303a2    	sub	x2, x29, #0xc0
1004489dc: aa0303e0    	mov	x0, x3
1004489e0: aa0403e1    	mov	x1, x4
1004489e4: 94001999    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
1004489e8: 140005c8    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004489ec: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
1004489f0: f9400102    	ldr	x2, [x8]
1004489f4: d10303a0    	sub	x0, x29, #0xc0
1004489f8: 94001a9e    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
1004489fc: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448a00: 71002d1f    	cmp	w8, #0xb
100448a04: 5400f160    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100448a08: f94073ea    	ldr	x10, [sp, #0xe0]
100448a0c: b9400149    	ldr	w9, [x10]
100448a10: b90303e9    	str	w9, [sp, #0x300]
100448a14: b8403149    	ldur	w9, [x10, #0x3]
100448a18: 910923eb    	add	x11, sp, #0x248
100448a1c: b80bb169    	stur	w9, [x11, #0xbb]
100448a20: 7100291f    	cmp	w8, #0xa
100448a24: f9408be1    	ldr	x1, [sp, #0x110]
100448a28: f94083e0    	ldr	x0, [sp, #0x100]
100448a2c: f9407bea    	ldr	x10, [sp, #0xf0]
100448a30: 5400fcc0    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
100448a34: f85483a9    	ldur	x9, [x29, #-0xb8]
100448a38: 381203a8    	sturb	w8, [x29, #-0xe0]
100448a3c: b94303e8    	ldr	w8, [sp, #0x300]
100448a40: f94063ec    	ldr	x12, [sp, #0xc0]
100448a44: b9000188    	str	w8, [x12]
100448a48: b84bb168    	ldur	w8, [x11, #0xbb]
100448a4c: b8003188    	stur	w8, [x12, #0x3]
100448a50: f81283a9    	stur	x9, [x29, #-0xd8]
100448a54: f9400142    	ldr	x2, [x10]
100448a58: d10383a3    	sub	x3, x29, #0xe0
100448a5c: 94001999    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448a60: 140005aa    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448a64: 7200053f    	tst	w9, #0x3
100448a68: 54000180    	b.eq	0x100448a98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x37a4>
100448a6c: 381403a8    	sturb	w8, [x29, #-0xc0]
100448a70: f94173e8    	ldr	x8, [sp, #0x2e0]
100448a74: f94073e9    	ldr	x9, [sp, #0xe0]
100448a78: f9000128    	str	x8, [x9]
100448a7c: f849f168    	ldur	x8, [x11, #0x9f]
100448a80: f8007128    	stur	x8, [x9, #0x7]
100448a84: d10303a2    	sub	x2, x29, #0xc0
100448a88: aa0303e0    	mov	x0, x3
100448a8c: aa0403e1    	mov	x1, x4
100448a90: 9400196e    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448a94: 1400059d    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448a98: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100448a9c: f9400102    	ldr	x2, [x8]
100448aa0: d10303a0    	sub	x0, x29, #0xc0
100448aa4: 940019dc    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100448aa8: 385503a8    	ldurb	w8, [x29, #-0xb0]
100448aac: f85403a0    	ldur	x0, [x29, #-0xc0]
100448ab0: 7100091f    	cmp	w8, #0x2
100448ab4: 54011ba0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448ab8: 92401c08    	and	x8, x0, #0xff
100448abc: f100291f    	cmp	x8, #0xa
100448ac0: f9408be1    	ldr	x1, [sp, #0x110]
100448ac4: f94083e9    	ldr	x9, [sp, #0x100]
100448ac8: f9407bea    	ldr	x10, [sp, #0xf0]
100448acc: 5400f500    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
100448ad0: f85483a8    	ldur	x8, [x29, #-0xb8]
100448ad4: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100448ad8: f9400142    	ldr	x2, [x10]
100448adc: d10383a3    	sub	x3, x29, #0xe0
100448ae0: aa0903e0    	mov	x0, x9
100448ae4: 94001977    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448ae8: 14000588    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448aec: 7200053f    	tst	w9, #0x3
100448af0: 54000180    	b.eq	0x100448b20 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x382c>
100448af4: 381403a8    	sturb	w8, [x29, #-0xc0]
100448af8: f94173e8    	ldr	x8, [sp, #0x2e0]
100448afc: f94073e9    	ldr	x9, [sp, #0xe0]
100448b00: f9000128    	str	x8, [x9]
100448b04: f849f168    	ldur	x8, [x11, #0x9f]
100448b08: f8007128    	stur	x8, [x9, #0x7]
100448b0c: d10303a2    	sub	x2, x29, #0xc0
100448b10: aa0303e0    	mov	x0, x3
100448b14: aa0403e1    	mov	x1, x4
100448b18: 9400194c    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448b1c: 1400057b    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448b20: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100448b24: f9400102    	ldr	x2, [x8]
100448b28: d10303a0    	sub	x0, x29, #0xc0
100448b2c: 940019ba    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100448b30: 385503a8    	ldurb	w8, [x29, #-0xb0]
100448b34: f85403a0    	ldur	x0, [x29, #-0xc0]
100448b38: 7100091f    	cmp	w8, #0x2
100448b3c: 54011760    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448b40: 92401c08    	and	x8, x0, #0xff
100448b44: f100291f    	cmp	x8, #0xa
100448b48: f9408be1    	ldr	x1, [sp, #0x110]
100448b4c: f94083e9    	ldr	x9, [sp, #0x100]
100448b50: f9407bea    	ldr	x10, [sp, #0xf0]
100448b54: 5400f0c0    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
100448b58: f85483a8    	ldur	x8, [x29, #-0xb8]
100448b5c: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100448b60: f9400142    	ldr	x2, [x10]
100448b64: d10383a3    	sub	x3, x29, #0xe0
100448b68: aa0903e0    	mov	x0, x9
100448b6c: 94001955    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448b70: 14000566    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448b74: 7200053f    	tst	w9, #0x3
100448b78: 54000180    	b.eq	0x100448ba8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x38b4>
100448b7c: 381403a8    	sturb	w8, [x29, #-0xc0]
100448b80: f94173e8    	ldr	x8, [sp, #0x2e0]
100448b84: f94073e9    	ldr	x9, [sp, #0xe0]
100448b88: f9000128    	str	x8, [x9]
100448b8c: f849f168    	ldur	x8, [x11, #0x9f]
100448b90: f8007128    	stur	x8, [x9, #0x7]
100448b94: d10303a2    	sub	x2, x29, #0xc0
100448b98: aa0303e0    	mov	x0, x3
100448b9c: aa0403e1    	mov	x1, x4
100448ba0: 9400192a    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448ba4: 14000559    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448ba8: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100448bac: f9400102    	ldr	x2, [x8]
100448bb0: d10303a0    	sub	x0, x29, #0xc0
100448bb4: 94001a2f    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
100448bb8: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448bbc: 71002d1f    	cmp	w8, #0xb
100448bc0: 5400e380    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100448bc4: f94073ea    	ldr	x10, [sp, #0xe0]
100448bc8: b9400149    	ldr	w9, [x10]
100448bcc: b90303e9    	str	w9, [sp, #0x300]
100448bd0: b8403149    	ldur	w9, [x10, #0x3]
100448bd4: 910923eb    	add	x11, sp, #0x248
100448bd8: b80bb169    	stur	w9, [x11, #0xbb]
100448bdc: 7100291f    	cmp	w8, #0xa
100448be0: f9408be1    	ldr	x1, [sp, #0x110]
100448be4: f94083e0    	ldr	x0, [sp, #0x100]
100448be8: f9407bea    	ldr	x10, [sp, #0xf0]
100448bec: 5400eee0    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
100448bf0: f85483a9    	ldur	x9, [x29, #-0xb8]
100448bf4: 381203a8    	sturb	w8, [x29, #-0xe0]
100448bf8: b94303e8    	ldr	w8, [sp, #0x300]
100448bfc: f94063ec    	ldr	x12, [sp, #0xc0]
100448c00: b9000188    	str	w8, [x12]
100448c04: b84bb168    	ldur	w8, [x11, #0xbb]
100448c08: b8003188    	stur	w8, [x12, #0x3]
100448c0c: f81283a9    	stur	x9, [x29, #-0xd8]
100448c10: f9400142    	ldr	x2, [x10]
100448c14: d10383a3    	sub	x3, x29, #0xe0
100448c18: 9400192a    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448c1c: 1400053b    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448c20: 7200053f    	tst	w9, #0x3
100448c24: 54000180    	b.eq	0x100448c54 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3960>
100448c28: 381403a8    	sturb	w8, [x29, #-0xc0]
100448c2c: f94173e8    	ldr	x8, [sp, #0x2e0]
100448c30: f94073e9    	ldr	x9, [sp, #0xe0]
100448c34: f9000128    	str	x8, [x9]
100448c38: f849f168    	ldur	x8, [x11, #0x9f]
100448c3c: f8007128    	stur	x8, [x9, #0x7]
100448c40: d10303a2    	sub	x2, x29, #0xc0
100448c44: aa0303e0    	mov	x0, x3
100448c48: aa0403e1    	mov	x1, x4
100448c4c: 940018ff    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448c50: 1400052e    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448c54: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100448c58: f9400102    	ldr	x2, [x8]
100448c5c: d10303a0    	sub	x0, x29, #0xc0
100448c60: 9400196d    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100448c64: 385503a8    	ldurb	w8, [x29, #-0xb0]
100448c68: f85403a0    	ldur	x0, [x29, #-0xc0]
100448c6c: 7100091f    	cmp	w8, #0x2
100448c70: 54010dc0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448c74: 92401c08    	and	x8, x0, #0xff
100448c78: f100291f    	cmp	x8, #0xa
100448c7c: f9408be1    	ldr	x1, [sp, #0x110]
100448c80: f94083e9    	ldr	x9, [sp, #0x100]
100448c84: f9407bea    	ldr	x10, [sp, #0xf0]
100448c88: 5400e720    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
100448c8c: f85483a8    	ldur	x8, [x29, #-0xb8]
100448c90: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100448c94: f9400142    	ldr	x2, [x10]
100448c98: d10383a3    	sub	x3, x29, #0xe0
100448c9c: aa0903e0    	mov	x0, x9
100448ca0: 94001908    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448ca4: 14000519    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448ca8: 7200053f    	tst	w9, #0x3
100448cac: 54000180    	b.eq	0x100448cdc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x39e8>
100448cb0: 381403a8    	sturb	w8, [x29, #-0xc0]
100448cb4: f94173e8    	ldr	x8, [sp, #0x2e0]
100448cb8: f94073e9    	ldr	x9, [sp, #0xe0]
100448cbc: f9000128    	str	x8, [x9]
100448cc0: f849f168    	ldur	x8, [x11, #0x9f]
100448cc4: f8007128    	stur	x8, [x9, #0x7]
100448cc8: d10303a2    	sub	x2, x29, #0xc0
100448ccc: aa0303e0    	mov	x0, x3
100448cd0: aa0403e1    	mov	x1, x4
100448cd4: 940018dd    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448cd8: 1400050c    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448cdc: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100448ce0: f9400102    	ldr	x2, [x8]
100448ce4: d10303a0    	sub	x0, x29, #0xc0
100448ce8: 940019e2    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
100448cec: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448cf0: 71002d1f    	cmp	w8, #0xb
100448cf4: 5400d9e0    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100448cf8: f94073ea    	ldr	x10, [sp, #0xe0]
100448cfc: b9400149    	ldr	w9, [x10]
100448d00: b90303e9    	str	w9, [sp, #0x300]
100448d04: b8403149    	ldur	w9, [x10, #0x3]
100448d08: 910923eb    	add	x11, sp, #0x248
100448d0c: b80bb169    	stur	w9, [x11, #0xbb]
100448d10: 7100291f    	cmp	w8, #0xa
100448d14: f9408be1    	ldr	x1, [sp, #0x110]
100448d18: f94083e0    	ldr	x0, [sp, #0x100]
100448d1c: f9407bea    	ldr	x10, [sp, #0xf0]
100448d20: 5400e540    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
100448d24: f85483a9    	ldur	x9, [x29, #-0xb8]
100448d28: 381203a8    	sturb	w8, [x29, #-0xe0]
100448d2c: b94303e8    	ldr	w8, [sp, #0x300]
100448d30: f94063ec    	ldr	x12, [sp, #0xc0]
100448d34: b9000188    	str	w8, [x12]
100448d38: b84bb168    	ldur	w8, [x11, #0xbb]
100448d3c: b8003188    	stur	w8, [x12, #0x3]
100448d40: f81283a9    	stur	x9, [x29, #-0xd8]
100448d44: f9400142    	ldr	x2, [x10]
100448d48: d10383a3    	sub	x3, x29, #0xe0
100448d4c: 940018dd    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448d50: 140004ee    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448d54: b94005ad    	ldr	w13, [x13, #0x4]
100448d58: 710005af    	subs	w15, w13, #0x1
100448d5c: 1a9f77ee    	cset	w14, vs
100448d60: 310005b0    	adds	w16, w13, #0x1
100448d64: 1a9f77f1    	cset	w17, vs
100448d68: 720f001f    	tst	w0, #0x20000
100448d6c: 1a8e022e    	csel	w14, w17, w14, eq
100448d70: 36004dce    	tbz	w14, #0x0, 0x100449728 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4434>
100448d74: 5280000e    	mov	w14, #0x0               ; =0
100448d78: 720f001f    	tst	w0, #0x20000
100448d7c: 1e7e1000    	fmov	d0, #-1.00000000
100448d80: 1e6e1001    	fmov	d1, #1.00000000
100448d84: 1e600c20    	fcsel	d0, d1, d0, eq
100448d88: 1e6201a1    	scvtf	d1, w13
100448d8c: 1e612808    	fadd	d8, d0, d1
100448d90: 52800031    	mov	w17, #0x1               ; =1
100448d94: 14000269    	b	0x100449738 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4444>
100448d98: 7200053f    	tst	w9, #0x3
100448d9c: 54000180    	b.eq	0x100448dcc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3ad8>
100448da0: 381403a8    	sturb	w8, [x29, #-0xc0]
100448da4: f94173e8    	ldr	x8, [sp, #0x2e0]
100448da8: f94073e9    	ldr	x9, [sp, #0xe0]
100448dac: f9000128    	str	x8, [x9]
100448db0: f849f168    	ldur	x8, [x11, #0x9f]
100448db4: f8007128    	stur	x8, [x9, #0x7]
100448db8: d10303a2    	sub	x2, x29, #0xc0
100448dbc: aa0303e0    	mov	x0, x3
100448dc0: aa0403e1    	mov	x1, x4
100448dc4: 940018a1    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448dc8: 140004d0    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448dcc: f9407be8    	ldr	x8, [sp, #0xf0]
100448dd0: f9400102    	ldr	x2, [x8]
100448dd4: d10303a0    	sub	x0, x29, #0xc0
100448dd8: aa1303e1    	mov	x1, x19
100448ddc: 9400190e    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100448de0: 385503a8    	ldurb	w8, [x29, #-0xb0]
100448de4: f85403a0    	ldur	x0, [x29, #-0xc0]
100448de8: 7100091f    	cmp	w8, #0x2
100448dec: 540101e0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448df0: 92401c09    	and	x9, x0, #0xff
100448df4: f100293f    	cmp	x9, #0xa
100448df8: f9408be1    	ldr	x1, [sp, #0x110]
100448dfc: f94083e9    	ldr	x9, [sp, #0x100]
100448e00: f9407bea    	ldr	x10, [sp, #0xf0]
100448e04: 540123e0    	b.eq	0x10044b280 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f8c>
100448e08: f85483a8    	ldur	x8, [x29, #-0xb8]
100448e0c: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100448e10: f9400142    	ldr	x2, [x10]
100448e14: d10383a3    	sub	x3, x29, #0xe0
100448e18: aa0903e0    	mov	x0, x9
100448e1c: 940018a9    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448e20: 140004ba    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448e24: 7200053f    	tst	w9, #0x3
100448e28: 54000180    	b.eq	0x100448e58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3b64>
100448e2c: 381403a8    	sturb	w8, [x29, #-0xc0]
100448e30: f94173e8    	ldr	x8, [sp, #0x2e0]
100448e34: f94073e9    	ldr	x9, [sp, #0xe0]
100448e38: f9000128    	str	x8, [x9]
100448e3c: f849f168    	ldur	x8, [x11, #0x9f]
100448e40: f8007128    	stur	x8, [x9, #0x7]
100448e44: d10303a2    	sub	x2, x29, #0xc0
100448e48: aa0303e0    	mov	x0, x3
100448e4c: aa0403e1    	mov	x1, x4
100448e50: 9400187e    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100448e54: 140004ad    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448e58: f9407be8    	ldr	x8, [sp, #0xf0]
100448e5c: f9400102    	ldr	x2, [x8]
100448e60: d10303a0    	sub	x0, x29, #0xc0
100448e64: aa1303e1    	mov	x1, x19
100448e68: 940018eb    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100448e6c: 385503a8    	ldurb	w8, [x29, #-0xb0]
100448e70: f85403a0    	ldur	x0, [x29, #-0xc0]
100448e74: 7100091f    	cmp	w8, #0x2
100448e78: 5400fd80    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100448e7c: 92401c08    	and	x8, x0, #0xff
100448e80: f100291f    	cmp	x8, #0xa
100448e84: f9408be1    	ldr	x1, [sp, #0x110]
100448e88: f94083e9    	ldr	x9, [sp, #0x100]
100448e8c: f9407bea    	ldr	x10, [sp, #0xf0]
100448e90: 54011f00    	b.eq	0x10044b270 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f7c>
100448e94: f85483a8    	ldur	x8, [x29, #-0xb8]
100448e98: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100448e9c: f9400142    	ldr	x2, [x10]
100448ea0: d10383a3    	sub	x3, x29, #0xe0
100448ea4: aa0903e0    	mov	x0, x9
100448ea8: 94001886    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100448eac: 14000497    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100448eb0: fd400760    	ldr	d0, [x27, #0x8]
100448eb4: f9407be8    	ldr	x8, [sp, #0xf0]
100448eb8: f9400100    	ldr	x0, [x8]
100448ebc: aa1403e1    	mov	x1, x20
100448ec0: aa1603e2    	mov	x2, x22
100448ec4: 94002246    	bl	0x1004517dc <__ZN13quickjs_oxide6engine8builtins12array_buffer11typed_array62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$28try_typed_array_number_write17hf73ee774e7469726E>
100448ec8: f9408be3    	ldr	x3, [sp, #0x110]
100448ecc: f94083e9    	ldr	x9, [sp, #0x100]
100448ed0: 3400d5a0    	cbz	w0, 0x10044a984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5690>
100448ed4: a9408921    	ldp	x1, x2, [x9, #0x8]
100448ed8: d10383a0    	sub	x0, x29, #0xe0
100448edc: 97ff9221    	bl	0x10042d760 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
100448ee0: 385203a8    	ldurb	w8, [x29, #-0xe0]
100448ee4: 7100291f    	cmp	w8, #0xa
100448ee8: 5400e1a0    	b.eq	0x10044ab1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5828>
100448eec: f94083e8    	ldr	x8, [sp, #0x100]
100448ef0: a9408901    	ldp	x1, x2, [x8, #0x8]
100448ef4: d10383a0    	sub	x0, x29, #0xe0
100448ef8: f9408be3    	ldr	x3, [sp, #0x110]
100448efc: 97ff9219    	bl	0x10042d760 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
100448f00: 385203a8    	ldurb	w8, [x29, #-0xe0]
100448f04: 7100291f    	cmp	w8, #0xa
100448f08: 5400e0a0    	b.eq	0x10044ab1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5828>
100448f0c: f94083e8    	ldr	x8, [sp, #0x100]
100448f10: a9408901    	ldp	x1, x2, [x8, #0x8]
100448f14: d10383a0    	sub	x0, x29, #0xe0
100448f18: f9408be3    	ldr	x3, [sp, #0x110]
100448f1c: 97ff9211    	bl	0x10042d760 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore11pop_current17hf5006ce21a4af049E>
100448f20: 385203a8    	ldurb	w8, [x29, #-0xe0]
100448f24: 7100291f    	cmp	w8, #0xa
100448f28: 5400dfa0    	b.eq	0x10044ab1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5828>
100448f2c: f94063ea    	ldr	x10, [sp, #0xc0]
100448f30: b9400149    	ldr	w9, [x10]
100448f34: f94073eb    	ldr	x11, [sp, #0xe0]
100448f38: b9000169    	str	w9, [x11]
100448f3c: b8403149    	ldur	w9, [x10, #0x3]
100448f40: b8003169    	stur	w9, [x11, #0x3]
100448f44: f85283a9    	ldur	x9, [x29, #-0xd8]
100448f48: 381403a8    	sturb	w8, [x29, #-0xc0]
100448f4c: f81483a9    	stur	x9, [x29, #-0xb8]
100448f50: f9407be8    	ldr	x8, [sp, #0xf0]
100448f54: f9400101    	ldr	x1, [x8]
100448f58: 910a43e0    	add	x0, sp, #0x290
100448f5c: d10303a2    	sub	x2, x29, #0xc0
100448f60: 97f03883    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
100448f64: 394a43e8    	ldrb	w8, [sp, #0x290]
100448f68: 71002d1f    	cmp	w8, #0xb
100448f6c: 54010ec1    	b.ne	0x10044b144 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e50>
100448f70: f9408fe8    	ldr	x8, [sp, #0x118]
100448f74: f9000513    	str	x19, [x8, #0x8]
100448f78: 14000465    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448f7c: fd400580    	ldr	d0, [x12, #0x8]
100448f80: 3900014d    	strb	w13, [x10]
100448f84: b9000548    	str	w8, [x10, #0x4]
100448f88: fd000540    	str	d0, [x10, #0x8]
100448f8c: 71009b7f    	cmp	w27, #0x26
100448f90: 54008be0    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448f94: 7100b37f    	cmp	w27, #0x2c
100448f98: 54008ba0    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448f9c: 528001c8    	mov	w8, #0xe                ; =14
100448fa0: 39000188    	strb	w8, [x12]
100448fa4: f90021eb    	str	x11, [x15, #0x40]
100448fa8: 14000459    	b	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
100448fac: d10303a0    	sub	x0, x29, #0xc0
100448fb0: aa0203e1    	mov	x1, x2
100448fb4: 9102e342    	add	x2, x26, #0xb8
100448fb8: 97ff7938    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
100448fbc: 385403a8    	ldurb	w8, [x29, #-0xc0]
100448fc0: 7100291f    	cmp	w8, #0xa
100448fc4: 5400fc40    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
100448fc8: f9407be9    	ldr	x9, [sp, #0xf0]
100448fcc: f9400122    	ldr	x2, [x9]
100448fd0: f9408be1    	ldr	x1, [sp, #0x110]
100448fd4: f94083e0    	ldr	x0, [sp, #0x100]
100448fd8: f94073ea    	ldr	x10, [sp, #0xe0]
100448fdc: 17fffd01    	b	0x1004483e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x30ec>
100448fe0: b85443a8    	ldur	w8, [x29, #-0xbc]
100448fe4: 52800069    	mov	w9, #0x3                ; =3
100448fe8: 390b03e9    	strb	w9, [sp, #0x2c0]
100448fec: b902c7e8    	str	w8, [sp, #0x2c4]
100448ff0: f9407fe9    	ldr	x9, [sp, #0xf8]
100448ff4: f9401528    	ldr	x8, [x9, #0x28]
100448ff8: 91000508    	add	x8, x8, #0x1
100448ffc: f9001528    	str	x8, [x9, #0x28]
100449000: 14000051    	b	0x100449144 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3e50>
100449004: 7200053f    	tst	w9, #0x3
100449008: 54001181    	b.ne	0x100449238 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f44>
10044900c: 1400001c    	b	0x10044907c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3d88>
100449010: 390663f4    	strb	w20, [sp, #0x198]
100449014: b94193e8    	ldr	w8, [sp, #0x190]
100449018: f9402fe9    	ldr	x9, [sp, #0x58]
10044901c: b9000128    	str	w8, [x9]
100449020: 910253e8    	add	x8, sp, #0x94
100449024: b84ff108    	ldur	w8, [x8, #0xff]
100449028: b8003128    	stur	w8, [x9, #0x3]
10044902c: f9401fe8    	ldr	x8, [sp, #0x38]
100449030: f900d3e8    	str	x8, [sp, #0x1a0]
100449034: f9407be8    	ldr	x8, [sp, #0xf0]
100449038: f9400102    	ldr	x2, [x8]
10044903c: 910663e3    	add	x3, sp, #0x198
100449040: f94083e0    	ldr	x0, [sp, #0x100]
100449044: f9408be1    	ldr	x1, [sp, #0x110]
100449048: 9400181e    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044904c: b500eee0    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449050: 52800028    	mov	w8, #0x1                ; =1
100449054: 6a53711f    	tst	w8, w19, lsr #28
100449058: 52800048    	mov	w8, #0x2                ; =2
10044905c: 9a880508    	cinc	x8, x8, ne
100449060: 8b160116    	add	x22, x8, x22
100449064: eb1c02df    	cmp	x22, x28
100449068: 540057c3    	b.lo	0x100449b60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x486c>
10044906c: 140009fe    	b	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100449070: 7100253f    	cmp	w9, #0x9
100449074: 54011fa1    	b.ne	0x10044b468 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6174>
100449078: f94001a1    	ldr	x1, [x13]
10044907c: d10303a0    	sub	x0, x29, #0xc0
100449080: 9102a342    	add	x2, x26, #0xa8
100449084: 97ff7905    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
100449088: 14000409    	b	0x10044a0ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4db8>
10044908c: f94001a1    	ldr	x1, [x13]
100449090: d10303a0    	sub	x0, x29, #0xc0
100449094: 97ff7901    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
100449098: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044909c: 7100291f    	cmp	w8, #0xa
1004490a0: f94083ec    	ldr	x12, [sp, #0x100]
1004490a4: f9407bed    	ldr	x13, [sp, #0xf0]
1004490a8: 910923ee    	add	x14, sp, #0x248
1004490ac: f94073ea    	ldr	x10, [sp, #0xe0]
1004490b0: 54ffa381    	b.ne	0x100448520 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x322c>
1004490b4: 140005df    	b	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
1004490b8: 381403bf    	sturb	wzr, [x29, #-0xc0]
1004490bc: d10303a2    	sub	x2, x29, #0xc0
1004490c0: 9400179c    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
1004490c4: 14000411    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004490c8: fd400900    	ldr	d0, [x8, #0x10]
1004490cc: fc1483a0    	stur	d0, [x29, #-0xb8]
1004490d0: 52800088    	mov	w8, #0x4                ; =4
1004490d4: 381403a8    	sturb	w8, [x29, #-0xc0]
1004490d8: d10303a2    	sub	x2, x29, #0xc0
1004490dc: 94001795    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
1004490e0: 1400040a    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004490e4: 39402508    	ldrb	w8, [x8, #0x9]
1004490e8: 381413a8    	sturb	w8, [x29, #-0xbf]
1004490ec: 52800048    	mov	w8, #0x2                ; =2
1004490f0: 381403a8    	sturb	w8, [x29, #-0xc0]
1004490f4: d10303a2    	sub	x2, x29, #0xc0
1004490f8: 9400178e    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
1004490fc: 14000403    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449100: f9400908    	ldr	x8, [x8, #0x10]
100449104: f81483a8    	stur	x8, [x29, #-0xb8]
100449108: 528000e8    	mov	w8, #0x7                ; =7
10044910c: 381403a8    	sturb	w8, [x29, #-0xc0]
100449110: d10303a2    	sub	x2, x29, #0xc0
100449114: 94001787    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
100449118: 140003fc    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044911c: 91002101    	add	x1, x8, #0x8
100449120: 910b03e0    	add	x0, sp, #0x2c0
100449124: 94002543    	bl	0x100452630 <__ZN13quickjs_oxide6engine6object16ordinary_storage23immediate_value_jsvalue17hc5220bf7094f18e2E>
100449128: 394b03e8    	ldrb	w8, [sp, #0x2c0]
10044912c: f9407fea    	ldr	x10, [sp, #0xf8]
100449130: f9401549    	ldr	x9, [x10, #0x28]
100449134: 91000529    	add	x9, x9, #0x1
100449138: f9001549    	str	x9, [x10, #0x28]
10044913c: 7100291f    	cmp	w8, #0xa
100449140: 540116e0    	b.eq	0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
100449144: 3dc0b3e0    	ldr	q0, [sp, #0x2c0]
100449148: 3d80afe0    	str	q0, [sp, #0x2b0]
10044914c: 910006c8    	add	x8, x22, #0x1
100449150: eb1c011f    	cmp	x8, x28
100449154: 540143e2    	b.hs	0x10044b9d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66dc>
100449158: f94013e9    	ldr	x9, [sp, #0x20]
10044915c: 8b081128    	add	x8, x9, x8, lsl #4
100449160: 39400109    	ldrb	w9, [x8]
100449164: 528001ca    	mov	w10, #0xe               ; =14
100449168: 3900010a    	strb	w10, [x8]
10044916c: 5100292a    	sub	w10, w9, #0xa
100449170: 7100115f    	cmp	w10, #0x4
100449174: f9401fee    	ldr	x14, [sp, #0x38]
100449178: 54013f49    	b.ls	0x10044b960 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x666c>
10044917c: 390b43e9    	strb	w9, [sp, #0x2d0]
100449180: f8401109    	ldur	x9, [x8, #0x1]
100449184: f94043ea    	ldr	x10, [sp, #0x80]
100449188: f9000149    	str	x9, [x10]
10044918c: f9400508    	ldr	x8, [x8, #0x8]
100449190: f8007148    	stur	x8, [x10, #0x7]
100449194: eb1c02df    	cmp	x22, x28
100449198: f9408bea    	ldr	x10, [sp, #0x110]
10044919c: f9407beb    	ldr	x11, [sp, #0xf0]
1004491a0: f94017ed    	ldr	x13, [sp, #0x28]
1004491a4: 54014042    	b.hs	0x10044b9ac <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66b8>
1004491a8: 394001c8    	ldrb	w8, [x14]
1004491ac: f84011c9    	ldur	x9, [x14, #0x1]
1004491b0: f81403a9    	stur	x9, [x29, #-0xc0]
1004491b4: f94005c9    	ldr	x9, [x14, #0x8]
1004491b8: d10303ac    	sub	x12, x29, #0xc0
1004491bc: f8007189    	stur	x9, [x12, #0x7]
1004491c0: 3dc0afe0    	ldr	q0, [sp, #0x2b0]
1004491c4: 3d8001c0    	str	q0, [x14]
1004491c8: 51002909    	sub	w9, w8, #0xa
1004491cc: 7100113f    	cmp	w9, #0x4
1004491d0: 54013bc9    	b.ls	0x10044b948 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6654>
1004491d4: 381103a8    	sturb	w8, [x29, #-0xf0]
1004491d8: f85403a8    	ldur	x8, [x29, #-0xc0]
1004491dc: f9403fe9    	ldr	x9, [sp, #0x78]
1004491e0: f9000128    	str	x8, [x9]
1004491e4: f8407188    	ldur	x8, [x12, #0x7]
1004491e8: f8007128    	stur	x8, [x9, #0x7]
1004491ec: f900214d    	str	x13, [x10, #0x40]
1004491f0: f9400161    	ldr	x1, [x11]
1004491f4: 910b83e0    	add	x0, sp, #0x2e0
1004491f8: 910b43e2    	add	x2, sp, #0x2d0
1004491fc: 97f037dc    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
100449200: 394b83e8    	ldrb	w8, [sp, #0x2e0]
100449204: 71002d1f    	cmp	w8, #0xb
100449208: 5400f6a1    	b.ne	0x10044b0dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5de8>
10044920c: f9407be8    	ldr	x8, [sp, #0xf0]
100449210: f9400101    	ldr	x1, [x8]
100449214: d10383a0    	sub	x0, x29, #0xe0
100449218: d103c3a2    	sub	x2, x29, #0xf0
10044921c: 97f037d4    	bl	0x10005716c <__ZN13quickjs_oxide6engine5value8js_value62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$15release_jsvalue17hddbdb91ed96f4b18E>
100449220: 385203a8    	ldurb	w8, [x29, #-0xe0]
100449224: 71002d1f    	cmp	w8, #0xb
100449228: 54007720    	b.eq	0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
10044922c: 140007b5    	b	0x10044b100 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e0c>
100449230: 7200055f    	tst	w10, #0x3
100449234: 54007380    	b.eq	0x10044a0a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4db0>
100449238: 381403a8    	sturb	w8, [x29, #-0xc0]
10044923c: f85203a9    	ldur	x9, [x29, #-0xe0]
100449240: f94073ea    	ldr	x10, [sp, #0xe0]
100449244: f9000149    	str	x9, [x10]
100449248: f84ff1c9    	ldur	x9, [x14, #0xff]
10044924c: f8007149    	stur	x9, [x10, #0x7]
100449250: 1400039f    	b	0x10044a0cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4dd8>
100449254: 7200053f    	tst	w9, #0x3
100449258: 540001a0    	b.eq	0x10044928c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x3f98>
10044925c: 381403a8    	sturb	w8, [x29, #-0xc0]
100449260: f94173e8    	ldr	x8, [sp, #0x2e0]
100449264: f94073e9    	ldr	x9, [sp, #0xe0]
100449268: f9000128    	str	x8, [x9]
10044926c: 910923e8    	add	x8, sp, #0x248
100449270: f849f108    	ldur	x8, [x8, #0x9f]
100449274: f8007128    	stur	x8, [x9, #0x7]
100449278: d10303a2    	sub	x2, x29, #0xc0
10044927c: aa0303e0    	mov	x0, x3
100449280: aa0403e1    	mov	x1, x4
100449284: 94001771    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449288: 140003a0    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044928c: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449290: f9400102    	ldr	x2, [x8]
100449294: d10303a0    	sub	x0, x29, #0xc0
100449298: 94001876    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044929c: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004492a0: 71002d1f    	cmp	w8, #0xb
1004492a4: 5400ac60    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
1004492a8: f94073ea    	ldr	x10, [sp, #0xe0]
1004492ac: b9400149    	ldr	w9, [x10]
1004492b0: b90303e9    	str	w9, [sp, #0x300]
1004492b4: b8403149    	ldur	w9, [x10, #0x3]
1004492b8: 910923eb    	add	x11, sp, #0x248
1004492bc: b80bb169    	stur	w9, [x11, #0xbb]
1004492c0: 7100291f    	cmp	w8, #0xa
1004492c4: f9408be1    	ldr	x1, [sp, #0x110]
1004492c8: f94083e0    	ldr	x0, [sp, #0x100]
1004492cc: f9407bea    	ldr	x10, [sp, #0xf0]
1004492d0: 5400b7c0    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
1004492d4: f85483a9    	ldur	x9, [x29, #-0xb8]
1004492d8: 381203a8    	sturb	w8, [x29, #-0xe0]
1004492dc: b94303e8    	ldr	w8, [sp, #0x300]
1004492e0: f94063ec    	ldr	x12, [sp, #0xc0]
1004492e4: b9000188    	str	w8, [x12]
1004492e8: b84bb168    	ldur	w8, [x11, #0xbb]
1004492ec: b8003188    	stur	w8, [x12, #0x3]
1004492f0: f81283a9    	stur	x9, [x29, #-0xd8]
1004492f4: f9400142    	ldr	x2, [x10]
1004492f8: d10383a3    	sub	x3, x29, #0xe0
1004492fc: 94001771    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449300: 14000382    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449304: 7200053f    	tst	w9, #0x3
100449308: 540001a0    	b.eq	0x10044933c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4048>
10044930c: 381403a8    	sturb	w8, [x29, #-0xc0]
100449310: f94173e8    	ldr	x8, [sp, #0x2e0]
100449314: f94073e9    	ldr	x9, [sp, #0xe0]
100449318: f9000128    	str	x8, [x9]
10044931c: 910923e8    	add	x8, sp, #0x248
100449320: f849f108    	ldur	x8, [x8, #0x9f]
100449324: f8007128    	stur	x8, [x9, #0x7]
100449328: d10303a2    	sub	x2, x29, #0xc0
10044932c: aa0303e0    	mov	x0, x3
100449330: aa0403e1    	mov	x1, x4
100449334: 94001745    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449338: 14000374    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044933c: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449340: f9400102    	ldr	x2, [x8]
100449344: d10303a0    	sub	x0, x29, #0xc0
100449348: 940017b3    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
10044934c: 385503a8    	ldurb	w8, [x29, #-0xb0]
100449350: f85403a0    	ldur	x0, [x29, #-0xc0]
100449354: 7100091f    	cmp	w8, #0x2
100449358: 5400d680    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044935c: 92401c08    	and	x8, x0, #0xff
100449360: f100291f    	cmp	x8, #0xa
100449364: f9408be1    	ldr	x1, [sp, #0x110]
100449368: f94083e9    	ldr	x9, [sp, #0x100]
10044936c: f9407bea    	ldr	x10, [sp, #0xf0]
100449370: 5400afe0    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
100449374: f85483a8    	ldur	x8, [x29, #-0xb8]
100449378: a93223a0    	stp	x0, x8, [x29, #-0xe0]
10044937c: f9400142    	ldr	x2, [x10]
100449380: d10383a3    	sub	x3, x29, #0xe0
100449384: aa0903e0    	mov	x0, x9
100449388: 9400174e    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044938c: 1400035f    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449390: 7200053f    	tst	w9, #0x3
100449394: 540001a0    	b.eq	0x1004493c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x40d4>
100449398: 381403a8    	sturb	w8, [x29, #-0xc0]
10044939c: f94173e8    	ldr	x8, [sp, #0x2e0]
1004493a0: f94073e9    	ldr	x9, [sp, #0xe0]
1004493a4: f9000128    	str	x8, [x9]
1004493a8: 910923e8    	add	x8, sp, #0x248
1004493ac: f849f108    	ldur	x8, [x8, #0x9f]
1004493b0: f8007128    	stur	x8, [x9, #0x7]
1004493b4: d10303a2    	sub	x2, x29, #0xc0
1004493b8: aa0303e0    	mov	x0, x3
1004493bc: aa0403e1    	mov	x1, x4
1004493c0: 94001722    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
1004493c4: 14000351    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004493c8: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
1004493cc: f9400102    	ldr	x2, [x8]
1004493d0: d10303a0    	sub	x0, x29, #0xc0
1004493d4: 94001827    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
1004493d8: 385403a8    	ldurb	w8, [x29, #-0xc0]
1004493dc: 71002d1f    	cmp	w8, #0xb
1004493e0: 5400a280    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
1004493e4: f94073ea    	ldr	x10, [sp, #0xe0]
1004493e8: b9400149    	ldr	w9, [x10]
1004493ec: b90303e9    	str	w9, [sp, #0x300]
1004493f0: b8403149    	ldur	w9, [x10, #0x3]
1004493f4: 910923eb    	add	x11, sp, #0x248
1004493f8: b80bb169    	stur	w9, [x11, #0xbb]
1004493fc: 7100291f    	cmp	w8, #0xa
100449400: f9408be1    	ldr	x1, [sp, #0x110]
100449404: f94083e0    	ldr	x0, [sp, #0x100]
100449408: f9407bea    	ldr	x10, [sp, #0xf0]
10044940c: 5400ade0    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
100449410: f85483a9    	ldur	x9, [x29, #-0xb8]
100449414: 381203a8    	sturb	w8, [x29, #-0xe0]
100449418: b94303e8    	ldr	w8, [sp, #0x300]
10044941c: f94063ec    	ldr	x12, [sp, #0xc0]
100449420: b9000188    	str	w8, [x12]
100449424: b84bb168    	ldur	w8, [x11, #0xbb]
100449428: b8003188    	stur	w8, [x12, #0x3]
10044942c: f81283a9    	stur	x9, [x29, #-0xd8]
100449430: f9400142    	ldr	x2, [x10]
100449434: d10383a3    	sub	x3, x29, #0xe0
100449438: 94001722    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044943c: 14000333    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449440: 7200053f    	tst	w9, #0x3
100449444: 540001a0    	b.eq	0x100449478 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4184>
100449448: 381403a8    	sturb	w8, [x29, #-0xc0]
10044944c: f94173e8    	ldr	x8, [sp, #0x2e0]
100449450: f94073e9    	ldr	x9, [sp, #0xe0]
100449454: f9000128    	str	x8, [x9]
100449458: 910923e8    	add	x8, sp, #0x248
10044945c: f849f108    	ldur	x8, [x8, #0x9f]
100449460: f8007128    	stur	x8, [x9, #0x7]
100449464: d10303a2    	sub	x2, x29, #0xc0
100449468: aa0303e0    	mov	x0, x3
10044946c: aa0403e1    	mov	x1, x4
100449470: 940016f6    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449474: 14000325    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449478: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
10044947c: f9400102    	ldr	x2, [x8]
100449480: d10303a0    	sub	x0, x29, #0xc0
100449484: 94001764    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100449488: 385503a8    	ldurb	w8, [x29, #-0xb0]
10044948c: f85403a0    	ldur	x0, [x29, #-0xc0]
100449490: 7100091f    	cmp	w8, #0x2
100449494: 5400cca0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449498: 92401c08    	and	x8, x0, #0xff
10044949c: f100291f    	cmp	x8, #0xa
1004494a0: f9408be1    	ldr	x1, [sp, #0x110]
1004494a4: f94083e9    	ldr	x9, [sp, #0x100]
1004494a8: f9407bea    	ldr	x10, [sp, #0xf0]
1004494ac: 5400a600    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
1004494b0: f85483a8    	ldur	x8, [x29, #-0xb8]
1004494b4: a93223a0    	stp	x0, x8, [x29, #-0xe0]
1004494b8: f9400142    	ldr	x2, [x10]
1004494bc: d10383a3    	sub	x3, x29, #0xe0
1004494c0: aa0903e0    	mov	x0, x9
1004494c4: 940016ff    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
1004494c8: 14000310    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004494cc: 7200053f    	tst	w9, #0x3
1004494d0: 540001a0    	b.eq	0x100449504 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4210>
1004494d4: 381403a8    	sturb	w8, [x29, #-0xc0]
1004494d8: f94173e8    	ldr	x8, [sp, #0x2e0]
1004494dc: f94073e9    	ldr	x9, [sp, #0xe0]
1004494e0: f9000128    	str	x8, [x9]
1004494e4: 910923e8    	add	x8, sp, #0x248
1004494e8: f849f108    	ldur	x8, [x8, #0x9f]
1004494ec: f8007128    	stur	x8, [x9, #0x7]
1004494f0: d10303a2    	sub	x2, x29, #0xc0
1004494f4: aa0303e0    	mov	x0, x3
1004494f8: aa0403e1    	mov	x1, x4
1004494fc: 940016d3    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449500: 14000302    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449504: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449508: f9400102    	ldr	x2, [x8]
10044950c: d10303a0    	sub	x0, x29, #0xc0
100449510: 940017d8    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
100449514: 385403a8    	ldurb	w8, [x29, #-0xc0]
100449518: 71002d1f    	cmp	w8, #0xb
10044951c: 540098a0    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100449520: f94073ea    	ldr	x10, [sp, #0xe0]
100449524: b9400149    	ldr	w9, [x10]
100449528: b90303e9    	str	w9, [sp, #0x300]
10044952c: b8403149    	ldur	w9, [x10, #0x3]
100449530: 910923eb    	add	x11, sp, #0x248
100449534: b80bb169    	stur	w9, [x11, #0xbb]
100449538: 7100291f    	cmp	w8, #0xa
10044953c: f9408be1    	ldr	x1, [sp, #0x110]
100449540: f94083e0    	ldr	x0, [sp, #0x100]
100449544: f9407bea    	ldr	x10, [sp, #0xf0]
100449548: 5400a400    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
10044954c: f85483a9    	ldur	x9, [x29, #-0xb8]
100449550: 381203a8    	sturb	w8, [x29, #-0xe0]
100449554: b94303e8    	ldr	w8, [sp, #0x300]
100449558: f94063ec    	ldr	x12, [sp, #0xc0]
10044955c: b9000188    	str	w8, [x12]
100449560: b84bb168    	ldur	w8, [x11, #0xbb]
100449564: b8003188    	stur	w8, [x12, #0x3]
100449568: f81283a9    	stur	x9, [x29, #-0xd8]
10044956c: f9400142    	ldr	x2, [x10]
100449570: d10383a3    	sub	x3, x29, #0xe0
100449574: 940016d3    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449578: 140002e4    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044957c: 7200053f    	tst	w9, #0x3
100449580: 540001a0    	b.eq	0x1004495b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x42c0>
100449584: 381403a8    	sturb	w8, [x29, #-0xc0]
100449588: f94173e8    	ldr	x8, [sp, #0x2e0]
10044958c: f94073e9    	ldr	x9, [sp, #0xe0]
100449590: f9000128    	str	x8, [x9]
100449594: 910923e8    	add	x8, sp, #0x248
100449598: f849f108    	ldur	x8, [x8, #0x9f]
10044959c: f8007128    	stur	x8, [x9, #0x7]
1004495a0: d10303a2    	sub	x2, x29, #0xc0
1004495a4: aa0303e0    	mov	x0, x3
1004495a8: aa0403e1    	mov	x1, x4
1004495ac: 940016a7    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
1004495b0: 140002d6    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004495b4: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
1004495b8: f9400102    	ldr	x2, [x8]
1004495bc: d10303a0    	sub	x0, x29, #0xc0
1004495c0: 94001715    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
1004495c4: 385503a8    	ldurb	w8, [x29, #-0xb0]
1004495c8: f85403a0    	ldur	x0, [x29, #-0xc0]
1004495cc: 7100091f    	cmp	w8, #0x2
1004495d0: 5400c2c0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
1004495d4: 92401c08    	and	x8, x0, #0xff
1004495d8: f100291f    	cmp	x8, #0xa
1004495dc: f9408be1    	ldr	x1, [sp, #0x110]
1004495e0: f94083e9    	ldr	x9, [sp, #0x100]
1004495e4: f9407bea    	ldr	x10, [sp, #0xf0]
1004495e8: 54009c20    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
1004495ec: f85483a8    	ldur	x8, [x29, #-0xb8]
1004495f0: a93223a0    	stp	x0, x8, [x29, #-0xe0]
1004495f4: f9400142    	ldr	x2, [x10]
1004495f8: d10383a3    	sub	x3, x29, #0xe0
1004495fc: aa0903e0    	mov	x0, x9
100449600: 940016b0    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449604: 140002c1    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449608: 7200053f    	tst	w9, #0x3
10044960c: 540001a0    	b.eq	0x100449640 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x434c>
100449610: 381403a8    	sturb	w8, [x29, #-0xc0]
100449614: f94173e8    	ldr	x8, [sp, #0x2e0]
100449618: f94073e9    	ldr	x9, [sp, #0xe0]
10044961c: f9000128    	str	x8, [x9]
100449620: 910923e8    	add	x8, sp, #0x248
100449624: f849f108    	ldur	x8, [x8, #0x9f]
100449628: f8007128    	stur	x8, [x9, #0x7]
10044962c: d10303a2    	sub	x2, x29, #0xc0
100449630: aa0303e0    	mov	x0, x3
100449634: aa0403e1    	mov	x1, x4
100449638: 94001684    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044963c: 140002b3    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449640: f9407be8    	ldr	x8, [sp, #0xf0]
100449644: f9400102    	ldr	x2, [x8]
100449648: d10303a0    	sub	x0, x29, #0xc0
10044964c: aa1403e1    	mov	x1, x20
100449650: 940016f1    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100449654: 385503a8    	ldurb	w8, [x29, #-0xb0]
100449658: f85403a0    	ldur	x0, [x29, #-0xc0]
10044965c: 7100091f    	cmp	w8, #0x2
100449660: 5400be40    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449664: 92401c08    	and	x8, x0, #0xff
100449668: f100291f    	cmp	x8, #0xa
10044966c: f9408be1    	ldr	x1, [sp, #0x110]
100449670: f94083e9    	ldr	x9, [sp, #0x100]
100449674: f9407bea    	ldr	x10, [sp, #0xf0]
100449678: 5400d7a0    	b.eq	0x10044b16c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e78>
10044967c: f85483a8    	ldur	x8, [x29, #-0xb8]
100449680: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100449684: f9400142    	ldr	x2, [x10]
100449688: d10383a3    	sub	x3, x29, #0xe0
10044968c: aa0903e0    	mov	x0, x9
100449690: 9400168c    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449694: 1400029d    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449698: 7200053f    	tst	w9, #0x3
10044969c: 540001a0    	b.eq	0x1004496d0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x43dc>
1004496a0: 381403a8    	sturb	w8, [x29, #-0xc0]
1004496a4: f94173e8    	ldr	x8, [sp, #0x2e0]
1004496a8: f94073e9    	ldr	x9, [sp, #0xe0]
1004496ac: f9000128    	str	x8, [x9]
1004496b0: 910923e8    	add	x8, sp, #0x248
1004496b4: f849f108    	ldur	x8, [x8, #0x9f]
1004496b8: f8007128    	stur	x8, [x9, #0x7]
1004496bc: d10303a2    	sub	x2, x29, #0xc0
1004496c0: aa0303e0    	mov	x0, x3
1004496c4: aa0403e1    	mov	x1, x4
1004496c8: 94001660    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
1004496cc: 1400028f    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004496d0: f9407be8    	ldr	x8, [sp, #0xf0]
1004496d4: f9400102    	ldr	x2, [x8]
1004496d8: d10303a0    	sub	x0, x29, #0xc0
1004496dc: aa1403e1    	mov	x1, x20
1004496e0: 940016cd    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
1004496e4: 385503a8    	ldurb	w8, [x29, #-0xb0]
1004496e8: f85403a0    	ldur	x0, [x29, #-0xc0]
1004496ec: 7100091f    	cmp	w8, #0x2
1004496f0: 5400b9c0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
1004496f4: 92401c08    	and	x8, x0, #0xff
1004496f8: f100291f    	cmp	x8, #0xa
1004496fc: f9408be1    	ldr	x1, [sp, #0x110]
100449700: f94083e9    	ldr	x9, [sp, #0x100]
100449704: f9407bea    	ldr	x10, [sp, #0xf0]
100449708: 5400d320    	b.eq	0x10044b16c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e78>
10044970c: f85483a8    	ldur	x8, [x29, #-0xb8]
100449710: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100449714: f9400142    	ldr	x2, [x10]
100449718: d10383a3    	sub	x3, x29, #0xe0
10044971c: aa0903e0    	mov	x0, x9
100449720: 94001668    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449724: 14000279    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449728: 5280000e    	mov	w14, #0x0               ; =0
10044972c: 52800011    	mov	w17, #0x0               ; =0
100449730: 720f001f    	tst	w0, #0x20000
100449734: 1a8f020f    	csel	w15, w16, w15, eq
100449738: 12001d4a    	and	w10, w10, #0xff
10044973c: 71000d5f    	cmp	w10, #0x3
100449740: 1a8f31a3    	csel	w3, w13, w15, lo
100449744: 1a9131ca    	csel	w10, w14, w17, lo
100449748: 37f80483    	tbnz	w3, #0x1f, 0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
10044974c: 3700046a    	tbnz	w10, #0x0, 0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
100449750: 5280ffca    	mov	w10, #0x7fe             ; =2046
100449754: 1ac82548    	lsr	w8, w10, w8
100449758: 2a080128    	orr	w8, w9, w8
10044975c: 7200011f    	tst	w8, #0x1
100449760: f94067e8    	ldr	x8, [sp, #0xc8]
100449764: f9406fea    	ldr	x10, [sp, #0xd8]
100449768: 9a8a1108    	csel	x8, x8, x10, ne
10044976c: f94077e9    	ldr	x9, [sp, #0xe8]
100449770: 9a891149    	csel	x9, x10, x9, ne
100449774: f9400121    	ldr	x1, [x9]
100449778: f9400116    	ldr	x22, [x8]
10044977c: eb160028    	subs	x8, x1, x22
100449780: 5400e603    	b.lo	0x10044b440 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x614c>
100449784: eb0b003f    	cmp	x1, x11
100449788: 5400ec08    	b.hi	0x10044b508 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6214>
10044978c: f9407fe9    	ldr	x9, [sp, #0xf8]
100449790: 12003d29    	and	w9, w9, #0xffff
100449794: eb09011f    	cmp	x8, x9
100449798: 54000209    	b.ls	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
10044979c: 8b161188    	add	x8, x12, x22, lsl #4
1004497a0: 8b091102    	add	x2, x8, x9, lsl #4
1004497a4: 39400048    	ldrb	w8, [x2]
1004497a8: 7100251f    	cmp	w8, #0x9
1004497ac: 54000168    	b.hi	0x1004497d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x44e4>
1004497b0: b9002bf1    	str	w17, [sp, #0x28]
1004497b4: b90023ef    	str	w15, [sp, #0x20]
1004497b8: a90313e0    	stp	x0, x4, [sp, #0x30]
1004497bc: f9407be8    	ldr	x8, [sp, #0xf0]
1004497c0: f9400101    	ldr	x1, [x8]
1004497c4: d10303a0    	sub	x0, x29, #0xc0
1004497c8: 940023c7    	bl	0x1004526e4 <__ZN13quickjs_oxide6engine6object16ordinary_storage62_$LT$impl$u20$quickjs_oxide..engine..api..runtime..Runtime$GT$17peek_dense_number17h2449f7698d3d5da4E>
1004497cc: b85403a8    	ldur	w8, [x29, #-0xc0]
1004497d0: 7100091f    	cmp	w8, #0x2
1004497d4: 54005901    	b.ne	0x10044a2f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5000>
1004497d8: 71036b7f    	cmp	w27, #0xda
1004497dc: 540002e1    	b.ne	0x100449838 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4544>
1004497e0: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
1004497e4: f9408be1    	ldr	x1, [sp, #0x110]
1004497e8: 52800002    	mov	w2, #0x0                ; =0
1004497ec: 940015ef    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004497f0: f9408be4    	ldr	x4, [sp, #0x110]
1004497f4: f94083e3    	ldr	x3, [sp, #0x100]
1004497f8: f9407beb    	ldr	x11, [sp, #0xf0]
1004497fc: 910923ec    	add	x12, sp, #0x248
100449800: b4000940    	cbz	x0, 0x100449928 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4634>
100449804: 39400008    	ldrb	w8, [x0]
100449808: 71001d1f    	cmp	w8, #0x7
10044980c: 540008e8    	b.hi	0x100449928 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4634>
100449810: 52800029    	mov	w9, #0x1                ; =1
100449814: 1ac82129    	lsl	w9, w9, w8
100449818: 5280138a    	mov	w10, #0x9c              ; =156
10044981c: 6a0a013f    	tst	w9, w10
100449820: 540006a0    	b.eq	0x1004498f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4600>
100449824: f8401009    	ldur	x9, [x0, #0x1]
100449828: f90173e9    	str	x9, [sp, #0x2e0]
10044982c: f9400409    	ldr	x9, [x0, #0x8]
100449830: f809f189    	stur	x9, [x12, #0x9f]
100449834: 14000032    	b	0x1004498fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4608>
100449838: 71033b7f    	cmp	w27, #0xce
10044983c: f9408be1    	ldr	x1, [sp, #0x110]
100449840: f94083e0    	ldr	x0, [sp, #0x100]
100449844: 54000060    	b.eq	0x100449850 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x455c>
100449848: 7103677f    	cmp	w27, #0xd9
10044984c: 540002c1    	b.ne	0x1004498a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x45b0>
100449850: 52800002    	mov	w2, #0x0                ; =0
100449854: f9407fe3    	ldr	x3, [sp, #0xf8]
100449858: 940015d4    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
10044985c: f9408be4    	ldr	x4, [sp, #0x110]
100449860: f94083e3    	ldr	x3, [sp, #0x100]
100449864: f9407beb    	ldr	x11, [sp, #0xf0]
100449868: 910923ec    	add	x12, sp, #0x248
10044986c: b4000a20    	cbz	x0, 0x1004499b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x46bc>
100449870: 39400008    	ldrb	w8, [x0]
100449874: 71001d1f    	cmp	w8, #0x7
100449878: 540009c8    	b.hi	0x1004499b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x46bc>
10044987c: 52800029    	mov	w9, #0x1                ; =1
100449880: 1ac82129    	lsl	w9, w9, w8
100449884: 5280138a    	mov	w10, #0x9c              ; =156
100449888: 6a0a013f    	tst	w9, w10
10044988c: 54000780    	b.eq	0x10044997c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4688>
100449890: f8401009    	ldur	x9, [x0, #0x1]
100449894: f90173e9    	str	x9, [sp, #0x2e0]
100449898: f9400409    	ldr	x9, [x0, #0x8]
10044989c: f809f189    	stur	x9, [x12, #0x9f]
1004498a0: 14000039    	b	0x100449984 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4690>
1004498a4: 52800022    	mov	w2, #0x1                ; =1
1004498a8: f9407fe3    	ldr	x3, [sp, #0xf8]
1004498ac: 940015bf    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
1004498b0: f9408be4    	ldr	x4, [sp, #0x110]
1004498b4: f94083e3    	ldr	x3, [sp, #0x100]
1004498b8: 910923eb    	add	x11, sp, #0x248
1004498bc: b4000be0    	cbz	x0, 0x100449a38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4744>
1004498c0: 39400008    	ldrb	w8, [x0]
1004498c4: 71001d1f    	cmp	w8, #0x7
1004498c8: 54000b88    	b.hi	0x100449a38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4744>
1004498cc: 52800029    	mov	w9, #0x1                ; =1
1004498d0: 1ac82129    	lsl	w9, w9, w8
1004498d4: 5280138a    	mov	w10, #0x9c              ; =156
1004498d8: 6a0a013f    	tst	w9, w10
1004498dc: 54000940    	b.eq	0x100449a04 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4710>
1004498e0: f8401009    	ldur	x9, [x0, #0x1]
1004498e4: f90173e9    	str	x9, [sp, #0x2e0]
1004498e8: f9400409    	ldr	x9, [x0, #0x8]
1004498ec: f809f169    	stur	x9, [x11, #0x9f]
1004498f0: 14000047    	b	0x100449a0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4718>
1004498f4: 7200053f    	tst	w9, #0x3
1004498f8: 54000180    	b.eq	0x100449928 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4634>
1004498fc: 381403a8    	sturb	w8, [x29, #-0xc0]
100449900: f94173e8    	ldr	x8, [sp, #0x2e0]
100449904: f94073e9    	ldr	x9, [sp, #0xe0]
100449908: f9000128    	str	x8, [x9]
10044990c: f849f188    	ldur	x8, [x12, #0x9f]
100449910: f8007128    	stur	x8, [x9, #0x7]
100449914: d10303a2    	sub	x2, x29, #0xc0
100449918: aa0303e0    	mov	x0, x3
10044991c: aa0403e1    	mov	x1, x4
100449920: 940015ca    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449924: 140001f9    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449928: f9400162    	ldr	x2, [x11]
10044992c: d10303a0    	sub	x0, x29, #0xc0
100449930: f9407fe1    	ldr	x1, [sp, #0xf8]
100449934: 94001638    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100449938: 385503a8    	ldurb	w8, [x29, #-0xb0]
10044993c: f85403a0    	ldur	x0, [x29, #-0xc0]
100449940: 7100091f    	cmp	w8, #0x2
100449944: 5400a720    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449948: 92401c09    	and	x9, x0, #0xff
10044994c: f100293f    	cmp	x9, #0xa
100449950: f9408be1    	ldr	x1, [sp, #0x110]
100449954: f94083e9    	ldr	x9, [sp, #0x100]
100449958: f9407bea    	ldr	x10, [sp, #0xf0]
10044995c: 5400a400    	b.eq	0x10044addc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ae8>
100449960: f85483a8    	ldur	x8, [x29, #-0xb8]
100449964: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100449968: f9400142    	ldr	x2, [x10]
10044996c: d10383a3    	sub	x3, x29, #0xe0
100449970: aa0903e0    	mov	x0, x9
100449974: 940015d3    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449978: 140001e4    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044997c: 7200053f    	tst	w9, #0x3
100449980: 54000180    	b.eq	0x1004499b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x46bc>
100449984: 381403a8    	sturb	w8, [x29, #-0xc0]
100449988: f94173e8    	ldr	x8, [sp, #0x2e0]
10044998c: f94073e9    	ldr	x9, [sp, #0xe0]
100449990: f9000128    	str	x8, [x9]
100449994: f849f188    	ldur	x8, [x12, #0x9f]
100449998: f8007128    	stur	x8, [x9, #0x7]
10044999c: d10303a2    	sub	x2, x29, #0xc0
1004499a0: aa0303e0    	mov	x0, x3
1004499a4: aa0403e1    	mov	x1, x4
1004499a8: 940015a8    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
1004499ac: 140001d7    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
1004499b0: f9400162    	ldr	x2, [x11]
1004499b4: d10303a0    	sub	x0, x29, #0xc0
1004499b8: f9407fe1    	ldr	x1, [sp, #0xf8]
1004499bc: 94001616    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
1004499c0: 385503a8    	ldurb	w8, [x29, #-0xb0]
1004499c4: f85403a0    	ldur	x0, [x29, #-0xc0]
1004499c8: 7100091f    	cmp	w8, #0x2
1004499cc: 5400a2e0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
1004499d0: 92401c08    	and	x8, x0, #0xff
1004499d4: f100291f    	cmp	x8, #0xa
1004499d8: f9408be1    	ldr	x1, [sp, #0x110]
1004499dc: f94083e9    	ldr	x9, [sp, #0x100]
1004499e0: f9407bea    	ldr	x10, [sp, #0xf0]
1004499e4: 54007c40    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
1004499e8: f85483a8    	ldur	x8, [x29, #-0xb8]
1004499ec: a93223a0    	stp	x0, x8, [x29, #-0xe0]
1004499f0: f9400142    	ldr	x2, [x10]
1004499f4: d10383a3    	sub	x3, x29, #0xe0
1004499f8: aa0903e0    	mov	x0, x9
1004499fc: 940015b1    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449a00: 140001c2    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449a04: 7200053f    	tst	w9, #0x3
100449a08: 54000180    	b.eq	0x100449a38 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4744>
100449a0c: 381403a8    	sturb	w8, [x29, #-0xc0]
100449a10: f94173e8    	ldr	x8, [sp, #0x2e0]
100449a14: f94073e9    	ldr	x9, [sp, #0xe0]
100449a18: f9000128    	str	x8, [x9]
100449a1c: f849f168    	ldur	x8, [x11, #0x9f]
100449a20: f8007128    	stur	x8, [x9, #0x7]
100449a24: d10303a2    	sub	x2, x29, #0xc0
100449a28: aa0303e0    	mov	x0, x3
100449a2c: aa0403e1    	mov	x1, x4
100449a30: 94001586    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449a34: 140001b5    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449a38: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449a3c: f9400102    	ldr	x2, [x8]
100449a40: d10303a0    	sub	x0, x29, #0xc0
100449a44: 9400168b    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
100449a48: 385403a8    	ldurb	w8, [x29, #-0xc0]
100449a4c: 71002d1f    	cmp	w8, #0xb
100449a50: 54006f00    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100449a54: f94073ea    	ldr	x10, [sp, #0xe0]
100449a58: b9400149    	ldr	w9, [x10]
100449a5c: b90303e9    	str	w9, [sp, #0x300]
100449a60: b8403149    	ldur	w9, [x10, #0x3]
100449a64: 910923eb    	add	x11, sp, #0x248
100449a68: b80bb169    	stur	w9, [x11, #0xbb]
100449a6c: 7100291f    	cmp	w8, #0xa
100449a70: f9408be1    	ldr	x1, [sp, #0x110]
100449a74: f94083e0    	ldr	x0, [sp, #0x100]
100449a78: f9407bea    	ldr	x10, [sp, #0xf0]
100449a7c: 54007a60    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
100449a80: f85483a9    	ldur	x9, [x29, #-0xb8]
100449a84: 381203a8    	sturb	w8, [x29, #-0xe0]
100449a88: b94303e8    	ldr	w8, [sp, #0x300]
100449a8c: f94063ec    	ldr	x12, [sp, #0xc0]
100449a90: b9000188    	str	w8, [x12]
100449a94: b84bb168    	ldur	w8, [x11, #0xbb]
100449a98: b8003188    	stur	w8, [x12, #0x3]
100449a9c: f81283a9    	stur	x9, [x29, #-0xd8]
100449aa0: f9400142    	ldr	x2, [x10]
100449aa4: d10383a3    	sub	x3, x29, #0xe0
100449aa8: 94001586    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449aac: 14000197    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449ab0: 7200053f    	tst	w9, #0x3
100449ab4: 540001a0    	b.eq	0x100449ae8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x47f4>
100449ab8: 381403a8    	sturb	w8, [x29, #-0xc0]
100449abc: f94173e8    	ldr	x8, [sp, #0x2e0]
100449ac0: f94073e9    	ldr	x9, [sp, #0xe0]
100449ac4: f9000128    	str	x8, [x9]
100449ac8: 910923e8    	add	x8, sp, #0x248
100449acc: f849f108    	ldur	x8, [x8, #0x9f]
100449ad0: f8007128    	stur	x8, [x9, #0x7]
100449ad4: d10303a2    	sub	x2, x29, #0xc0
100449ad8: f94083e0    	ldr	x0, [sp, #0x100]
100449adc: f9408be1    	ldr	x1, [sp, #0x110]
100449ae0: 9400155a    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449ae4: 14000017    	b	0x100449b40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x484c>
100449ae8: f9407be8    	ldr	x8, [sp, #0xf0]
100449aec: f9400102    	ldr	x2, [x8]
100449af0: d10303a0    	sub	x0, x29, #0xc0
100449af4: aa1403e1    	mov	x1, x20
100449af8: f94083e3    	ldr	x3, [sp, #0x100]
100449afc: f9408be4    	ldr	x4, [sp, #0x110]
100449b00: 940015c5    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100449b04: 385503a8    	ldurb	w8, [x29, #-0xb0]
100449b08: f85403a0    	ldur	x0, [x29, #-0xc0]
100449b0c: 7100091f    	cmp	w8, #0x2
100449b10: 540098c0    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449b14: 92401c08    	and	x8, x0, #0xff
100449b18: f100291f    	cmp	x8, #0xa
100449b1c: 5400b280    	b.eq	0x10044b16c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e78>
100449b20: f85483a8    	ldur	x8, [x29, #-0xb8]
100449b24: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100449b28: f9407be8    	ldr	x8, [sp, #0xf0]
100449b2c: f9400102    	ldr	x2, [x8]
100449b30: d10383a3    	sub	x3, x29, #0xe0
100449b34: f94083e0    	ldr	x0, [sp, #0x100]
100449b38: f9408be1    	ldr	x1, [sp, #0x110]
100449b3c: 94001561    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449b40: b5009740    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449b44: 52800028    	mov	w8, #0x1                ; =1
100449b48: 6a53711f    	tst	w8, w19, lsr #28
100449b4c: 52800048    	mov	w8, #0x2                ; =2
100449b50: 9a880508    	cinc	x8, x8, ne
100449b54: 8b394116    	add	x22, x8, w25, uxtw
100449b58: eb1c02df    	cmp	x22, x28
100449b5c: 5400e842    	b.hs	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
100449b60: b8767b19    	ldr	w25, [x24, x22, lsl #2]
100449b64: 1400016b    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
100449b68: 7200051f    	tst	w8, #0x3
100449b6c: 54003480    	b.eq	0x10044a1fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f08>
100449b70: f85203a8    	ldur	x8, [x29, #-0xe0]
100449b74: f94073ea    	ldr	x10, [sp, #0xe0]
100449b78: f9000148    	str	x8, [x10]
100449b7c: 910923e9    	add	x9, sp, #0x248
100449b80: f84ff128    	ldur	x8, [x9, #0xff]
100449b84: f8007148    	stur	x8, [x10, #0x7]
100449b88: aa0a03e8    	mov	x8, x10
100449b8c: aa0803ea    	mov	x10, x8
100449b90: b9400108    	ldr	w8, [x8]
100449b94: b902e3e8    	str	w8, [sp, #0x2e0]
100449b98: b8403148    	ldur	w8, [x10, #0x3]
100449b9c: b809b128    	stur	w8, [x9, #0x9b]
100449ba0: f85483b3    	ldur	x19, [x29, #-0xb8]
100449ba4: f9407fe0    	ldr	x0, [sp, #0xf8]
100449ba8: aa1c03e1    	mov	x1, x28
100449bac: f9408be2    	ldr	x2, [sp, #0x110]
100449bb0: 97f4d0db    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
100449bb4: aa0003e8    	mov	x8, x0
100449bb8: aa0103e0    	mov	x0, x1
100449bbc: 36000068    	tbz	w8, #0x0, 0x100449bc8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x48d4>
100449bc0: b5002a40    	cbnz	x0, 0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449bc4: 14000014    	b	0x100449c14 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4920>
100449bc8: eb00039f    	cmp	x28, x0
100449bcc: 5400f489    	b.ls	0x10044ba5c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6768>
100449bd0: f9407fe8    	ldr	x8, [sp, #0xf8]
100449bd4: 8b001108    	add	x8, x8, x0, lsl #4
100449bd8: 3900011b    	strb	w27, [x8]
100449bdc: b942e3e9    	ldr	w9, [sp, #0x2e0]
100449be0: b8001109    	stur	w9, [x8, #0x1]
100449be4: 910923e9    	add	x9, sp, #0x248
100449be8: b849b129    	ldur	w9, [x9, #0x9b]
100449bec: b9000509    	str	w9, [x8, #0x4]
100449bf0: f9000513    	str	x19, [x8, #0x8]
100449bf4: f9401fe9    	ldr	x9, [sp, #0x38]
100449bf8: 91000529    	add	x9, x9, #0x1
100449bfc: f9408be8    	ldr	x8, [sp, #0x110]
100449c00: f9002109    	str	x9, [x8, #0x40]
100449c04: f1000d3f    	cmp	x9, #0x3
100449c08: 54005043    	b.lo	0x10044a610 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x531c>
100449c0c: f9001fe9    	str	x9, [sp, #0x38]
100449c10: 8b180136    	add	x22, x9, x24
100449c14: f9407fe8    	ldr	x8, [sp, #0xf8]
100449c18: eb1c02df    	cmp	x22, x28
100449c1c: 5400e0c2    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
100449c20: 8b161102    	add	x2, x8, x22, lsl #4
100449c24: 3940005b    	ldrb	w27, [x2]
100449c28: 51002b68    	sub	w8, w27, #0xa
100449c2c: 7100151f    	cmp	w8, #0x5
100449c30: 540035e3    	b.lo	0x10044a2ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ff8>
100449c34: 71001f7f    	cmp	w27, #0x7
100449c38: 54004348    	b.hi	0x10044a4a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x51ac>
100449c3c: 52800028    	mov	w8, #0x1                ; =1
100449c40: 1adb2108    	lsl	w8, w8, w27
100449c44: 52801389    	mov	w9, #0x9c               ; =156
100449c48: 6a09011f    	tst	w8, w9
100449c4c: 54002ea0    	b.eq	0x10044a220 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f2c>
100449c50: f8401048    	ldur	x8, [x2, #0x1]
100449c54: f81203a8    	stur	x8, [x29, #-0xe0]
100449c58: f9400448    	ldr	x8, [x2, #0x8]
100449c5c: 910923e9    	add	x9, sp, #0x248
100449c60: f80ff128    	stur	x8, [x9, #0xff]
100449c64: 14000171    	b	0x10044a228 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f34>
100449c68: b9400508    	ldr	w8, [x8, #0x4]
100449c6c: 71000509    	subs	w9, w8, #0x1
100449c70: 1a9f77eb    	cset	w11, vs
100449c74: 3100050a    	adds	w10, w8, #0x1
100449c78: 1a9f77ec    	cset	w12, vs
100449c7c: f9407fee    	ldr	x14, [sp, #0xf8]
100449c80: 721301df    	tst	w14, #0x2000
100449c84: 1a8c016b    	csel	w11, w11, w12, eq
100449c88: 3600252b    	tbz	w11, #0x0, 0x10044a12c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e38>
100449c8c: 1e6e1000    	fmov	d0, #1.00000000
100449c90: 1e7e1001    	fmov	d1, #-1.00000000
100449c94: 1e600c20    	fcsel	d0, d1, d0, eq
100449c98: 1e620101    	scvtf	d1, w8
100449c9c: 1e612800    	fadd	d0, d0, d1
100449ca0: fc1483a0    	stur	d0, [x29, #-0xb8]
100449ca4: 52800028    	mov	w8, #0x1                ; =1
100449ca8: 14000124    	b	0x10044a138 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e44>
100449cac: 5280000c    	mov	w12, #0x0               ; =0
100449cb0: bd400520    	ldr	s0, [x9, #0x4]
100449cb4: 0f20a400    	sshll.2d	v0, v0, #0x0
100449cb8: 5e61d800    	scvtf	d0, d0
100449cbc: 17fff648    	b	0x1004475dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x22e8>
100449cc0: 5280000b    	mov	w11, #0x0               ; =0
100449cc4: bd400580    	ldr	s0, [x12, #0x4]
100449cc8: 0f20a400    	sshll.2d	v0, v0, #0x0
100449ccc: 5e61d800    	scvtf	d0, d0
100449cd0: 17fff660    	b	0x100447650 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x235c>
100449cd4: 360024ac    	tbz	w12, #0x0, 0x10044a168 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e74>
100449cd8: 7103537f    	cmp	w27, #0xd4
100449cdc: 540002c1    	b.ne	0x100449d34 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a40>
100449ce0: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100449ce4: f9408be1    	ldr	x1, [sp, #0x110]
100449ce8: 52800002    	mov	w2, #0x0                ; =0
100449cec: 940014af    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449cf0: f9408be4    	ldr	x4, [sp, #0x110]
100449cf4: f94083e3    	ldr	x3, [sp, #0x100]
100449cf8: 910923eb    	add	x11, sp, #0x248
100449cfc: b4000600    	cbz	x0, 0x100449dbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ac8>
100449d00: 39400008    	ldrb	w8, [x0]
100449d04: 71001d1f    	cmp	w8, #0x7
100449d08: 540005a8    	b.hi	0x100449dbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ac8>
100449d0c: 52800029    	mov	w9, #0x1                ; =1
100449d10: 1ac82129    	lsl	w9, w9, w8
100449d14: 5280138a    	mov	w10, #0x9c              ; =156
100449d18: 6a0a013f    	tst	w9, w10
100449d1c: 54000360    	b.eq	0x100449d88 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a94>
100449d20: f8401009    	ldur	x9, [x0, #0x1]
100449d24: f90173e9    	str	x9, [sp, #0x2e0]
100449d28: f9400409    	ldr	x9, [x0, #0x8]
100449d2c: f809f169    	stur	x9, [x11, #0x9f]
100449d30: 14000018    	b	0x100449d90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4a9c>
100449d34: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100449d38: f9408be1    	ldr	x1, [sp, #0x110]
100449d3c: 52800022    	mov	w2, #0x1                ; =1
100449d40: 9400149a    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449d44: f9408be4    	ldr	x4, [sp, #0x110]
100449d48: f94083e3    	ldr	x3, [sp, #0x100]
100449d4c: 910923eb    	add	x11, sp, #0x248
100449d50: b40007a0    	cbz	x0, 0x100449e44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b50>
100449d54: 39400008    	ldrb	w8, [x0]
100449d58: 71001d1f    	cmp	w8, #0x7
100449d5c: 54000748    	b.hi	0x100449e44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b50>
100449d60: 52800029    	mov	w9, #0x1                ; =1
100449d64: 1ac82129    	lsl	w9, w9, w8
100449d68: 5280138a    	mov	w10, #0x9c              ; =156
100449d6c: 6a0a013f    	tst	w9, w10
100449d70: 54000500    	b.eq	0x100449e10 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b1c>
100449d74: f8401009    	ldur	x9, [x0, #0x1]
100449d78: f90173e9    	str	x9, [sp, #0x2e0]
100449d7c: f9400409    	ldr	x9, [x0, #0x8]
100449d80: f809f169    	stur	x9, [x11, #0x9f]
100449d84: 14000025    	b	0x100449e18 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b24>
100449d88: 7200053f    	tst	w9, #0x3
100449d8c: 54000180    	b.eq	0x100449dbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ac8>
100449d90: 381403a8    	sturb	w8, [x29, #-0xc0]
100449d94: f94173e8    	ldr	x8, [sp, #0x2e0]
100449d98: f94073e9    	ldr	x9, [sp, #0xe0]
100449d9c: f9000128    	str	x8, [x9]
100449da0: f849f168    	ldur	x8, [x11, #0x9f]
100449da4: f8007128    	stur	x8, [x9, #0x7]
100449da8: d10303a2    	sub	x2, x29, #0xc0
100449dac: aa0303e0    	mov	x0, x3
100449db0: aa0403e1    	mov	x1, x4
100449db4: 940014a5    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449db8: 140000d4    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449dbc: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449dc0: f9400102    	ldr	x2, [x8]
100449dc4: d10303a0    	sub	x0, x29, #0xc0
100449dc8: 94001513    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100449dcc: 385503a8    	ldurb	w8, [x29, #-0xb0]
100449dd0: f85403a0    	ldur	x0, [x29, #-0xc0]
100449dd4: 7100091f    	cmp	w8, #0x2
100449dd8: 54008280    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449ddc: 92401c08    	and	x8, x0, #0xff
100449de0: f100291f    	cmp	x8, #0xa
100449de4: f9408be1    	ldr	x1, [sp, #0x110]
100449de8: f94083e9    	ldr	x9, [sp, #0x100]
100449dec: f9407bea    	ldr	x10, [sp, #0xf0]
100449df0: 54005be0    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
100449df4: f85483a8    	ldur	x8, [x29, #-0xb8]
100449df8: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100449dfc: f9400142    	ldr	x2, [x10]
100449e00: d10383a3    	sub	x3, x29, #0xe0
100449e04: aa0903e0    	mov	x0, x9
100449e08: 940014ae    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449e0c: 140000bf    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449e10: 7200053f    	tst	w9, #0x3
100449e14: 54000180    	b.eq	0x100449e44 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4b50>
100449e18: 381403a8    	sturb	w8, [x29, #-0xc0]
100449e1c: f94173e8    	ldr	x8, [sp, #0x2e0]
100449e20: f94073e9    	ldr	x9, [sp, #0xe0]
100449e24: f9000128    	str	x8, [x9]
100449e28: f849f168    	ldur	x8, [x11, #0x9f]
100449e2c: f8007128    	stur	x8, [x9, #0x7]
100449e30: d10303a2    	sub	x2, x29, #0xc0
100449e34: aa0303e0    	mov	x0, x3
100449e38: aa0403e1    	mov	x1, x4
100449e3c: 94001483    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449e40: 140000b2    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449e44: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449e48: f9400102    	ldr	x2, [x8]
100449e4c: d10303a0    	sub	x0, x29, #0xc0
100449e50: 94001588    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
100449e54: 385403a8    	ldurb	w8, [x29, #-0xc0]
100449e58: 71002d1f    	cmp	w8, #0xb
100449e5c: 54004ea0    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
100449e60: f94073ea    	ldr	x10, [sp, #0xe0]
100449e64: b9400149    	ldr	w9, [x10]
100449e68: b90303e9    	str	w9, [sp, #0x300]
100449e6c: b8403149    	ldur	w9, [x10, #0x3]
100449e70: 910923eb    	add	x11, sp, #0x248
100449e74: b80bb169    	stur	w9, [x11, #0xbb]
100449e78: 7100291f    	cmp	w8, #0xa
100449e7c: f9408be1    	ldr	x1, [sp, #0x110]
100449e80: f94083e0    	ldr	x0, [sp, #0x100]
100449e84: f9407bea    	ldr	x10, [sp, #0xf0]
100449e88: 54005a00    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
100449e8c: f85483a9    	ldur	x9, [x29, #-0xb8]
100449e90: 381203a8    	sturb	w8, [x29, #-0xe0]
100449e94: b94303e8    	ldr	w8, [sp, #0x300]
100449e98: f94063ec    	ldr	x12, [sp, #0xc0]
100449e9c: b9000188    	str	w8, [x12]
100449ea0: b84bb168    	ldur	w8, [x11, #0xbb]
100449ea4: b8003188    	stur	w8, [x12, #0x3]
100449ea8: f81283a9    	stur	x9, [x29, #-0xd8]
100449eac: f9400142    	ldr	x2, [x10]
100449eb0: d10383a3    	sub	x3, x29, #0xe0
100449eb4: 94001483    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449eb8: 14000094    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449ebc: 360017eb    	tbz	w11, #0x0, 0x10044a1b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ec4>
100449ec0: 71038b7f    	cmp	w27, #0xe2
100449ec4: 540002c1    	b.ne	0x100449f1c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c28>
100449ec8: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100449ecc: f9408be1    	ldr	x1, [sp, #0x110]
100449ed0: 52800002    	mov	w2, #0x0                ; =0
100449ed4: 94001435    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449ed8: f9408be4    	ldr	x4, [sp, #0x110]
100449edc: f94083e3    	ldr	x3, [sp, #0x100]
100449ee0: 910923eb    	add	x11, sp, #0x248
100449ee4: b4000600    	cbz	x0, 0x100449fa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4cb0>
100449ee8: 39400008    	ldrb	w8, [x0]
100449eec: 71001d1f    	cmp	w8, #0x7
100449ef0: 540005a8    	b.hi	0x100449fa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4cb0>
100449ef4: 52800029    	mov	w9, #0x1                ; =1
100449ef8: 1ac82129    	lsl	w9, w9, w8
100449efc: 5280138a    	mov	w10, #0x9c              ; =156
100449f00: 6a0a013f    	tst	w9, w10
100449f04: 54000360    	b.eq	0x100449f70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c7c>
100449f08: f8401009    	ldur	x9, [x0, #0x1]
100449f0c: f90173e9    	str	x9, [sp, #0x2e0]
100449f10: f9400409    	ldr	x9, [x0, #0x8]
100449f14: f809f169    	stur	x9, [x11, #0x9f]
100449f18: 14000018    	b	0x100449f78 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4c84>
100449f1c: a94f83e3    	ldp	x3, x0, [sp, #0xf8]
100449f20: f9408be1    	ldr	x1, [sp, #0x110]
100449f24: 52800022    	mov	w2, #0x1                ; =1
100449f28: 94001420    	bl	0x10044efa8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots12direct_value17h79cfba7e199a9a47E>
100449f2c: f9408be4    	ldr	x4, [sp, #0x110]
100449f30: f94083e3    	ldr	x3, [sp, #0x100]
100449f34: 910923eb    	add	x11, sp, #0x248
100449f38: b40007a0    	cbz	x0, 0x10044a02c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d38>
100449f3c: 39400008    	ldrb	w8, [x0]
100449f40: 71001d1f    	cmp	w8, #0x7
100449f44: 54000748    	b.hi	0x10044a02c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d38>
100449f48: 52800029    	mov	w9, #0x1                ; =1
100449f4c: 1ac82129    	lsl	w9, w9, w8
100449f50: 5280138a    	mov	w10, #0x9c              ; =156
100449f54: 6a0a013f    	tst	w9, w10
100449f58: 54000500    	b.eq	0x100449ff8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d04>
100449f5c: f8401009    	ldur	x9, [x0, #0x1]
100449f60: f90173e9    	str	x9, [sp, #0x2e0]
100449f64: f9400409    	ldr	x9, [x0, #0x8]
100449f68: f809f169    	stur	x9, [x11, #0x9f]
100449f6c: 14000025    	b	0x10044a000 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d0c>
100449f70: 7200053f    	tst	w9, #0x3
100449f74: 54000180    	b.eq	0x100449fa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4cb0>
100449f78: 381403a8    	sturb	w8, [x29, #-0xc0]
100449f7c: f94173e8    	ldr	x8, [sp, #0x2e0]
100449f80: f94073e9    	ldr	x9, [sp, #0xe0]
100449f84: f9000128    	str	x8, [x9]
100449f88: f849f168    	ldur	x8, [x11, #0x9f]
100449f8c: f8007128    	stur	x8, [x9, #0x7]
100449f90: d10303a2    	sub	x2, x29, #0xc0
100449f94: aa0303e0    	mov	x0, x3
100449f98: aa0403e1    	mov	x1, x4
100449f9c: 9400142b    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
100449fa0: 1400005a    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449fa4: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
100449fa8: f9400102    	ldr	x2, [x8]
100449fac: d10303a0    	sub	x0, x29, #0xc0
100449fb0: 94001499    	bl	0x10044f214 <__ZN13quickjs_oxide6engine2vm7execute10read_local28_$u7b$$u7b$closure$u7d$$u7d$17h184b4062faae193eE>
100449fb4: 385503a8    	ldurb	w8, [x29, #-0xb0]
100449fb8: f85403a0    	ldur	x0, [x29, #-0xc0]
100449fbc: 7100091f    	cmp	w8, #0x2
100449fc0: 54007340    	b.eq	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
100449fc4: 92401c08    	and	x8, x0, #0xff
100449fc8: f100291f    	cmp	x8, #0xa
100449fcc: f9408be1    	ldr	x1, [sp, #0x110]
100449fd0: f94083e9    	ldr	x9, [sp, #0x100]
100449fd4: f9407bea    	ldr	x10, [sp, #0xf0]
100449fd8: 54004ca0    	b.eq	0x10044a96c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5678>
100449fdc: f85483a8    	ldur	x8, [x29, #-0xb8]
100449fe0: a93223a0    	stp	x0, x8, [x29, #-0xe0]
100449fe4: f9400142    	ldr	x2, [x10]
100449fe8: d10383a3    	sub	x3, x29, #0xe0
100449fec: aa0903e0    	mov	x0, x9
100449ff0: 94001434    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
100449ff4: 14000045    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
100449ff8: 7200053f    	tst	w9, #0x3
100449ffc: 54000180    	b.eq	0x10044a02c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4d38>
10044a000: 381403a8    	sturb	w8, [x29, #-0xc0]
10044a004: f94173e8    	ldr	x8, [sp, #0x2e0]
10044a008: f94073e9    	ldr	x9, [sp, #0xe0]
10044a00c: f9000128    	str	x8, [x9]
10044a010: f849f168    	ldur	x8, [x11, #0x9f]
10044a014: f8007128    	stur	x8, [x9, #0x7]
10044a018: d10303a2    	sub	x2, x29, #0xc0
10044a01c: aa0303e0    	mov	x0, x3
10044a020: aa0403e1    	mov	x1, x4
10044a024: 94001409    	bl	0x10044f048 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4push17h20598c27e08905d8E>
10044a028: 14000038    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a02c: a94f07e8    	ldp	x8, x1, [sp, #0xf0]
10044a030: f9400102    	ldr	x2, [x8]
10044a034: d10303a0    	sub	x0, x29, #0xc0
10044a038: 9400150e    	bl	0x10044f470 <__ZN13quickjs_oxide6engine2vm7execute8read_arg28_$u7b$$u7b$closure$u7d$$u7d$17hef67312943b6e515E>
10044a03c: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044a040: 71002d1f    	cmp	w8, #0xb
10044a044: 54003f60    	b.eq	0x10044a830 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x553c>
10044a048: f94073ea    	ldr	x10, [sp, #0xe0]
10044a04c: b9400149    	ldr	w9, [x10]
10044a050: b90303e9    	str	w9, [sp, #0x300]
10044a054: b8403149    	ldur	w9, [x10, #0x3]
10044a058: 910923eb    	add	x11, sp, #0x248
10044a05c: b80bb169    	stur	w9, [x11, #0xbb]
10044a060: 7100291f    	cmp	w8, #0xa
10044a064: f9408be1    	ldr	x1, [sp, #0x110]
10044a068: f94083e0    	ldr	x0, [sp, #0x100]
10044a06c: f9407bea    	ldr	x10, [sp, #0xf0]
10044a070: 54004ac0    	b.eq	0x10044a9c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56d4>
10044a074: f85483a9    	ldur	x9, [x29, #-0xb8]
10044a078: 381203a8    	sturb	w8, [x29, #-0xe0]
10044a07c: b94303e8    	ldr	w8, [sp, #0x300]
10044a080: f94063ec    	ldr	x12, [sp, #0xc0]
10044a084: b9000188    	str	w8, [x12]
10044a088: b84bb168    	ldur	w8, [x11, #0xbb]
10044a08c: b8003188    	stur	w8, [x12, #0x3]
10044a090: f81283a9    	stur	x9, [x29, #-0xd8]
10044a094: f9400142    	ldr	x2, [x10]
10044a098: d10383a3    	sub	x3, x29, #0xe0
10044a09c: 94001409    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044a0a0: 1400001a    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a0a4: d10303a0    	sub	x0, x29, #0xc0
10044a0a8: 97ff74fc    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044a0ac: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044a0b0: 7100291f    	cmp	w8, #0xa
10044a0b4: f9408bec    	ldr	x12, [sp, #0x110]
10044a0b8: f94083e0    	ldr	x0, [sp, #0x100]
10044a0bc: f9407bed    	ldr	x13, [sp, #0xf0]
10044a0c0: 910923ee    	add	x14, sp, #0x248
10044a0c4: f94073ea    	ldr	x10, [sp, #0xe0]
10044a0c8: 54007420    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044a0cc: b9400149    	ldr	w9, [x10]
10044a0d0: b902e3e9    	str	w9, [sp, #0x2e0]
10044a0d4: b8403149    	ldur	w9, [x10, #0x3]
10044a0d8: b809b1c9    	stur	w9, [x14, #0x9b]
10044a0dc: f85483a9    	ldur	x9, [x29, #-0xb8]
10044a0e0: 381403a8    	sturb	w8, [x29, #-0xc0]
10044a0e4: b942e3e8    	ldr	w8, [sp, #0x2e0]
10044a0e8: b9000148    	str	w8, [x10]
10044a0ec: b849b1c8    	ldur	w8, [x14, #0x9b]
10044a0f0: b8003148    	stur	w8, [x10, #0x3]
10044a0f4: f81483a9    	stur	x9, [x29, #-0xb8]
10044a0f8: f94001a2    	ldr	x2, [x13]
10044a0fc: d10303a3    	sub	x3, x29, #0xc0
10044a100: aa0c03e1    	mov	x1, x12
10044a104: 940013ef    	bl	0x10044f0c0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor12commit_owned17he0752445d611756aE>
10044a108: b5006900    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a10c: f940a3f9    	ldr	x25, [sp, #0x140]
10044a110: f94087e8    	ldr	x8, [sp, #0x108]
10044a114: f9400108    	ldr	x8, [x8]
10044a118: f9402d1c    	ldr	x28, [x8, #0x58]
10044a11c: 2a1903e9    	mov	w9, w25
10044a120: eb09039f    	cmp	x28, x9
10044a124: 54fd9fa8    	b.hi	0x100445518 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x224>
10044a128: 14000162    	b	0x10044a6b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53bc>
10044a12c: 52800008    	mov	w8, #0x0                ; =0
10044a130: 1a8a0129    	csel	w9, w9, w10, eq
10044a134: b81443a9    	stur	w9, [x29, #-0xbc]
10044a138: b81403a8    	stur	w8, [x29, #-0xc0]
10044a13c: d10303a3    	sub	x3, x29, #0xc0
10044a140: aa0d03e1    	mov	x1, x13
10044a144: aa1303e2    	mov	x2, x19
10044a148: 94001ba8    	bl	0x100450fe8 <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots27commit_number_local_discard17hecbdd4c596cb35adE>
10044a14c: b50066e0    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a150: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a154: 7211011f    	tst	w8, #0x8000
10044a158: 52800048    	mov	w8, #0x2                ; =2
10044a15c: 1a880508    	cinc	w8, w8, ne
10044a160: 0b140119    	add	w25, w8, w20
10044a164: 17ffffeb    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
10044a168: bd400541    	ldr	s1, [x10, #0x4]
10044a16c: 0f20a421    	sshll.2d	v1, v1, #0x0
10044a170: 5e61d821    	scvtf	d1, d1
10044a174: 5311692a    	ubfx	w10, w9, #17, #10
10044a178: 7102415f    	cmp	w10, #0x90
10044a17c: 5400010c    	b.gt	0x10044a19c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ea8>
10044a180: 71023d5f    	cmp	w10, #0x8f
10044a184: 54000fe0    	b.eq	0x10044a380 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x508c>
10044a188: 7102415f    	cmp	w10, #0x90
10044a18c: 54000ee1    	b.ne	0x10044a368 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5074>
10044a190: 1e612000    	fcmp	d0, d1
10044a194: 1a9f87ea    	cset	w10, ls
10044a198: 1400010b    	b	0x10044a5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52d0>
10044a19c: 7102455f    	cmp	w10, #0x91
10044a1a0: 54000f60    	b.eq	0x10044a38c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5098>
10044a1a4: 7102495f    	cmp	w10, #0x92
10044a1a8: 54000e01    	b.ne	0x10044a368 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5074>
10044a1ac: 1e612000    	fcmp	d0, d1
10044a1b0: 1a9fb7ea    	cset	w10, ge
10044a1b4: 14000104    	b	0x10044a5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52d0>
10044a1b8: bd400521    	ldr	s1, [x9, #0x4]
10044a1bc: 0f20a421    	sshll.2d	v1, v1, #0x0
10044a1c0: 5e61d821    	scvtf	d1, d1
10044a1c4: 1e612000    	fcmp	d0, d1
10044a1c8: 1a9f57e9    	cset	w9, mi
10044a1cc: 7205019f    	tst	w12, #0x8000000
10044a1d0: 1a9f17ea    	cset	w10, eq
10044a1d4: 4a090149    	eor	w9, w10, w9
10044a1d8: 36001fe9    	tbz	w9, #0x0, 0x10044a5d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52e0>
10044a1dc: 52800029    	mov	w9, #0x1                ; =1
10044a1e0: 6a53713f    	tst	w9, w19, lsr #28
10044a1e4: 52800049    	mov	w9, #0x2                ; =2
10044a1e8: 9a890529    	cinc	x9, x9, ne
10044a1ec: 8b080136    	add	x22, x9, x8
10044a1f0: eb1c02df    	cmp	x22, x28
10044a1f4: 54ffcb63    	b.lo	0x100449b60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x486c>
10044a1f8: 1400059b    	b	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
10044a1fc: d10303a0    	sub	x0, x29, #0xc0
10044a200: aa1403e1    	mov	x1, x20
10044a204: 97ff74a5    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044a208: 385403bb    	ldurb	w27, [x29, #-0xc0]
10044a20c: 71002b7f    	cmp	w27, #0xa
10044a210: 910923e9    	add	x9, sp, #0x248
10044a214: f94073e8    	ldr	x8, [sp, #0xe0]
10044a218: 54ffcba1    	b.ne	0x100449b8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4898>
10044a21c: 140000f8    	b	0x10044a5fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5308>
10044a220: 7200051f    	tst	w8, #0x3
10044a224: 540013e0    	b.eq	0x10044a4a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x51ac>
10044a228: f85203a8    	ldur	x8, [x29, #-0xe0]
10044a22c: f94073ea    	ldr	x10, [sp, #0xe0]
10044a230: f9000148    	str	x8, [x10]
10044a234: 910923e9    	add	x9, sp, #0x248
10044a238: f84ff128    	ldur	x8, [x9, #0xff]
10044a23c: f8007148    	stur	x8, [x10, #0x7]
10044a240: aa0a03e8    	mov	x8, x10
10044a244: aa0803ea    	mov	x10, x8
10044a248: b9400108    	ldr	w8, [x8]
10044a24c: b902e3e8    	str	w8, [sp, #0x2e0]
10044a250: b8403148    	ldur	w8, [x10, #0x3]
10044a254: b809b128    	stur	w8, [x9, #0x9b]
10044a258: f85483b3    	ldur	x19, [x29, #-0xb8]
10044a25c: f9407fe0    	ldr	x0, [sp, #0xf8]
10044a260: aa1c03e1    	mov	x1, x28
10044a264: f9408be2    	ldr	x2, [sp, #0x110]
10044a268: 97f4cf2d    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044a26c: aa0003e8    	mov	x8, x0
10044a270: aa0103e0    	mov	x0, x1
10044a274: 36000068    	tbz	w8, #0x0, 0x10044a280 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f8c>
10044a278: b5fff480    	cbnz	x0, 0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a27c: 14000014    	b	0x10044a2cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4fd8>
10044a280: eb00039f    	cmp	x28, x0
10044a284: 5400bec9    	b.ls	0x10044ba5c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6768>
10044a288: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a28c: 8b001108    	add	x8, x8, x0, lsl #4
10044a290: 3900011b    	strb	w27, [x8]
10044a294: b942e3e9    	ldr	w9, [sp, #0x2e0]
10044a298: b8001109    	stur	w9, [x8, #0x1]
10044a29c: 910923e9    	add	x9, sp, #0x248
10044a2a0: b849b129    	ldur	w9, [x9, #0x9b]
10044a2a4: b9000509    	str	w9, [x8, #0x4]
10044a2a8: f9000513    	str	x19, [x8, #0x8]
10044a2ac: f9401fe9    	ldr	x9, [sp, #0x38]
10044a2b0: 91000529    	add	x9, x9, #0x1
10044a2b4: f9408be8    	ldr	x8, [sp, #0x110]
10044a2b8: f9002109    	str	x9, [x8, #0x40]
10044a2bc: f1000d3f    	cmp	x9, #0x3
10044a2c0: 54001a83    	b.lo	0x10044a610 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x531c>
10044a2c4: f9001fe9    	str	x9, [sp, #0x38]
10044a2c8: 8b180136    	add	x22, x9, x24
10044a2cc: eb1c02df    	cmp	x22, x28
10044a2d0: 5400ab22    	b.hs	0x10044b834 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6540>
10044a2d4: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a2d8: 8b161102    	add	x2, x8, x22, lsl #4
10044a2dc: 39400056    	ldrb	w22, [x2]
10044a2e0: 51002ac8    	sub	w8, w22, #0xa
10044a2e4: 7100151f    	cmp	w8, #0x5
10044a2e8: 54000262    	b.hs	0x10044a334 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5040>
10044a2ec: 940310a1    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044a2f0: 17ffff86    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a2f4: b85443a9    	ldur	w9, [x29, #-0xbc]
10044a2f8: fc5483a9    	ldur	d9, [x29, #-0xb8]
10044a2fc: 7200011f    	tst	w8, #0x1
10044a300: 52800068    	mov	w8, #0x3                ; =3
10044a304: 1a880508    	cinc	w8, w8, ne
10044a308: 290327e8    	stp	w8, w9, [sp, #0x18]
10044a30c: f94083e8    	ldr	x8, [sp, #0x100]
10044a310: a940851b    	ldp	x27, x1, [x8, #0x8]
10044a314: aa1b03e0    	mov	x0, x27
10044a318: f9007fe1    	str	x1, [sp, #0xf8]
10044a31c: f9408be2    	ldr	x2, [sp, #0x110]
10044a320: 97f4ceff    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044a324: aa0103f6    	mov	x22, x1
10044a328: 36000380    	tbz	w0, #0x0, 0x10044a398 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x50a4>
10044a32c: b40004f6    	cbz	x22, 0x10044a3c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x50d4>
10044a330: 14000489    	b	0x10044b554 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6260>
10044a334: 71001edf    	cmp	w22, #0x7
10044a338: 54001528    	b.hi	0x10044a5dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52e8>
10044a33c: 52800028    	mov	w8, #0x1                ; =1
10044a340: 1ad62108    	lsl	w8, w8, w22
10044a344: 52801389    	mov	w9, #0x9c               ; =156
10044a348: 6a09011f    	tst	w8, w9
10044a34c: 54000de0    	b.eq	0x10044a508 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5214>
10044a350: f8401048    	ldur	x8, [x2, #0x1]
10044a354: f81203a8    	stur	x8, [x29, #-0xe0]
10044a358: f9400448    	ldr	x8, [x2, #0x8]
10044a35c: 910923e9    	add	x9, sp, #0x248
10044a360: f80ff128    	stur	x8, [x9, #0xff]
10044a364: 1400006b    	b	0x10044a510 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x521c>
10044a368: 51022d4b    	sub	w11, w10, #0x8b
10044a36c: 7100057f    	cmp	w11, #0x1
10044a370: 54001208    	b.hi	0x10044a5b0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52bc>
10044a374: 1e612000    	fcmp	d0, d1
10044a378: 1a9f17ea    	cset	w10, eq
10044a37c: 14000092    	b	0x10044a5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52d0>
10044a380: 1e612000    	fcmp	d0, d1
10044a384: 1a9f57ea    	cset	w10, mi
10044a388: 1400008f    	b	0x10044a5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52d0>
10044a38c: 1e612000    	fcmp	d0, d1
10044a390: 1a9fd7ea    	cset	w10, gt
10044a394: 1400008c    	b	0x10044a5c4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52d0>
10044a398: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a39c: eb16011f    	cmp	x8, x22
10044a3a0: 5400b689    	b.ls	0x10044ba70 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x677c>
10044a3a4: 8b161368    	add	x8, x27, x22, lsl #4
10044a3a8: 294327ea    	ldp	w10, w9, [sp, #0x18]
10044a3ac: 3900010a    	strb	w10, [x8]
10044a3b0: b9000509    	str	w9, [x8, #0x4]
10044a3b4: fd000509    	str	d9, [x8, #0x8]
10044a3b8: f9408be9    	ldr	x9, [sp, #0x110]
10044a3bc: f9402128    	ldr	x8, [x9, #0x40]
10044a3c0: 91000508    	add	x8, x8, #0x1
10044a3c4: f9002128    	str	x8, [x9, #0x40]
10044a3c8: b9402be8    	ldr	w8, [sp, #0x28]
10044a3cc: 7100011f    	cmp	w8, #0x0
10044a3d0: 52800068    	mov	w8, #0x3                ; =3
10044a3d4: 1a880508    	cinc	w8, w8, ne
10044a3d8: f9401be9    	ldr	x9, [sp, #0x30]
10044a3dc: 378002e9    	tbnz	w9, #0x10, 0x10044a438 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5144>
10044a3e0: f9406fe9    	ldr	x9, [sp, #0xd8]
10044a3e4: f9400129    	ldr	x9, [x9]
10044a3e8: f94077ea    	ldr	x10, [sp, #0xe8]
10044a3ec: f940014a    	ldr	x10, [x10]
10044a3f0: eb09014a    	subs	x10, x10, x9
10044a3f4: 9a8a33ea    	csel	x10, xzr, x10, lo
10044a3f8: f9401feb    	ldr	x11, [sp, #0x38]
10044a3fc: eb0b015f    	cmp	x10, x11
10044a400: 54008fa9    	b.ls	0x10044b5f4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6300>
10044a404: f94083ea    	ldr	x10, [sp, #0x100]
10044a408: f940094a    	ldr	x10, [x10, #0x10]
10044a40c: f9401feb    	ldr	x11, [sp, #0x38]
10044a410: 8b0b0136    	add	x22, x9, x11
10044a414: eb0a02df    	cmp	x22, x10
10044a418: 5400b3c2    	b.hs	0x10044ba90 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x679c>
10044a41c: f94083e9    	ldr	x9, [sp, #0x100]
10044a420: f9400529    	ldr	x9, [x9, #0x8]
10044a424: 8b161129    	add	x9, x9, x22, lsl #4
10044a428: 3940012a    	ldrb	w10, [x9]
10044a42c: 7100395f    	cmp	w10, #0xe
10044a430: 540002e1    	b.ne	0x10044a48c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5198>
10044a434: 140004c1    	b	0x10044b738 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6444>
10044a438: f94067e9    	ldr	x9, [sp, #0xc8]
10044a43c: f9400129    	ldr	x9, [x9]
10044a440: f9406fea    	ldr	x10, [sp, #0xd8]
10044a444: f940014a    	ldr	x10, [x10]
10044a448: eb09014a    	subs	x10, x10, x9
10044a44c: 9a8a33ea    	csel	x10, xzr, x10, lo
10044a450: f9401feb    	ldr	x11, [sp, #0x38]
10044a454: eb0b015f    	cmp	x10, x11
10044a458: 54009049    	b.ls	0x10044b660 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x636c>
10044a45c: f94083ea    	ldr	x10, [sp, #0x100]
10044a460: f940094a    	ldr	x10, [x10, #0x10]
10044a464: f9401feb    	ldr	x11, [sp, #0x38]
10044a468: 8b0b0136    	add	x22, x9, x11
10044a46c: eb0a02df    	cmp	x22, x10
10044a470: 5400b0a2    	b.hs	0x10044ba84 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6790>
10044a474: f94083e9    	ldr	x9, [sp, #0x100]
10044a478: f9400529    	ldr	x9, [x9, #0x8]
10044a47c: 8b161129    	add	x9, x9, x22, lsl #4
10044a480: 3940012a    	ldrb	w10, [x9]
10044a484: 7100395f    	cmp	w10, #0xe
10044a488: 54009220    	b.eq	0x10044b6cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x63d8>
10044a48c: 39000128    	strb	w8, [x9]
10044a490: b94023e8    	ldr	w8, [sp, #0x20]
10044a494: b9000528    	str	w8, [x9, #0x4]
10044a498: fd000528    	str	d8, [x9, #0x8]
10044a49c: 14000013    	b	0x10044a4e8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x51f4>
10044a4a0: d10303a0    	sub	x0, x29, #0xc0
10044a4a4: aa1403e1    	mov	x1, x20
10044a4a8: 97ff73fc    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044a4ac: 385403bb    	ldurb	w27, [x29, #-0xc0]
10044a4b0: 71002b7f    	cmp	w27, #0xa
10044a4b4: 910923e9    	add	x9, sp, #0x248
10044a4b8: f94073e8    	ldr	x8, [sp, #0xe0]
10044a4bc: 54ffec41    	b.ne	0x10044a244 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4f50>
10044a4c0: 1400004f    	b	0x10044a5fc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5308>
10044a4c4: b85443a8    	ldur	w8, [x29, #-0xbc]
10044a4c8: b81443a8    	stur	w8, [x29, #-0xbc]
10044a4cc: 52800068    	mov	w8, #0x3                ; =3
10044a4d0: 381403a8    	sturb	w8, [x29, #-0xc0]
10044a4d4: d10303a2    	sub	x2, x29, #0xc0
10044a4d8: f94083e0    	ldr	x0, [sp, #0x100]
10044a4dc: f9408be1    	ldr	x1, [sp, #0x110]
10044a4e0: 94001294    	bl	0x10044ef30 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor11commit_push17h7bb7923a809ee564E>
10044a4e4: b5004a20    	cbnz	x0, 0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a4e8: 52800028    	mov	w8, #0x1                ; =1
10044a4ec: 6a53711f    	tst	w8, w19, lsr #28
10044a4f0: 52800048    	mov	w8, #0x2                ; =2
10044a4f4: 9a880508    	cinc	x8, x8, ne
10044a4f8: 8b140116    	add	x22, x8, x20
10044a4fc: eb1c02df    	cmp	x22, x28
10044a500: 54ffb303    	b.lo	0x100449b60 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x486c>
10044a504: 140004d8    	b	0x10044b864 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6570>
10044a508: 7200051f    	tst	w8, #0x3
10044a50c: 54000680    	b.eq	0x10044a5dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52e8>
10044a510: f85203a8    	ldur	x8, [x29, #-0xe0]
10044a514: f94073ea    	ldr	x10, [sp, #0xe0]
10044a518: f9000148    	str	x8, [x10]
10044a51c: 910923e9    	add	x9, sp, #0x248
10044a520: f84ff128    	ldur	x8, [x9, #0xff]
10044a524: f8007148    	stur	x8, [x10, #0x7]
10044a528: aa0a03e8    	mov	x8, x10
10044a52c: aa0803ea    	mov	x10, x8
10044a530: b9400108    	ldr	w8, [x8]
10044a534: b902e3e8    	str	w8, [sp, #0x2e0]
10044a538: b8403148    	ldur	w8, [x10, #0x3]
10044a53c: b809b128    	stur	w8, [x9, #0x9b]
10044a540: f85483b3    	ldur	x19, [x29, #-0xb8]
10044a544: f9407fe0    	ldr	x0, [sp, #0xf8]
10044a548: aa1c03e1    	mov	x1, x28
10044a54c: f9408be2    	ldr	x2, [sp, #0x110]
10044a550: 97f4ce73    	bl	0x10017df1c <__ZN13quickjs_oxide6engine2vm5stack9SlotStore18operand_push_index17h1b9ec4e7f23767c0E>
10044a554: aa0003e8    	mov	x8, x0
10044a558: aa0103e0    	mov	x0, x1
10044a55c: 36000068    	tbz	w8, #0x0, 0x10044a568 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5274>
10044a560: b5ffdd40    	cbnz	x0, 0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a564: 14000010    	b	0x10044a5a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x52b0>
10044a568: eb00039f    	cmp	x28, x0
10044a56c: 5400a789    	b.ls	0x10044ba5c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6768>
10044a570: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a574: 8b001108    	add	x8, x8, x0, lsl #4
10044a578: 39000116    	strb	w22, [x8]
10044a57c: b942e3e9    	ldr	w9, [sp, #0x2e0]
10044a580: b8001109    	stur	w9, [x8, #0x1]
10044a584: 910923e9    	add	x9, sp, #0x248
10044a588: b849b129    	ldur	w9, [x9, #0x9b]
10044a58c: b9000509    	str	w9, [x8, #0x4]
10044a590: f9000513    	str	x19, [x8, #0x8]
10044a594: f9401fe8    	ldr	x8, [sp, #0x38]
10044a598: 91000508    	add	x8, x8, #0x1
10044a59c: f9408be9    	ldr	x9, [sp, #0x110]
10044a5a0: f9002128    	str	x8, [x9, #0x40]
10044a5a4: d2800000    	mov	x0, #0x0                ; =0
10044a5a8: b4ffdb3f    	cbz	xzr, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
10044a5ac: 1400021f    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a5b0: 5102354a    	sub	w10, w10, #0x8d
10044a5b4: 7100055f    	cmp	w10, #0x1
10044a5b8: 54009848    	b.hi	0x10044b8c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65cc>
10044a5bc: 1e612000    	fcmp	d0, d1
10044a5c0: 1a9f07ea    	cset	w10, ne
10044a5c4: 7205013f    	tst	w9, #0x8000000
10044a5c8: 1a9f17e9    	cset	w9, eq
10044a5cc: 4a0a0129    	eor	w9, w9, w10
10044a5d0: 3707e069    	tbnz	w9, #0x0, 0x10044a1dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4ee8>
10044a5d4: 91001299    	add	x25, x20, #0x4
10044a5d8: 17fffece    	b	0x10044a110 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e1c>
10044a5dc: d10303a0    	sub	x0, x29, #0xc0
10044a5e0: aa1403e1    	mov	x1, x20
10044a5e4: 97ff73ad    	bl	0x100427498 <__ZN13quickjs_oxide6engine2vm5stack14copy_reference17h19a7a1fe7c6e12beE>
10044a5e8: 385403b6    	ldurb	w22, [x29, #-0xc0]
10044a5ec: 71002adf    	cmp	w22, #0xa
10044a5f0: 910923e9    	add	x9, sp, #0x248
10044a5f4: f94073e8    	ldr	x8, [sp, #0xe0]
10044a5f8: 54fff9a1    	b.ne	0x10044a52c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5238>
10044a5fc: f85483a0    	ldur	x0, [x29, #-0xb8]
10044a600: b4ffd860    	cbz	x0, 0x10044a10c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e18>
10044a604: 14000209    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a608: 9403100a    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044a60c: 17fffebf    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a610: 94031008    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044a614: 17fffebd    	b	0x10044a108 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x4e14>
10044a618: d0001394    	adrp	x20, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044a61c: 397eea9f    	ldrb	wzr, [x20, #0xfba]
10044a620: 52800620    	mov	w0, #0x31               ; =49
10044a624: 940325bb    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a628: b4008f60    	cbz	x0, 0x10044b814 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6520>
10044a62c: 52800f28    	mov	w8, #0x79               ; =121
10044a630: 3900c008    	strb	w8, [x0, #0x30]
10044a634: f0000ac8    	adrp	x8, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044a638: 91224d08    	add	x8, x8, #0x893
10044a63c: ad400500    	ldp	q0, q1, [x8]
10044a640: ad000400    	stp	q0, q1, [x0]
10044a644: 3dc00900    	ldr	q0, [x8, #0x20]
10044a648: 3d800800    	str	q0, [x0, #0x20]
10044a64c: 528000a8    	mov	w8, #0x5                ; =5
10044a650: 381883a8    	sturb	w8, [x29, #-0x78]
10044a654: 52800628    	mov	w8, #0x31               ; =49
10044a658: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044a65c: f81803bf    	stur	xzr, [x29, #-0x80]
10044a660: f81683a8    	stur	x8, [x29, #-0x98]
10044a664: f81403bf    	stur	xzr, [x29, #-0xc0]
10044a668: 397eea9f    	ldrb	wzr, [x20, #0xfba]
10044a66c: 52800a00    	mov	w0, #0x50               ; =80
10044a670: 940325a8    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a674: b40024c0    	cbz	x0, 0x10044ab0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5818>
10044a678: ad7b07a0    	ldp	q0, q1, [x29, #-0xa0]
10044a67c: ad010400    	stp	q0, q1, [x0, #0x20]
10044a680: 3cd803a0    	ldur	q0, [x29, #-0x80]
10044a684: 3d801000    	str	q0, [x0, #0x40]
10044a688: ad7a03a1    	ldp	q1, q0, [x29, #-0xc0]
10044a68c: ad000001    	stp	q1, q0, [x0]
10044a690: f90006a0    	str	x0, [x21, #0x8]
10044a694: 52800928    	mov	w8, #0x49               ; =73
10044a698: aa1903f4    	mov	x20, x25
10044a69c: aa1303f9    	mov	x25, x19
10044a6a0: 390002a8    	strb	w8, [x21]
10044a6a4: f9408fe8    	ldr	x8, [sp, #0x118]
10044a6a8: a902d119    	stp	x25, x20, [x8, #0x28]
10044a6ac: 17ffeb3a    	b	0x100445394 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0xa0>
10044a6b0: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044a6b4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a6b8: 52800460    	mov	w0, #0x23               ; =35
10044a6bc: 94032595    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a6c0: b4008c80    	cbz	x0, 0x10044b850 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x655c>
10044a6c4: 528d8c28    	mov	w8, #0x6c61             ; =27745
10044a6c8: 72ac8d28    	movk	w8, #0x6469, lsl #16
10044a6cc: b801f008    	stur	w8, [x0, #0x1f]
10044a6d0: f0000ac8    	adrp	x8, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044a6d4: 91231108    	add	x8, x8, #0x8c4
10044a6d8: ad400500    	ldp	q0, q1, [x8]
10044a6dc: ad000400    	stp	q0, q1, [x0]
10044a6e0: 528000a8    	mov	w8, #0x5                ; =5
10044a6e4: 381883a8    	sturb	w8, [x29, #-0x78]
10044a6e8: 52800468    	mov	w8, #0x23               ; =35
10044a6ec: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044a6f0: f81803bf    	stur	xzr, [x29, #-0x80]
10044a6f4: f81683a8    	stur	x8, [x29, #-0x98]
10044a6f8: f81403bf    	stur	xzr, [x29, #-0xc0]
10044a6fc: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a700: 52800a00    	mov	w0, #0x50               ; =80
10044a704: 94032583    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a708: b4005ec0    	cbz	x0, 0x10044b2e0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fec>
10044a70c: ad7b07a0    	ldp	q0, q1, [x29, #-0xa0]
10044a710: ad010400    	stp	q0, q1, [x0, #0x20]
10044a714: 3cd803a0    	ldur	q0, [x29, #-0x80]
10044a718: 3d801000    	str	q0, [x0, #0x40]
10044a71c: ad7a03a1    	ldp	q1, q0, [x29, #-0xc0]
10044a720: ad000001    	stp	q1, q0, [x0]
10044a724: f90006a0    	str	x0, [x21, #0x8]
10044a728: 52800928    	mov	w8, #0x49               ; =73
10044a72c: aa1903f4    	mov	x20, x25
10044a730: 17ffffdc    	b	0x10044a6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53ac>
10044a734: aa1903f3    	mov	x19, x25
10044a738: 94030f8e    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044a73c: 140001bb    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a740: 121e7b68    	and	w8, w27, #0xfffffffd
10044a744: 7102311f    	cmp	w8, #0x8c
10044a748: 54000161    	b.ne	0x10044a774 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5480>
10044a74c: 71023b7f    	cmp	w27, #0x8e
10044a750: 1a9f17e8    	cset	w8, eq
10044a754: 52800849    	mov	w9, #0x42               ; =66
10044a758: 1400031a    	b	0x10044b3c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60cc>
10044a75c: b0000ac0    	adrp	x0, 0x1005a3000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x34ce8>
10044a760: 910f6c00    	add	x0, x0, #0x3db
10044a764: 90001222    	adrp	x2, 0x10068e000 <dyld_stub_binder+0x10068e000>
10044a768: 912fc042    	add	x2, x2, #0xbf0
10044a76c: 52800481    	mov	w1, #0x24               ; =36
10044a770: 9402d8f8    	bl	0x100500b50 <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044a774: aa1b03e0    	mov	x0, x27
10044a778: 94001c8e    	bl	0x1004519b0 <__ZN13quickjs_oxide6engine2vm7numeric9operation11NumericKind10for_opcode17h93c0709190b2326dE>
10044a77c: 12001c08    	and	w8, w0, #0xff
10044a780: 7100651f    	cmp	w8, #0x19
10044a784: 540061c1    	b.ne	0x10044b3bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60c8>
10044a788: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044a78c: 91249421    	add	x1, x1, #0x925
10044a790: aa1903f3    	mov	x19, x25
10044a794: 528000a0    	mov	w0, #0x5                ; =5
10044a798: 528003e2    	mov	w2, #0x1f               ; =31
10044a79c: 97f4af35    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044a7a0: 140001a2    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a7a4: f9001713    	str	x19, [x24, #0x28]
10044a7a8: b940fbe9    	ldr	w9, [sp, #0xf8]
10044a7ac: 1400031d    	b	0x10044b420 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
10044a7b0: 528007c8    	mov	w8, #0x3e               ; =62
10044a7b4: 1400019f    	b	0x10044ae30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b3c>
10044a7b8: b0000a80    	adrp	x0, 0x10059b000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x2cce8>
10044a7bc: 91360400    	add	x0, x0, #0xd81
10044a7c0: 90001202    	adrp	x2, 0x10068a000 <dyld_stub_binder+0x10068a000>
10044a7c4: 91164042    	add	x2, x2, #0x590
10044a7c8: 52800341    	mov	w1, #0x1a               ; =26
10044a7cc: 9402d8e1    	bl	0x100500b50 <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044a7d0: b0000ac0    	adrp	x0, 0x1005a3000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x34ce8>
10044a7d4: 910f6c00    	add	x0, x0, #0x3db
10044a7d8: 90001222    	adrp	x2, 0x10068e000 <dyld_stub_binder+0x10068e000>
10044a7dc: 9134e042    	add	x2, x2, #0xd38
10044a7e0: 52800481    	mov	w1, #0x24               ; =36
10044a7e4: 9402d8db    	bl	0x100500b50 <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044a7e8: aa1903f3    	mov	x19, x25
10044a7ec: 94030f61    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044a7f0: 14000358    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044a7f4: 7101cb7f    	cmp	w27, #0x72
10044a7f8: 54000d01    	b.ne	0x10044a998 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x56a4>
10044a7fc: 52800628    	mov	w8, #0x31               ; =49
10044a800: 1400018c    	b	0x10044ae30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b3c>
10044a804: 52800096    	mov	w22, #0x4               ; =4
10044a808: 7102bf7f    	cmp	w27, #0xaf
10044a80c: 540016cc    	b.gt	0x10044aae4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x57f0>
10044a810: 7102bb7f    	cmp	w27, #0xae
10044a814: 54003680    	b.eq	0x10044aee4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bf0>
10044a818: 7102bf7f    	cmp	w27, #0xaf
10044a81c: f9408be1    	ldr	x1, [sp, #0x110]
10044a820: f94083e0    	ldr	x0, [sp, #0x100]
10044a824: 540036a1    	b.ne	0x10044aef8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c04>
10044a828: 52800036    	mov	w22, #0x1               ; =1
10044a82c: 140001b3    	b	0x10044aef8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c04>
10044a830: f85483a0    	ldur	x0, [x29, #-0xb8]
10044a834: 1400017d    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a838: 121f1b68    	and	w8, w27, #0xfe
10044a83c: 7102e11f    	cmp	w8, #0xb8
10044a840: 1a9f17e8    	cset	w8, eq
10044a844: 7102db7f    	cmp	w27, #0xb6
10044a848: 52801729    	mov	w9, #0xb9               ; =185
10044a84c: 7a491364    	ccmp	w27, w9, #0x4, ne
10044a850: 528003c9    	mov	w9, #0x1e               ; =30
10044a854: 390002a9    	strb	w9, [x21]
10044a858: f9407fe9    	ldr	x9, [sp, #0xf8]
10044a85c: 790006a9    	strh	w9, [x21, #0x2]
10044a860: 1a9f17e9    	cset	w9, eq
10044a864: 390012a8    	strb	w8, [x21, #0x4]
10044a868: 390016a9    	strb	w9, [x21, #0x5]
10044a86c: aa1903f4    	mov	x20, x25
10044a870: 17ffff8d    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044a874: 51002928    	sub	w8, w9, #0xa
10044a878: 7100151f    	cmp	w8, #0x5
10044a87c: 54005d02    	b.hs	0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
10044a880: 94030f3c    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044a884: 14000333    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044a888: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044a88c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a890: 52800454    	mov	w20, #0x22              ; =34
10044a894: 52800440    	mov	w0, #0x22               ; =34
10044a898: 9403251e    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a89c: b4008a00    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044a8a0: 528e6c88    	mov	w8, #0x7364             ; =29540
10044a8a4: 79004008    	strh	w8, [x0, #0x20]
10044a8a8: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044a8ac: 91076d08    	add	x8, x8, #0x1db
10044a8b0: ad400500    	ldp	q0, q1, [x8]
10044a8b4: ad000400    	stp	q0, q1, [x0]
10044a8b8: 528000a8    	mov	w8, #0x5                ; =5
10044a8bc: 381883a8    	sturb	w8, [x29, #-0x78]
10044a8c0: 52800448    	mov	w8, #0x22               ; =34
10044a8c4: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044a8c8: f81803bf    	stur	xzr, [x29, #-0x80]
10044a8cc: f81683a8    	stur	x8, [x29, #-0x98]
10044a8d0: f81403bf    	stur	xzr, [x29, #-0xc0]
10044a8d4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a8d8: 52800a00    	mov	w0, #0x50               ; =80
10044a8dc: 9403250d    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a8e0: b5002700    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044a8e4: 52800100    	mov	w0, #0x8                ; =8
10044a8e8: 52800a01    	mov	w1, #0x50               ; =80
10044a8ec: 9402d715    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044a8f0: 1400045a    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044a8f4: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044a8f8: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a8fc: 528002b4    	mov	w20, #0x15              ; =21
10044a900: 528002a0    	mov	w0, #0x15               ; =21
10044a904: 94032503    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a908: b40086a0    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044a90c: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044a910: 91071908    	add	x8, x8, #0x1c6
10044a914: 3dc00100    	ldr	q0, [x8]
10044a918: 3d800000    	str	q0, [x0]
10044a91c: f840d108    	ldur	x8, [x8, #0xd]
10044a920: f800d008    	stur	x8, [x0, #0xd]
10044a924: 528000a8    	mov	w8, #0x5                ; =5
10044a928: 381883a8    	sturb	w8, [x29, #-0x78]
10044a92c: 528002a8    	mov	w8, #0x15               ; =21
10044a930: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044a934: f81803bf    	stur	xzr, [x29, #-0x80]
10044a938: f81683a8    	stur	x8, [x29, #-0x98]
10044a93c: f81403bf    	stur	xzr, [x29, #-0xc0]
10044a940: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a944: 52800a00    	mov	w0, #0x50               ; =80
10044a948: 940324f2    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a94c: b50023a0    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044a950: 52800100    	mov	w0, #0x8                ; =8
10044a954: 52800a01    	mov	w1, #0x50               ; =80
10044a958: 9402d6fa    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044a95c: 1400043f    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044a960: aa1903f3    	mov	x19, x25
10044a964: 94030f03    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044a968: 140002fa    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044a96c: 52800488    	mov	w8, #0x24               ; =36
10044a970: 390002a8    	strb	w8, [x21]
10044a974: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a978: 790006a8    	strh	w8, [x21, #0x2]
10044a97c: 0f000420    	movi.2s	v0, #0x1
10044a980: 14000246    	b	0x10044b298 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fa4>
10044a984: 528003e8    	mov	w8, #0x1f               ; =31
10044a988: 390002a8    	strb	w8, [x21]
10044a98c: b90006bf    	str	wzr, [x21, #0x4]
10044a990: aa1903f4    	mov	x20, x25
10044a994: 17ffff44    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044a998: aa1b03e0    	mov	x0, x27
10044a99c: 94001c05    	bl	0x1004519b0 <__ZN13quickjs_oxide6engine2vm7numeric9operation11NumericKind10for_opcode17h93c0709190b2326dE>
10044a9a0: 12001c08    	and	w8, w0, #0xff
10044a9a4: 7100651f    	cmp	w8, #0x19
10044a9a8: 540050a1    	b.ne	0x10044b3bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60c8>
10044a9ac: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044a9b0: 91249421    	add	x1, x1, #0x925
10044a9b4: aa1903f3    	mov	x19, x25
10044a9b8: 528000a0    	mov	w0, #0x5                ; =5
10044a9bc: 528003e2    	mov	w2, #0x1f               ; =31
10044a9c0: 97f4aeac    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044a9c4: 14000119    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044a9c8: 52800488    	mov	w8, #0x24               ; =36
10044a9cc: 390002a8    	strb	w8, [x21]
10044a9d0: f9407fe8    	ldr	x8, [sp, #0xf8]
10044a9d4: 790006a8    	strh	w8, [x21, #0x2]
10044a9d8: 0f000440    	movi.2s	v0, #0x2
10044a9dc: 1400022f    	b	0x10044b298 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fa4>
10044a9e0: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044a9e4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044a9e8: 52800454    	mov	w20, #0x22              ; =34
10044a9ec: 52800440    	mov	w0, #0x22               ; =34
10044a9f0: 940324c8    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044a9f4: b4007f40    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044a9f8: 528e6c88    	mov	w8, #0x7364             ; =29540
10044a9fc: 79004008    	strh	w8, [x0, #0x20]
10044aa00: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044aa04: 91076d08    	add	x8, x8, #0x1db
10044aa08: ad400500    	ldp	q0, q1, [x8]
10044aa0c: ad000400    	stp	q0, q1, [x0]
10044aa10: 528000a8    	mov	w8, #0x5                ; =5
10044aa14: 381883a8    	sturb	w8, [x29, #-0x78]
10044aa18: 52800448    	mov	w8, #0x22               ; =34
10044aa1c: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044aa20: f81803bf    	stur	xzr, [x29, #-0x80]
10044aa24: f81683a8    	stur	x8, [x29, #-0x98]
10044aa28: f81403bf    	stur	xzr, [x29, #-0xc0]
10044aa2c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044aa30: 52800a00    	mov	w0, #0x50               ; =80
10044aa34: 940324b7    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044aa38: b5001c40    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044aa3c: 52800100    	mov	w0, #0x8                ; =8
10044aa40: 52800a01    	mov	w1, #0x50               ; =80
10044aa44: 9402d6bf    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044aa48: 14000404    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044aa4c: aa1903f3    	mov	x19, x25
10044aa50: 94030ec8    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044aa54: 140000f5    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044aa58: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044aa5c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044aa60: 528002b4    	mov	w20, #0x15              ; =21
10044aa64: 528002a0    	mov	w0, #0x15               ; =21
10044aa68: 940324aa    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044aa6c: b4007b80    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044aa70: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044aa74: 91071908    	add	x8, x8, #0x1c6
10044aa78: 3dc00100    	ldr	q0, [x8]
10044aa7c: 3d800000    	str	q0, [x0]
10044aa80: f840d108    	ldur	x8, [x8, #0xd]
10044aa84: f800d008    	stur	x8, [x0, #0xd]
10044aa88: 528000a8    	mov	w8, #0x5                ; =5
10044aa8c: 381883a8    	sturb	w8, [x29, #-0x78]
10044aa90: 528002a8    	mov	w8, #0x15               ; =21
10044aa94: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044aa98: f81803bf    	stur	xzr, [x29, #-0x80]
10044aa9c: f81683a8    	stur	x8, [x29, #-0x98]
10044aaa0: f81403bf    	stur	xzr, [x29, #-0xc0]
10044aaa4: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044aaa8: 52800a00    	mov	w0, #0x50               ; =80
10044aaac: 94032499    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044aab0: b5001880    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044aab4: 52800100    	mov	w0, #0x8                ; =8
10044aab8: 52800a01    	mov	w1, #0x50               ; =80
10044aabc: 9402d6a1    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044aac0: 140003e6    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044aac4: 90000ae9    	adrp	x9, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044aac8: 911b5d29    	add	x9, x9, #0x6d7
10044aacc: 528006ca    	mov	w10, #0x36              ; =54
10044aad0: 140000cf    	b	0x10044ae0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b18>
10044aad4: 90000ae9    	adrp	x9, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044aad8: 911a9929    	add	x9, x9, #0x6a6
10044aadc: 5280062a    	mov	w10, #0x31              ; =49
10044aae0: 140000cb    	b	0x10044ae0c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b18>
10044aae4: 7102c37f    	cmp	w27, #0xb0
10044aae8: 54002020    	b.eq	0x10044aeec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5bf8>
10044aaec: 7102c77f    	cmp	w27, #0xb1
10044aaf0: f9408be1    	ldr	x1, [sp, #0x110]
10044aaf4: f94083e0    	ldr	x0, [sp, #0x100]
10044aaf8: 54002001    	b.ne	0x10044aef8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c04>
10044aafc: 52800076    	mov	w22, #0x3               ; =3
10044ab00: 140000fe    	b	0x10044aef8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c04>
10044ab04: f85283a8    	ldur	x8, [x29, #-0xd8]
10044ab08: 14000112    	b	0x10044af50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c5c>
10044ab0c: 52800100    	mov	w0, #0x8                ; =8
10044ab10: 52800a01    	mov	w1, #0x50               ; =80
10044ab14: 9402d68b    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ab18: 140003d0    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044ab1c: f85283b4    	ldur	x20, [x29, #-0xd8]
10044ab20: 1400018d    	b	0x10044b154 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e60>
10044ab24: aa1903f3    	mov	x19, x25
10044ab28: 94030e92    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044ab2c: 140000bf    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044ab30: 52800348    	mov	w8, #0x1a               ; =26
10044ab34: 390002a8    	strb	w8, [x21]
10044ab38: 390012bf    	strb	wzr, [x21, #0x4]
10044ab3c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044ab40: b9000aa8    	str	w8, [x21, #0x8]
10044ab44: aa1903f4    	mov	x20, x25
10044ab48: 17fffed7    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044ab4c: f90006a9    	str	x9, [x21, #0x8]
10044ab50: 140000b7    	b	0x10044ae2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b38>
10044ab54: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044ab58: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ab5c: 528004d4    	mov	w20, #0x26              ; =38
10044ab60: 528004c0    	mov	w0, #0x26               ; =38
10044ab64: 9403246b    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044ab68: b40073a0    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044ab6c: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044ab70: 91099d08    	add	x8, x8, #0x267
10044ab74: ad400500    	ldp	q0, q1, [x8]
10044ab78: ad000400    	stp	q0, q1, [x0]
10044ab7c: f841e108    	ldur	x8, [x8, #0x1e]
10044ab80: f801e008    	stur	x8, [x0, #0x1e]
10044ab84: 528000a8    	mov	w8, #0x5                ; =5
10044ab88: 381883a8    	sturb	w8, [x29, #-0x78]
10044ab8c: 528004c8    	mov	w8, #0x26               ; =38
10044ab90: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044ab94: f81803bf    	stur	xzr, [x29, #-0x80]
10044ab98: f81683a8    	stur	x8, [x29, #-0x98]
10044ab9c: f81403bf    	stur	xzr, [x29, #-0xc0]
10044aba0: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044aba4: 52800a00    	mov	w0, #0x50               ; =80
10044aba8: 9403245a    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044abac: b50010a0    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044abb0: 52800100    	mov	w0, #0x8                ; =8
10044abb4: 52800a01    	mov	w1, #0x50               ; =80
10044abb8: 9402d662    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044abbc: 140003a7    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044abc0: aa1903f3    	mov	x19, x25
10044abc4: 94030e6b    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044abc8: 14000098    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044abcc: aa1903f3    	mov	x19, x25
10044abd0: 94030e68    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044abd4: 14000095    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044abd8: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044abdc: 9126d021    	add	x1, x1, #0x9b4
10044abe0: aa1903f3    	mov	x19, x25
10044abe4: 528000a0    	mov	w0, #0x5                ; =5
10044abe8: 52800462    	mov	w2, #0x23               ; =35
10044abec: 97f4ae21    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044abf0: 1400008e    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044abf4: 94030e5f    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044abf8: f90006a0    	str	x0, [x21, #0x8]
10044abfc: 52800928    	mov	w8, #0x49               ; =73
10044ac00: 390002a8    	strb	w8, [x21]
10044ac04: 140000f3    	b	0x10044afd0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5cdc>
10044ac08: 7100291f    	cmp	w8, #0xa
10044ac0c: 54001a00    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044ac10: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044ac14: 91257c21    	add	x1, x1, #0x95f
10044ac18: aa1903f3    	mov	x19, x25
10044ac1c: 528000a0    	mov	w0, #0x5                ; =5
10044ac20: 52800222    	mov	w2, #0x11               ; =17
10044ac24: 97f4ae13    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044ac28: 14000080    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044ac2c: 7100291f    	cmp	w8, #0xa
10044ac30: 540018e0    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044ac34: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044ac38: 91266421    	add	x1, x1, #0x999
10044ac3c: aa1903f3    	mov	x19, x25
10044ac40: 528000a0    	mov	w0, #0x5                ; =5
10044ac44: 52800362    	mov	w2, #0x1b               ; =27
10044ac48: 97f4ae0a    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044ac4c: 14000077    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044ac50: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044ac54: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ac58: 52800334    	mov	w20, #0x19              ; =25
10044ac5c: 52800320    	mov	w0, #0x19               ; =25
10044ac60: 9403242c    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044ac64: b4006bc0    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044ac68: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044ac6c: 91088108    	add	x8, x8, #0x220
10044ac70: 3dc00100    	ldr	q0, [x8]
10044ac74: 3d800000    	str	q0, [x0]
10044ac78: 3cc09100    	ldur	q0, [x8, #0x9]
10044ac7c: 3c809000    	stur	q0, [x0, #0x9]
10044ac80: 528000a8    	mov	w8, #0x5                ; =5
10044ac84: 381883a8    	sturb	w8, [x29, #-0x78]
10044ac88: 52800328    	mov	w8, #0x19               ; =25
10044ac8c: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044ac90: f81803bf    	stur	xzr, [x29, #-0x80]
10044ac94: f81683a8    	stur	x8, [x29, #-0x98]
10044ac98: f81403bf    	stur	xzr, [x29, #-0xc0]
10044ac9c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044aca0: 52800a00    	mov	w0, #0x50               ; =80
10044aca4: 9403241b    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044aca8: b50008c0    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044acac: 52800100    	mov	w0, #0x8                ; =8
10044acb0: 52800a01    	mov	w1, #0x50               ; =80
10044acb4: 9402d623    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044acb8: 14000368    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044acbc: b9403be9    	ldr	w9, [sp, #0x38]
10044acc0: 140000bf    	b	0x10044afbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5cc8>
10044acc4: 7100191f    	cmp	w8, #0x6
10044acc8: 54003aa1    	b.ne	0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
10044accc: f85483b4    	ldur	x20, [x29, #-0xb8]
10044acd0: f9401688    	ldr	x8, [x20, #0x28]
10044acd4: b4000068    	cbz	x8, 0x10044ace0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59ec>
10044acd8: f9401a80    	ldr	x0, [x20, #0x30]
10044acdc: 940323f2    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044ace0: f9402280    	ldr	x0, [x20, #0x40]
10044ace4: b4000040    	cbz	x0, 0x10044acec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x59f8>
10044ace8: 940323ef    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044acec: aa1403e0    	mov	x0, x20
10044acf0: 940323ed    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044acf4: 140001ca    	b	0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
10044acf8: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044acfc: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ad00: 528004d4    	mov	w20, #0x26              ; =38
10044ad04: 528004c0    	mov	w0, #0x26               ; =38
10044ad08: 94032402    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044ad0c: b4006680    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044ad10: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044ad14: 91099d08    	add	x8, x8, #0x267
10044ad18: ad400500    	ldp	q0, q1, [x8]
10044ad1c: ad000400    	stp	q0, q1, [x0]
10044ad20: f841e108    	ldur	x8, [x8, #0x1e]
10044ad24: f801e008    	stur	x8, [x0, #0x1e]
10044ad28: 528000a8    	mov	w8, #0x5                ; =5
10044ad2c: 381883a8    	sturb	w8, [x29, #-0x78]
10044ad30: 528004c8    	mov	w8, #0x26               ; =38
10044ad34: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044ad38: f81803bf    	stur	xzr, [x29, #-0x80]
10044ad3c: f81683a8    	stur	x8, [x29, #-0x98]
10044ad40: f81403bf    	stur	xzr, [x29, #-0xc0]
10044ad44: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ad48: 52800a00    	mov	w0, #0x50               ; =80
10044ad4c: 940323f1    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044ad50: b5000380    	cbnz	x0, 0x10044adc0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5acc>
10044ad54: 52800100    	mov	w0, #0x8                ; =8
10044ad58: 52800a01    	mov	w1, #0x50               ; =80
10044ad5c: 9402d5f9    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ad60: 1400033e    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044ad64: d0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044ad68: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044ad6c: 52800334    	mov	w20, #0x19              ; =25
10044ad70: 52800320    	mov	w0, #0x19               ; =25
10044ad74: 940323e7    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044ad78: b4006320    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044ad7c: 90000ae8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044ad80: 91088108    	add	x8, x8, #0x220
10044ad84: 3dc00100    	ldr	q0, [x8]
10044ad88: 3d800000    	str	q0, [x0]
10044ad8c: 3cc09100    	ldur	q0, [x8, #0x9]
10044ad90: 3c809000    	stur	q0, [x0, #0x9]
10044ad94: 528000a8    	mov	w8, #0x5                ; =5
10044ad98: 381883a8    	sturb	w8, [x29, #-0x78]
10044ad9c: 52800328    	mov	w8, #0x19               ; =25
10044ada0: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044ada4: f81803bf    	stur	xzr, [x29, #-0x80]
10044ada8: f81683a8    	stur	x8, [x29, #-0x98]
10044adac: f81403bf    	stur	xzr, [x29, #-0xc0]
10044adb0: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044adb4: 52800a00    	mov	w0, #0x50               ; =80
10044adb8: 940323d6    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044adbc: b4005420    	cbz	x0, 0x10044b840 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x654c>
10044adc0: ad7b07a0    	ldp	q0, q1, [x29, #-0xa0]
10044adc4: ad010400    	stp	q0, q1, [x0, #0x20]
10044adc8: 3cd803a0    	ldur	q0, [x29, #-0x80]
10044adcc: 3d801000    	str	q0, [x0, #0x40]
10044add0: ad7a03a1    	ldp	q1, q0, [x29, #-0xc0]
10044add4: ad000001    	stp	q1, q0, [x0]
10044add8: 14000014    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044addc: 7200011f    	tst	w8, #0x1
10044ade0: 52800468    	mov	w8, #0x23               ; =35
10044ade4: 1a881508    	cinc	w8, w8, eq
10044ade8: 390002a8    	strb	w8, [x21]
10044adec: f9407fe8    	ldr	x8, [sp, #0xf8]
10044adf0: 790006a8    	strh	w8, [x21, #0x2]
10044adf4: 14000128    	b	0x10044b294 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fa0>
10044adf8: 94030dde    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044adfc: 140001d5    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044ae00: 90000ae9    	adrp	x9, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044ae04: 9119e129    	add	x9, x9, #0x678
10044ae08: 528005ca    	mov	w10, #0x2e              ; =46
10044ae0c: f900151f    	str	xzr, [x8, #0x28]
10044ae10: 52800088    	mov	w8, #0x4                ; =4
10044ae14: 381403a8    	sturb	w8, [x29, #-0xc0]
10044ae18: a934aba9    	stp	x9, x10, [x29, #-0xb8]
10044ae1c: d10303a0    	sub	x0, x29, #0xc0
10044ae20: aa1903f3    	mov	x19, x25
10044ae24: 97f45e48    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044ae28: f90006a0    	str	x0, [x21, #0x8]
10044ae2c: 52800928    	mov	w8, #0x49               ; =73
10044ae30: 390002a8    	strb	w8, [x21]
10044ae34: aa1903f4    	mov	x20, x25
10044ae38: 17fffe1b    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044ae3c: 94030dcd    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044ae40: 140001c4    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044ae44: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044ae48: 9123fc21    	add	x1, x1, #0x8ff
10044ae4c: aa1903f3    	mov	x19, x25
10044ae50: 528000a0    	mov	w0, #0x5                ; =5
10044ae54: 528004c2    	mov	w2, #0x26               ; =38
10044ae58: 97f4ad86    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044ae5c: 17fffff3    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044ae60: aa1903f3    	mov	x19, x25
10044ae64: 94030dc3    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044ae68: 17fffff0    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044ae6c: 528007a8    	mov	w8, #0x3d               ; =61
10044ae70: 17fffff0    	b	0x10044ae30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b3c>
10044ae74: d10303a0    	sub	x0, x29, #0xc0
10044ae78: aa1903f3    	mov	x19, x25
10044ae7c: f94083e1    	ldr	x1, [sp, #0x100]
10044ae80: f9408be2    	ldr	x2, [sp, #0x110]
10044ae84: 94001697    	bl	0x1004508e0 <__ZN13quickjs_oxide6engine2vm7execute11FrameCursor10move_owned17h023bc52615fde5ebE>
10044ae88: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044ae8c: 7100291f    	cmp	w8, #0xa
10044ae90: 540005e0    	b.eq	0x10044af4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c58>
10044ae94: b85443a9    	ldur	w9, [x29, #-0xbc]
10044ae98: 910923eb    	add	x11, sp, #0x248
10044ae9c: b80fb169    	stur	w9, [x11, #0xfb]
10044aea0: d10303a9    	sub	x9, x29, #0xc0
10044aea4: b8401129    	ldur	w9, [x9, #0x1]
10044aea8: b81203a9    	stur	w9, [x29, #-0xe0]
10044aeac: f85483a9    	ldur	x9, [x29, #-0xb8]
10044aeb0: 390502e8    	strb	w8, [x23, #0x140]
10044aeb4: 910506e8    	add	x8, x23, #0x141
10044aeb8: b85203aa    	ldur	w10, [x29, #-0xe0]
10044aebc: b900010a    	str	w10, [x8]
10044aec0: b84fb168    	ldur	w8, [x11, #0xfb]
10044aec4: b90146e8    	str	w8, [x23, #0x144]
10044aec8: f900a6e9    	str	x9, [x23, #0x148]
10044aecc: f940a3f4    	ldr	x20, [sp, #0x140]
10044aed0: 528008c8    	mov	w8, #0x46               ; =70
10044aed4: 17fffdf3    	b	0x10044a6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53ac>
10044aed8: 390502ff    	strb	wzr, [x23, #0x140]
10044aedc: 528008c8    	mov	w8, #0x46               ; =70
10044aee0: 17fffdf0    	b	0x10044a6a0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53ac>
10044aee4: 52800016    	mov	w22, #0x0               ; =0
10044aee8: 1400000b    	b	0x10044af14 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c20>
10044aeec: 52800056    	mov	w22, #0x2               ; =2
10044aef0: f9408be1    	ldr	x1, [sp, #0x110]
10044aef4: f94083e0    	ldr	x0, [sp, #0x100]
10044aef8: aa1903f3    	mov	x19, x25
10044aefc: d2800002    	mov	x2, #0x0                ; =0
10044af00: 97ff87af    	bl	0x10042cdbc <__ZN13quickjs_oxide6engine2vm5stack6window10FrameSlots4peek17h1919a76aa29b9f5bE>
10044af04: 36000060    	tbz	w0, #0x0, 0x10044af10 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c1c>
10044af08: f90006a1    	str	x1, [x21, #0x8]
10044af0c: 17ffffc8    	b	0x10044ae2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b38>
10044af10: f940a3f4    	ldr	x20, [sp, #0x140]
10044af14: 528008e8    	mov	w8, #0x47               ; =71
10044af18: 390002a8    	strb	w8, [x21]
10044af1c: 390006b6    	strb	w22, [x21, #0x1]
10044af20: 17fffde1    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044af24: f94087e8    	ldr	x8, [sp, #0x108]
10044af28: f9400108    	ldr	x8, [x8]
10044af2c: 39442902    	ldrb	w2, [x8, #0x10a]
10044af30: d10303a0    	sub	x0, x29, #0xc0
10044af34: 910483e1    	add	x1, sp, #0x120
10044af38: aa1903f3    	mov	x19, x25
10044af3c: 94000af7    	bl	0x10044db18 <__ZN13quickjs_oxide6engine2vm7execute15deferred_action17h2233d5b0a1374355E>
10044af40: 385403a8    	ldurb	w8, [x29, #-0xc0]
10044af44: 7101291f    	cmp	w8, #0x4a
10044af48: 54001221    	b.ne	0x10044b18c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e98>
10044af4c: f85483a8    	ldur	x8, [x29, #-0xb8]
10044af50: f90006a8    	str	x8, [x21, #0x8]
10044af54: 17ffffb6    	b	0x10044ae2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b38>
10044af58: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044af5c: 91251021    	add	x1, x1, #0x944
10044af60: aa1903f3    	mov	x19, x25
10044af64: 528000a0    	mov	w0, #0x5                ; =5
10044af68: 52800362    	mov	w2, #0x1b               ; =27
10044af6c: 97f4ad41    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044af70: 17ffffae    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044af74: f0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044af78: 91275c21    	add	x1, x1, #0x9d7
10044af7c: aa1903f3    	mov	x19, x25
10044af80: 528000a0    	mov	w0, #0x5                ; =5
10044af84: 52800382    	mov	w2, #0x1c               ; =28
10044af88: 97f4ad3a    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044af8c: 17ffffa7    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044af90: b4000140    	cbz	x0, 0x10044afb8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5cc4>
10044af94: f9401688    	ldr	x8, [x20, #0x28]
10044af98: b4000068    	cbz	x8, 0x10044afa4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5cb0>
10044af9c: f9401a80    	ldr	x0, [x20, #0x30]
10044afa0: 94032341    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044afa4: f9402280    	ldr	x0, [x20, #0x40]
10044afa8: b4000040    	cbz	x0, 0x10044afb0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5cbc>
10044afac: 9403233e    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044afb0: aa1403e0    	mov	x0, x20
10044afb4: 9403233c    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044afb8: 52800029    	mov	w9, #0x1                ; =1
10044afbc: 52800408    	mov	w8, #0x20               ; =32
10044afc0: 390002a8    	strb	w8, [x21]
10044afc4: f9407fe8    	ldr	x8, [sp, #0xf8]
10044afc8: b90006a8    	str	w8, [x21, #0x4]
10044afcc: 390022a9    	strb	w9, [x21, #0x8]
10044afd0: b85503a8    	ldur	w8, [x29, #-0xb0]
10044afd4: 7100091f    	cmp	w8, #0x2
10044afd8: 54000100    	b.eq	0x10044aff8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d04>
10044afdc: f85403a0    	ldur	x0, [x29, #-0xc0]
10044afe0: f9400008    	ldr	x8, [x0]
10044afe4: f1000508    	subs	x8, x8, #0x1
10044afe8: f9000008    	str	x8, [x0]
10044afec: 54000061    	b.ne	0x10044aff8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5d04>
10044aff0: aa1903f3    	mov	x19, x25
10044aff4: 97efe74a    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044aff8: aa1903f4    	mov	x20, x25
10044affc: 17fffdaa    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b000: 52800228    	mov	w8, #0x11               ; =17
10044b004: 14000074    	b	0x10044b1d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ee0>
10044b008: 52800348    	mov	w8, #0x1a               ; =26
10044b00c: 390002a8    	strb	w8, [x21]
10044b010: 52800088    	mov	w8, #0x4                ; =4
10044b014: 390012a8    	strb	w8, [x21, #0x4]
10044b018: aa1903f4    	mov	x20, x25
10044b01c: 17fffda2    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b020: 3dc0bbe0    	ldr	q0, [sp, #0x2e0]
10044b024: 3c9203a0    	stur	q0, [x29, #-0xe0]
10044b028: f9417be8    	ldr	x8, [sp, #0x2f0]
10044b02c: f81303a8    	stur	x8, [x29, #-0xd0]
10044b030: d10303a0    	sub	x0, x29, #0xc0
10044b034: d10383a1    	sub	x1, x29, #0xe0
10044b038: aa1903f3    	mov	x19, x25
10044b03c: 97fa025f    	bl	0x1002cb9b8 <__ZN49_$LT$T$u20$as$u20$alloc..string..SpecToString$GT$14spec_to_string17h32513371fca93ff9E>
10044b040: d10303a1    	sub	x1, x29, #0xc0
10044b044: aa1903f3    	mov	x19, x25
10044b048: 528000a0    	mov	w0, #0x5                ; =5
10044b04c: 97fa0289    	bl	0x1002cba70 <__ZN13quickjs_oxide6engine3api5error5Error3new17h4cec04b22d647e9eE>
10044b050: 17ffff76    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b054: 52800708    	mov	w8, #0x38               ; =56
10044b058: 1400005f    	b	0x10044b1d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ee0>
10044b05c: 721f197f    	tst	w11, #0xfe
10044b060: 1a9f17e8    	cset	w8, eq
10044b064: 71009b7f    	cmp	w27, #0x26
10044b068: 52800589    	mov	w9, #0x2c               ; =44
10044b06c: 5280048a    	mov	w10, #0x24              ; =36
10044b070: 390002aa    	strb	w10, [x21]
10044b074: 7a491364    	ccmp	w27, w9, #0x4, ne
10044b078: f9407fe9    	ldr	x9, [sp, #0xf8]
10044b07c: 790006a9    	strh	w9, [x21, #0x2]
10044b080: 52802029    	mov	w9, #0x101              ; =257
10044b084: 79000aa9    	strh	w9, [x21, #0x4]
10044b088: 1a9f17e9    	cset	w9, eq
10044b08c: 39001aa8    	strb	w8, [x21, #0x6]
10044b090: 39001ea9    	strb	w9, [x21, #0x7]
10044b094: aa1903f4    	mov	x20, x25
10044b098: 17fffd83    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b09c: 52800468    	mov	w8, #0x23               ; =35
10044b0a0: 1400004d    	b	0x10044b1d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5ee0>
10044b0a4: d0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044b0a8: 91257c21    	add	x1, x1, #0x95f
10044b0ac: aa1903f3    	mov	x19, x25
10044b0b0: 528000a0    	mov	w0, #0x5                ; =5
10044b0b4: 52800222    	mov	w2, #0x11               ; =17
10044b0b8: 97f4acee    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044b0bc: 17ffff5b    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b0c0: 52800488    	mov	w8, #0x24               ; =36
10044b0c4: 390002a8    	strb	w8, [x21]
10044b0c8: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b0cc: 790006a8    	strh	w8, [x21, #0x2]
10044b0d0: b0000d28    	adrp	x8, 0x1005f0000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x81ce8>
10044b0d4: fd46b500    	ldr	d0, [x8, #0xd68]
10044b0d8: 14000070    	b	0x10044b298 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fa4>
10044b0dc: 910b83e0    	add	x0, sp, #0x2e0
10044b0e0: 14000009    	b	0x10044b104 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e10>
10044b0e4: d0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044b0e8: 9125c021    	add	x1, x1, #0x970
10044b0ec: aa1903f3    	mov	x19, x25
10044b0f0: 528000a0    	mov	w0, #0x5                ; =5
10044b0f4: 52800522    	mov	w2, #0x29               ; =41
10044b0f8: 97f4acde    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044b0fc: 17ffff4b    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b100: d10383a0    	sub	x0, x29, #0xe0
10044b104: 97f45d90    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044b108: 14000112    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044b10c: aa1b03f4    	mov	x20, x27
10044b110: 14000011    	b	0x10044b154 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e60>
10044b114: 9107e3e0    	add	x0, sp, #0x1f8
10044b118: aa1903f3    	mov	x19, x25
10044b11c: 97f45d8a    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044b120: 17ffff42    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b124: aa1903f3    	mov	x19, x25
10044b128: 94030d12    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044b12c: 17ffff3f    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b130: aa1903f3    	mov	x19, x25
10044b134: 94030d0f    	bl	0x10050e570 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore24operand_slot_not_a_value17hd23a2e9532afc3b5E>
10044b138: 17ffff3c    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b13c: 9109c3e0    	add	x0, sp, #0x270
10044b140: 14000002    	b	0x10044b148 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5e54>
10044b144: 910a43e0    	add	x0, sp, #0x290
10044b148: aa1903f3    	mov	x19, x25
10044b14c: 97f45d7e    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044b150: aa0003f4    	mov	x20, x0
10044b154: f90006b4    	str	x20, [x21, #0x8]
10044b158: 17ffff35    	b	0x10044ae2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b38>
10044b15c: b5000be8    	cbnz	x8, 0x10044b2d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe4>
10044b160: 910c03e0    	add	x0, sp, #0x300
10044b164: 97ef9979    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044b168: 1400005c    	b	0x10044b2d8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe4>
10044b16c: 52800488    	mov	w8, #0x24               ; =36
10044b170: 390002a8    	strb	w8, [x21]
10044b174: 790006b4    	strh	w20, [x21, #0x2]
10044b178: 17fffe01    	b	0x10044a97c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5688>
10044b17c: b5001508    	cbnz	x8, 0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
10044b180: 910c03e0    	add	x0, sp, #0x300
10044b184: 97ef9971    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044b188: 140000a5    	b	0x10044b41c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6128>
10044b18c: d10303a9    	sub	x9, x29, #0xc0
10044b190: b8401129    	ldur	w9, [x9, #0x1]
10044b194: b9024be9    	str	w9, [sp, #0x248]
10044b198: b85443a9    	ldur	w9, [x29, #-0xbc]
10044b19c: 910923ea    	add	x10, sp, #0x248
10044b1a0: b8003149    	stur	w9, [x10, #0x3]
10044b1a4: 7101251f    	cmp	w8, #0x49
10044b1a8: 54000d41    	b.ne	0x10044b350 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x605c>
10044b1ac: d0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044b1b0: 9128d821    	add	x1, x1, #0xa36
10044b1b4: aa1903f3    	mov	x19, x25
10044b1b8: 528000a0    	mov	w0, #0x5                ; =5
10044b1bc: 52800522    	mov	w2, #0x29               ; =41
10044b1c0: 97f4acac    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044b1c4: 17ffff19    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b1c8: f85483b6    	ldur	x22, [x29, #-0xb8]
10044b1cc: 140000e2    	b	0x10044b554 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6260>
10044b1d0: 52800728    	mov	w8, #0x39               ; =57
10044b1d4: 390002a8    	strb	w8, [x21]
10044b1d8: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b1dc: 790006a8    	strh	w8, [x21, #0x2]
10044b1e0: aa1903f4    	mov	x20, x25
10044b1e4: 17fffd30    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b1e8: d10383a0    	sub	x0, x29, #0xe0
10044b1ec: 97f45d56    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044b1f0: 140000d8    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044b1f4: 52800488    	mov	w8, #0x24               ; =36
10044b1f8: 390002a8    	strb	w8, [x21]
10044b1fc: 7100bf7f    	cmp	w27, #0x2f
10044b200: 1a9f17e8    	cset	w8, eq
10044b204: f9407fe9    	ldr	x9, [sp, #0xf8]
10044b208: 790006a9    	strh	w9, [x21, #0x2]
10044b20c: 52802049    	mov	w9, #0x102              ; =258
10044b210: 79000aa9    	strh	w9, [x21, #0x4]
10044b214: 39001abf    	strb	wzr, [x21, #0x6]
10044b218: 39001ea8    	strb	w8, [x21, #0x7]
10044b21c: aa1903f4    	mov	x20, x25
10044b220: 17fffd21    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b224: f0000ac1    	adrp	x1, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044b228: 91076c21    	add	x1, x1, #0x1db
10044b22c: 52800442    	mov	w2, #0x22               ; =34
10044b230: 1400000c    	b	0x10044b260 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5f6c>
10044b234: d10383a0    	sub	x0, x29, #0xe0
10044b238: 97f45d43    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044b23c: 17fffe6f    	b	0x10044abf8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5904>
10044b240: 121e7a88    	and	w8, w20, #0xfffffffd
10044b244: 7102311f    	cmp	w8, #0x8c
10044b248: 54000a21    	b.ne	0x10044b38c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6098>
10044b24c: 71023a9f    	cmp	w20, #0x8e
10044b250: 17fffd40    	b	0x10044a750 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x545c>
10044b254: f0000ac1    	adrp	x1, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044b258: 91071821    	add	x1, x1, #0x1c6
10044b25c: 528002a2    	mov	w2, #0x15               ; =21
10044b260: aa1903f3    	mov	x19, x25
10044b264: 528000a0    	mov	w0, #0x5                ; =5
10044b268: 97f4ac82    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044b26c: 17fffeef    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b270: 52800488    	mov	w8, #0x24               ; =36
10044b274: 390002a8    	strb	w8, [x21]
10044b278: 790006b3    	strh	w19, [x21, #0x2]
10044b27c: 17fffdc0    	b	0x10044a97c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5688>
10044b280: 7200011f    	tst	w8, #0x1
10044b284: 52800468    	mov	w8, #0x23               ; =35
10044b288: 1a881508    	cinc	w8, w8, eq
10044b28c: 390002a8    	strb	w8, [x21]
10044b290: 790006b3    	strh	w19, [x21, #0x2]
10044b294: 0f008420    	movi.4h	v0, #0x1
10044b298: bd0006a0    	str	s0, [x21, #0x4]
10044b29c: aa1903f4    	mov	x20, x25
10044b2a0: 17fffd01    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b2a4: 7100191f    	cmp	w8, #0x6
10044b2a8: 54000161    	b.ne	0x10044b2d4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fe0>
10044b2ac: f85483b4    	ldur	x20, [x29, #-0xb8]
10044b2b0: f9401688    	ldr	x8, [x20, #0x28]
10044b2b4: b4000068    	cbz	x8, 0x10044b2c0 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fcc>
10044b2b8: f9401a80    	ldr	x0, [x20, #0x30]
10044b2bc: 9403227a    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044b2c0: f9402280    	ldr	x0, [x20, #0x40]
10044b2c4: b4000040    	cbz	x0, 0x10044b2cc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5fd8>
10044b2c8: 94032277    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044b2cc: aa1403e0    	mov	x0, x20
10044b2d0: 94032275    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044b2d4: b900fbff    	str	wzr, [sp, #0xf8]
10044b2d8: b940fbe9    	ldr	w9, [sp, #0xf8]
10044b2dc: 14000051    	b	0x10044b420 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x612c>
10044b2e0: 52800100    	mov	w0, #0x8                ; =8
10044b2e4: 52800a01    	mov	w1, #0x50               ; =80
10044b2e8: 9402d496    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b2ec: 140001db    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b2f0: 71025b7f    	cmp	w27, #0x96
10044b2f4: 52800348    	mov	w8, #0x1a               ; =26
10044b2f8: 390002a8    	strb	w8, [x21]
10044b2fc: 52800208    	mov	w8, #0x10               ; =16
10044b300: 390012a8    	strb	w8, [x21, #0x4]
10044b304: 1a9f17e8    	cset	w8, eq
10044b308: 390016a8    	strb	w8, [x21, #0x5]
10044b30c: f9407fe8    	ldr	x8, [sp, #0xf8]
10044b310: b9000aa8    	str	w8, [x21, #0x8]
10044b314: aa1903f4    	mov	x20, x25
10044b318: 17fffce3    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b31c: aa1903f3    	mov	x19, x25
10044b320: 94030cc4    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b324: 17fffec1    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b328: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b32c: 91270042    	add	x2, x2, #0x9c0
10044b330: 14000046    	b	0x10044b448 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6154>
10044b334: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b338: 91270042    	add	x2, x2, #0x9c0
10044b33c: aa1903f3    	mov	x19, x25
10044b340: aa0103e0    	mov	x0, x1
10044b344: aa1c03e1    	mov	x1, x28
10044b348: 9402d535    	bl	0x10050081c <__ZN4core5slice5index24slice_end_index_len_fail17h658aaf9fdfc67c91E>
10044b34c: 140001c3    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b350: f85483a9    	ldur	x9, [x29, #-0xb8]
10044b354: b9424bea    	ldr	w10, [sp, #0x248]
10044b358: b80012aa    	stur	w10, [x21, #0x1]
10044b35c: 910923ea    	add	x10, sp, #0x248
10044b360: b840314a    	ldur	w10, [x10, #0x3]
10044b364: b90006aa    	str	w10, [x21, #0x4]
10044b368: 390002a8    	strb	w8, [x21]
10044b36c: f90006a9    	str	x9, [x21, #0x8]
10044b370: aa1903f4    	mov	x20, x25
10044b374: 17fffccc    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b378: aa1403e1    	mov	x1, x20
10044b37c: 97f1d3d2    	bl	0x1000c02c4 <__ZN4core3ptr132drop_in_place$LT$core..result..Result$LT$core..convert..Infallible$C$quickjs_oxide..engine..api..runtime_error..RuntimeError$GT$$GT$17h392d3b5fe08b5889E>
10044b380: 14000023    	b	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
10044b384: f9401fe8    	ldr	x8, [sp, #0x38]
10044b388: 17fffef2    	b	0x10044af50 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5c5c>
10044b38c: aa1403e0    	mov	x0, x20
10044b390: 94001988    	bl	0x1004519b0 <__ZN13quickjs_oxide6engine2vm7numeric9operation11NumericKind10for_opcode17h93c0709190b2326dE>
10044b394: 12001c08    	and	w8, w0, #0xff
10044b398: 7100651f    	cmp	w8, #0x19
10044b39c: 54000101    	b.ne	0x10044b3bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x60c8>
10044b3a0: d0000ac1    	adrp	x1, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044b3a4: 9127cc21    	add	x1, x1, #0x9f3
10044b3a8: aa1903f3    	mov	x19, x25
10044b3ac: 528000a0    	mov	w0, #0x5                ; =5
10044b3b0: 52800362    	mov	w2, #0x1b               ; =27
10044b3b4: 97f4ac2f    	bl	0x100176470 <__ZN13quickjs_oxide6engine3api5error5Error3new17hf43d548b111f8abdE>
10044b3b8: 17fffe9c    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b3bc: 52800869    	mov	w9, #0x43               ; =67
10044b3c0: 390002a9    	strb	w9, [x21]
10044b3c4: 390006a8    	strb	w8, [x21, #0x1]
10044b3c8: aa1903f4    	mov	x20, x25
10044b3cc: 17fffcb6    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b3d0: aa1903f3    	mov	x19, x25
10044b3d4: 94030c97    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b3d8: 1400005e    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044b3dc: 7100191f    	cmp	w8, #0x6
10044b3e0: 54000161    	b.ne	0x10044b40c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6118>
10044b3e4: f85483b4    	ldur	x20, [x29, #-0xb8]
10044b3e8: f9401688    	ldr	x8, [x20, #0x28]
10044b3ec: b4000068    	cbz	x8, 0x10044b3f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6104>
10044b3f0: f9401a80    	ldr	x0, [x20, #0x30]
10044b3f4: 9403222c    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044b3f8: f9402280    	ldr	x0, [x20, #0x40]
10044b3fc: b4000040    	cbz	x0, 0x10044b404 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6110>
10044b400: 94032229    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044b404: aa1403e0    	mov	x0, x20
10044b408: 94032227    	bl	0x100513ca4 <dyld_stub_binder+0x100513ca4>
10044b40c: f9407fe9    	ldr	x9, [sp, #0xf8]
10044b410: f9401528    	ldr	x8, [x9, #0x28]
10044b414: 91000508    	add	x8, x8, #0x1
10044b418: f9001528    	str	x8, [x9, #0x28]
10044b41c: 52800009    	mov	w9, #0x0                ; =0
10044b420: 71032f7f    	cmp	w27, #0xcb
10044b424: 52800428    	mov	w8, #0x21               ; =33
10044b428: 390002a8    	strb	w8, [x21]
10044b42c: 1a9f07e8    	cset	w8, ne
10044b430: 390006a8    	strb	w8, [x21, #0x1]
10044b434: 39000aa9    	strb	w9, [x21, #0x2]
10044b438: aa1903f4    	mov	x20, x25
10044b43c: 17fffc9a    	b	0x10044a6a4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x53b0>
10044b440: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b444: 911d6042    	add	x2, x2, #0x758
10044b448: aa1903f3    	mov	x19, x25
10044b44c: aa1603e0    	mov	x0, x22
10044b450: 9402d569    	bl	0x1005009f4 <__ZN4core5slice5index22slice_index_order_fail17h1d9efe3670e787d1E>
10044b454: 14000181    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b458: 9108a3e0    	add	x0, sp, #0x228
10044b45c: aa1903f3    	mov	x19, x25
10044b460: 97f45cb9    	bl	0x100162744 <__ZN13quickjs_oxide6engine2vm9exception25runtime_error_to_vm_error17h7bbbea80d23e9b6fE>
10044b464: 17fffe71    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b468: 52800668    	mov	w8, #0x33               ; =51
10044b46c: 17fffe71    	b	0x10044ae30 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b3c>
10044b470: aa1903f3    	mov	x19, x25
10044b474: 94030c6f    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b478: 17fffe6c    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b47c: aa1903f3    	mov	x19, x25
10044b480: 94030c6c    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b484: 17fffe69    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b488: aa1903f3    	mov	x19, x25
10044b48c: 94030c69    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b490: 17fffe66    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b494: aa1903f3    	mov	x19, x25
10044b498: 94030c66    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b49c: 17fffe63    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b4a0: 94030c64    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b4a4: 17fffdd5    	b	0x10044abf8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5904>
10044b4a8: f0001202    	adrp	x2, 0x10068e000 <dyld_stub_binder+0x10068e000>
10044b4ac: 912fc042    	add	x2, x2, #0xbf0
10044b4b0: 52800481    	mov	w1, #0x24               ; =36
10044b4b4: 90000ac0    	adrp	x0, 0x1005a3000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x34ce8>
10044b4b8: 910f6c00    	add	x0, x0, #0x3db
10044b4bc: aa1903f3    	mov	x19, x25
10044b4c0: 9402d5a4    	bl	0x100500b50 <__ZN4core6option13expect_failed17h2829752eef520ac6E>
10044b4c4: 14000165    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b4c8: aa1903f3    	mov	x19, x25
10044b4cc: 94030c59    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b4d0: 14000020    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044b4d4: 94030c57    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b4d8: 1400001e    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044b4dc: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b4e0: 911d6042    	add	x2, x2, #0x758
10044b4e4: aa0903fc    	mov	x28, x9
10044b4e8: 17ffff95    	b	0x10044b33c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6048>
10044b4ec: 90001220    	adrp	x0, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b4f0: 913da000    	add	x0, x0, #0xf68
10044b4f4: aa1903f3    	mov	x19, x25
10044b4f8: 9402d55d    	bl	0x100500a6c <__ZN4core4cell22panic_already_borrowed17ha05dfdc27881e579E>
10044b4fc: 14000157    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b500: 94030c4c    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b504: 14000013    	b	0x10044b550 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x625c>
10044b508: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b50c: 911d6042    	add	x2, x2, #0x758
10044b510: aa0b03fc    	mov	x28, x11
10044b514: 17ffff8a    	b	0x10044b33c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6048>
10044b518: aa1903f3    	mov	x19, x25
10044b51c: 94030c45    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b520: 17fffe42    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b524: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b528: 911d6042    	add	x2, x2, #0x758
10044b52c: aa0a03fc    	mov	x28, x10
10044b530: 17ffff83    	b	0x10044b33c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6048>
10044b534: aa1903f3    	mov	x19, x25
10044b538: 94030c3e    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b53c: 17fffe3b    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b540: aa1903f3    	mov	x19, x25
10044b544: 94030c3b    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b548: 17fffe38    	b	0x10044ae28 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b34>
10044b54c: 94030c39    	bl	0x10050e630 <__ZN13quickjs_oxide6engine2vm5stack9SlotStore23operand_stack_underflow17h4f710a9c67d63033E>
10044b550: aa0003f6    	mov	x22, x0
10044b554: f90006b6    	str	x22, [x21, #0x8]
10044b558: 17fffe35    	b	0x10044ae2c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x5b38>
10044b55c: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b560: 91294042    	add	x2, x2, #0xa50
10044b564: 17ffffb9    	b	0x10044b448 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6154>
10044b568: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b56c: 91294042    	add	x2, x2, #0xa50
10044b570: 17ffff73    	b	0x10044b33c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6048>
10044b574: b00011e8    	adrp	x8, 0x100688000 <dyld_stub_binder+0x100688000>
10044b578: 91326108    	add	x8, x8, #0xc98
10044b57c: f81403a8    	stur	x8, [x29, #-0xc0]
10044b580: b00011e1    	adrp	x1, 0x100688000 <dyld_stub_binder+0x100688000>
10044b584: 9132a021    	add	x1, x1, #0xca8
10044b588: 14000090    	b	0x10044b7c8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64d4>
10044b58c: b0001200    	adrp	x0, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044b590: 911f6000    	add	x0, x0, #0x7d8
10044b594: aa1903f3    	mov	x19, x25
10044b598: 9402d546    	bl	0x100500ab0 <__ZN4core4cell30panic_already_mutably_borrowed17h8ed64df0236bfa1cE>
10044b59c: 1400012f    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b5a0: b00011e8    	adrp	x8, 0x100688000 <dyld_stub_binder+0x100688000>
10044b5a4: 91326108    	add	x8, x8, #0xc98
10044b5a8: f81403a8    	stur	x8, [x29, #-0xc0]
10044b5ac: b00011e1    	adrp	x1, 0x100688000 <dyld_stub_binder+0x100688000>
10044b5b0: 9132a021    	add	x1, x1, #0xca8
10044b5b4: 14000091    	b	0x10044b7f8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6504>
10044b5b8: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b5bc: 911d6042    	add	x2, x2, #0x758
10044b5c0: aa0803fc    	mov	x28, x8
10044b5c4: 17ffff5e    	b	0x10044b33c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6048>
10044b5c8: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b5cc: 911d6042    	add	x2, x2, #0x758
10044b5d0: aa0b03f6    	mov	x22, x11
10044b5d4: 17ffff9d    	b	0x10044b448 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6154>
10044b5d8: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b5dc: 911d6042    	add	x2, x2, #0x758
10044b5e0: aa0903f6    	mov	x22, x9
10044b5e4: 17ffff99    	b	0x10044b448 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6154>
10044b5e8: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b5ec: 911d6042    	add	x2, x2, #0x758
10044b5f0: 17ffff53    	b	0x10044b33c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6048>
10044b5f4: b0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044b5f8: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b5fc: 52800454    	mov	w20, #0x22              ; =34
10044b600: 52800440    	mov	w0, #0x22               ; =34
10044b604: 940321c3    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b608: b4001ea0    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044b60c: 528e6c88    	mov	w8, #0x7364             ; =29540
10044b610: 79004008    	strh	w8, [x0, #0x20]
10044b614: f0000ac8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044b618: 91076d08    	add	x8, x8, #0x1db
10044b61c: ad400500    	ldp	q0, q1, [x8]
10044b620: ad000400    	stp	q0, q1, [x0]
10044b624: 528000a8    	mov	w8, #0x5                ; =5
10044b628: 381883a8    	sturb	w8, [x29, #-0x78]
10044b62c: 52800448    	mov	w8, #0x22               ; =34
10044b630: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044b634: f81803bf    	stur	xzr, [x29, #-0x80]
10044b638: f81683a8    	stur	x8, [x29, #-0x98]
10044b63c: f81403bf    	stur	xzr, [x29, #-0xc0]
10044b640: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b644: 52800a00    	mov	w0, #0x50               ; =80
10044b648: 940321b2    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b64c: b5000a40    	cbnz	x0, 0x10044b794 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64a0>
10044b650: 52800100    	mov	w0, #0x8                ; =8
10044b654: 52800a01    	mov	w1, #0x50               ; =80
10044b658: 9402d3ba    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b65c: 140000ff    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b660: b0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044b664: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b668: 528004d4    	mov	w20, #0x26              ; =38
10044b66c: 528004c0    	mov	w0, #0x26               ; =38
10044b670: 940321a8    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b674: b4001b40    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044b678: f0000ac8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044b67c: 91099d08    	add	x8, x8, #0x267
10044b680: ad400500    	ldp	q0, q1, [x8]
10044b684: ad000400    	stp	q0, q1, [x0]
10044b688: f841e108    	ldur	x8, [x8, #0x1e]
10044b68c: f801e008    	stur	x8, [x0, #0x1e]
10044b690: 528000a8    	mov	w8, #0x5                ; =5
10044b694: 381883a8    	sturb	w8, [x29, #-0x78]
10044b698: 528004c8    	mov	w8, #0x26               ; =38
10044b69c: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044b6a0: f81803bf    	stur	xzr, [x29, #-0x80]
10044b6a4: f81683a8    	stur	x8, [x29, #-0x98]
10044b6a8: f81403bf    	stur	xzr, [x29, #-0xc0]
10044b6ac: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b6b0: 52800a00    	mov	w0, #0x50               ; =80
10044b6b4: 94032197    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b6b8: b50006e0    	cbnz	x0, 0x10044b794 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64a0>
10044b6bc: 52800100    	mov	w0, #0x8                ; =8
10044b6c0: 52800a01    	mov	w1, #0x50               ; =80
10044b6c4: 9402d39f    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b6c8: 140000e4    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b6cc: b0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044b6d0: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b6d4: 52800334    	mov	w20, #0x19              ; =25
10044b6d8: 52800320    	mov	w0, #0x19               ; =25
10044b6dc: 9403218d    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b6e0: b40017e0    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044b6e4: f0000ac8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044b6e8: 91088108    	add	x8, x8, #0x220
10044b6ec: 3dc00100    	ldr	q0, [x8]
10044b6f0: 3d800000    	str	q0, [x0]
10044b6f4: 3cc09100    	ldur	q0, [x8, #0x9]
10044b6f8: 3c809000    	stur	q0, [x0, #0x9]
10044b6fc: 528000a8    	mov	w8, #0x5                ; =5
10044b700: 381883a8    	sturb	w8, [x29, #-0x78]
10044b704: 52800328    	mov	w8, #0x19               ; =25
10044b708: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044b70c: f81803bf    	stur	xzr, [x29, #-0x80]
10044b710: f81683a8    	stur	x8, [x29, #-0x98]
10044b714: f81403bf    	stur	xzr, [x29, #-0xc0]
10044b718: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b71c: 52800a00    	mov	w0, #0x50               ; =80
10044b720: 9403217c    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b724: b5000380    	cbnz	x0, 0x10044b794 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x64a0>
10044b728: 52800100    	mov	w0, #0x8                ; =8
10044b72c: 52800a01    	mov	w1, #0x50               ; =80
10044b730: 9402d384    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b734: 140000c9    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b738: b0001393    	adrp	x19, 0x1006bc000 <dyld_stub_binder+0x1006bc000>
10044b73c: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b740: 528002b4    	mov	w20, #0x15              ; =21
10044b744: 528002a0    	mov	w0, #0x15               ; =21
10044b748: 94032172    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b74c: b4001480    	cbz	x0, 0x10044b9dc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66e8>
10044b750: f0000ac8    	adrp	x8, 0x1005a6000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x37ce8>
10044b754: 91071908    	add	x8, x8, #0x1c6
10044b758: 3dc00100    	ldr	q0, [x8]
10044b75c: 3d800000    	str	q0, [x0]
10044b760: f840d108    	ldur	x8, [x8, #0xd]
10044b764: f800d008    	stur	x8, [x0, #0xd]
10044b768: 528000a8    	mov	w8, #0x5                ; =5
10044b76c: 381883a8    	sturb	w8, [x29, #-0x78]
10044b770: 528002a8    	mov	w8, #0x15               ; =21
10044b774: a93723a0    	stp	x0, x8, [x29, #-0x90]
10044b778: f81803bf    	stur	xzr, [x29, #-0x80]
10044b77c: f81683a8    	stur	x8, [x29, #-0x98]
10044b780: f81403bf    	stur	xzr, [x29, #-0xc0]
10044b784: 397eea7f    	ldrb	wzr, [x19, #0xfba]
10044b788: 52800a00    	mov	w0, #0x50               ; =80
10044b78c: 94032161    	bl	0x100513d10 <dyld_stub_binder+0x100513d10>
10044b790: b40015e0    	cbz	x0, 0x10044ba4c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6758>
10044b794: aa0003f6    	mov	x22, x0
10044b798: ad7b07a0    	ldp	q0, q1, [x29, #-0xa0]
10044b79c: ad010400    	stp	q0, q1, [x0, #0x20]
10044b7a0: 3cd803a0    	ldur	q0, [x29, #-0x80]
10044b7a4: 3d801000    	str	q0, [x0, #0x40]
10044b7a8: ad7a03a1    	ldp	q1, q0, [x29, #-0xc0]
10044b7ac: ad000001    	stp	q1, q0, [x0]
10044b7b0: 17ffff69    	b	0x10044b554 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6260>
10044b7b4: b0001208    	adrp	x8, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044b7b8: 9125c108    	add	x8, x8, #0x970
10044b7bc: f81403a8    	stur	x8, [x29, #-0xc0]
10044b7c0: b0001201    	adrp	x1, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044b7c4: 91260021    	add	x1, x1, #0x980
10044b7c8: d10303a0    	sub	x0, x29, #0xc0
10044b7cc: 52800028    	mov	w8, #0x1                ; =1
10044b7d0: 910b03e9    	add	x9, sp, #0x2c0
10044b7d4: a900a408    	stp	x8, x9, [x0, #0x8]
10044b7d8: a901fc1f    	stp	xzr, xzr, [x0, #0x18]
10044b7dc: 9402d3f1    	bl	0x1005007a0 <__ZN4core9panicking9panic_fmt17heec96bfc27e6c546E>
10044b7e0: 1400009e    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b7e4: b0001208    	adrp	x8, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044b7e8: 9125c108    	add	x8, x8, #0x970
10044b7ec: f81403a8    	stur	x8, [x29, #-0xc0]
10044b7f0: b0001201    	adrp	x1, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044b7f4: 91260021    	add	x1, x1, #0x980
10044b7f8: d10303a0    	sub	x0, x29, #0xc0
10044b7fc: 52800028    	mov	w8, #0x1                ; =1
10044b800: 910b03e9    	add	x9, sp, #0x2c0
10044b804: a900a408    	stp	x8, x9, [x0, #0x8]
10044b808: a901fc1f    	stp	xzr, xzr, [x0, #0x18]
10044b80c: 9402d3e5    	bl	0x1005007a0 <__ZN4core9panicking9panic_fmt17heec96bfc27e6c546E>
10044b810: 14000092    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b814: 52800020    	mov	w0, #0x1                ; =1
10044b818: 52800621    	mov	w1, #0x31               ; =49
10044b81c: 9402d349    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b820: 1400008e    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b824: 52800100    	mov	w0, #0x8                ; =8
10044b828: 52800a01    	mov	w1, #0x50               ; =80
10044b82c: 9402d345    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b830: 1400008a    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b834: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b838: 9126a108    	add	x8, x8, #0x9a8
10044b83c: 1400005e    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b840: 52800100    	mov	w0, #0x8                ; =8
10044b844: 52800a01    	mov	w1, #0x50               ; =80
10044b848: 9402d33e    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b84c: 14000083    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b850: aa1903f3    	mov	x19, x25
10044b854: 52800020    	mov	w0, #0x1                ; =1
10044b858: 52800461    	mov	w1, #0x23               ; =35
10044b85c: 9402d339    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b860: 1400007e    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b864: f00011a8    	adrp	x8, 0x100682000 <dyld_stub_binder+0x100682000>
10044b868: 9134c108    	add	x8, x8, #0xd30
10044b86c: 14000052    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b870: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b874: 9127c108    	add	x8, x8, #0x9f0
10044b878: 1400004f    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b87c: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b880: 91276108    	add	x8, x8, #0x9d8
10044b884: 1400004c    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b888: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b88c: 912de108    	add	x8, x8, #0xb78
10044b890: 14000049    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b894: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b898: 911f4108    	add	x8, x8, #0x7d0
10044b89c: 14000046    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b8a0: f00011a9    	adrp	x9, 0x100682000 <dyld_stub_binder+0x100682000>
10044b8a4: 9134c129    	add	x9, x9, #0xd30
10044b8a8: f90023e9    	str	x9, [sp, #0x40]
10044b8ac: aa0803f6    	mov	x22, x8
10044b8b0: 14000042    	b	0x10044b9b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c4>
10044b8b4: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b8b8: 91308108    	add	x8, x8, #0xc20
10044b8bc: 1400003e    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b8c0: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b8c4: 9116e108    	add	x8, x8, #0x5b8
10044b8c8: f81403a8    	stur	x8, [x29, #-0xc0]
10044b8cc: 90001221    	adrp	x1, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b8d0: 91172021    	add	x1, x1, #0x5c8
10044b8d4: 14000028    	b	0x10044b974 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6680>
10044b8d8: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b8dc: 91158042    	add	x2, x2, #0x560
10044b8e0: 14000003    	b	0x10044b8ec <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65f8>
10044b8e4: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b8e8: 9115e042    	add	x2, x2, #0x578
10044b8ec: 52800501    	mov	w1, #0x28               ; =40
10044b8f0: d0000ac0    	adrp	x0, 0x1005a5000 <__ZN10num_bigint7biguint7convert19get_half_radix_base5BASES17h24d1a4a72cdfab77E+0x36ce8>
10044b8f4: 91283800    	add	x0, x0, #0xa0e
10044b8f8: 17fffef1    	b	0x10044b4bc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x61c8>
10044b8fc: aa0103f3    	mov	x19, x1
10044b900: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b904: 9126a042    	add	x2, x2, #0x9a8
10044b908: aa0803f4    	mov	x20, x8
10044b90c: 14000024    	b	0x10044b99c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66a8>
10044b910: b00011e2    	adrp	x2, 0x100688000 <dyld_stub_binder+0x100688000>
10044b914: 91336042    	add	x2, x2, #0xcd8
10044b918: aa1403e0    	mov	x0, x20
10044b91c: f9400be1    	ldr	x1, [sp, #0x10]
10044b920: 9402d38c    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044b924: 1400004d    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b928: b00011e2    	adrp	x2, 0x100688000 <dyld_stub_binder+0x100688000>
10044b92c: 91336042    	add	x2, x2, #0xcd8
10044b930: aa1603e1    	mov	x1, x22
10044b934: 9402d387    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044b938: 14000048    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b93c: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b940: 91152108    	add	x8, x8, #0x548
10044b944: 1400001c    	b	0x10044b9b4 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c0>
10044b948: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b94c: 912a6108    	add	x8, x8, #0xa98
10044b950: f81403a8    	stur	x8, [x29, #-0xc0]
10044b954: 90001221    	adrp	x1, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b958: 912aa021    	add	x1, x1, #0xaa8
10044b95c: 14000006    	b	0x10044b974 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6680>
10044b960: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b964: 912b0108    	add	x8, x8, #0xac0
10044b968: f81403a8    	stur	x8, [x29, #-0xc0]
10044b96c: 90001221    	adrp	x1, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b970: 912b4021    	add	x1, x1, #0xad0
10044b974: d10303a0    	sub	x0, x29, #0xc0
10044b978: 52800028    	mov	w8, #0x1                ; =1
10044b97c: 910b03e9    	add	x9, sp, #0x2c0
10044b980: a900a408    	stp	x8, x9, [x0, #0x8]
10044b984: a901fc1f    	stp	xzr, xzr, [x0, #0x18]
10044b988: aa1903f3    	mov	x19, x25
10044b98c: 9402d385    	bl	0x1005007a0 <__ZN4core9panicking9panic_fmt17heec96bfc27e6c546E>
10044b990: 14000032    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b994: 90001222    	adrp	x2, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b998: 911f4042    	add	x2, x2, #0x7d0
10044b99c: aa1403e0    	mov	x0, x20
10044b9a0: aa1303e1    	mov	x1, x19
10044b9a4: 9402d36b    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044b9a8: 1400002c    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b9ac: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b9b0: 912a0108    	add	x8, x8, #0xa80
10044b9b4: f90023e8    	str	x8, [sp, #0x40]
10044b9b8: aa1903f3    	mov	x19, x25
10044b9bc: aa1603e0    	mov	x0, x22
10044b9c0: aa1c03e1    	mov	x1, x28
10044b9c4: f94023e2    	ldr	x2, [sp, #0x40]
10044b9c8: 9402d362    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044b9cc: 14000023    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b9d0: 90001229    	adrp	x9, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b9d4: 9129a129    	add	x9, x9, #0xa68
10044b9d8: 17ffffb4    	b	0x10044b8a8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x65b4>
10044b9dc: aa1903f3    	mov	x19, x25
10044b9e0: 52800020    	mov	w0, #0x1                ; =1
10044b9e4: aa1403e1    	mov	x1, x20
10044b9e8: 9402d2d6    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044b9ec: 1400001b    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044b9f0: 90001220    	adrp	x0, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044b9f4: 911ee000    	add	x0, x0, #0x7b8
10044b9f8: 17fffee7    	b	0x10044b594 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62a0>
10044b9fc: b00011e2    	adrp	x2, 0x100688000 <dyld_stub_binder+0x100688000>
10044ba00: 91320042    	add	x2, x2, #0xc80
10044ba04: 9402d353    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044ba08: 14000014    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044ba0c: 90001220    	adrp	x0, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044ba10: 912ba000    	add	x0, x0, #0xae8
10044ba14: 17fffee0    	b	0x10044b594 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x62a0>
10044ba18: b00011e2    	adrp	x2, 0x100688000 <dyld_stub_binder+0x100688000>
10044ba1c: 91320042    	add	x2, x2, #0xc80
10044ba20: 9402d34c    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044ba24: 1400000d    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044ba28: f00011a2    	adrp	x2, 0x100682000 <dyld_stub_binder+0x100682000>
10044ba2c: 9134c042    	add	x2, x2, #0xd30
10044ba30: aa1c03e1    	mov	x1, x28
10044ba34: 9402d347    	bl	0x100500750 <__ZN4core9panicking18panic_bounds_check17h486908b9a487d47cE>
10044ba38: 14000008    	b	0x10044ba58 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6764>
10044ba3c: f9000be1    	str	x1, [sp, #0x10]
10044ba40: b0001202    	adrp	x2, 0x10068c000 <dyld_stub_binder+0x10068c000>
10044ba44: 9126c042    	add	x2, x2, #0x9b0
10044ba48: 17ffffb4    	b	0x10044b918 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6624>
10044ba4c: 52800100    	mov	w0, #0x8                ; =8
10044ba50: 52800a01    	mov	w1, #0x50               ; =80
10044ba54: 9402d2bb    	bl	0x100500540 <__ZN5alloc5alloc18handle_alloc_error17head785610f4500cfE>
10044ba58: d4200020    	brk	#0x1
10044ba5c: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044ba60: 911f4108    	add	x8, x8, #0x7d0
10044ba64: f90023e8    	str	x8, [sp, #0x40]
10044ba68: aa0003f6    	mov	x22, x0
10044ba6c: 17ffffd3    	b	0x10044b9b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c4>
10044ba70: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044ba74: 911f4108    	add	x8, x8, #0x7d0
10044ba78: f90023e8    	str	x8, [sp, #0x40]
10044ba7c: f9407ffc    	ldr	x28, [sp, #0xf8]
10044ba80: 17ffffce    	b	0x10044b9b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c4>
10044ba84: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044ba88: 91308108    	add	x8, x8, #0xc20
10044ba8c: 14000003    	b	0x10044ba98 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x67a4>
10044ba90: 90001228    	adrp	x8, 0x10068f000 <dyld_stub_binder+0x10068f000>
10044ba94: 912de108    	add	x8, x8, #0xb78
10044ba98: f90023e8    	str	x8, [sp, #0x40]
10044ba9c: aa0a03fc    	mov	x28, x10
10044baa0: 17ffffc6    	b	0x10044b9b8 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x66c4>
10044baa4: 14000067    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044baa8: 14000066    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044baac: 14000065    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bab0: 14000064    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bab4: f9408fe8    	ldr	x8, [sp, #0x118]
10044bab8: a902e519    	stp	x25, x25, [x8, #0x28]
10044babc: 9403201d    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bac0: aa0003f5    	mov	x21, x0
10044bac4: b85503a8    	ldur	w8, [x29, #-0xb0]
10044bac8: 7100091f    	cmp	w8, #0x2
10044bacc: 54000780    	b.eq	0x10044bbbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
10044bad0: f85403a0    	ldur	x0, [x29, #-0xc0]
10044bad4: f9400008    	ldr	x8, [x0]
10044bad8: f1000508    	subs	x8, x8, #0x1
10044badc: f9000008    	str	x8, [x0]
10044bae0: 540006e1    	b.ne	0x10044bbbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
10044bae4: 97efe48e    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044bae8: aa1903f3    	mov	x19, x25
10044baec: 14000062    	b	0x10044bc74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6980>
10044baf0: 9402d460    	bl	0x100500c70 <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044baf4: f9408fe8    	ldr	x8, [sp, #0x118]
10044baf8: a902e519    	stp	x25, x25, [x8, #0x28]
10044bafc: 9403200d    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bb00: aa0003f5    	mov	x21, x0
10044bb04: f9400288    	ldr	x8, [x20]
10044bb08: f1000508    	subs	x8, x8, #0x1
10044bb0c: f9000288    	str	x8, [x20]
10044bb10: 54000561    	b.ne	0x10044bbbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
10044bb14: 910c03e0    	add	x0, sp, #0x300
10044bb18: 97ef970c    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044bb1c: aa1903f3    	mov	x19, x25
10044bb20: 14000055    	b	0x10044bc74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6980>
10044bb24: 9402d453    	bl	0x100500c70 <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044bb28: aa0003f5    	mov	x21, x0
10044bb2c: f9400388    	ldr	x8, [x28]
10044bb30: f1000508    	subs	x8, x8, #0x1
10044bb34: f9000388    	str	x8, [x28]
10044bb38: 54000421    	b.ne	0x10044bbbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
10044bb3c: 910c03e0    	add	x0, sp, #0x300
10044bb40: 97ef9702    	bl	0x100031748 <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17hc838d053c4cb5cbeE>
10044bb44: aa1903f3    	mov	x19, x25
10044bb48: 1400004b    	b	0x10044bc74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6980>
10044bb4c: 9402d449    	bl	0x100500c70 <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044bb50: f9401708    	ldr	x8, [x24, #0x28]
10044bb54: d1000508    	sub	x8, x8, #0x1
10044bb58: f9001708    	str	x8, [x24, #0x28]
10044bb5c: f9408fe8    	ldr	x8, [sp, #0x118]
10044bb60: a902e519    	stp	x25, x25, [x8, #0x28]
10044bb64: 94031ff3    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bb68: 14000001    	b	0x10044bb6c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6878>
10044bb6c: f9407fe9    	ldr	x9, [sp, #0xf8]
10044bb70: f9401528    	ldr	x8, [x9, #0x28]
10044bb74: 91000508    	add	x8, x8, #0x1
10044bb78: f9001528    	str	x8, [x9, #0x28]
10044bb7c: f9408fe8    	ldr	x8, [sp, #0x118]
10044bb80: a902e519    	stp	x25, x25, [x8, #0x28]
10044bb84: 94031feb    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bb88: 14000001    	b	0x10044bb8c <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6898>
10044bb8c: aa0003f5    	mov	x21, x0
10044bb90: b85503a8    	ldur	w8, [x29, #-0xb0]
10044bb94: 7100091f    	cmp	w8, #0x2
10044bb98: 54000120    	b.eq	0x10044bbbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
10044bb9c: f85403a0    	ldur	x0, [x29, #-0xc0]
10044bba0: f9400008    	ldr	x8, [x0]
10044bba4: f1000508    	subs	x8, x8, #0x1
10044bba8: f9000008    	str	x8, [x0]
10044bbac: 54000081    	b.ne	0x10044bbbc <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x68c8>
10044bbb0: 97efe45b    	bl	0x100044d1c <__ZN5alloc2rc15Rc$LT$T$C$A$GT$9drop_slow17h12358889595844cbE>
10044bbb4: aa1903f3    	mov	x19, x25
10044bbb8: 1400002f    	b	0x10044bc74 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x6980>
10044bbbc: f9408fe8    	ldr	x8, [sp, #0x118]
10044bbc0: a902e519    	stp	x25, x25, [x8, #0x28]
10044bbc4: aa1503e0    	mov	x0, x21
10044bbc8: 94031fda    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bbcc: 9402d429    	bl	0x100500c70 <__ZN4core9panicking16panic_in_cleanup17he8958c706877a061E>
10044bbd0: f9408fe8    	ldr	x8, [sp, #0x118]
10044bbd4: a902e519    	stp	x25, x25, [x8, #0x28]
10044bbd8: 94031fd6    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bbdc: 14000019    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbe0: 14000018    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbe4: 14000017    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbe8: 14000016    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbec: 14000015    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbf0: 14000014    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbf4: 14000013    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbf8: 14000012    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bbfc: 14000011    	b	0x10044bc40 <__ZN13quickjs_oxide6engine2vm7execute13execute_frame17hd95a0349415c7223E+0x694c>
10044bc00: f9408fe8    	ldr	x8, [sp, #0x118]
10044bc04: a902e519    	stp	x25, x25, [x8, #0x28]
10044bc08: 94031fca    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bc0c: f9407fe9    	ldr	x9, [sp, #0xf8]
10044bc10: f9401528    	ldr	x8, [x9, #0x28]
10044bc14: d1000508    	sub	x8, x8, #0x1
10044bc18: f9001528    	str	x8, [x9, #0x28]
10044bc1c: f9408fe8    	ldr	x8, [sp, #0x118]
10044bc20: a902e519    	stp	x25, x25, [x8, #0x28]
10044bc24: 94031fc3    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bc28: f94016c8    	ldr	x8, [x22, #0x28]
10044bc2c: d1000508    	sub	x8, x8, #0x1
10044bc30: f90016c8    	str	x8, [x22, #0x28]
10044bc34: f9408fe8    	ldr	x8, [sp, #0x118]
10044bc38: a902e519    	stp	x25, x25, [x8, #0x28]
10044bc3c: 94031fbd    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bc40: aa0003f5    	mov	x21, x0
10044bc44: d10303a0    	sub	x0, x29, #0xc0
10044bc48: 97f111b9    	bl	0x10009032c <__ZN4core3ptr65drop_in_place$LT$quickjs_oxide..engine..api..error..ErrorData$GT$17hdbf522962b5e705eE>
10044bc4c: f9408fe8    	ldr	x8, [sp, #0x118]
10044bc50: a902e519    	stp	x25, x25, [x8, #0x28]
10044bc54: aa1503e0    	mov	x0, x21
10044bc58: 94031fb6    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bc5c: f9408fe8    	ldr	x8, [sp, #0x118]
10044bc60: a902e513    	stp	x19, x25, [x8, #0x28]
10044bc64: 94031fb3    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
10044bc68: aa0003f5    	mov	x21, x0
10044bc6c: d10303a0    	sub	x0, x29, #0xc0
10044bc70: 97f111af    	bl	0x10009032c <__ZN4core3ptr65drop_in_place$LT$quickjs_oxide..engine..api..error..ErrorData$GT$17hdbf522962b5e705eE>
10044bc74: f9408fe8    	ldr	x8, [sp, #0x118]
10044bc78: a902e513    	stp	x19, x25, [x8, #0x28]
10044bc7c: aa1503e0    	mov	x0, x21
10044bc80: 94031fac    	bl	0x100513b30 <dyld_stub_binder+0x100513b30>
