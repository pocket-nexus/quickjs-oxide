
/tmp/oxide-three-build-combined/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000577dc0 <quickjs_oxide::engine::vm::execute::binary_number_result>:
  577dc0:	push   %rbp
  577dc1:	push   %r15
  577dc3:	push   %r14
  577dc5:	push   %r13
  577dc7:	push   %r12
  577dc9:	push   %rbx
  577dca:	sub    $0x58,%rsp
  577dce:	movzbl %sil,%eax
  577dd2:	add    $0xffffff81,%eax
  577dd5:	cmp    $0x13,%eax
  577dd8:	ja     578cd0 <quickjs_oxide::engine::vm::execute::binary_number_result+0xf10>
  577dde:	mov    %rdi,%rbx
  577de1:	lea    0xf9464(%rip),%rsi        # 67124c <_fini+0x19f00>
  577de8:	movslq (%rsi,%rax,4),%rax
  577dec:	add    %rsi,%rax
  577def:	jmp    *%rax
  577df1:	cmpb   $0x0,(%rdx)
  577df4:	jne    577e1d <quickjs_oxide::engine::vm::execute::binary_number_result+0x5d>
  577df6:	cvtsi2sdl 0x4(%rdx),%xmm0
  577dfb:	cmpb   $0x0,(%rcx)
  577dfe:	je     577e27 <quickjs_oxide::engine::vm::execute::binary_number_result+0x67>
  577e00:	movsd  0x8(%rcx),%xmm1
  577e05:	jmp    577e2c <quickjs_oxide::engine::vm::execute::binary_number_result+0x6c>
  577e07:	cmpb   $0x0,(%rdx)
  577e0a:	jne    577e3a <quickjs_oxide::engine::vm::execute::binary_number_result+0x7a>
  577e0c:	cvtsi2sdl 0x4(%rdx),%xmm0
  577e11:	cmpb   $0x0,(%rcx)
  577e14:	je     577e44 <quickjs_oxide::engine::vm::execute::binary_number_result+0x84>
  577e16:	movsd  0x8(%rcx),%xmm1
  577e1b:	jmp    577e49 <quickjs_oxide::engine::vm::execute::binary_number_result+0x89>
  577e1d:	movsd  0x8(%rdx),%xmm0
  577e22:	cmpb   $0x0,(%rcx)
  577e25:	jne    577e00 <quickjs_oxide::engine::vm::execute::binary_number_result+0x40>
  577e27:	cvtsi2sdl 0x4(%rcx),%xmm1
  577e2c:	ucomisd %xmm1,%xmm0
  577e30:	setnp  %al
  577e33:	sete   %cl
  577e36:	and    %al,%cl
  577e38:	jmp    577e55 <quickjs_oxide::engine::vm::execute::binary_number_result+0x95>
  577e3a:	movsd  0x8(%rdx),%xmm0
  577e3f:	cmpb   $0x0,(%rcx)
  577e42:	jne    577e16 <quickjs_oxide::engine::vm::execute::binary_number_result+0x56>
  577e44:	cvtsi2sdl 0x4(%rcx),%xmm1
  577e49:	ucomisd %xmm1,%xmm0
  577e4d:	setp   %al
  577e50:	setne  %cl
  577e53:	or     %al,%cl
  577e55:	mov    %cl,0x1(%rbx)
  577e58:	mov    $0x2,%al
  577e5a:	jmp    578cbf <quickjs_oxide::engine::vm::execute::binary_number_result+0xeff>
  577e5f:	cmpb   $0x0,(%rdx)
  577e62:	jne    578035 <quickjs_oxide::engine::vm::execute::binary_number_result+0x275>
  577e68:	cvtsi2sdl 0x4(%rdx),%xmm0
  577e6d:	jmp    57803a <quickjs_oxide::engine::vm::execute::binary_number_result+0x27a>
  577e72:	cmpb   $0x0,(%rdx)
  577e75:	jne    5780e0 <quickjs_oxide::engine::vm::execute::binary_number_result+0x320>
  577e7b:	cvtsi2sdl 0x4(%rdx),%xmm0
  577e80:	cmpb   $0x0,(%rcx)
  577e83:	je     5780ee <quickjs_oxide::engine::vm::execute::binary_number_result+0x32e>
  577e89:	movsd  0x8(%rcx),%xmm1
  577e8e:	call   *0x2950c4(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  577e94:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  577e99:	cmpb   $0x0,(%rdx)
  577e9c:	jne    5780fe <quickjs_oxide::engine::vm::execute::binary_number_result+0x33e>
  577ea2:	cvtsi2sdl 0x4(%rdx),%xmm0
  577ea7:	jmp    578103 <quickjs_oxide::engine::vm::execute::binary_number_result+0x343>
  577eac:	cmpb   $0x0,(%rdx)
  577eaf:	jne    5781a9 <quickjs_oxide::engine::vm::execute::binary_number_result+0x3e9>
  577eb5:	cvtsi2sdl 0x4(%rdx),%xmm0
  577eba:	cmpb   $0x0,(%rcx)
  577ebd:	je     5781b7 <quickjs_oxide::engine::vm::execute::binary_number_result+0x3f7>
  577ec3:	movsd  0x8(%rcx),%xmm1
  577ec8:	mulsd  %xmm1,%xmm0
  577ecc:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  577ed1:	cmpb   $0x0,(%rdx)
  577ed4:	jne    5781c5 <quickjs_oxide::engine::vm::execute::binary_number_result+0x405>
  577eda:	cvtsi2sdl 0x4(%rdx),%xmm0
  577edf:	cmpb   $0x0,(%rcx)
  577ee2:	je     5781d3 <quickjs_oxide::engine::vm::execute::binary_number_result+0x413>
  577ee8:	movsd  0x8(%rcx),%xmm1
  577eed:	jmp    5781d8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x418>
  577ef2:	cmpb   $0x0,(%rdx)
  577ef5:	jne    5781e1 <quickjs_oxide::engine::vm::execute::binary_number_result+0x421>
  577efb:	cvtsi2sdl 0x4(%rdx),%xmm0
  577f00:	cmpb   $0x0,(%rcx)
  577f03:	je     5781ef <quickjs_oxide::engine::vm::execute::binary_number_result+0x42f>
  577f09:	movsd  0x8(%rcx),%xmm1
  577f0e:	divsd  %xmm1,%xmm0
  577f12:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  577f17:	cmpb   $0x0,(%rdx)
  577f1a:	jne    5781fd <quickjs_oxide::engine::vm::execute::binary_number_result+0x43d>
  577f20:	cvtsi2sdl 0x4(%rdx),%xmm0
  577f25:	jmp    578202 <quickjs_oxide::engine::vm::execute::binary_number_result+0x442>
  577f2a:	testb  $0x1,(%rdx)
  577f2d:	je     5782a8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x4e8>
  577f33:	movsd  0x8(%rdx),%xmm0
  577f38:	movzbl (%rcx),%esi
  577f3b:	test   $0x1,%sil
  577f3f:	jne    5782c6 <quickjs_oxide::engine::vm::execute::binary_number_result+0x506>
  577f45:	cvtsi2sdl 0x4(%rcx),%xmm1
  577f4a:	jmp    5782cb <quickjs_oxide::engine::vm::execute::binary_number_result+0x50b>
  577f4f:	testb  $0x1,(%rdx)
  577f52:	je     5782d1 <quickjs_oxide::engine::vm::execute::binary_number_result+0x511>
  577f58:	movsd  0x8(%rdx),%xmm0
  577f5d:	movzbl (%rcx),%esi
  577f60:	test   $0x1,%sil
  577f64:	jne    5782ef <quickjs_oxide::engine::vm::execute::binary_number_result+0x52f>
  577f6a:	cvtsi2sdl 0x4(%rcx),%xmm1
  577f6f:	jmp    5782f4 <quickjs_oxide::engine::vm::execute::binary_number_result+0x534>
  577f74:	cmpb   $0x0,(%rdx)
  577f77:	jne    578357 <quickjs_oxide::engine::vm::execute::binary_number_result+0x597>
  577f7d:	cvtsi2sdl 0x4(%rdx),%xmm0
  577f82:	cmpb   $0x0,(%rcx)
  577f85:	je     578365 <quickjs_oxide::engine::vm::execute::binary_number_result+0x5a5>
  577f8b:	movsd  0x8(%rcx),%xmm1
  577f90:	ucomisd %xmm1,%xmm0
  577f94:	jmp    57867a <quickjs_oxide::engine::vm::execute::binary_number_result+0x8ba>
  577f99:	cmpb   $0x0,(%rdx)
  577f9c:	jne    578373 <quickjs_oxide::engine::vm::execute::binary_number_result+0x5b3>
  577fa2:	cvtsi2sdl 0x4(%rdx),%xmm0
  577fa7:	cmpb   $0x0,(%rcx)
  577faa:	je     578381 <quickjs_oxide::engine::vm::execute::binary_number_result+0x5c1>
  577fb0:	movsd  0x8(%rcx),%xmm1
  577fb5:	jmp    578386 <quickjs_oxide::engine::vm::execute::binary_number_result+0x5c6>
  577fba:	cmpb   $0x0,(%rdx)
  577fbd:	jne    57843e <quickjs_oxide::engine::vm::execute::binary_number_result+0x67e>
  577fc3:	cvtsi2sdl 0x4(%rdx),%xmm0
  577fc8:	jmp    578443 <quickjs_oxide::engine::vm::execute::binary_number_result+0x683>
  577fcd:	cmpb   $0x0,(%rdx)
  577fd0:	jne    5784e9 <quickjs_oxide::engine::vm::execute::binary_number_result+0x729>
  577fd6:	cvtsi2sdl 0x4(%rdx),%xmm0
  577fdb:	jmp    5784ee <quickjs_oxide::engine::vm::execute::binary_number_result+0x72e>
  577fe0:	cmpb   $0x0,(%rdx)
  577fe3:	jne    578594 <quickjs_oxide::engine::vm::execute::binary_number_result+0x7d4>
  577fe9:	cvtsi2sdl 0x4(%rdx),%xmm0
  577fee:	jmp    578599 <quickjs_oxide::engine::vm::execute::binary_number_result+0x7d9>
  577ff3:	cmpb   $0x0,(%rdx)
  577ff6:	jne    578641 <quickjs_oxide::engine::vm::execute::binary_number_result+0x881>
  577ffc:	cvtsi2sdl 0x4(%rdx),%xmm0
  578001:	cmpb   $0x0,(%rcx)
  578004:	je     57864f <quickjs_oxide::engine::vm::execute::binary_number_result+0x88f>
  57800a:	movsd  0x8(%rcx),%xmm1
  57800f:	jmp    578654 <quickjs_oxide::engine::vm::execute::binary_number_result+0x894>
  578014:	cmpb   $0x0,(%rdx)
  578017:	jne    578663 <quickjs_oxide::engine::vm::execute::binary_number_result+0x8a3>
  57801d:	cvtsi2sdl 0x4(%rdx),%xmm0
  578022:	cmpb   $0x0,(%rcx)
  578025:	je     578671 <quickjs_oxide::engine::vm::execute::binary_number_result+0x8b1>
  57802b:	movsd  0x8(%rcx),%xmm1
  578030:	jmp    578676 <quickjs_oxide::engine::vm::execute::binary_number_result+0x8b6>
  578035:	movsd  0x8(%rdx),%xmm0
  57803a:	movq   %xmm0,%rax
  57803f:	movabs $0x7fffffffffffffff,%r14
  578049:	and    %r14,%rax
  57804c:	movabs $0x7ff0000000000000,%r15
  578056:	cmp    %r15,%rax
  578059:	setg   %dl
  57805c:	sete   %sil
  578060:	test   %rax,%rax
  578063:	sete   %al
  578066:	or     %sil,%al
  578069:	xor    %ebp,%ebp
  57806b:	or     %dl,%al
  57806d:	jne    5786b1 <quickjs_oxide::engine::vm::execute::binary_number_result+0x8f1>
  578073:	mov    %rcx,%r12
  578076:	call   *0x294d5c(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  57807c:	movsd  0xe34a4(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  578084:	call   *0x294ece(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  57808a:	movsd  0xe3496(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  578092:	addsd  %xmm0,%xmm3
  578096:	xorpd  %xmm2,%xmm2
  57809a:	movapd %xmm0,%xmm1
  57809e:	cmpltsd %xmm2,%xmm1
  5780a3:	andpd  %xmm1,%xmm3
  5780a7:	andnpd %xmm0,%xmm1
  5780ab:	orpd   %xmm3,%xmm1
  5780af:	ucomisd 0xe3529(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  5780b7:	jae    578685 <quickjs_oxide::engine::vm::execute::binary_number_result+0x8c5>
  5780bd:	movapd %xmm1,%xmm0
  5780c1:	maxsd  0xe3437(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  5780c9:	minsd  0xe3437(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  5780d1:	cvttsd2si %xmm0,%eax
  5780d5:	xor    %ebp,%ebp
  5780d7:	ucomisd %xmm1,%xmm1
  5780db:	jmp    5786ab <quickjs_oxide::engine::vm::execute::binary_number_result+0x8eb>
  5780e0:	movsd  0x8(%rdx),%xmm0
  5780e5:	cmpb   $0x0,(%rcx)
  5780e8:	jne    577e89 <quickjs_oxide::engine::vm::execute::binary_number_result+0xc9>
  5780ee:	cvtsi2sdl 0x4(%rcx),%xmm1
  5780f3:	call   *0x294e5f(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5780f9:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  5780fe:	movsd  0x8(%rdx),%xmm0
  578103:	movq   %xmm0,%rax
  578108:	movabs $0x7fffffffffffffff,%r14
  578112:	and    %r14,%rax
  578115:	movabs $0x7ff0000000000000,%r15
  57811f:	cmp    %r15,%rax
  578122:	setg   %dl
  578125:	sete   %sil
  578129:	test   %rax,%rax
  57812c:	sete   %al
  57812f:	or     %sil,%al
  578132:	xor    %ebp,%ebp
  578134:	or     %dl,%al
  578136:	jne    5787aa <quickjs_oxide::engine::vm::execute::binary_number_result+0x9ea>
  57813c:	mov    %rcx,%r12
  57813f:	call   *0x294c93(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578145:	movsd  0xe33db(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  57814d:	call   *0x294e05(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  578153:	movsd  0xe33cd(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  57815b:	addsd  %xmm0,%xmm3
  57815f:	xorpd  %xmm2,%xmm2
  578163:	movapd %xmm0,%xmm1
  578167:	cmpltsd %xmm2,%xmm1
  57816c:	andpd  %xmm1,%xmm3
  578170:	andnpd %xmm0,%xmm1
  578174:	orpd   %xmm3,%xmm1
  578178:	ucomisd 0xe3460(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  578180:	jae    57877e <quickjs_oxide::engine::vm::execute::binary_number_result+0x9be>
  578186:	movapd %xmm1,%xmm0
  57818a:	maxsd  0xe336e(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  578192:	minsd  0xe336e(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  57819a:	cvttsd2si %xmm0,%eax
  57819e:	xor    %ebp,%ebp
  5781a0:	ucomisd %xmm1,%xmm1
  5781a4:	jmp    5787a4 <quickjs_oxide::engine::vm::execute::binary_number_result+0x9e4>
  5781a9:	movsd  0x8(%rdx),%xmm0
  5781ae:	cmpb   $0x0,(%rcx)
  5781b1:	jne    577ec3 <quickjs_oxide::engine::vm::execute::binary_number_result+0x103>
  5781b7:	cvtsi2sdl 0x4(%rcx),%xmm1
  5781bc:	mulsd  %xmm1,%xmm0
  5781c0:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  5781c5:	movsd  0x8(%rdx),%xmm0
  5781ca:	cmpb   $0x0,(%rcx)
  5781cd:	jne    577ee8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x128>
  5781d3:	cvtsi2sdl 0x4(%rcx),%xmm1
  5781d8:	ucomisd %xmm0,%xmm1
  5781dc:	jmp    578658 <quickjs_oxide::engine::vm::execute::binary_number_result+0x898>
  5781e1:	movsd  0x8(%rdx),%xmm0
  5781e6:	cmpb   $0x0,(%rcx)
  5781e9:	jne    577f09 <quickjs_oxide::engine::vm::execute::binary_number_result+0x149>
  5781ef:	cvtsi2sdl 0x4(%rcx),%xmm1
  5781f4:	divsd  %xmm1,%xmm0
  5781f8:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  5781fd:	movsd  0x8(%rdx),%xmm0
  578202:	movq   %xmm0,%rax
  578207:	movabs $0x7fffffffffffffff,%r14
  578211:	and    %r14,%rax
  578214:	movabs $0x7ff0000000000000,%r15
  57821e:	cmp    %r15,%rax
  578221:	setg   %dl
  578224:	sete   %sil
  578228:	test   %rax,%rax
  57822b:	sete   %al
  57822e:	or     %sil,%al
  578231:	xor    %ebp,%ebp
  578233:	or     %dl,%al
  578235:	jne    5788a3 <quickjs_oxide::engine::vm::execute::binary_number_result+0xae3>
  57823b:	mov    %rcx,%r12
  57823e:	call   *0x294b94(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578244:	movsd  0xe32dc(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  57824c:	call   *0x294d06(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  578252:	movsd  0xe32ce(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  57825a:	addsd  %xmm0,%xmm3
  57825e:	xorpd  %xmm2,%xmm2
  578262:	movapd %xmm0,%xmm1
  578266:	cmpltsd %xmm2,%xmm1
  57826b:	andpd  %xmm1,%xmm3
  57826f:	andnpd %xmm0,%xmm1
  578273:	orpd   %xmm3,%xmm1
  578277:	ucomisd 0xe3361(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  57827f:	jae    578877 <quickjs_oxide::engine::vm::execute::binary_number_result+0xab7>
  578285:	movapd %xmm1,%xmm0
  578289:	maxsd  0xe326f(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  578291:	minsd  0xe326f(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  578299:	cvttsd2si %xmm0,%eax
  57829d:	xor    %ebp,%ebp
  57829f:	ucomisd %xmm1,%xmm1
  5782a3:	jmp    57889d <quickjs_oxide::engine::vm::execute::binary_number_result+0xadd>
  5782a8:	movzbl (%rcx),%esi
  5782ab:	mov    0x4(%rdx),%edx
  5782ae:	test   $0x1,%sil
  5782b2:	je     578970 <quickjs_oxide::engine::vm::execute::binary_number_result+0xbb0>
  5782b8:	cvtsi2sd %edx,%xmm0
  5782bc:	test   $0x1,%sil
  5782c0:	je     577f45 <quickjs_oxide::engine::vm::execute::binary_number_result+0x185>
  5782c6:	movsd  0x8(%rcx),%xmm1
  5782cb:	addsd  %xmm1,%xmm0
  5782cf:	jmp    5782f8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x538>
  5782d1:	movzbl (%rcx),%esi
  5782d4:	mov    0x4(%rdx),%edx
  5782d7:	test   $0x1,%sil
  5782db:	je     578980 <quickjs_oxide::engine::vm::execute::binary_number_result+0xbc0>
  5782e1:	cvtsi2sd %edx,%xmm0
  5782e5:	test   $0x1,%sil
  5782e9:	je     577f6a <quickjs_oxide::engine::vm::execute::binary_number_result+0x1aa>
  5782ef:	movsd  0x8(%rcx),%xmm1
  5782f4:	subsd  %xmm1,%xmm0
  5782f8:	movapd %xmm0,%xmm1
  5782fc:	maxsd  0xe31fc(%rip),%xmm1        # 65b500 <_fini+0x41b4>
  578304:	minsd  0xe31fc(%rip),%xmm1        # 65b508 <_fini+0x41bc>
  57830c:	cvttsd2si %xmm1,%ecx
  578310:	xor    %eax,%eax
  578312:	ucomisd %xmm0,%xmm0
  578316:	cmovnp %ecx,%eax
  578319:	xorps  %xmm1,%xmm1
  57831c:	cvtsi2sd %eax,%xmm1
  578320:	ucomisd %xmm1,%xmm0
  578324:	jne    578cb8 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef8>
  57832a:	jp     578cb8 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef8>
  578330:	xorpd  %xmm1,%xmm1
  578334:	ucomisd %xmm1,%xmm0
  578338:	jne    578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  57833e:	jp     578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  578344:	movq   %xmm0,%rcx
  578349:	test   %rcx,%rcx
  57834c:	je     578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  578352:	jmp    578cb8 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef8>
  578357:	movsd  0x8(%rdx),%xmm0
  57835c:	cmpb   $0x0,(%rcx)
  57835f:	jne    577f8b <quickjs_oxide::engine::vm::execute::binary_number_result+0x1cb>
  578365:	cvtsi2sdl 0x4(%rcx),%xmm1
  57836a:	ucomisd %xmm1,%xmm0
  57836e:	jmp    57867a <quickjs_oxide::engine::vm::execute::binary_number_result+0x8ba>
  578373:	movsd  0x8(%rdx),%xmm0
  578378:	cmpb   $0x0,(%rcx)
  57837b:	jne    577fb0 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1f0>
  578381:	cvtsi2sdl 0x4(%rcx),%xmm1
  578386:	movq   %xmm1,%rax
  57838b:	movabs $0x7fffffffffffffff,%r14
  578395:	and    %rax,%r14
  578398:	movapd %xmm0,0x10(%rsp)
  57839e:	call   *0x294c2c(%rip)        # 80cfd0 <pow@GLIBC_2.29>
  5783a4:	movapd 0x10(%rsp),%xmm2
  5783aa:	movabs $0x7ff0000000000000,%rax
  5783b4:	andpd  0xe0064(%rip),%xmm2        # 658420 <_fini+0x10d4>
  5783bc:	movapd %xmm0,%xmm1
  5783c0:	cmp    %rax,%r14
  5783c3:	jl     5783cd <quickjs_oxide::engine::vm::execute::binary_number_result+0x60d>
  5783c5:	movsd  0xe30fb(%rip),%xmm1        # 65b4c8 <_fini+0x417c>
  5783cd:	cmpeqsd 0xe315a(%rip),%xmm2        # 65b530 <_fini+0x41e4>
  5783d6:	andpd  %xmm2,%xmm1
  5783da:	andnpd %xmm0,%xmm2
  5783de:	orpd   %xmm1,%xmm2
  5783e2:	movapd %xmm2,%xmm0
  5783e6:	maxsd  0xe3112(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  5783ee:	minsd  0xe3112(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  5783f6:	cvttsd2si %xmm0,%ecx
  5783fa:	xor    %eax,%eax
  5783fc:	ucomisd %xmm2,%xmm2
  578400:	cmovnp %ecx,%eax
  578403:	xorps  %xmm0,%xmm0
  578406:	cvtsi2sd %eax,%xmm0
  57840a:	ucomisd %xmm0,%xmm2
  57840e:	jne    578434 <quickjs_oxide::engine::vm::execute::binary_number_result+0x674>
  578410:	jp     578434 <quickjs_oxide::engine::vm::execute::binary_number_result+0x674>
  578412:	xorpd  %xmm0,%xmm0
  578416:	ucomisd %xmm0,%xmm2
  57841a:	jne    578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  578420:	jp     578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  578426:	movq   %xmm2,%rcx
  57842b:	test   %rcx,%rcx
  57842e:	je     578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  578434:	movsd  %xmm2,0x8(%rbx)
  578439:	jmp    578cbd <quickjs_oxide::engine::vm::execute::binary_number_result+0xefd>
  57843e:	movsd  0x8(%rdx),%xmm0
  578443:	movq   %xmm0,%rax
  578448:	movabs $0x7fffffffffffffff,%r14
  578452:	and    %r14,%rax
  578455:	movabs $0x7ff0000000000000,%r15
  57845f:	cmp    %r15,%rax
  578462:	setg   %dl
  578465:	sete   %sil
  578469:	test   %rax,%rax
  57846c:	sete   %al
  57846f:	or     %sil,%al
  578472:	xor    %ebp,%ebp
  578474:	or     %dl,%al
  578476:	jne    5789bc <quickjs_oxide::engine::vm::execute::binary_number_result+0xbfc>
  57847c:	mov    %rcx,%r12
  57847f:	call   *0x294953(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578485:	movsd  0xe309b(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  57848d:	call   *0x294ac5(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  578493:	movsd  0xe308d(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  57849b:	addsd  %xmm0,%xmm3
  57849f:	xorpd  %xmm2,%xmm2
  5784a3:	movapd %xmm0,%xmm1
  5784a7:	cmpltsd %xmm2,%xmm1
  5784ac:	andpd  %xmm1,%xmm3
  5784b0:	andnpd %xmm0,%xmm1
  5784b4:	orpd   %xmm3,%xmm1
  5784b8:	ucomisd 0xe3120(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  5784c0:	jae    578990 <quickjs_oxide::engine::vm::execute::binary_number_result+0xbd0>
  5784c6:	movapd %xmm1,%xmm0
  5784ca:	maxsd  0xe302e(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  5784d2:	minsd  0xe302e(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  5784da:	cvttsd2si %xmm0,%eax
  5784de:	xor    %ebp,%ebp
  5784e0:	ucomisd %xmm1,%xmm1
  5784e4:	jmp    5789b6 <quickjs_oxide::engine::vm::execute::binary_number_result+0xbf6>
  5784e9:	movsd  0x8(%rdx),%xmm0
  5784ee:	movq   %xmm0,%rax
  5784f3:	movabs $0x7fffffffffffffff,%r14
  5784fd:	and    %r14,%rax
  578500:	movabs $0x7ff0000000000000,%r15
  57850a:	cmp    %r15,%rax
  57850d:	setg   %dl
  578510:	sete   %sil
  578514:	test   %rax,%rax
  578517:	sete   %al
  57851a:	or     %sil,%al
  57851d:	xor    %ebp,%ebp
  57851f:	or     %dl,%al
  578521:	jne    578ab5 <quickjs_oxide::engine::vm::execute::binary_number_result+0xcf5>
  578527:	mov    %rcx,%r12
  57852a:	call   *0x2948a8(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578530:	movsd  0xe2ff0(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  578538:	call   *0x294a1a(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  57853e:	movsd  0xe2fe2(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  578546:	addsd  %xmm0,%xmm3
  57854a:	xorpd  %xmm2,%xmm2
  57854e:	movapd %xmm0,%xmm1
  578552:	cmpltsd %xmm2,%xmm1
  578557:	andpd  %xmm1,%xmm3
  57855b:	andnpd %xmm0,%xmm1
  57855f:	orpd   %xmm3,%xmm1
  578563:	ucomisd 0xe3075(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  57856b:	jae    578a89 <quickjs_oxide::engine::vm::execute::binary_number_result+0xcc9>
  578571:	movapd %xmm1,%xmm0
  578575:	maxsd  0xe2f83(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  57857d:	minsd  0xe2f83(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  578585:	cvttsd2si %xmm0,%eax
  578589:	xor    %ebp,%ebp
  57858b:	ucomisd %xmm1,%xmm1
  57858f:	jmp    578aaf <quickjs_oxide::engine::vm::execute::binary_number_result+0xcef>
  578594:	movsd  0x8(%rdx),%xmm0
  578599:	movq   %xmm0,%rax
  57859e:	movabs $0x7fffffffffffffff,%r15
  5785a8:	and    %r15,%rax
  5785ab:	movabs $0x7ff0000000000000,%r12
  5785b5:	cmp    %r12,%rax
  5785b8:	setg   %dl
  5785bb:	sete   %sil
  5785bf:	test   %rax,%rax
  5785c2:	sete   %al
  5785c5:	or     %sil,%al
  5785c8:	xor    %r14d,%r14d
  5785cb:	or     %dl,%al
  5785cd:	jne    578bb0 <quickjs_oxide::engine::vm::execute::binary_number_result+0xdf0>
  5785d3:	mov    %rcx,%r13
  5785d6:	call   *0x2947fc(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5785dc:	movsd  0xe2f44(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  5785e4:	call   *0x29496e(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5785ea:	movsd  0xe2f36(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  5785f2:	addsd  %xmm0,%xmm3
  5785f6:	xorpd  %xmm2,%xmm2
  5785fa:	movapd %xmm0,%xmm1
  5785fe:	cmpltsd %xmm2,%xmm1
  578603:	andpd  %xmm1,%xmm3
  578607:	andnpd %xmm0,%xmm1
  57860b:	orpd   %xmm3,%xmm1
  57860f:	ucomisd 0xe2fc9(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  578617:	jae    578b82 <quickjs_oxide::engine::vm::execute::binary_number_result+0xdc2>
  57861d:	movapd %xmm1,%xmm0
  578621:	maxsd  0xe2ed7(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  578629:	minsd  0xe2ed7(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  578631:	cvttsd2si %xmm0,%eax
  578635:	xor    %r14d,%r14d
  578638:	ucomisd %xmm1,%xmm1
  57863c:	jmp    578ba9 <quickjs_oxide::engine::vm::execute::binary_number_result+0xde9>
  578641:	movsd  0x8(%rdx),%xmm0
  578646:	cmpb   $0x0,(%rcx)
  578649:	jne    57800a <quickjs_oxide::engine::vm::execute::binary_number_result+0x24a>
  57864f:	cvtsi2sdl 0x4(%rcx),%xmm1
  578654:	ucomisd %xmm1,%xmm0
  578658:	setae  0x1(%rbx)
  57865c:	mov    $0x2,%al
  57865e:	jmp    578cbf <quickjs_oxide::engine::vm::execute::binary_number_result+0xeff>
  578663:	movsd  0x8(%rdx),%xmm0
  578668:	cmpb   $0x0,(%rcx)
  57866b:	jne    57802b <quickjs_oxide::engine::vm::execute::binary_number_result+0x26b>
  578671:	cvtsi2sdl 0x4(%rcx),%xmm1
  578676:	ucomisd %xmm0,%xmm1
  57867a:	seta   0x1(%rbx)
  57867e:	mov    $0x2,%al
  578680:	jmp    578cbf <quickjs_oxide::engine::vm::execute::binary_number_result+0xeff>
  578685:	movsd  0xe2f5b(%rip),%xmm0        # 65b5e8 <_fini+0x429c>
  57868d:	addsd  %xmm1,%xmm0
  578691:	xor    %ebp,%ebp
  578693:	ucomisd %xmm0,%xmm0
  578697:	maxsd  0xe2e61(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  57869f:	minsd  0xe2e61(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  5786a7:	cvttsd2si %xmm0,%eax
  5786ab:	cmovnp %eax,%ebp
  5786ae:	mov    %r12,%rcx
  5786b1:	cmpb   $0x0,(%rcx)
  5786b4:	jne    5786c0 <quickjs_oxide::engine::vm::execute::binary_number_result+0x900>
  5786b6:	xorps  %xmm0,%xmm0
  5786b9:	cvtsi2sdl 0x4(%rcx),%xmm0
  5786be:	jmp    5786c5 <quickjs_oxide::engine::vm::execute::binary_number_result+0x905>
  5786c0:	movsd  0x8(%rcx),%xmm0
  5786c5:	movq   %xmm0,%rax
  5786ca:	and    %r14,%rax
  5786cd:	cmp    %r15,%rax
  5786d0:	setg   %cl
  5786d3:	sete   %dl
  5786d6:	test   %rax,%rax
  5786d9:	sete   %sil
  5786dd:	or     %dl,%sil
  5786e0:	xor    %eax,%eax
  5786e2:	or     %cl,%sil
  5786e5:	jne    578777 <quickjs_oxide::engine::vm::execute::binary_number_result+0x9b7>
  5786eb:	call   *0x2946e7(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5786f1:	movsd  0xe2e2f(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  5786f9:	call   *0x294859(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5786ff:	movsd  0xe2e21(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  578707:	addsd  %xmm0,%xmm3
  57870b:	xorpd  %xmm2,%xmm2
  57870f:	movapd %xmm0,%xmm1
  578713:	cmpltsd %xmm2,%xmm1
  578718:	andpd  %xmm1,%xmm3
  57871c:	andnpd %xmm0,%xmm1
  578720:	orpd   %xmm3,%xmm1
  578724:	ucomisd 0xe2eb4(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  57872c:	jae    57874e <quickjs_oxide::engine::vm::execute::binary_number_result+0x98e>
  57872e:	movapd %xmm1,%xmm0
  578732:	maxsd  0xe2dc6(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  57873a:	minsd  0xe2dc6(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  578742:	cvttsd2si %xmm0,%ecx
  578746:	xor    %eax,%eax
  578748:	ucomisd %xmm1,%xmm1
  57874c:	jmp    578774 <quickjs_oxide::engine::vm::execute::binary_number_result+0x9b4>
  57874e:	movsd  0xe2e92(%rip),%xmm0        # 65b5e8 <_fini+0x429c>
  578756:	addsd  %xmm1,%xmm0
  57875a:	xor    %eax,%eax
  57875c:	ucomisd %xmm0,%xmm0
  578760:	maxsd  0xe2d98(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  578768:	minsd  0xe2d98(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  578770:	cvttsd2si %xmm0,%ecx
  578774:	cmovnp %ecx,%eax
  578777:	or     %ebp,%eax
  578779:	jmp    578cb1 <quickjs_oxide::engine::vm::execute::binary_number_result+0xef1>
  57877e:	movsd  0xe2e62(%rip),%xmm0        # 65b5e8 <_fini+0x429c>
  578786:	addsd  %xmm1,%xmm0
  57878a:	xor    %ebp,%ebp
  57878c:	ucomisd %xmm0,%xmm0
  578790:	maxsd  0xe2d68(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  578798:	minsd  0xe2d68(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  5787a0:	cvttsd2si %xmm0,%eax
  5787a4:	cmovnp %eax,%ebp
  5787a7:	mov    %r12,%rcx
  5787aa:	cmpb   $0x0,(%rcx)
  5787ad:	jne    5787b9 <quickjs_oxide::engine::vm::execute::binary_number_result+0x9f9>
  5787af:	xorps  %xmm0,%xmm0
  5787b2:	cvtsi2sdl 0x4(%rcx),%xmm0
  5787b7:	jmp    5787be <quickjs_oxide::engine::vm::execute::binary_number_result+0x9fe>
  5787b9:	movsd  0x8(%rcx),%xmm0
  5787be:	movq   %xmm0,%rax
  5787c3:	and    %r14,%rax
  5787c6:	cmp    %r15,%rax
  5787c9:	setg   %cl
  5787cc:	sete   %dl
  5787cf:	test   %rax,%rax
  5787d2:	sete   %sil
  5787d6:	or     %dl,%sil
  5787d9:	xor    %eax,%eax
  5787db:	or     %cl,%sil
  5787de:	jne    578870 <quickjs_oxide::engine::vm::execute::binary_number_result+0xab0>
  5787e4:	call   *0x2945ee(%rip)        # 80cdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5787ea:	movsd  0xe2d36(%rip),%xmm1        # 65b528 <_fini+0x41dc>
  5787f2:	call   *0x294760(%rip)        # 80cf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5787f8:	movsd  0xe2d28(%rip),%xmm3        # 65b528 <_fini+0x41dc>
  578800:	addsd  %xmm0,%xmm3
  578804:	xorpd  %xmm2,%xmm2
  578808:	movapd %xmm0,%xmm1
  57880c:	cmpltsd %xmm2,%xmm1
  578811:	andpd  %xmm1,%xmm3
  578815:	andnpd %xmm0,%xmm1
  578819:	orpd   %xmm3,%xmm1
  57881d:	ucomisd 0xe2dbb(%rip),%xmm1        # 65b5e0 <_fini+0x4294>
  578825:	jae    578847 <quickjs_oxide::engine::vm::execute::binary_number_result+0xa87>
  578827:	movapd %xmm1,%xmm0
  57882b:	maxsd  0xe2ccd(%rip),%xmm0        # 65b500 <_fini+0x41b4>
  578833:	minsd  0xe2ccd(%rip),%xmm0        # 65b508 <_fini+0x41bc>
  57883b:	cvttsd2si %xmm0,%ecx
  57883f:	xor    %eax,%eax
  578841:	ucomisd %xmm1,%xmm1
  578845:	jmp    57886d <quickjs_oxide::engine::vm::execute::binary_number_result+0xaad>
  578847:	movsd  0xe2d99(%rip),%xmm0        # 65b5e8 <_fini+0x429c>
  57884f:	repnz
