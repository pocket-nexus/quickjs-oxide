
/tmp/oxide-six-build-numeric/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000577dc0 <quickjs_oxide::engine::vm::execute::binary_number_result>:
  577dc0:	push   %r15
  577dc2:	push   %r14
  577dc4:	push   %rbx
  577dc5:	sub    $0x50,%rsp
  577dc9:	movzbl %sil,%eax
  577dcd:	add    $0xffffff81,%eax
  577dd0:	cmp    $0x13,%eax
  577dd3:	ja     578e17 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1057>
  577dd9:	lea    0xf946c(%rip),%rsi        # 67124c <_fini+0x19dc0>
  577de0:	movslq (%rsi,%rax,4),%rax
  577de4:	add    %rsi,%rax
  577de7:	jmp    *%rax
  577de9:	cmpb   $0x0,(%rdx)
  577dec:	jne    577e15 <quickjs_oxide::engine::vm::execute::binary_number_result+0x55>
  577dee:	cvtsi2sdl 0x4(%rdx),%xmm0
  577df3:	cmpb   $0x0,(%rcx)
  577df6:	je     577e1f <quickjs_oxide::engine::vm::execute::binary_number_result+0x5f>
  577df8:	movsd  0x8(%rcx),%xmm1
  577dfd:	jmp    577e24 <quickjs_oxide::engine::vm::execute::binary_number_result+0x64>
  577dff:	cmpb   $0x0,(%rdx)
  577e02:	jne    577e32 <quickjs_oxide::engine::vm::execute::binary_number_result+0x72>
  577e04:	cvtsi2sdl 0x4(%rdx),%xmm0
  577e09:	cmpb   $0x0,(%rcx)
  577e0c:	je     577e3c <quickjs_oxide::engine::vm::execute::binary_number_result+0x7c>
  577e0e:	movsd  0x8(%rcx),%xmm1
  577e13:	jmp    577e41 <quickjs_oxide::engine::vm::execute::binary_number_result+0x81>
  577e15:	movsd  0x8(%rdx),%xmm0
  577e1a:	cmpb   $0x0,(%rcx)
  577e1d:	jne    577df8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x38>
  577e1f:	cvtsi2sdl 0x4(%rcx),%xmm1
  577e24:	ucomisd %xmm1,%xmm0
  577e28:	setnp  %al
  577e2b:	sete   %cl
  577e2e:	and    %al,%cl
  577e30:	jmp    577e4d <quickjs_oxide::engine::vm::execute::binary_number_result+0x8d>
  577e32:	movsd  0x8(%rdx),%xmm0
  577e37:	cmpb   $0x0,(%rcx)
  577e3a:	jne    577e0e <quickjs_oxide::engine::vm::execute::binary_number_result+0x4e>
  577e3c:	cvtsi2sdl 0x4(%rcx),%xmm1
  577e41:	ucomisd %xmm1,%xmm0
  577e45:	setp   %al
  577e48:	setne  %cl
  577e4b:	or     %al,%cl
  577e4d:	mov    %cl,0x1(%rdi)
  577e50:	mov    $0x2,%al
  577e52:	jmp    578e0b <quickjs_oxide::engine::vm::execute::binary_number_result+0x104b>
  577e57:	cmpl   $0x1,(%rdx)
  577e5a:	jne    578405 <quickjs_oxide::engine::vm::execute::binary_number_result+0x645>
  577e60:	movq   0x8(%rdx),%xmm0
  577e65:	movq   %xmm0,%rax
  577e6a:	movabs $0x7fffffffffffffff,%rdx
  577e74:	and    %rax,%rdx
  577e77:	movabs $0x7ff0000000000000,%rax
  577e81:	cmp    %rax,%rdx
  577e84:	setg   %al
  577e87:	sete   %sil
  577e8b:	test   %rdx,%rdx
  577e8e:	sete   %dl
  577e91:	or     %sil,%dl
  577e94:	xor    %ebx,%ebx
  577e96:	or     %al,%dl
  577e98:	jne    578408 <quickjs_oxide::engine::vm::execute::binary_number_result+0x648>
  577e9e:	mov    %rcx,%r14
  577ea1:	mov    %rdi,%r15
  577ea4:	call   *0x293f2e(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  577eaa:	movsd  0xe3676(%rip),%xmm1        # 65b528 <_fini+0x409c>
  577eb2:	call   *0x2940a0(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  577eb8:	movsd  0xe3668(%rip),%xmm3        # 65b528 <_fini+0x409c>
  577ec0:	addsd  %xmm0,%xmm3
  577ec4:	xorpd  %xmm2,%xmm2
  577ec8:	movapd %xmm0,%xmm1
  577ecc:	cmpltsd %xmm2,%xmm1
  577ed1:	andpd  %xmm1,%xmm3
  577ed5:	andnpd %xmm0,%xmm1
  577ed9:	orpd   %xmm3,%xmm1
  577edd:	ucomisd 0xe36fb(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  577ee5:	jae    578b1a <quickjs_oxide::engine::vm::execute::binary_number_result+0xd5a>
  577eeb:	movapd %xmm1,%xmm0
  577eef:	maxsd  0xe3609(%rip),%xmm0        # 65b500 <_fini+0x4074>
  577ef7:	minsd  0xe3609(%rip),%xmm0        # 65b508 <_fini+0x407c>
  577eff:	cvttsd2si %xmm0,%eax
  577f03:	xor    %ebx,%ebx
  577f05:	ucomisd %xmm1,%xmm1
  577f09:	jmp    578b40 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd80>
  577f0e:	cmpb   $0x0,(%rdx)
  577f11:	jne    5784ba <quickjs_oxide::engine::vm::execute::binary_number_result+0x6fa>
  577f17:	cvtsi2sdl 0x4(%rdx),%xmm0
  577f1c:	cmpb   $0x0,(%rcx)
  577f1f:	je     5784c8 <quickjs_oxide::engine::vm::execute::binary_number_result+0x708>
  577f25:	mov    %rdi,%rbx
  577f28:	movsd  0x8(%rcx),%xmm1
  577f2d:	jmp    5784d0 <quickjs_oxide::engine::vm::execute::binary_number_result+0x710>
  577f32:	cmpl   $0x1,(%rdx)
  577f35:	jne    578534 <quickjs_oxide::engine::vm::execute::binary_number_result+0x774>
  577f3b:	movq   0x8(%rdx),%xmm0
  577f40:	movq   %xmm0,%rax
  577f45:	movabs $0x7fffffffffffffff,%rdx
  577f4f:	and    %rax,%rdx
  577f52:	movabs $0x7ff0000000000000,%rax
  577f5c:	cmp    %rax,%rdx
  577f5f:	setg   %al
  577f62:	sete   %sil
  577f66:	test   %rdx,%rdx
  577f69:	sete   %dl
  577f6c:	or     %sil,%dl
  577f6f:	xor    %ebx,%ebx
  577f71:	or     %al,%dl
  577f73:	jne    578537 <quickjs_oxide::engine::vm::execute::binary_number_result+0x777>
  577f79:	mov    %rcx,%r14
  577f7c:	mov    %rdi,%r15
  577f7f:	call   *0x293e53(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  577f85:	movsd  0xe359b(%rip),%xmm1        # 65b528 <_fini+0x409c>
  577f8d:	call   *0x293fc5(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  577f93:	movsd  0xe358d(%rip),%xmm3        # 65b528 <_fini+0x409c>
  577f9b:	addsd  %xmm0,%xmm3
  577f9f:	xorpd  %xmm2,%xmm2
  577fa3:	movapd %xmm0,%xmm1
  577fa7:	cmpltsd %xmm2,%xmm1
  577fac:	andpd  %xmm1,%xmm3
  577fb0:	andnpd %xmm0,%xmm1
  577fb4:	orpd   %xmm3,%xmm1
  577fb8:	ucomisd 0xe3620(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  577fc0:	jae    578b8a <quickjs_oxide::engine::vm::execute::binary_number_result+0xdca>
  577fc6:	movapd %xmm1,%xmm0
  577fca:	maxsd  0xe352e(%rip),%xmm0        # 65b500 <_fini+0x4074>
  577fd2:	minsd  0xe352e(%rip),%xmm0        # 65b508 <_fini+0x407c>
  577fda:	cvttsd2si %xmm0,%eax
  577fde:	xor    %ebx,%ebx
  577fe0:	ucomisd %xmm1,%xmm1
  577fe4:	jmp    578bb0 <quickjs_oxide::engine::vm::execute::binary_number_result+0xdf0>
  577fe9:	cmpb   $0x0,(%rdx)
  577fec:	jne    5785e9 <quickjs_oxide::engine::vm::execute::binary_number_result+0x829>
  577ff2:	cvtsi2sdl 0x4(%rdx),%xmm0
  577ff7:	cmpb   $0x0,(%rcx)
  577ffa:	je     5785f7 <quickjs_oxide::engine::vm::execute::binary_number_result+0x837>
  578000:	movsd  0x8(%rcx),%xmm1
  578005:	mulsd  %xmm1,%xmm0
  578009:	jmp    578745 <quickjs_oxide::engine::vm::execute::binary_number_result+0x985>
  57800e:	cmpb   $0x0,(%rdx)
  578011:	jne    578605 <quickjs_oxide::engine::vm::execute::binary_number_result+0x845>
  578017:	cvtsi2sdl 0x4(%rdx),%xmm0
  57801c:	cmpb   $0x0,(%rcx)
  57801f:	je     578613 <quickjs_oxide::engine::vm::execute::binary_number_result+0x853>
  578025:	movsd  0x8(%rcx),%xmm1
  57802a:	jmp    578618 <quickjs_oxide::engine::vm::execute::binary_number_result+0x858>
  57802f:	cmpb   $0x0,(%rdx)
  578032:	jne    578621 <quickjs_oxide::engine::vm::execute::binary_number_result+0x861>
  578038:	cvtsi2sdl 0x4(%rdx),%xmm0
  57803d:	cmpb   $0x0,(%rcx)
  578040:	je     57862f <quickjs_oxide::engine::vm::execute::binary_number_result+0x86f>
  578046:	movsd  0x8(%rcx),%xmm1
  57804b:	divsd  %xmm1,%xmm0
  57804f:	jmp    578745 <quickjs_oxide::engine::vm::execute::binary_number_result+0x985>
  578054:	cmpl   $0x1,(%rdx)
  578057:	jne    57863d <quickjs_oxide::engine::vm::execute::binary_number_result+0x87d>
  57805d:	movq   0x8(%rdx),%xmm0
  578062:	movq   %xmm0,%rax
  578067:	movabs $0x7fffffffffffffff,%rdx
  578071:	and    %rax,%rdx
  578074:	movabs $0x7ff0000000000000,%rax
  57807e:	cmp    %rax,%rdx
  578081:	setg   %al
  578084:	sete   %sil
  578088:	test   %rdx,%rdx
  57808b:	sete   %dl
  57808e:	or     %sil,%dl
  578091:	xor    %ebx,%ebx
  578093:	or     %al,%dl
  578095:	jne    578640 <quickjs_oxide::engine::vm::execute::binary_number_result+0x880>
  57809b:	mov    %rcx,%r14
  57809e:	mov    %rdi,%r15
  5780a1:	call   *0x293d31(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5780a7:	movsd  0xe3479(%rip),%xmm1        # 65b528 <_fini+0x409c>
  5780af:	call   *0x293ea3(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5780b5:	movsd  0xe346b(%rip),%xmm3        # 65b528 <_fini+0x409c>
  5780bd:	addsd  %xmm0,%xmm3
  5780c1:	xorpd  %xmm2,%xmm2
  5780c5:	movapd %xmm0,%xmm1
  5780c9:	cmpltsd %xmm2,%xmm1
  5780ce:	andpd  %xmm1,%xmm3
  5780d2:	andnpd %xmm0,%xmm1
  5780d6:	orpd   %xmm3,%xmm1
  5780da:	ucomisd 0xe34fe(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  5780e2:	jae    578bff <quickjs_oxide::engine::vm::execute::binary_number_result+0xe3f>
  5780e8:	movapd %xmm1,%xmm0
  5780ec:	maxsd  0xe340c(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5780f4:	minsd  0xe340c(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5780fc:	cvttsd2si %xmm0,%eax
  578100:	xor    %ebx,%ebx
  578102:	ucomisd %xmm1,%xmm1
  578106:	jmp    578c25 <quickjs_oxide::engine::vm::execute::binary_number_result+0xe65>
  57810b:	testb  $0x1,(%rdx)
  57810e:	je     5786f5 <quickjs_oxide::engine::vm::execute::binary_number_result+0x935>
  578114:	movsd  0x8(%rdx),%xmm0
  578119:	movzbl (%rcx),%esi
  57811c:	test   $0x1,%sil
  578120:	jne    578713 <quickjs_oxide::engine::vm::execute::binary_number_result+0x953>
  578126:	cvtsi2sdl 0x4(%rcx),%xmm1
  57812b:	jmp    578718 <quickjs_oxide::engine::vm::execute::binary_number_result+0x958>
  578130:	testb  $0x1,(%rdx)
  578133:	je     57871e <quickjs_oxide::engine::vm::execute::binary_number_result+0x95e>
  578139:	movsd  0x8(%rdx),%xmm0
  57813e:	movzbl (%rcx),%esi
  578141:	test   $0x1,%sil
  578145:	jne    57873c <quickjs_oxide::engine::vm::execute::binary_number_result+0x97c>
  57814b:	cvtsi2sdl 0x4(%rcx),%xmm1
  578150:	jmp    578741 <quickjs_oxide::engine::vm::execute::binary_number_result+0x981>
  578155:	cmpb   $0x0,(%rdx)
  578158:	jne    5787a4 <quickjs_oxide::engine::vm::execute::binary_number_result+0x9e4>
  57815e:	cvtsi2sdl 0x4(%rdx),%xmm0
  578163:	cmpb   $0x0,(%rcx)
  578166:	je     5787b2 <quickjs_oxide::engine::vm::execute::binary_number_result+0x9f2>
  57816c:	movsd  0x8(%rcx),%xmm1
  578171:	ucomisd %xmm1,%xmm0
  578175:	jmp    578aef <quickjs_oxide::engine::vm::execute::binary_number_result+0xd2f>
  57817a:	cmpb   $0x0,(%rdx)
  57817d:	jne    5787c0 <quickjs_oxide::engine::vm::execute::binary_number_result+0xa00>
  578183:	cvtsi2sdl 0x4(%rdx),%xmm0
  578188:	cmpb   $0x0,(%rcx)
  57818b:	je     5787ce <quickjs_oxide::engine::vm::execute::binary_number_result+0xa0e>
  578191:	mov    %rdi,%rbx
  578194:	movsd  0x8(%rcx),%xmm1
  578199:	jmp    5787d6 <quickjs_oxide::engine::vm::execute::binary_number_result+0xa16>
  57819e:	cmpl   $0x1,(%rdx)
  5781a1:	jne    578891 <quickjs_oxide::engine::vm::execute::binary_number_result+0xad1>
  5781a7:	movq   0x8(%rdx),%xmm0
  5781ac:	movq   %xmm0,%rax
  5781b1:	movabs $0x7fffffffffffffff,%rdx
  5781bb:	and    %rax,%rdx
  5781be:	movabs $0x7ff0000000000000,%rax
  5781c8:	cmp    %rax,%rdx
  5781cb:	setg   %al
  5781ce:	sete   %sil
  5781d2:	test   %rdx,%rdx
  5781d5:	sete   %dl
  5781d8:	or     %sil,%dl
  5781db:	xor    %ebx,%ebx
  5781dd:	or     %al,%dl
  5781df:	jne    578894 <quickjs_oxide::engine::vm::execute::binary_number_result+0xad4>
  5781e5:	mov    %rcx,%r14
  5781e8:	mov    %rdi,%r15
  5781eb:	call   *0x293be7(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5781f1:	movsd  0xe332f(%rip),%xmm1        # 65b528 <_fini+0x409c>
  5781f9:	call   *0x293d59(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5781ff:	movsd  0xe3321(%rip),%xmm3        # 65b528 <_fini+0x409c>
  578207:	addsd  %xmm0,%xmm3
  57820b:	xorpd  %xmm2,%xmm2
  57820f:	movapd %xmm0,%xmm1
  578213:	cmpltsd %xmm2,%xmm1
  578218:	andpd  %xmm1,%xmm3
  57821c:	andnpd %xmm0,%xmm1
  578220:	orpd   %xmm3,%xmm1
  578224:	ucomisd 0xe33b4(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  57822c:	jae    578c72 <quickjs_oxide::engine::vm::execute::binary_number_result+0xeb2>
  578232:	movapd %xmm1,%xmm0
  578236:	maxsd  0xe32c2(%rip),%xmm0        # 65b500 <_fini+0x4074>
  57823e:	minsd  0xe32c2(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578246:	cvttsd2si %xmm0,%eax
  57824a:	xor    %ebx,%ebx
  57824c:	ucomisd %xmm1,%xmm1
  578250:	jmp    578c98 <quickjs_oxide::engine::vm::execute::binary_number_result+0xed8>
  578255:	cmpl   $0x1,(%rdx)
  578258:	jne    578949 <quickjs_oxide::engine::vm::execute::binary_number_result+0xb89>
  57825e:	movq   0x8(%rdx),%xmm0
  578263:	movq   %xmm0,%rax
  578268:	movabs $0x7fffffffffffffff,%rdx
  578272:	and    %rax,%rdx
  578275:	movabs $0x7ff0000000000000,%rax
  57827f:	cmp    %rax,%rdx
  578282:	setg   %al
  578285:	sete   %sil
  578289:	test   %rdx,%rdx
  57828c:	sete   %dl
  57828f:	or     %sil,%dl
  578292:	xor    %ebx,%ebx
  578294:	or     %al,%dl
  578296:	jne    57894c <quickjs_oxide::engine::vm::execute::binary_number_result+0xb8c>
  57829c:	mov    %rcx,%r14
  57829f:	mov    %rdi,%r15
  5782a2:	call   *0x293b30(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5782a8:	movsd  0xe3278(%rip),%xmm1        # 65b528 <_fini+0x409c>
  5782b0:	call   *0x293ca2(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5782b6:	movsd  0xe326a(%rip),%xmm3        # 65b528 <_fini+0x409c>
  5782be:	addsd  %xmm0,%xmm3
  5782c2:	xorpd  %xmm2,%xmm2
  5782c6:	movapd %xmm0,%xmm1
  5782ca:	cmpltsd %xmm2,%xmm1
  5782cf:	andpd  %xmm1,%xmm3
  5782d3:	andnpd %xmm0,%xmm1
  5782d7:	orpd   %xmm3,%xmm1
  5782db:	ucomisd 0xe32fd(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  5782e3:	jae    578ce5 <quickjs_oxide::engine::vm::execute::binary_number_result+0xf25>
  5782e9:	movapd %xmm1,%xmm0
  5782ed:	maxsd  0xe320b(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5782f5:	minsd  0xe320b(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5782fd:	cvttsd2si %xmm0,%eax
  578301:	xor    %ebx,%ebx
  578303:	ucomisd %xmm1,%xmm1
  578307:	jmp    578d0b <quickjs_oxide::engine::vm::execute::binary_number_result+0xf4b>
  57830c:	cmpl   $0x1,(%rdx)
  57830f:	jne    5789fe <quickjs_oxide::engine::vm::execute::binary_number_result+0xc3e>
  578315:	movq   0x8(%rdx),%xmm0
  57831a:	movq   %xmm0,%rax
  57831f:	movabs $0x7fffffffffffffff,%rdx
  578329:	and    %rax,%rdx
  57832c:	movabs $0x7ff0000000000000,%rax
  578336:	cmp    %rax,%rdx
  578339:	setg   %al
  57833c:	sete   %sil
  578340:	test   %rdx,%rdx
  578343:	sete   %dl
  578346:	or     %sil,%dl
  578349:	xor    %ebx,%ebx
  57834b:	or     %al,%dl
  57834d:	jne    578a01 <quickjs_oxide::engine::vm::execute::binary_number_result+0xc41>
  578353:	mov    %rcx,%r14
  578356:	mov    %rdi,%r15
  578359:	call   *0x293a79(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  57835f:	movsd  0xe31c1(%rip),%xmm1        # 65b528 <_fini+0x409c>
  578367:	call   *0x293beb(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  57836d:	movsd  0xe31b3(%rip),%xmm3        # 65b528 <_fini+0x409c>
  578375:	addsd  %xmm0,%xmm3
  578379:	xorpd  %xmm2,%xmm2
  57837d:	movapd %xmm0,%xmm1
  578381:	cmpltsd %xmm2,%xmm1
  578386:	andpd  %xmm1,%xmm3
  57838a:	andnpd %xmm0,%xmm1
  57838e:	orpd   %xmm3,%xmm1
  578392:	ucomisd 0xe3246(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  57839a:	jae    578d5a <quickjs_oxide::engine::vm::execute::binary_number_result+0xf9a>
  5783a0:	movapd %xmm1,%xmm0
  5783a4:	maxsd  0xe3154(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5783ac:	minsd  0xe3154(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5783b4:	cvttsd2si %xmm0,%eax
  5783b8:	xor    %ebx,%ebx
  5783ba:	ucomisd %xmm1,%xmm1
  5783be:	jmp    578d80 <quickjs_oxide::engine::vm::execute::binary_number_result+0xfc0>
  5783c3:	cmpb   $0x0,(%rdx)
  5783c6:	jne    578ab6 <quickjs_oxide::engine::vm::execute::binary_number_result+0xcf6>
  5783cc:	cvtsi2sdl 0x4(%rdx),%xmm0
  5783d1:	cmpb   $0x0,(%rcx)
  5783d4:	je     578ac4 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd04>
  5783da:	movsd  0x8(%rcx),%xmm1
  5783df:	jmp    578ac9 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd09>
  5783e4:	cmpb   $0x0,(%rdx)
  5783e7:	jne    578ad8 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd18>
  5783ed:	cvtsi2sdl 0x4(%rdx),%xmm0
  5783f2:	cmpb   $0x0,(%rcx)
  5783f5:	je     578ae6 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd26>
  5783fb:	movsd  0x8(%rcx),%xmm1
  578400:	jmp    578aeb <quickjs_oxide::engine::vm::execute::binary_number_result+0xd2b>
  578405:	mov    0x4(%rdx),%ebx
  578408:	cmpl   $0x1,(%rcx)
  57840b:	jne    578b52 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd92>
  578411:	movq   0x8(%rcx),%xmm0
  578416:	movq   %xmm0,%rax
  57841b:	movabs $0x7fffffffffffffff,%rcx
  578425:	and    %rax,%rcx
  578428:	movabs $0x7ff0000000000000,%rax
  578432:	cmp    %rax,%rcx
  578435:	setg   %dl
  578438:	sete   %al
  57843b:	test   %rcx,%rcx
  57843e:	sete   %cl
  578441:	or     %al,%cl
  578443:	xor    %eax,%eax
  578445:	or     %dl,%cl
  578447:	jne    578b83 <quickjs_oxide::engine::vm::execute::binary_number_result+0xdc3>
  57844d:	mov    %rdi,%r14
  578450:	call   *0x293982(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578456:	movsd  0xe30ca(%rip),%xmm1        # 65b528 <_fini+0x409c>
  57845e:	call   *0x293af4(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  578464:	movsd  0xe30bc(%rip),%xmm3        # 65b528 <_fini+0x409c>
  57846c:	addsd  %xmm0,%xmm3
  578470:	xorpd  %xmm2,%xmm2
  578474:	movapd %xmm0,%xmm1
  578478:	cmpltsd %xmm2,%xmm1
  57847d:	andpd  %xmm1,%xmm3
  578481:	andnpd %xmm0,%xmm1
  578485:	orpd   %xmm3,%xmm1
  578489:	ucomisd 0xe314f(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  578491:	jae    578b57 <quickjs_oxide::engine::vm::execute::binary_number_result+0xd97>
  578497:	movapd %xmm1,%xmm0
  57849b:	maxsd  0xe305d(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5784a3:	minsd  0xe305d(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5784ab:	cvttsd2si %xmm0,%ecx
  5784af:	xor    %eax,%eax
  5784b1:	ucomisd %xmm1,%xmm1
  5784b5:	jmp    578b7d <quickjs_oxide::engine::vm::execute::binary_number_result+0xdbd>
  5784ba:	movsd  0x8(%rdx),%xmm0
  5784bf:	cmpb   $0x0,(%rcx)
  5784c2:	jne    577f25 <quickjs_oxide::engine::vm::execute::binary_number_result+0x165>
  5784c8:	mov    %rdi,%rbx
  5784cb:	cvtsi2sdl 0x4(%rcx),%xmm1
  5784d0:	call   *0x293a82(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5784d6:	movapd %xmm0,%xmm1
  5784da:	maxsd  0xe301e(%rip),%xmm1        # 65b500 <_fini+0x4074>
  5784e2:	minsd  0xe301e(%rip),%xmm1        # 65b508 <_fini+0x407c>
  5784ea:	cvttsd2si %xmm1,%ecx
  5784ee:	xor    %eax,%eax
  5784f0:	ucomisd %xmm0,%xmm0
  5784f4:	cmovnp %ecx,%eax
  5784f7:	xorps  %xmm1,%xmm1
  5784fa:	cvtsi2sd %eax,%xmm1
  5784fe:	ucomisd %xmm1,%xmm0
  578502:	jne    578527 <quickjs_oxide::engine::vm::execute::binary_number_result+0x767>
  578504:	jp     578527 <quickjs_oxide::engine::vm::execute::binary_number_result+0x767>
  578506:	xorpd  %xmm1,%xmm1
  57850a:	ucomisd %xmm1,%xmm0
  57850e:	jne    57851c <quickjs_oxide::engine::vm::execute::binary_number_result+0x75c>
  578510:	jp     57851c <quickjs_oxide::engine::vm::execute::binary_number_result+0x75c>
  578512:	movq   %xmm0,%rcx
  578517:	test   %rcx,%rcx
  57851a:	jne    578527 <quickjs_oxide::engine::vm::execute::binary_number_result+0x767>
  57851c:	mov    %rbx,%rdi
  57851f:	mov    %eax,0x4(%rbx)
  578522:	jmp    578e00 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1040>
  578527:	mov    %rbx,%rdi
  57852a:	movsd  %xmm0,0x8(%rbx)
  57852f:	jmp    578e09 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1049>
  578534:	mov    0x4(%rdx),%ebx
  578537:	cmpl   $0x1,(%rcx)
  57853a:	jne    578bc2 <quickjs_oxide::engine::vm::execute::binary_number_result+0xe02>
  578540:	movq   0x8(%rcx),%xmm0
  578545:	movq   %xmm0,%rax
  57854a:	movabs $0x7fffffffffffffff,%rcx
  578554:	and    %rax,%rcx
  578557:	movabs $0x7ff0000000000000,%rax
  578561:	cmp    %rax,%rcx
  578564:	setg   %dl
  578567:	sete   %al
  57856a:	test   %rcx,%rcx
  57856d:	sete   %cl
  578570:	or     %al,%cl
  578572:	xor    %eax,%eax
  578574:	or     %dl,%cl
  578576:	jne    578bc5 <quickjs_oxide::engine::vm::execute::binary_number_result+0xe05>
  57857c:	mov    %rdi,%r14
  57857f:	call   *0x293853(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578585:	movsd  0xe2f9b(%rip),%xmm1        # 65b528 <_fini+0x409c>
  57858d:	call   *0x2939c5(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  578593:	movsd  0xe2f8d(%rip),%xmm3        # 65b528 <_fini+0x409c>
  57859b:	addsd  %xmm0,%xmm3
  57859f:	xorpd  %xmm2,%xmm2
  5785a3:	movapd %xmm0,%xmm1
  5785a7:	cmpltsd %xmm2,%xmm1
  5785ac:	andpd  %xmm1,%xmm3
  5785b0:	andnpd %xmm0,%xmm1
  5785b4:	orpd   %xmm3,%xmm1
  5785b8:	ucomisd 0xe3020(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  5785c0:	jae    578bcc <quickjs_oxide::engine::vm::execute::binary_number_result+0xe0c>
  5785c6:	movapd %xmm1,%xmm0
  5785ca:	maxsd  0xe2f2e(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5785d2:	minsd  0xe2f2e(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5785da:	cvttsd2si %xmm0,%ecx
  5785de:	xor    %eax,%eax
  5785e0:	ucomisd %xmm1,%xmm1
  5785e4:	jmp    578bf2 <quickjs_oxide::engine::vm::execute::binary_number_result+0xe32>
  5785e9:	movsd  0x8(%rdx),%xmm0
  5785ee:	cmpb   $0x0,(%rcx)
  5785f1:	jne    578000 <quickjs_oxide::engine::vm::execute::binary_number_result+0x240>
  5785f7:	cvtsi2sdl 0x4(%rcx),%xmm1
  5785fc:	mulsd  %xmm1,%xmm0
  578600:	jmp    578745 <quickjs_oxide::engine::vm::execute::binary_number_result+0x985>
  578605:	movsd  0x8(%rdx),%xmm0
  57860a:	cmpb   $0x0,(%rcx)
  57860d:	jne    578025 <quickjs_oxide::engine::vm::execute::binary_number_result+0x265>
  578613:	cvtsi2sdl 0x4(%rcx),%xmm1
  578618:	ucomisd %xmm0,%xmm1
  57861c:	jmp    578acd <quickjs_oxide::engine::vm::execute::binary_number_result+0xd0d>
  578621:	movsd  0x8(%rdx),%xmm0
  578626:	cmpb   $0x0,(%rcx)
  578629:	jne    578046 <quickjs_oxide::engine::vm::execute::binary_number_result+0x286>
  57862f:	cvtsi2sdl 0x4(%rcx),%xmm1
  578634:	divsd  %xmm1,%xmm0
  578638:	jmp    578745 <quickjs_oxide::engine::vm::execute::binary_number_result+0x985>
  57863d:	mov    0x4(%rdx),%ebx
  578640:	cmpl   $0x1,(%rcx)
  578643:	jne    578c37 <quickjs_oxide::engine::vm::execute::binary_number_result+0xe77>
  578649:	movq   0x8(%rcx),%xmm0
  57864e:	movq   %xmm0,%rax
  578653:	movabs $0x7fffffffffffffff,%rcx
  57865d:	and    %rax,%rcx
  578660:	movabs $0x7ff0000000000000,%rax
  57866a:	cmp    %rax,%rcx
  57866d:	setg   %al
  578670:	sete   %dl
  578673:	test   %rcx,%rcx
  578676:	sete   %sil
  57867a:	or     %dl,%sil
  57867d:	xor    %ecx,%ecx
  57867f:	or     %al,%sil
  578682:	jne    578c68 <quickjs_oxide::engine::vm::execute::binary_number_result+0xea8>
  578688:	mov    %rdi,%r14
  57868b:	call   *0x293747(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578691:	movsd  0xe2e8f(%rip),%xmm1        # 65b528 <_fini+0x409c>
  578699:	call   *0x2938b9(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  57869f:	movsd  0xe2e81(%rip),%xmm3        # 65b528 <_fini+0x409c>
  5786a7:	addsd  %xmm0,%xmm3
  5786ab:	xorpd  %xmm2,%xmm2
  5786af:	movapd %xmm0,%xmm1
  5786b3:	cmpltsd %xmm2,%xmm1
  5786b8:	andpd  %xmm1,%xmm3
  5786bc:	andnpd %xmm0,%xmm1
  5786c0:	orpd   %xmm3,%xmm1
  5786c4:	ucomisd 0xe2f14(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  5786cc:	jae    578c3c <quickjs_oxide::engine::vm::execute::binary_number_result+0xe7c>
  5786d2:	movapd %xmm1,%xmm0
  5786d6:	maxsd  0xe2e22(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5786de:	minsd  0xe2e22(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5786e6:	cvttsd2si %xmm0,%eax
  5786ea:	xor    %ecx,%ecx
  5786ec:	ucomisd %xmm1,%xmm1
  5786f0:	jmp    578c62 <quickjs_oxide::engine::vm::execute::binary_number_result+0xea2>
  5786f5:	movzbl (%rcx),%esi
  5786f8:	mov    0x4(%rdx),%edx
  5786fb:	test   $0x1,%sil
  5786ff:	je     578afa <quickjs_oxide::engine::vm::execute::binary_number_result+0xd3a>
  578705:	cvtsi2sd %edx,%xmm0
  578709:	test   $0x1,%sil
  57870d:	je     578126 <quickjs_oxide::engine::vm::execute::binary_number_result+0x366>
  578713:	movsd  0x8(%rcx),%xmm1
  578718:	addsd  %xmm1,%xmm0
  57871c:	jmp    578745 <quickjs_oxide::engine::vm::execute::binary_number_result+0x985>
  57871e:	movzbl (%rcx),%esi
  578721:	mov    0x4(%rdx),%edx
  578724:	test   $0x1,%sil
  578728:	je     578b0a <quickjs_oxide::engine::vm::execute::binary_number_result+0xd4a>
  57872e:	cvtsi2sd %edx,%xmm0
  578732:	test   $0x1,%sil
  578736:	je     57814b <quickjs_oxide::engine::vm::execute::binary_number_result+0x38b>
  57873c:	movsd  0x8(%rcx),%xmm1
  578741:	subsd  %xmm1,%xmm0
  578745:	movapd %xmm0,%xmm1
  578749:	maxsd  0xe2daf(%rip),%xmm1        # 65b500 <_fini+0x4074>
  578751:	minsd  0xe2daf(%rip),%xmm1        # 65b508 <_fini+0x407c>
  578759:	cvttsd2si %xmm1,%ecx
  57875d:	xor    %eax,%eax
  57875f:	ucomisd %xmm0,%xmm0
  578763:	cmovnp %ecx,%eax
  578766:	xorps  %xmm1,%xmm1
  578769:	cvtsi2sd %eax,%xmm1
  57876d:	ucomisd %xmm1,%xmm0
  578771:	jne    578e04 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1044>
  578777:	jp     578e04 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1044>
  57877d:	xorpd  %xmm1,%xmm1
  578781:	ucomisd %xmm1,%xmm0
  578785:	jne    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  57878b:	jp     578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578791:	movq   %xmm0,%rcx
  578796:	test   %rcx,%rcx
  578799:	je     578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  57879f:	jmp    578e04 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1044>
  5787a4:	movsd  0x8(%rdx),%xmm0
  5787a9:	cmpb   $0x0,(%rcx)
  5787ac:	jne    57816c <quickjs_oxide::engine::vm::execute::binary_number_result+0x3ac>
  5787b2:	cvtsi2sdl 0x4(%rcx),%xmm1
  5787b7:	ucomisd %xmm1,%xmm0
  5787bb:	jmp    578aef <quickjs_oxide::engine::vm::execute::binary_number_result+0xd2f>
  5787c0:	movsd  0x8(%rdx),%xmm0
  5787c5:	cmpb   $0x0,(%rcx)
  5787c8:	jne    578191 <quickjs_oxide::engine::vm::execute::binary_number_result+0x3d1>
  5787ce:	mov    %rdi,%rbx
  5787d1:	cvtsi2sdl 0x4(%rcx),%xmm1
  5787d6:	movq   %xmm1,%rax
  5787db:	movabs $0x7fffffffffffffff,%r14
  5787e5:	and    %rax,%r14
  5787e8:	movapd %xmm0,0x10(%rsp)
  5787ee:	call   *0x2937dc(%rip)        # 80bfd0 <pow@GLIBC_2.29>
  5787f4:	movapd 0x10(%rsp),%xmm2
  5787fa:	movabs $0x7ff0000000000000,%rax
  578804:	andpd  0xdfc14(%rip),%xmm2        # 658420 <_fini+0xf94>
  57880c:	movapd %xmm0,%xmm1
  578810:	cmp    %rax,%r14
  578813:	jl     57881d <quickjs_oxide::engine::vm::execute::binary_number_result+0xa5d>
  578815:	movsd  0xe2cab(%rip),%xmm1        # 65b4c8 <_fini+0x403c>
  57881d:	cmpeqsd 0xe2d0a(%rip),%xmm2        # 65b530 <_fini+0x40a4>
  578826:	andpd  %xmm2,%xmm1
  57882a:	andnpd %xmm0,%xmm2
  57882e:	orpd   %xmm1,%xmm2
  578832:	movapd %xmm2,%xmm0
  578836:	maxsd  0xe2cc2(%rip),%xmm0        # 65b500 <_fini+0x4074>
  57883e:	minsd  0xe2cc2(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578846:	cvttsd2si %xmm0,%ecx
  57884a:	xor    %eax,%eax
  57884c:	ucomisd %xmm2,%xmm2
  578850:	cmovnp %ecx,%eax
  578853:	xorps  %xmm0,%xmm0
  578856:	cvtsi2sd %eax,%xmm0
  57885a:	ucomisd %xmm0,%xmm2
  57885e:	mov    %rbx,%rdi
  578861:	jne    578887 <quickjs_oxide::engine::vm::execute::binary_number_result+0xac7>
  578863:	jp     578887 <quickjs_oxide::engine::vm::execute::binary_number_result+0xac7>
  578865:	xorpd  %xmm0,%xmm0
  578869:	ucomisd %xmm0,%xmm2
  57886d:	jne    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578873:	jp     578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578879:	movq   %xmm2,%rcx
  57887e:	test   %rcx,%rcx
  578881:	je     578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578887:	movsd  %xmm2,0x8(%rdi)
  57888c:	jmp    578e09 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1049>
  578891:	mov    0x4(%rdx),%ebx
  578894:	cmpl   $0x1,(%rcx)
  578897:	jne    578caa <quickjs_oxide::engine::vm::execute::binary_number_result+0xeea>
  57889d:	movq   0x8(%rcx),%xmm0
  5788a2:	movq   %xmm0,%rax
  5788a7:	movabs $0x7fffffffffffffff,%rcx
  5788b1:	and    %rax,%rcx
  5788b4:	movabs $0x7ff0000000000000,%rax
  5788be:	cmp    %rax,%rcx
  5788c1:	setg   %al
  5788c4:	sete   %dl
  5788c7:	test   %rcx,%rcx
  5788ca:	sete   %sil
  5788ce:	or     %dl,%sil
  5788d1:	xor    %ecx,%ecx
  5788d3:	or     %al,%sil
  5788d6:	jne    578cdb <quickjs_oxide::engine::vm::execute::binary_number_result+0xf1b>
  5788dc:	mov    %rdi,%r14
  5788df:	call   *0x2934f3(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  5788e5:	movsd  0xe2c3b(%rip),%xmm1        # 65b528 <_fini+0x409c>
  5788ed:	call   *0x293665(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5788f3:	movsd  0xe2c2d(%rip),%xmm3        # 65b528 <_fini+0x409c>
  5788fb:	addsd  %xmm0,%xmm3
  5788ff:	xorpd  %xmm2,%xmm2
  578903:	movapd %xmm0,%xmm1
  578907:	cmpltsd %xmm2,%xmm1
  57890c:	andpd  %xmm1,%xmm3
  578910:	andnpd %xmm0,%xmm1
  578914:	orpd   %xmm3,%xmm1
  578918:	ucomisd 0xe2cc0(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  578920:	jae    578caf <quickjs_oxide::engine::vm::execute::binary_number_result+0xeef>
  578926:	movapd %xmm1,%xmm0
  57892a:	maxsd  0xe2bce(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578932:	minsd  0xe2bce(%rip),%xmm0        # 65b508 <_fini+0x407c>
  57893a:	cvttsd2si %xmm0,%eax
  57893e:	xor    %ecx,%ecx
  578940:	ucomisd %xmm1,%xmm1
  578944:	jmp    578cd5 <quickjs_oxide::engine::vm::execute::binary_number_result+0xf15>
  578949:	mov    0x4(%rdx),%ebx
  57894c:	cmpl   $0x1,(%rcx)
  57894f:	jne    578d1d <quickjs_oxide::engine::vm::execute::binary_number_result+0xf5d>
  578955:	movq   0x8(%rcx),%xmm0
  57895a:	movq   %xmm0,%rax
  57895f:	movabs $0x7fffffffffffffff,%rcx
  578969:	and    %rax,%rcx
  57896c:	movabs $0x7ff0000000000000,%rax
  578976:	cmp    %rax,%rcx
  578979:	setg   %dl
  57897c:	sete   %al
  57897f:	test   %rcx,%rcx
  578982:	sete   %cl
  578985:	or     %al,%cl
  578987:	xor    %eax,%eax
  578989:	or     %dl,%cl
  57898b:	jne    578d20 <quickjs_oxide::engine::vm::execute::binary_number_result+0xf60>
  578991:	mov    %rdi,%r14
  578994:	call   *0x29343e(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  57899a:	movsd  0xe2b86(%rip),%xmm1        # 65b528 <_fini+0x409c>
  5789a2:	call   *0x2935b0(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  5789a8:	movsd  0xe2b78(%rip),%xmm3        # 65b528 <_fini+0x409c>
  5789b0:	addsd  %xmm0,%xmm3
  5789b4:	xorpd  %xmm2,%xmm2
  5789b8:	movapd %xmm0,%xmm1
  5789bc:	cmpltsd %xmm2,%xmm1
  5789c1:	andpd  %xmm1,%xmm3
  5789c5:	andnpd %xmm0,%xmm1
  5789c9:	orpd   %xmm3,%xmm1
  5789cd:	ucomisd 0xe2c0b(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  5789d5:	jae    578d27 <quickjs_oxide::engine::vm::execute::binary_number_result+0xf67>
  5789db:	movapd %xmm1,%xmm0
  5789df:	maxsd  0xe2b19(%rip),%xmm0        # 65b500 <_fini+0x4074>
  5789e7:	minsd  0xe2b19(%rip),%xmm0        # 65b508 <_fini+0x407c>
  5789ef:	cvttsd2si %xmm0,%ecx
  5789f3:	xor    %eax,%eax
  5789f5:	ucomisd %xmm1,%xmm1
  5789f9:	jmp    578d4d <quickjs_oxide::engine::vm::execute::binary_number_result+0xf8d>
  5789fe:	mov    0x4(%rdx),%ebx
  578a01:	cmpl   $0x1,(%rcx)
  578a04:	jne    578d92 <quickjs_oxide::engine::vm::execute::binary_number_result+0xfd2>
  578a0a:	movq   0x8(%rcx),%xmm0
  578a0f:	movq   %xmm0,%rax
  578a14:	movabs $0x7fffffffffffffff,%rcx
  578a1e:	and    %rax,%rcx
  578a21:	movabs $0x7ff0000000000000,%rax
  578a2b:	cmp    %rax,%rcx
  578a2e:	setg   %al
  578a31:	sete   %dl
  578a34:	test   %rcx,%rcx
  578a37:	sete   %sil
  578a3b:	or     %dl,%sil
  578a3e:	xor    %ecx,%ecx
  578a40:	or     %al,%sil
  578a43:	jne    578dc3 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1003>
  578a49:	mov    %rdi,%r14
  578a4c:	call   *0x293386(%rip)        # 80bdd8 <_GLOBAL_OFFSET_TABLE_+0xd0>
  578a52:	movsd  0xe2ace(%rip),%xmm1        # 65b528 <_fini+0x409c>
  578a5a:	call   *0x2934f8(%rip)        # 80bf58 <_GLOBAL_OFFSET_TABLE_+0x250>
  578a60:	movsd  0xe2ac0(%rip),%xmm3        # 65b528 <_fini+0x409c>
  578a68:	addsd  %xmm0,%xmm3
  578a6c:	xorpd  %xmm2,%xmm2
  578a70:	movapd %xmm0,%xmm1
  578a74:	cmpltsd %xmm2,%xmm1
  578a79:	andpd  %xmm1,%xmm3
  578a7d:	andnpd %xmm0,%xmm1
  578a81:	orpd   %xmm3,%xmm1
  578a85:	ucomisd 0xe2b53(%rip),%xmm1        # 65b5e0 <_fini+0x4154>
  578a8d:	jae    578d97 <quickjs_oxide::engine::vm::execute::binary_number_result+0xfd7>
  578a93:	movapd %xmm1,%xmm0
  578a97:	maxsd  0xe2a61(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578a9f:	minsd  0xe2a61(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578aa7:	cvttsd2si %xmm0,%eax
  578aab:	xor    %ecx,%ecx
  578aad:	ucomisd %xmm1,%xmm1
  578ab1:	jmp    578dbd <quickjs_oxide::engine::vm::execute::binary_number_result+0xffd>
  578ab6:	movsd  0x8(%rdx),%xmm0
  578abb:	cmpb   $0x0,(%rcx)
  578abe:	jne    5783da <quickjs_oxide::engine::vm::execute::binary_number_result+0x61a>
  578ac4:	cvtsi2sdl 0x4(%rcx),%xmm1
  578ac9:	ucomisd %xmm1,%xmm0
  578acd:	setae  0x1(%rdi)
  578ad1:	mov    $0x2,%al
  578ad3:	jmp    578e0b <quickjs_oxide::engine::vm::execute::binary_number_result+0x104b>
  578ad8:	movsd  0x8(%rdx),%xmm0
  578add:	cmpb   $0x0,(%rcx)
  578ae0:	jne    5783fb <quickjs_oxide::engine::vm::execute::binary_number_result+0x63b>
  578ae6:	cvtsi2sdl 0x4(%rcx),%xmm1
  578aeb:	ucomisd %xmm0,%xmm1
  578aef:	seta   0x1(%rdi)
  578af3:	mov    $0x2,%al
  578af5:	jmp    578e0b <quickjs_oxide::engine::vm::execute::binary_number_result+0x104b>
  578afa:	mov    0x4(%rcx),%eax
  578afd:	add    %edx,%eax
  578aff:	jno    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578b05:	jmp    578705 <quickjs_oxide::engine::vm::execute::binary_number_result+0x945>
  578b0a:	mov    %edx,%eax
  578b0c:	sub    0x4(%rcx),%eax
  578b0f:	jno    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578b15:	jmp    57872e <quickjs_oxide::engine::vm::execute::binary_number_result+0x96e>
  578b1a:	movsd  0xe2ac6(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578b22:	addsd  %xmm1,%xmm0
  578b26:	xor    %ebx,%ebx
  578b28:	ucomisd %xmm0,%xmm0
  578b2c:	maxsd  0xe29cc(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578b34:	minsd  0xe29cc(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578b3c:	cvttsd2si %xmm0,%eax
  578b40:	cmovnp %eax,%ebx
  578b43:	mov    %r15,%rdi
  578b46:	mov    %r14,%rcx
  578b49:	cmpl   $0x1,(%rcx)
  578b4c:	je     578411 <quickjs_oxide::engine::vm::execute::binary_number_result+0x651>
  578b52:	mov    0x4(%rcx),%eax
  578b55:	jmp    578b83 <quickjs_oxide::engine::vm::execute::binary_number_result+0xdc3>
  578b57:	movsd  0xe2a89(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578b5f:	addsd  %xmm1,%xmm0
  578b63:	xor    %eax,%eax
  578b65:	ucomisd %xmm0,%xmm0
  578b69:	maxsd  0xe298f(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578b71:	minsd  0xe298f(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578b79:	cvttsd2si %xmm0,%ecx
  578b7d:	cmovnp %ecx,%eax
  578b80:	mov    %r14,%rdi
  578b83:	or     %ebx,%eax
  578b85:	jmp    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578b8a:	movsd  0xe2a56(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578b92:	addsd  %xmm1,%xmm0
  578b96:	xor    %ebx,%ebx
  578b98:	ucomisd %xmm0,%xmm0
  578b9c:	maxsd  0xe295c(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578ba4:	minsd  0xe295c(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578bac:	cvttsd2si %xmm0,%eax
  578bb0:	cmovnp %eax,%ebx
  578bb3:	mov    %r15,%rdi
  578bb6:	mov    %r14,%rcx
  578bb9:	cmpl   $0x1,(%rcx)
  578bbc:	je     578540 <quickjs_oxide::engine::vm::execute::binary_number_result+0x780>
  578bc2:	mov    0x4(%rcx),%eax
  578bc5:	and    %ebx,%eax
  578bc7:	jmp    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578bcc:	movsd  0xe2a14(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578bd4:	addsd  %xmm1,%xmm0
  578bd8:	xor    %eax,%eax
  578bda:	ucomisd %xmm0,%xmm0
  578bde:	maxsd  0xe291a(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578be6:	minsd  0xe291a(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578bee:	cvttsd2si %xmm0,%ecx
  578bf2:	cmovnp %ecx,%eax
  578bf5:	mov    %r14,%rdi
  578bf8:	and    %ebx,%eax
  578bfa:	jmp    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578bff:	movsd  0xe29e1(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578c07:	addsd  %xmm1,%xmm0
  578c0b:	xor    %ebx,%ebx
  578c0d:	ucomisd %xmm0,%xmm0
  578c11:	maxsd  0xe28e7(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578c19:	minsd  0xe28e7(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578c21:	cvttsd2si %xmm0,%eax
  578c25:	cmovnp %eax,%ebx
  578c28:	mov    %r15,%rdi
  578c2b:	mov    %r14,%rcx
  578c2e:	cmpl   $0x1,(%rcx)
  578c31:	je     578649 <quickjs_oxide::engine::vm::execute::binary_number_result+0x889>
  578c37:	mov    0x4(%rcx),%ecx
  578c3a:	jmp    578c68 <quickjs_oxide::engine::vm::execute::binary_number_result+0xea8>
  578c3c:	movsd  0xe29a4(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578c44:	addsd  %xmm1,%xmm0
  578c48:	xor    %ecx,%ecx
  578c4a:	ucomisd %xmm0,%xmm0
  578c4e:	maxsd  0xe28aa(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578c56:	minsd  0xe28aa(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578c5e:	cvttsd2si %xmm0,%eax
  578c62:	cmovnp %eax,%ecx
  578c65:	mov    %r14,%rdi
  578c68:	sar    %cl,%ebx
  578c6a:	mov    %ebx,0x4(%rdi)
  578c6d:	jmp    578e00 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1040>
  578c72:	movsd  0xe296e(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578c7a:	addsd  %xmm1,%xmm0
  578c7e:	xor    %ebx,%ebx
  578c80:	ucomisd %xmm0,%xmm0
  578c84:	maxsd  0xe2874(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578c8c:	minsd  0xe2874(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578c94:	cvttsd2si %xmm0,%eax
  578c98:	cmovnp %eax,%ebx
  578c9b:	mov    %r15,%rdi
  578c9e:	mov    %r14,%rcx
  578ca1:	cmpl   $0x1,(%rcx)
  578ca4:	je     57889d <quickjs_oxide::engine::vm::execute::binary_number_result+0xadd>
  578caa:	mov    0x4(%rcx),%ecx
  578cad:	jmp    578cdb <quickjs_oxide::engine::vm::execute::binary_number_result+0xf1b>
  578caf:	movsd  0xe2931(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578cb7:	addsd  %xmm1,%xmm0
  578cbb:	xor    %ecx,%ecx
  578cbd:	ucomisd %xmm0,%xmm0
  578cc1:	maxsd  0xe2837(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578cc9:	minsd  0xe2837(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578cd1:	cvttsd2si %xmm0,%eax
  578cd5:	cmovnp %eax,%ecx
  578cd8:	mov    %r14,%rdi
  578cdb:	shl    %cl,%ebx
  578cdd:	mov    %ebx,0x4(%rdi)
  578ce0:	jmp    578e00 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1040>
  578ce5:	movsd  0xe28fb(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578ced:	addsd  %xmm1,%xmm0
  578cf1:	xor    %ebx,%ebx
  578cf3:	ucomisd %xmm0,%xmm0
  578cf7:	maxsd  0xe2801(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578cff:	minsd  0xe2801(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578d07:	cvttsd2si %xmm0,%eax
  578d0b:	cmovnp %eax,%ebx
  578d0e:	mov    %r15,%rdi
  578d11:	mov    %r14,%rcx
  578d14:	cmpl   $0x1,(%rcx)
  578d17:	je     578955 <quickjs_oxide::engine::vm::execute::binary_number_result+0xb95>
  578d1d:	mov    0x4(%rcx),%eax
  578d20:	xor    %ebx,%eax
  578d22:	jmp    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578d27:	movsd  0xe28b9(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578d2f:	addsd  %xmm1,%xmm0
  578d33:	xor    %eax,%eax
  578d35:	ucomisd %xmm0,%xmm0
  578d39:	maxsd  0xe27bf(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578d41:	minsd  0xe27bf(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578d49:	cvttsd2si %xmm0,%ecx
  578d4d:	cmovnp %ecx,%eax
  578d50:	mov    %r14,%rdi
  578d53:	xor    %ebx,%eax
  578d55:	jmp    578dfd <quickjs_oxide::engine::vm::execute::binary_number_result+0x103d>
  578d5a:	movsd  0xe2886(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578d62:	addsd  %xmm1,%xmm0
  578d66:	xor    %ebx,%ebx
  578d68:	ucomisd %xmm0,%xmm0
  578d6c:	maxsd  0xe278c(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578d74:	minsd  0xe278c(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578d7c:	cvttsd2si %xmm0,%eax
  578d80:	cmovnp %eax,%ebx
  578d83:	mov    %r15,%rdi
  578d86:	mov    %r14,%rcx
  578d89:	cmpl   $0x1,(%rcx)
  578d8c:	je     578a0a <quickjs_oxide::engine::vm::execute::binary_number_result+0xc4a>
  578d92:	mov    0x4(%rcx),%ecx
  578d95:	jmp    578dc3 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1003>
  578d97:	movsd  0xe2849(%rip),%xmm0        # 65b5e8 <_fini+0x415c>
  578d9f:	addsd  %xmm1,%xmm0
  578da3:	xor    %ecx,%ecx
  578da5:	ucomisd %xmm0,%xmm0
  578da9:	maxsd  0xe274f(%rip),%xmm0        # 65b500 <_fini+0x4074>
  578db1:	minsd  0xe274f(%rip),%xmm0        # 65b508 <_fini+0x407c>
  578db9:	cvttsd2si %xmm0,%eax
  578dbd:	cmovnp %eax,%ecx
  578dc0:	mov    %r14,%rdi
  578dc3:	shr    %cl,%ebx
  578dc5:	xorps  %xmm0,%xmm0
  578dc8:	cvtsi2sd %rbx,%xmm0
  578dcd:	movapd %xmm0,%xmm1
  578dd1:	maxsd  0xe2727(%rip),%xmm1        # 65b500 <_fini+0x4074>
  578dd9:	minsd  0xe2727(%rip),%xmm1        # 65b508 <_fini+0x407c>
  578de1:	cvttsd2si %xmm1,%ecx
  578de5:	xor    %eax,%eax
  578de7:	ucomisd %xmm0,%xmm0
  578deb:	cmovnp %ecx,%eax
  578dee:	xorps  %xmm1,%xmm1
  578df1:	cvtsi2sd %eax,%xmm1
  578df5:	ucomisd %xmm1,%xmm0
  578df9:	jne    578e04 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1044>
  578dfb:	jp     578e04 <quickjs_oxide::engine::vm::execute::binary_number_result+0x1044>
  578dfd:	mov    %eax,0x4(%rdi)
  578e00:	mov    $0x3,%al
  578e02:	jmp    578e0b <quickjs_oxide::engine::vm::execute::binary_number_result+0x104b>
  578e04:	movsd  %xmm0,0x8(%rdi)
  578e09:	mov    $0x4,%al
  578e0b:	mov    %al,(%rdi)
  578e0d:	add    $0x50,%rsp
  578e11:	pop    %rbx
  578e12:	pop    %r14
  578e14:	pop    %r15
  578e16:	ret
  578e17:	lea    0x268cea(%rip),%rax        # 7e1b08 <__do_global_dtors_aux_fini_array_entry+0x18250>
  578e1e:	mov    %rax,0x20(%rsp)
  578e23:	movq   $0x1,0x28(%rsp)
  578e2c:	lea    0x8(%rsp),%rax
  578e31:	mov    %rax,0x30(%rsp)
  578e36:	xorps  %xmm0,%xmm0
  578e39:	movups %xmm0,0x38(%rsp)
  578e3e:	lea    0x268cd3(%rip),%rsi        # 7e1b18 <__do_global_dtors_aux_fini_array_entry+0x18260>
  578e45:	lea    0x20(%rsp),%rdi
  578e4a:	call   57eb0 <core::panicking::panic_fmt>
