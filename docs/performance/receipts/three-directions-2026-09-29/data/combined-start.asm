
/tmp/oxide-three-build-combined/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

00000000004bebb0 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start>:
  4bebb0:	55                   	push   %rbp
  4bebb1:	41 57                	push   %r15
  4bebb3:	41 56                	push   %r14
  4bebb5:	41 55                	push   %r13
  4bebb7:	41 54                	push   %r12
  4bebb9:	53                   	push   %rbx
  4bebba:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4bebc1:	48 89 fb             	mov    %rdi,%rbx
  4bebc4:	4c 8b a4 24 50 03 00 	mov    0x350(%rsp),%r12
  4bebcb:	00 
  4bebcc:	49 83 3c 24 00       	cmpq   $0x0,(%r12)
  4bebd1:	74 23                	je     4bebf6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x46>
  4bebd3:	c6 43 08 04          	movb   $0x4,0x8(%rbx)
  4bebd7:	48 8d 05 75 f9 1c 00 	lea    0x1cf975(%rip),%rax        # 68e553 <num_bigint::biguint::convert::get_radix_base::BASES+0x137a3>
  4bebde:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4bebe2:	48 c7 43 18 30 00 00 	movq   $0x30,0x18(%rbx)
  4bebe9:	00 
  4bebea:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  4bebf1:	e9 73 05 00 00       	jmp    4bf169 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5b9>
  4bebf6:	41 0f b6 44 24 08    	movzbl 0x8(%r12),%eax
  4bebfc:	3c 02                	cmp    $0x2,%al
  4bebfe:	48 89 74 24 18       	mov    %rsi,0x18(%rsp)
  4bec03:	0f 83 0e 01 00 00    	jae    4bed17 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x167>
  4bec09:	41 89 d7             	mov    %edx,%r15d
  4bec0c:	89 cd                	mov    %ecx,%ebp
  4bec0e:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bec13:	45 31 f6             	xor    %r14d,%r14d
  4bec16:	ba 08 01 00 00       	mov    $0x108,%edx
  4bec1b:	31 f6                	xor    %esi,%esi
  4bec1d:	ff 15 35 e1 34 00    	call   *0x34e135(%rip)        # 80cd58 <memset@GLIBC_2.2.5>
  4bec23:	b8 01 00 00 00       	mov    $0x1,%eax
  4bec28:	48 8d 0d 2a d5 1c 00 	lea    0x1cd52a(%rip),%rcx        # 68c159 <num_bigint::biguint::convert::get_radix_base::BASES+0x113a9>
  4bec2f:	49 81 fe ff 00 00 00 	cmp    $0xff,%r14
  4bec36:	74 69                	je     4beca1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf1>
  4bec38:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  4bec3f:	00 
  4bec40:	0f 87 f7 0f 00 00    	ja     4bfc3d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x108d>
  4bec46:	0f b6 54 08 ff       	movzbl -0x1(%rax,%rcx,1),%edx
  4bec4b:	42 88 54 34 70       	mov    %dl,0x70(%rsp,%r14,1)
  4bec50:	4c 8b b4 24 70 01 00 	mov    0x170(%rsp),%r14
  4bec57:	00 
  4bec58:	49 ff c6             	inc    %r14
  4bec5b:	4c 89 b4 24 70 01 00 	mov    %r14,0x170(%rsp)
  4bec62:	00 
  4bec63:	48 83 f8 1f          	cmp    $0x1f,%rax
  4bec67:	74 38                	je     4beca1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf1>
  4bec69:	49 81 fe ff 00 00 00 	cmp    $0xff,%r14
  4bec70:	74 2f                	je     4beca1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf1>
  4bec72:	0f 87 c5 0f 00 00    	ja     4bfc3d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x108d>
  4bec78:	0f b6 14 08          	movzbl (%rax,%rcx,1),%edx
  4bec7c:	42 88 54 34 70       	mov    %dl,0x70(%rsp,%r14,1)
  4bec81:	4c 8b b4 24 70 01 00 	mov    0x170(%rsp),%r14
  4bec88:	00 
  4bec89:	49 ff c6             	inc    %r14
  4bec8c:	4c 89 b4 24 70 01 00 	mov    %r14,0x170(%rsp)
  4bec93:	00 
  4bec94:	48 83 c0 02          	add    $0x2,%rax
  4bec98:	49 81 fe ff 00 00 00 	cmp    $0xff,%r14
  4bec9f:	75 9f                	jne    4bec40 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x90>
  4beca1:	4c 8d b4 24 b8 01 00 	lea    0x1b8(%rsp),%r14
  4beca8:	00 
  4beca9:	48 8d 74 24 70       	lea    0x70(%rsp),%rsi
  4becae:	ba 08 01 00 00       	mov    $0x108,%edx
  4becb3:	4c 89 f7             	mov    %r14,%rdi
  4becb6:	ff 15 24 e3 34 00    	call   *0x34e324(%rip)        # 80cfe0 <memcpy@GLIBC_2.14>
  4becbc:	48 8d bc 24 80 01 00 	lea    0x180(%rsp),%rdi
  4becc3:	00 
  4becc4:	48 8b 74 24 18       	mov    0x18(%rsp),%rsi
  4becc9:	44 89 fa             	mov    %r15d,%edx
  4beccc:	89 e9                	mov    %ebp,%ecx
  4becce:	41 b8 04 00 00 00    	mov    $0x4,%r8d
  4becd4:	4d 89 f1             	mov    %r14,%r9
  4becd7:	e8 54 9b c6 ff       	call   128830 <quickjs_oxide::engine::builtins::error::construction::<impl quickjs_oxide::engine::api::runtime::Runtime>::new_native_error_from_message_jsvalue>
  4becdc:	0f b6 8c 24 80 01 00 	movzbl 0x180(%rsp),%ecx
  4bece3:	00 
  4bece4:	80 f9 0b             	cmp    $0xb,%cl
  4bece7:	0f 85 8b 00 00 00    	jne    4bed78 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1c8>
  4beced:	0f 10 84 24 88 01 00 	movups 0x188(%rsp),%xmm0
  4becf4:	00 
  4becf5:	0f 11 44 24 77       	movups %xmm0,0x77(%rsp)
  4becfa:	0f 11 43 18          	movups %xmm0,0x18(%rbx)
  4becfe:	48 c7 43 08 00 00 00 	movq   $0x0,0x8(%rbx)
  4bed05:	00 
  4bed06:	b8 01 00 00 00       	mov    $0x1,%eax
  4bed0b:	ba 10 00 00 00       	mov    $0x10,%edx
  4bed10:	31 c9                	xor    %ecx,%ecx
  4bed12:	e9 a2 00 00 00       	jmp    4bedb9 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x209>
  4bed17:	48 8b bc 24 58 03 00 	mov    0x358(%rsp),%rdi
  4bed1e:	00 
  4bed1f:	49 83 c4 08          	add    $0x8,%r12
  4bed23:	3c 09                	cmp    $0x9,%al
  4bed25:	89 4c 24 3c          	mov    %ecx,0x3c(%rsp)
  4bed29:	89 54 24 38          	mov    %edx,0x38(%rsp)
  4bed2d:	44 89 4c 24 48       	mov    %r9d,0x48(%rsp)
  4bed32:	44 89 44 24 4c       	mov    %r8d,0x4c(%rsp)
  4bed37:	48 89 7c 24 30       	mov    %rdi,0x30(%rsp)
  4bed3c:	75 0b                	jne    4bed49 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x199>
  4bed3e:	b0 02                	mov    $0x2,%al
  4bed40:	48 89 04 24          	mov    %rax,(%rsp)
  4bed44:	e9 cc 00 00 00       	jmp    4bee15 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x265>
  4bed49:	4c 8b 6f 08          	mov    0x8(%rdi),%r13
  4bed4d:	48 8b 77 10          	mov    0x10(%rdi),%rsi
  4bed51:	48 85 f6             	test   %rsi,%rsi
  4bed54:	4c 8d 35 c5 93 19 00 	lea    0x1993c5(%rip),%r14        # 658120 <_fini+0xdd4>
  4bed5b:	4d 0f 45 f5          	cmovne %r13,%r14
  4bed5f:	41 0f b6 c8          	movzbl %r8b,%ecx
  4bed63:	b0 02                	mov    $0x2,%al
  4bed65:	80 f9 03             	cmp    $0x3,%cl
  4bed68:	73 58                	jae    4bedc2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x212>
  4bed6a:	48 89 04 24          	mov    %rax,(%rsp)
  4bed6e:	bd 01 00 00 00       	mov    $0x1,%ebp
  4bed73:	4d 89 f5             	mov    %r14,%r13
  4bed76:	eb 73                	jmp    4bedeb <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x23b>
  4bed78:	48 8b 84 24 90 01 00 	mov    0x190(%rsp),%rax
  4bed7f:	00 
  4bed80:	48 89 44 24 7f       	mov    %rax,0x7f(%rsp)
  4bed85:	0f 10 84 24 81 01 00 	movups 0x181(%rsp),%xmm0
  4bed8c:	00 
  4bed8d:	0f 29 44 24 70       	movaps %xmm0,0x70(%rsp)
  4bed92:	48 8b 84 24 98 01 00 	mov    0x198(%rsp),%rax
  4bed99:	00 
  4bed9a:	48 8b 54 24 7f       	mov    0x7f(%rsp),%rdx
  4bed9f:	48 89 53 18          	mov    %rdx,0x18(%rbx)
  4beda3:	0f 28 44 24 70       	movaps 0x70(%rsp),%xmm0
  4beda8:	0f 11 43 09          	movups %xmm0,0x9(%rbx)
  4bedac:	88 4b 08             	mov    %cl,0x8(%rbx)
  4bedaf:	b9 01 00 00 00       	mov    $0x1,%ecx
  4bedb4:	ba 20 00 00 00       	mov    $0x20,%edx
  4bedb9:	48 89 04 13          	mov    %rax,(%rbx,%rdx,1)
  4bedbd:	e9 a4 03 00 00       	jmp    4bf166 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5b6>
  4bedc2:	48 89 04 24          	mov    %rax,(%rsp)
  4bedc6:	83 f9 03             	cmp    $0x3,%ecx
  4bedc9:	75 18                	jne    4bede3 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x233>
  4bedcb:	48 8b 6f 18          	mov    0x18(%rdi),%rbp
  4bedcf:	48 39 f5             	cmp    %rsi,%rbp
  4bedd2:	76 17                	jbe    4bedeb <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x23b>
  4bedd4:	48 8d 15 bd 60 31 00 	lea    0x3160bd(%rip),%rdx        # 7d4e98 <__do_global_dtors_aux_fini_array_entry+0xa5e0>
  4beddb:	48 89 ef             	mov    %rbp,%rdi
  4bedde:	e8 6d 91 b9 ff       	call   57f50 <core::slice::index::slice_end_index_len_fail>
  4bede3:	41 bd 08 00 00 00    	mov    $0x8,%r13d
  4bede9:	31 ed                	xor    %ebp,%ebp
  4bedeb:	49 89 ef             	mov    %rbp,%r15
  4bedee:	49 c1 e7 04          	shl    $0x4,%r15
  4bedf2:	31 c0                	xor    %eax,%eax
  4bedf4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  4bedfb:	00 00 00 00 00 
  4bee00:	49 39 c7             	cmp    %rax,%r15
  4bee03:	0f 84 e2 00 00 00    	je     4beeeb <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x33b>
  4bee09:	41 80 7c 05 00 09    	cmpb   $0x9,0x0(%r13,%rax,1)
  4bee0f:	48 8d 40 10          	lea    0x10(%rax),%rax
  4bee13:	75 eb                	jne    4bee00 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x250>
  4bee15:	45 31 ed             	xor    %r13d,%r13d
  4bee18:	45 31 f6             	xor    %r14d,%r14d
  4bee1b:	31 ed                	xor    %ebp,%ebp
  4bee1d:	48 c1 e5 20          	shl    $0x20,%rbp
  4bee21:	41 c1 e6 10          	shl    $0x10,%r14d
  4bee25:	49 09 ee             	or     %rbp,%r14
  4bee28:	41 0f b6 cd          	movzbl %r13b,%ecx
  4bee2c:	c1 e1 08             	shl    $0x8,%ecx
  4bee2f:	4c 09 f1             	or     %r14,%rcx
  4bee32:	0f b6 04 24          	movzbl (%rsp),%eax
  4bee36:	48 09 c8             	or     %rcx,%rax
  4bee39:	48 83 f8 02          	cmp    $0x2,%rax
  4bee3d:	75 75                	jne    4beeb4 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x304>
  4bee3f:	8b 6c 24 4c          	mov    0x4c(%rsp),%ebp
  4bee43:	40 80 fd 03          	cmp    $0x3,%bpl
  4bee47:	0f 85 cd 01 00 00    	jne    4bf01a <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x46a>
  4bee4d:	48 c7 44 24 60 00 00 	movq   $0x0,0x60(%rsp)
  4bee54:	00 00 
  4bee56:	4c 8b 7c 24 30       	mov    0x30(%rsp),%r15
  4bee5b:	4d 8b 77 18          	mov    0x18(%r15),%r14
  4bee5f:	4d 85 f6             	test   %r14,%r14
  4bee62:	0f 84 13 03 00 00    	je     4bf17b <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5cb>
  4bee68:	4c 89 f5             	mov    %r14,%rbp
  4bee6b:	48 c1 e5 04          	shl    $0x4,%rbp
  4bee6f:	4c 89 f0             	mov    %r14,%rax
  4bee72:	48 c1 e8 3c          	shr    $0x3c,%rax
  4bee76:	0f 95 c0             	setne  %al
  4bee79:	48 b9 f8 ff ff ff ff 	movabs $0x7ffffffffffffff8,%rcx
  4bee80:	ff ff 7f 
  4bee83:	48 39 cd             	cmp    %rcx,%rbp
  4bee86:	0f 97 c1             	seta   %cl
  4bee89:	08 c1                	or     %al,%cl
  4bee8b:	0f 84 bf 08 00 00    	je     4bf750 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xba0>
  4bee91:	c6 43 08 04          	movb   $0x4,0x8(%rbx)
  4bee95:	48 8d 05 6e f6 1c 00 	lea    0x1cf66e(%rip),%rax        # 68e50a <num_bigint::biguint::convert::get_radix_base::BASES+0x1375a>
  4bee9c:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4beea0:	48 c7 43 18 24 00 00 	movq   $0x24,0x18(%rbx)
  4beea7:	00 
  4beea8:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  4beeaf:	e9 b5 02 00 00       	jmp    4bf169 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5b9>
  4beeb4:	48 c1 e6 20          	shl    $0x20,%rsi
  4beeb8:	41 c1 e7 10          	shl    $0x10,%r15d
  4beebc:	0f b6 ca             	movzbl %dl,%ecx
  4beebf:	c1 e1 08             	shl    $0x8,%ecx
  4beec2:	44 09 f9             	or     %r15d,%ecx
  4beec5:	40 0f b6 d7          	movzbl %dil,%edx
  4beec9:	48 09 ca             	or     %rcx,%rdx
  4beecc:	48 09 f2             	or     %rsi,%rdx
  4beecf:	48 89 53 18          	mov    %rdx,0x18(%rbx)
  4beed3:	48 8b 4c 24 08       	mov    0x8(%rsp),%rcx
  4beed8:	48 89 4b 20          	mov    %rcx,0x20(%rbx)
  4beedc:	48 c7 43 08 00 00 00 	movq   $0x0,0x8(%rbx)
  4beee3:	00 
  4beee4:	31 c9                	xor    %ecx,%ecx
  4beee6:	e9 77 02 00 00       	jmp    4bf162 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5b2>
  4beeeb:	48 89 0c 24          	mov    %rcx,(%rsp)
  4beeef:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4beef4:	48 8b 74 24 18       	mov    0x18(%rsp),%rsi
  4beef9:	8b 54 24 38          	mov    0x38(%rsp),%edx
  4beefd:	8b 4c 24 3c          	mov    0x3c(%rsp),%ecx
  4bef01:	4d 89 e0             	mov    %r12,%r8
  4bef04:	e8 47 f8 ce ff       	call   1ae750 <quickjs_oxide::engine::value::conversion::<impl quickjs_oxide::engine::api::runtime::Runtime>::string_from_primitive_jsvalue>
  4bef09:	0f b6 44 24 70       	movzbl 0x70(%rsp),%eax
  4bef0e:	88 44 24 28          	mov    %al,0x28(%rsp)
  4bef12:	3c 0b                	cmp    $0xb,%al
  4bef14:	0f 85 98 01 00 00    	jne    4bf0b2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x502>
  4bef1a:	0f b6 7c 24 78       	movzbl 0x78(%rsp),%edi
  4bef1f:	48 8b 84 24 80 00 00 	mov    0x80(%rsp),%rax
  4bef26:	00 
  4bef27:	40 80 ff 0a          	cmp    $0xa,%dil
  4bef2b:	0f 85 04 05 00 00    	jne    4bf435 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x885>
  4bef31:	48 89 44 24 10       	mov    %rax,0x10(%rsp)
  4bef36:	48 89 44 24 40       	mov    %rax,0x40(%rsp)
  4bef3b:	48 8d 05 96 fe 1a 00 	lea    0x1afe96(%rip),%rax        # 66edd8 <_fini+0x17a8c>
  4bef42:	48 8b 0c 24          	mov    (%rsp),%rcx
  4bef46:	48 63 0c 88          	movslq (%rax,%rcx,4),%rcx
  4bef4a:	48 01 c1             	add    %rax,%rcx
  4bef4d:	ff e1                	jmp    *%rcx
  4bef4f:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bef54:	48 8b 74 24 18       	mov    0x18(%rsp),%rsi
  4bef59:	8b 54 24 38          	mov    0x38(%rsp),%edx
  4bef5d:	8b 4c 24 3c          	mov    0x3c(%rsp),%ecx
  4bef61:	4d 89 f0             	mov    %r14,%r8
  4bef64:	4c 8b 7c 24 10       	mov    0x10(%rsp),%r15
  4bef69:	e8 52 b9 ce ff       	call   1aa8c0 <quickjs_oxide::engine::value::conversion::<impl quickjs_oxide::engine::api::runtime::Runtime>::number_from_primitive_jsvalue>
  4bef6e:	8b 4c 24 48          	mov    0x48(%rsp),%ecx
  4bef72:	0f b6 44 24 70       	movzbl 0x70(%rsp),%eax
  4bef77:	3c 0b                	cmp    $0xb,%al
  4bef79:	0f 85 78 06 00 00    	jne    4bf5f7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xa47>
  4bef7f:	0f b6 7c 24 78       	movzbl 0x78(%rsp),%edi
  4bef84:	f2 0f 10 84 24 80 00 	movsd  0x80(%rsp),%xmm0
  4bef8b:	00 00 
  4bef8d:	40 80 ff 0a          	cmp    $0xa,%dil
  4bef91:	0f 85 8c 07 00 00    	jne    4bf723 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xb73>
  4bef97:	48 8b 44 24 18       	mov    0x18(%rsp),%rax
  4bef9c:	48 8b 30             	mov    (%rax),%rsi
  4bef9f:	0f b6 c9             	movzbl %cl,%ecx
  4befa2:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4befa7:	4c 8d 44 24 40       	lea    0x40(%rsp),%r8
  4befac:	48 8b 14 24          	mov    (%rsp),%rdx
  4befb0:	e8 8b e6 fc ff       	call   48d640 <quickjs_oxide::engine::builtins::primitive::text::finish_index>
  4befb5:	0f b6 44 24 70       	movzbl 0x70(%rsp),%eax
  4befba:	88 44 24 28          	mov    %al,0x28(%rsp)
  4befbe:	3c 0b                	cmp    $0xb,%al
  4befc0:	74 10                	je     4befd2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x422>
  4befc2:	8b 44 24 71          	mov    0x71(%rsp),%eax
  4befc6:	8b 4c 24 74          	mov    0x74(%rsp),%ecx
  4befca:	89 4c 24 23          	mov    %ecx,0x23(%rsp)
  4befce:	89 44 24 20          	mov    %eax,0x20(%rsp)
  4befd2:	48 8b 6c 24 78       	mov    0x78(%rsp),%rbp
  4befd7:	48 8b bc 24 80 00 00 	mov    0x80(%rsp),%rdi
  4befde:	00 
  4befdf:	48 8b 84 24 88 00 00 	mov    0x88(%rsp),%rax
  4befe6:	00 
  4befe7:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4befec:	48 89 fe             	mov    %rdi,%rsi
  4befef:	48 c1 ee 20          	shr    $0x20,%rsi
  4beff3:	89 fa                	mov    %edi,%edx
  4beff5:	c1 ea 08             	shr    $0x8,%edx
  4beff8:	41 89 ff             	mov    %edi,%r15d
  4beffb:	41 c1 ef 10          	shr    $0x10,%r15d
  4befff:	41 89 ed             	mov    %ebp,%r13d
  4bf002:	41 c1 ed 08          	shr    $0x8,%r13d
  4bf006:	41 89 ee             	mov    %ebp,%r14d
  4bf009:	41 c1 ee 10          	shr    $0x10,%r14d
  4bf00d:	48 89 2c 24          	mov    %rbp,(%rsp)
  4bf011:	48 c1 ed 20          	shr    $0x20,%rbp
  4bf015:	e9 88 0b 00 00       	jmp    4bfba2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xff2>
  4bf01a:	0f b6 05 b4 ea 34 00 	movzbl 0x34eab4(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
  4bf021:	bf 10 00 00 00       	mov    $0x10,%edi
  4bf026:	ff 15 bc dd 34 00    	call   *0x34ddbc(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
  4bf02c:	49 89 c5             	mov    %rax,%r13
  4bf02f:	48 85 c0             	test   %rax,%rax
  4bf032:	4c 8b 74 24 18       	mov    0x18(%rsp),%r14
  4bf037:	48 8b 44 24 30       	mov    0x30(%rsp),%rax
  4bf03c:	0f 84 a6 0b 00 00    	je     4bfbe8 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1038>
  4bf042:	48 83 78 10 00       	cmpq   $0x0,0x10(%rax)
  4bf047:	0f 84 46 01 00 00    	je     4bf193 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5e3>
  4bf04d:	48 8b 50 08          	mov    0x8(%rax),%rdx
  4bf051:	49 8b 36             	mov    (%r14),%rsi
  4bf054:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bf059:	e8 b2 a5 c3 ff       	call   f9610 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::dup_jsvalue>
  4bf05e:	0f b6 44 24 70       	movzbl 0x70(%rsp),%eax
  4bf063:	3c 0b                	cmp    $0xb,%al
  4bf065:	0f 85 4f 03 00 00    	jne    4bf3ba <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x80a>
  4bf06b:	0f b6 44 24 78       	movzbl 0x78(%rsp),%eax
  4bf070:	48 8b 4c 24 79       	mov    0x79(%rsp),%rcx
  4bf075:	48 89 8c 24 b8 01 00 	mov    %rcx,0x1b8(%rsp)
  4bf07c:	00 
  4bf07d:	48 8b 8c 24 80 00 00 	mov    0x80(%rsp),%rcx
  4bf084:	00 
  4bf085:	48 89 8c 24 bf 01 00 	mov    %rcx,0x1bf(%rsp)
  4bf08c:	00 
  4bf08d:	48 8b 8c 24 b8 01 00 	mov    0x1b8(%rsp),%rcx
  4bf094:	00 
  4bf095:	48 8b 94 24 bf 01 00 	mov    0x1bf(%rsp),%rdx
  4bf09c:	00 
  4bf09d:	48 89 8c 24 80 01 00 	mov    %rcx,0x180(%rsp)
  4bf0a4:	00 
  4bf0a5:	48 89 94 24 87 01 00 	mov    %rdx,0x187(%rsp)
  4bf0ac:	00 
  4bf0ad:	e9 e3 00 00 00       	jmp    4bf195 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5e5>
  4bf0b2:	8b 44 24 71          	mov    0x71(%rsp),%eax
  4bf0b6:	8b 4c 24 74          	mov    0x74(%rsp),%ecx
  4bf0ba:	89 4c 24 23          	mov    %ecx,0x23(%rsp)
  4bf0be:	89 44 24 20          	mov    %eax,0x20(%rsp)
  4bf0c2:	0f b6 44 24 78       	movzbl 0x78(%rsp),%eax
  4bf0c7:	48 89 04 24          	mov    %rax,(%rsp)
  4bf0cb:	44 0f b6 6c 24 79    	movzbl 0x79(%rsp),%r13d
  4bf0d1:	44 0f b7 74 24 7a    	movzwl 0x7a(%rsp),%r14d
  4bf0d7:	8b 6c 24 7c          	mov    0x7c(%rsp),%ebp
  4bf0db:	48 8b bc 24 80 00 00 	mov    0x80(%rsp),%rdi
  4bf0e2:	00 
  4bf0e3:	48 8b 84 24 88 00 00 	mov    0x88(%rsp),%rax
  4bf0ea:	00 
  4bf0eb:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf0f0:	89 fa                	mov    %edi,%edx
  4bf0f2:	c1 ea 08             	shr    $0x8,%edx
  4bf0f5:	41 89 ff             	mov    %edi,%r15d
  4bf0f8:	41 c1 ef 10          	shr    $0x10,%r15d
  4bf0fc:	48 89 fe             	mov    %rdi,%rsi
  4bf0ff:	48 c1 ee 20          	shr    $0x20,%rsi
  4bf103:	8b 44 24 20          	mov    0x20(%rsp),%eax
  4bf107:	8b 4c 24 23          	mov    0x23(%rsp),%ecx
  4bf10b:	89 4b 0c             	mov    %ecx,0xc(%rbx)
  4bf10e:	89 43 09             	mov    %eax,0x9(%rbx)
  4bf111:	48 c1 e5 20          	shl    $0x20,%rbp
  4bf115:	41 c1 e6 10          	shl    $0x10,%r14d
  4bf119:	49 09 ee             	or     %rbp,%r14
  4bf11c:	41 0f b6 cd          	movzbl %r13b,%ecx
  4bf120:	c1 e1 08             	shl    $0x8,%ecx
  4bf123:	4c 09 f1             	or     %r14,%rcx
  4bf126:	0f b6 04 24          	movzbl (%rsp),%eax
  4bf12a:	48 09 c8             	or     %rcx,%rax
  4bf12d:	41 c1 e7 10          	shl    $0x10,%r15d
  4bf131:	0f b6 ca             	movzbl %dl,%ecx
  4bf134:	c1 e1 08             	shl    $0x8,%ecx
  4bf137:	44 09 f9             	or     %r15d,%ecx
  4bf13a:	40 0f b6 d7          	movzbl %dil,%edx
  4bf13e:	48 09 ca             	or     %rcx,%rdx
  4bf141:	48 c1 e6 20          	shl    $0x20,%rsi
  4bf145:	48 09 d6             	or     %rdx,%rsi
  4bf148:	48 89 73 18          	mov    %rsi,0x18(%rbx)
  4bf14c:	48 8b 4c 24 08       	mov    0x8(%rsp),%rcx
  4bf151:	48 89 4b 20          	mov    %rcx,0x20(%rbx)
  4bf155:	0f b6 4c 24 28       	movzbl 0x28(%rsp),%ecx
  4bf15a:	88 4b 08             	mov    %cl,0x8(%rbx)
  4bf15d:	b9 01 00 00 00       	mov    $0x1,%ecx
  4bf162:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4bf166:	48 89 0b             	mov    %rcx,(%rbx)
  4bf169:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4bf170:	5b                   	pop    %rbx
  4bf171:	41 5c                	pop    %r12
  4bf173:	41 5d                	pop    %r13
  4bf175:	41 5e                	pop    %r14
  4bf177:	41 5f                	pop    %r15
  4bf179:	5d                   	pop    %rbp
  4bf17a:	c3                   	ret
  4bf17b:	41 bd 08 00 00 00    	mov    $0x8,%r13d
  4bf181:	31 ed                	xor    %ebp,%ebp
  4bf183:	48 c7 44 24 30 00 00 	movq   $0x0,0x30(%rsp)
  4bf18a:	00 00 
  4bf18c:	4c 8b 74 24 18       	mov    0x18(%rsp),%r14
  4bf191:	eb 2d                	jmp    4bf1c0 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x610>
  4bf193:	31 c0                	xor    %eax,%eax
  4bf195:	41 88 45 00          	mov    %al,0x0(%r13)
  4bf199:	48 8b 84 24 80 01 00 	mov    0x180(%rsp),%rax
  4bf1a0:	00 
  4bf1a1:	48 8b 8c 24 87 01 00 	mov    0x187(%rsp),%rcx
  4bf1a8:	00 
  4bf1a9:	49 89 45 01          	mov    %rax,0x1(%r13)
  4bf1ad:	49 89 4d 08          	mov    %rcx,0x8(%r13)
  4bf1b1:	bd 01 00 00 00       	mov    $0x1,%ebp
  4bf1b6:	b8 01 00 00 00       	mov    $0x1,%eax
  4bf1bb:	48 89 44 24 30       	mov    %rax,0x30(%rsp)
  4bf1c0:	4d 8b 3e             	mov    (%r14),%r15
  4bf1c3:	48 8d bc 24 f8 02 00 	lea    0x2f8(%rsp),%rdi
  4bf1ca:	00 
  4bf1cb:	4c 89 fe             	mov    %r15,%rsi
  4bf1ce:	4c 89 e2             	mov    %r12,%rdx
  4bf1d1:	4c 89 2c 24          	mov    %r13,(%rsp)
  4bf1d5:	e8 36 a4 c3 ff       	call   f9610 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::dup_jsvalue>
  4bf1da:	80 bc 24 f8 02 00 00 	cmpb   $0xb,0x2f8(%rsp)
  4bf1e1:	0b 
  4bf1e2:	4c 89 7c 24 18       	mov    %r15,0x18(%rsp)
  4bf1e7:	0f 85 02 01 00 00    	jne    4bf2ef <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x73f>
  4bf1ed:	49 ff 07             	incq   (%r15)
  4bf1f0:	0f 84 7d 0a 00 00    	je     4bfc73 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10c3>
  4bf1f6:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bf1fb:	be 01 00 00 00       	mov    $0x1,%esi
  4bf200:	31 d2                	xor    %edx,%edx
  4bf202:	e8 39 36 c5 ff       	call   112840 <quickjs_oxide::engine::value::primitive::JsString::try_from_utf8>
  4bf207:	80 7c 24 70 01       	cmpb   $0x1,0x70(%rsp)
  4bf20c:	0f 84 e5 09 00 00    	je     4bfbf7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1047>
  4bf212:	48 8b 44 24 78       	mov    0x78(%rsp),%rax
  4bf217:	48 c1 e5 04          	shl    $0x4,%rbp
  4bf21b:	4c 01 ed             	add    %r13,%rbp
  4bf21e:	4c 89 ac 24 80 00 00 	mov    %r13,0x80(%rsp)
  4bf225:	00 
  4bf226:	48 8b 4c 24 30       	mov    0x30(%rsp),%rcx
  4bf22b:	48 89 8c 24 90 00 00 	mov    %rcx,0x90(%rsp)
  4bf232:	00 
  4bf233:	4c 89 ac 24 88 00 00 	mov    %r13,0x88(%rsp)
  4bf23a:	00 
  4bf23b:	48 89 ac 24 98 00 00 	mov    %rbp,0x98(%rsp)
  4bf242:	00 
  4bf243:	4c 89 7c 24 70       	mov    %r15,0x70(%rsp)
  4bf248:	8b 4c 24 38          	mov    0x38(%rsp),%ecx
  4bf24c:	89 8c 24 a0 00 00 00 	mov    %ecx,0xa0(%rsp)
  4bf253:	8b 4c 24 3c          	mov    0x3c(%rsp),%ecx
  4bf257:	89 8c 24 a4 00 00 00 	mov    %ecx,0xa4(%rsp)
  4bf25e:	8b 4c 24 4c          	mov    0x4c(%rsp),%ecx
  4bf262:	88 8c 24 a8 00 00 00 	mov    %cl,0xa8(%rsp)
  4bf269:	8b 4c 24 48          	mov    0x48(%rsp),%ecx
  4bf26d:	88 8c 24 a9 00 00 00 	mov    %cl,0xa9(%rsp)
  4bf274:	c6 84 24 aa 00 00 00 	movb   $0x0,0xaa(%rsp)
  4bf27b:	00 
  4bf27c:	48 89 44 24 78       	mov    %rax,0x78(%rsp)
  4bf281:	0f b6 05 4d e8 34 00 	movzbl 0x34e84d(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
  4bf288:	bf 40 00 00 00       	mov    $0x40,%edi
  4bf28d:	ff 15 55 db 34 00    	call   *0x34db55(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
  4bf293:	48 85 c0             	test   %rax,%rax
  4bf296:	0f 84 90 09 00 00    	je     4bfc2c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x107c>
  4bf29c:	48 8d 8c 24 00 03 00 	lea    0x300(%rsp),%rcx
  4bf2a3:	00 
  4bf2a4:	0f 10 44 24 70       	movups 0x70(%rsp),%xmm0
  4bf2a9:	0f 10 8c 24 80 00 00 	movups 0x80(%rsp),%xmm1
  4bf2b0:	00 
  4bf2b1:	0f 10 94 24 90 00 00 	movups 0x90(%rsp),%xmm2
  4bf2b8:	00 
  4bf2b9:	0f 10 9c 24 a0 00 00 	movups 0xa0(%rsp),%xmm3
  4bf2c0:	00 
  4bf2c1:	0f 11 58 30          	movups %xmm3,0x30(%rax)
  4bf2c5:	0f 11 50 20          	movups %xmm2,0x20(%rax)
  4bf2c9:	0f 11 48 10          	movups %xmm1,0x10(%rax)
  4bf2cd:	0f 11 00             	movups %xmm0,(%rax)
  4bf2d0:	0f 10 01             	movups (%rcx),%xmm0
  4bf2d3:	0f 11 43 18          	movups %xmm0,0x18(%rbx)
  4bf2d7:	48 c7 43 08 01 00 00 	movq   $0x1,0x8(%rbx)
  4bf2de:	00 
  4bf2df:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4bf2e3:	48 c7 03 00 00 00 00 	movq   $0x0,(%rbx)
  4bf2ea:	e9 7a fe ff ff       	jmp    4bf169 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5b9>
  4bf2ef:	48 85 ed             	test   %rbp,%rbp
  4bf2f2:	0f 84 8d 00 00 00    	je     4bf385 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7d5>
  4bf2f8:	48 c1 e5 04          	shl    $0x4,%rbp
  4bf2fc:	45 31 e4             	xor    %r12d,%r12d
  4bf2ff:	4c 8d 74 24 70       	lea    0x70(%rsp),%r14
  4bf304:	eb 1d                	jmp    4bf323 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x773>
  4bf306:	4c 89 f7             	mov    %r14,%rdi
  4bf309:	ff d5                	call   *%rbp
  4bf30b:	4d 89 ee             	mov    %r13,%r14
  4bf30e:	4c 8b 2c 24          	mov    (%rsp),%r13
  4bf312:	4c 89 fd             	mov    %r15,%rbp
  4bf315:	4c 8b 7c 24 18       	mov    0x18(%rsp),%r15
  4bf31a:	49 83 c4 10          	add    $0x10,%r12
  4bf31e:	4c 39 e5             	cmp    %r12,%rbp
  4bf321:	74 62                	je     4bf385 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7d5>
  4bf323:	43 0f 10 44 25 00    	movups 0x0(%r13,%r12,1),%xmm0
  4bf329:	0f 29 84 24 c0 02 00 	movaps %xmm0,0x2c0(%rsp)
  4bf330:	00 
  4bf331:	80 bc 24 c0 02 00 00 	cmpb   $0xa,0x2c0(%rsp)
  4bf338:	0a 
  4bf339:	74 4a                	je     4bf385 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7d5>
  4bf33b:	4c 89 f7             	mov    %r14,%rdi
  4bf33e:	4c 89 fe             	mov    %r15,%rsi
  4bf341:	48 8d 94 24 c0 02 00 	lea    0x2c0(%rsp),%rdx
  4bf348:	00 
  4bf349:	e8 d2 a3 c1 ff       	call   d9720 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::release_jsvalue>
  4bf34e:	80 7c 24 70 06       	cmpb   $0x6,0x70(%rsp)
  4bf353:	75 c5                	jne    4bf31a <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x76a>
  4bf355:	49 89 ef             	mov    %rbp,%r15
  4bf358:	4d 89 f5             	mov    %r14,%r13
  4bf35b:	4c 8b 74 24 78       	mov    0x78(%rsp),%r14
  4bf360:	49 83 7e 28 00       	cmpq   $0x0,0x28(%r14)
  4bf365:	74 0a                	je     4bf371 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7c1>
  4bf367:	49 8b 7e 30          	mov    0x30(%r14),%rdi
  4bf36b:	ff 15 cf da 34 00    	call   *0x34dacf(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bf371:	49 8b 7e 40          	mov    0x40(%r14),%rdi
  4bf375:	48 85 ff             	test   %rdi,%rdi
  4bf378:	48 8b 2d c1 da 34 00 	mov    0x34dac1(%rip),%rbp        # 80ce40 <free@GLIBC_2.2.5>
  4bf37f:	74 85                	je     4bf306 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x756>
  4bf381:	ff d5                	call   *%rbp
  4bf383:	eb 81                	jmp    4bf306 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x756>
  4bf385:	48 83 7c 24 30 00    	cmpq   $0x0,0x30(%rsp)
  4bf38b:	74 09                	je     4bf396 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7e6>
  4bf38d:	4c 89 ef             	mov    %r13,%rdi
  4bf390:	ff 15 aa da 34 00    	call   *0x34daaa(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bf396:	0f 10 84 24 f8 02 00 	movups 0x2f8(%rsp),%xmm0
  4bf39d:	00 
  4bf39e:	0f 10 8c 24 08 03 00 	movups 0x308(%rsp),%xmm1
  4bf3a5:	00 
  4bf3a6:	0f 11 4b 18          	movups %xmm1,0x18(%rbx)
  4bf3aa:	0f 11 43 08          	movups %xmm0,0x8(%rbx)
  4bf3ae:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  4bf3b5:	e9 af fd ff ff       	jmp    4bf169 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5b9>
  4bf3ba:	8b 4c 24 71          	mov    0x71(%rsp),%ecx
  4bf3be:	8b 54 24 74          	mov    0x74(%rsp),%edx
  4bf3c2:	89 53 0c             	mov    %edx,0xc(%rbx)
  4bf3c5:	89 4b 09             	mov    %ecx,0x9(%rbx)
  4bf3c8:	0f b6 4c 24 78       	movzbl 0x78(%rsp),%ecx
  4bf3cd:	48 8b 54 24 79       	mov    0x79(%rsp),%rdx
  4bf3d2:	48 89 94 24 b8 01 00 	mov    %rdx,0x1b8(%rsp)
  4bf3d9:	00 
  4bf3da:	48 8b 94 24 80 00 00 	mov    0x80(%rsp),%rdx
  4bf3e1:	00 
  4bf3e2:	48 89 94 24 bf 01 00 	mov    %rdx,0x1bf(%rsp)
  4bf3e9:	00 
  4bf3ea:	48 8b 94 24 88 00 00 	mov    0x88(%rsp),%rdx
  4bf3f1:	00 
  4bf3f2:	48 8b b4 24 b8 01 00 	mov    0x1b8(%rsp),%rsi
  4bf3f9:	00 
  4bf3fa:	48 8b bc 24 bf 01 00 	mov    0x1bf(%rsp),%rdi
  4bf401:	00 
  4bf402:	48 89 7b 18          	mov    %rdi,0x18(%rbx)
  4bf406:	48 89 73 11          	mov    %rsi,0x11(%rbx)
  4bf40a:	88 43 08             	mov    %al,0x8(%rbx)
  4bf40d:	88 4b 10             	mov    %cl,0x10(%rbx)
  4bf410:	48 89 53 20          	mov    %rdx,0x20(%rbx)
  4bf414:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  4bf41b:	4c 89 ef             	mov    %r13,%rdi
  4bf41e:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4bf425:	5b                   	pop    %rbx
  4bf426:	41 5c                	pop    %r12
  4bf428:	41 5d                	pop    %r13
  4bf42a:	41 5e                	pop    %r14
  4bf42c:	41 5f                	pop    %r15
  4bf42e:	5d                   	pop    %rbp
  4bf42f:	ff 25 0b da 34 00    	jmp    *0x34da0b(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bf435:	0f b6 54 24 79       	movzbl 0x79(%rsp),%edx
  4bf43a:	44 0f b7 7c 24 7a    	movzwl 0x7a(%rsp),%r15d
  4bf440:	8b 74 24 7c          	mov    0x7c(%rsp),%esi
  4bf444:	b1 01                	mov    $0x1,%cl
  4bf446:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf44b:	48 89 0c 24          	mov    %rcx,(%rsp)
  4bf44f:	e9 c1 f9 ff ff       	jmp    4bee15 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x265>
  4bf454:	48 8b 44 24 18       	mov    0x18(%rsp),%rax
  4bf459:	4c 8b 30             	mov    (%rax),%r14
  4bf45c:	48 85 ed             	test   %rbp,%rbp
  4bf45f:	0f 84 82 04 00 00    	je     4bf8e7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xd37>
  4bf465:	48 b8 f8 ff ff ff ff 	movabs $0x7ffffffffffffff8,%rax
  4bf46c:	ff ff 7f 
  4bf46f:	48 83 c0 07          	add    $0x7,%rax
  4bf473:	48 89 44 24 28       	mov    %rax,0x28(%rsp)
  4bf478:	eb 13                	jmp    4bf48d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8dd>
  4bf47a:	49 83 c5 10          	add    $0x10,%r13
  4bf47e:	48 89 6c 24 10       	mov    %rbp,0x10(%rsp)
  4bf483:	49 83 c7 f0          	add    $0xfffffffffffffff0,%r15
  4bf487:	0f 84 5f 04 00 00    	je     4bf8ec <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xd3c>
  4bf48d:	41 80 7d 00 05       	cmpb   $0x5,0x0(%r13)
  4bf492:	0f 85 b4 00 00 00    	jne    4bf54c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x99c>
  4bf498:	49 8b 46 28          	mov    0x28(%r14),%rax
  4bf49c:	48 3b 44 24 28       	cmp    0x28(%rsp),%rax
  4bf4a1:	0f 83 bb 07 00 00    	jae    4bfc62 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10b2>
  4bf4a7:	4c 89 f2             	mov    %r14,%rdx
  4bf4aa:	48 8d 48 01          	lea    0x1(%rax),%rcx
  4bf4ae:	49 89 4e 28          	mov    %rcx,0x28(%r14)
  4bf4b2:	41 8b 6d 04          	mov    0x4(%r13),%ebp
  4bf4b6:	41 8b 7d 08          	mov    0x8(%r13),%edi
  4bf4ba:	b1 01                	mov    $0x1,%cl
  4bf4bc:	48 89 0c 24          	mov    %rcx,(%rsp)
  4bf4c0:	49 39 ae 90 01 00 00 	cmp    %rbp,0x190(%r14)
  4bf4c7:	0f 86 a7 06 00 00    	jbe    4bfb74 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfc4>
  4bf4cd:	48 8b 8a 88 01 00 00 	mov    0x188(%rdx),%rcx
  4bf4d4:	48 8d 14 6d 00 00 00 	lea    0x0(,%rbp,2),%rdx
  4bf4db:	00 
  4bf4dc:	48 01 ea             	add    %rbp,%rdx
  4bf4df:	39 7c d1 10          	cmp    %edi,0x10(%rcx,%rdx,8)
  4bf4e3:	0f 85 8b 06 00 00    	jne    4bfb74 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfc4>
  4bf4e9:	48 8d 0c d1          	lea    (%rcx,%rdx,8),%rcx
  4bf4ed:	48 8b 11             	mov    (%rcx),%rdx
  4bf4f0:	48 8d 72 fe          	lea    -0x2(%rdx),%rsi
  4bf4f4:	48 83 fe 03          	cmp    $0x3,%rsi
  4bf4f8:	41 b8 03 00 00 00    	mov    $0x3,%r8d
  4bf4fe:	49 0f 43 f0          	cmovae %r8,%rsi
  4bf502:	48 83 fe 02          	cmp    $0x2,%rsi
  4bf506:	0f 85 f0 05 00 00    	jne    4bfafc <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf4c>
  4bf50c:	89 d6                	mov    %edx,%esi
  4bf50e:	83 e6 fe             	and    $0xfffffffe,%esi
  4bf511:	83 fe 02             	cmp    $0x2,%esi
  4bf514:	0f 84 5a 06 00 00    	je     4bfb74 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfc4>
  4bf51a:	83 79 14 00          	cmpl   $0x0,0x14(%rcx)
  4bf51e:	0f 84 50 06 00 00    	je     4bfb74 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfc4>
  4bf524:	83 fa 04             	cmp    $0x4,%edx
  4bf527:	0f 85 3a 06 00 00    	jne    4bfb67 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfb7>
  4bf52d:	48 8b 41 08          	mov    0x8(%rcx),%rax
  4bf531:	48 ff 00             	incq   (%rax)
  4bf534:	0f 84 39 07 00 00    	je     4bfc73 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10c3>
  4bf53a:	48 8b 69 08          	mov    0x8(%rcx),%rbp
  4bf53e:	48 89 ac 24 b8 01 00 	mov    %rbp,0x1b8(%rsp)
  4bf545:	00 
  4bf546:	49 ff 4e 28          	decq   0x28(%r14)
  4bf54a:	eb 46                	jmp    4bf592 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x9e2>
  4bf54c:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bf551:	48 8b 74 24 18       	mov    0x18(%rsp),%rsi
  4bf556:	8b 54 24 38          	mov    0x38(%rsp),%edx
  4bf55a:	8b 4c 24 3c          	mov    0x3c(%rsp),%ecx
  4bf55e:	4d 89 e8             	mov    %r13,%r8
  4bf561:	e8 ea f1 ce ff       	call   1ae750 <quickjs_oxide::engine::value::conversion::<impl quickjs_oxide::engine::api::runtime::Runtime>::string_from_primitive_jsvalue>
  4bf566:	0f b6 44 24 70       	movzbl 0x70(%rsp),%eax
  4bf56b:	3c 0b                	cmp    $0xb,%al
  4bf56d:	0f 85 84 00 00 00    	jne    4bf5f7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xa47>
  4bf573:	0f b6 7c 24 78       	movzbl 0x78(%rsp),%edi
  4bf578:	48 8b ac 24 80 00 00 	mov    0x80(%rsp),%rbp
  4bf57f:	00 
  4bf580:	40 80 ff 0a          	cmp    $0xa,%dil
  4bf584:	0f 85 c4 05 00 00    	jne    4bfb4e <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf9e>
  4bf58a:	48 89 ac 24 b8 01 00 	mov    %rbp,0x1b8(%rsp)
  4bf591:	00 
  4bf592:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bf597:	48 8b 74 24 10       	mov    0x10(%rsp),%rsi
  4bf59c:	48 89 ea             	mov    %rbp,%rdx
  4bf59f:	48 89 6c 24 08       	mov    %rbp,0x8(%rsp)
  4bf5a4:	e8 87 88 e1 ff       	call   2d7e30 <quickjs_oxide::engine::value::primitive::JsString::try_concat>
  4bf5a9:	80 7c 24 70 01       	cmpb   $0x1,0x70(%rsp)
  4bf5ae:	0f 84 54 05 00 00    	je     4bfb08 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf58>
  4bf5b4:	48 8b 6c 24 78       	mov    0x78(%rsp),%rbp
  4bf5b9:	48 8b 44 24 10       	mov    0x10(%rsp),%rax
  4bf5be:	48 ff 08             	decq   (%rax)
  4bf5c1:	75 0a                	jne    4bf5cd <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xa1d>
  4bf5c3:	48 8d 7c 24 40       	lea    0x40(%rsp),%rdi
  4bf5c8:	e8 03 df be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bf5cd:	48 89 6c 24 40       	mov    %rbp,0x40(%rsp)
  4bf5d2:	48 8b 44 24 08       	mov    0x8(%rsp),%rax
  4bf5d7:	48 ff 08             	decq   (%rax)
  4bf5da:	0f 85 9a fe ff ff    	jne    4bf47a <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8ca>
  4bf5e0:	48 89 6c 24 10       	mov    %rbp,0x10(%rsp)
  4bf5e5:	48 8d bc 24 b8 01 00 	lea    0x1b8(%rsp),%rdi
  4bf5ec:	00 
  4bf5ed:	e8 de de be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bf5f2:	e9 83 fe ff ff       	jmp    4bf47a <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8ca>
  4bf5f7:	88 44 24 28          	mov    %al,0x28(%rsp)
  4bf5fb:	8b 44 24 71          	mov    0x71(%rsp),%eax
  4bf5ff:	8b 4c 24 74          	mov    0x74(%rsp),%ecx
  4bf603:	89 4c 24 23          	mov    %ecx,0x23(%rsp)
  4bf607:	89 44 24 20          	mov    %eax,0x20(%rsp)
  4bf60b:	0f b6 44 24 78       	movzbl 0x78(%rsp),%eax
  4bf610:	48 89 04 24          	mov    %rax,(%rsp)
  4bf614:	44 0f b6 6c 24 79    	movzbl 0x79(%rsp),%r13d
  4bf61a:	44 0f b7 74 24 7a    	movzwl 0x7a(%rsp),%r14d
  4bf620:	8b 6c 24 7c          	mov    0x7c(%rsp),%ebp
  4bf624:	48 8b bc 24 80 00 00 	mov    0x80(%rsp),%rdi
  4bf62b:	00 
  4bf62c:	48 8b 84 24 88 00 00 	mov    0x88(%rsp),%rax
  4bf633:	00 
  4bf634:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf639:	89 fa                	mov    %edi,%edx
  4bf63b:	c1 ea 08             	shr    $0x8,%edx
  4bf63e:	41 89 ff             	mov    %edi,%r15d
  4bf641:	41 c1 ef 10          	shr    $0x10,%r15d
  4bf645:	48 89 fe             	mov    %rdi,%rsi
  4bf648:	48 c1 ee 20          	shr    $0x20,%rsi
  4bf64c:	e9 51 05 00 00       	jmp    4bfba2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xff2>
  4bf651:	48 8b 44 24 18       	mov    0x18(%rsp),%rax
  4bf656:	48 8b 30             	mov    (%rax),%rsi
  4bf659:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bf65e:	8b 54 24 38          	mov    0x38(%rsp),%edx
  4bf662:	8b 4c 24 3c          	mov    0x3c(%rsp),%ecx
  4bf666:	4c 8b 44 24 10       	mov    0x10(%rsp),%r8
  4bf66b:	e8 20 12 e3 ff       	call   2f0890 <quickjs_oxide::engine::object::allocation::<impl quickjs_oxide::engine::api::runtime::Runtime>::new_string_iterator>
  4bf670:	0f b6 44 24 70       	movzbl 0x70(%rsp),%eax
  4bf675:	3c 0b                	cmp    $0xb,%al
  4bf677:	0f 85 df 01 00 00    	jne    4bf85c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xcac>
  4bf67d:	0f 10 44 24 78       	movups 0x78(%rsp),%xmm0
  4bf682:	0f 29 84 24 80 01 00 	movaps %xmm0,0x180(%rsp)
  4bf689:	00 
  4bf68a:	48 8d bc 24 80 01 00 	lea    0x180(%rsp),%rdi
  4bf691:	00 
  4bf692:	e8 99 ca c2 ff       	call   ec130 <quickjs_oxide::engine::object::ObjectRef::into_handle>
  4bf697:	89 c6                	mov    %eax,%esi
  4bf699:	89 d0                	mov    %edx,%eax
  4bf69b:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf6a0:	40 b7 09             	mov    $0x9,%dil
  4bf6a3:	31 c9                	xor    %ecx,%ecx
  4bf6a5:	e9 a1 fd ff ff       	jmp    4bf44b <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x89b>
  4bf6aa:	f6 44 24 48 01       	testb  $0x1,0x48(%rsp)
  4bf6af:	0f 84 f0 01 00 00    	je     4bf8a5 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xcf5>
  4bf6b5:	4c 8b 7c 24 10       	mov    0x10(%rsp),%r15
  4bf6ba:	4c 89 ff             	mov    %r15,%rdi
  4bf6bd:	e8 3e 0f e3 ff       	call   2f0600 <quickjs_oxide::engine::value::primitive::JsString::to_well_formed>
  4bf6c2:	48 89 44 24 78       	mov    %rax,0x78(%rsp)
  4bf6c7:	c6 44 24 70 06       	movb   $0x6,0x70(%rsp)
  4bf6cc:	48 8b 44 24 18       	mov    0x18(%rsp),%rax
  4bf6d1:	48 8b 30             	mov    (%rax),%rsi
  4bf6d4:	48 8d bc 24 b8 01 00 	lea    0x1b8(%rsp),%rdi
  4bf6db:	00 
  4bf6dc:	48 8d 54 24 70       	lea    0x70(%rsp),%rdx
  4bf6e1:	e8 da bf c2 ff       	call   eb6c0 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::into_jsvalue>
  4bf6e6:	0f b6 84 24 b8 01 00 	movzbl 0x1b8(%rsp),%eax
  4bf6ed:	00 
  4bf6ee:	3c 0b                	cmp    $0xb,%al
  4bf6f0:	0f 85 59 02 00 00    	jne    4bf94f <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xd9f>
  4bf6f6:	0f b6 bc 24 c0 01 00 	movzbl 0x1c0(%rsp),%edi
  4bf6fd:	00 
  4bf6fe:	0f b6 94 24 c1 01 00 	movzbl 0x1c1(%rsp),%edx
  4bf705:	00 
  4bf706:	44 0f b7 bc 24 c2 01 	movzwl 0x1c2(%rsp),%r15d
  4bf70d:	00 00 
  4bf70f:	8b b4 24 c4 01 00 00 	mov    0x1c4(%rsp),%esi
  4bf716:	48 8b 8c 24 c8 01 00 	mov    0x1c8(%rsp),%rcx
  4bf71d:	00 
  4bf71e:	e9 99 01 00 00       	jmp    4bf8bc <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xd0c>
  4bf723:	0f b6 54 24 79       	movzbl 0x79(%rsp),%edx
  4bf728:	44 0f b7 7c 24 7a    	movzwl 0x7a(%rsp),%r15d
  4bf72e:	8b 74 24 7c          	mov    0x7c(%rsp),%esi
  4bf732:	f2 0f 11 44 24 08    	movsd  %xmm0,0x8(%rsp)
  4bf738:	c6 44 24 28 0b       	movb   $0xb,0x28(%rsp)
  4bf73d:	b0 01                	mov    $0x1,%al
  4bf73f:	48 89 04 24          	mov    %rax,(%rsp)
  4bf743:	31 ed                	xor    %ebp,%ebp
  4bf745:	45 31 f6             	xor    %r14d,%r14d
  4bf748:	45 31 ed             	xor    %r13d,%r13d
  4bf74b:	e9 52 04 00 00       	jmp    4bfba2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xff2>
  4bf750:	48 c7 44 24 78 00 00 	movq   $0x0,0x78(%rsp)
  4bf757:	00 00 
  4bf759:	48 8d bc 24 b8 01 00 	lea    0x1b8(%rsp),%rdi
  4bf760:	00 
  4bf761:	48 8d 4c 24 70       	lea    0x70(%rsp),%rcx
  4bf766:	be 08 00 00 00       	mov    $0x8,%esi
  4bf76b:	48 89 ea             	mov    %rbp,%rdx
  4bf76e:	e8 5d 91 b9 ff       	call   588d0 <alloc::raw_vec::finish_grow>
  4bf773:	83 bc 24 b8 01 00 00 	cmpl   $0x1,0x1b8(%rsp)
  4bf77a:	01 
  4bf77b:	0f 84 10 f7 ff ff    	je     4bee91 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2e1>
  4bf781:	48 8b 84 24 c0 01 00 	mov    0x1c0(%rsp),%rax
  4bf788:	00 
  4bf789:	48 89 04 24          	mov    %rax,(%rsp)
  4bf78d:	48 89 44 24 58       	mov    %rax,0x58(%rsp)
  4bf792:	4c 89 74 24 50       	mov    %r14,0x50(%rsp)
  4bf797:	49 8b 77 10          	mov    0x10(%r15),%rsi
  4bf79b:	49 39 f6             	cmp    %rsi,%r14
  4bf79e:	0f 87 ad 04 00 00    	ja     4bfc51 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10a1>
  4bf7a4:	4d 8b 6f 08          	mov    0x8(%r15),%r13
  4bf7a8:	4c 89 e8             	mov    %r13,%rax
  4bf7ab:	48 01 e8             	add    %rbp,%rax
  4bf7ae:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf7b3:	49 8d 45 10          	lea    0x10(%r13),%rax
  4bf7b7:	48 8b 4c 24 18       	mov    0x18(%rsp),%rcx
  4bf7bc:	48 8b 09             	mov    (%rcx),%rcx
  4bf7bf:	48 89 4c 24 30       	mov    %rcx,0x30(%rsp)
  4bf7c4:	48 83 c5 f0          	add    $0xfffffffffffffff0,%rbp
  4bf7c8:	48 c1 ed 04          	shr    $0x4,%rbp
  4bf7cc:	48 ff c5             	inc    %rbp
  4bf7cf:	45 31 ff             	xor    %r15d,%r15d
  4bf7d2:	45 31 f6             	xor    %r14d,%r14d
  4bf7d5:	eb 39                	jmp    4bf810 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xc60>
  4bf7d7:	48 8d 84 24 e0 02 00 	lea    0x2e0(%rsp),%rax
  4bf7de:	00 
  4bf7df:	0f 10 00             	movups (%rax),%xmm0
  4bf7e2:	48 8b 04 24          	mov    (%rsp),%rax
  4bf7e6:	42 0f 11 04 38       	movups %xmm0,(%rax,%r15,1)
  4bf7eb:	49 ff c6             	inc    %r14
  4bf7ee:	4c 89 74 24 60       	mov    %r14,0x60(%rsp)
  4bf7f3:	31 c0                	xor    %eax,%eax
  4bf7f5:	4c 3b 6c 24 08       	cmp    0x8(%rsp),%r13
  4bf7fa:	0f 95 c0             	setne  %al
  4bf7fd:	c1 e0 04             	shl    $0x4,%eax
  4bf800:	4c 01 e8             	add    %r13,%rax
  4bf803:	49 83 c7 10          	add    $0x10,%r15
  4bf807:	4c 39 f5             	cmp    %r14,%rbp
  4bf80a:	0f 84 d8 02 00 00    	je     4bfae8 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf38>
  4bf810:	4c 89 ea             	mov    %r13,%rdx
  4bf813:	49 89 c5             	mov    %rax,%r13
  4bf816:	48 8d bc 24 d8 02 00 	lea    0x2d8(%rsp),%rdi
  4bf81d:	00 
  4bf81e:	48 8b 74 24 30       	mov    0x30(%rsp),%rsi
  4bf823:	e8 e8 9d c3 ff       	call   f9610 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::dup_jsvalue>
  4bf828:	80 bc 24 d8 02 00 00 	cmpb   $0xb,0x2d8(%rsp)
  4bf82f:	0b 
  4bf830:	0f 85 ef 01 00 00    	jne    4bfa25 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xe75>
  4bf836:	4c 3b 74 24 50       	cmp    0x50(%rsp),%r14
  4bf83b:	75 9a                	jne    4bf7d7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xc27>
  4bf83d:	48 8d 7c 24 50       	lea    0x50(%rsp),%rdi
  4bf842:	48 8d 35 1f 56 31 00 	lea    0x31561f(%rip),%rsi        # 7d4e68 <__do_global_dtors_aux_fini_array_entry+0xa5b0>
  4bf849:	e8 b2 f5 c1 ff       	call   dee00 <alloc::raw_vec::RawVec<T,A>::grow_one>
  4bf84e:	48 8b 44 24 58       	mov    0x58(%rsp),%rax
  4bf853:	48 89 04 24          	mov    %rax,(%rsp)
  4bf857:	e9 7b ff ff ff       	jmp    4bf7d7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xc27>
  4bf85c:	88 44 24 28          	mov    %al,0x28(%rsp)
  4bf860:	8b 44 24 71          	mov    0x71(%rsp),%eax
  4bf864:	8b 4c 24 74          	mov    0x74(%rsp),%ecx
  4bf868:	89 4c 24 23          	mov    %ecx,0x23(%rsp)
  4bf86c:	89 44 24 20          	mov    %eax,0x20(%rsp)
  4bf870:	48 8b 6c 24 78       	mov    0x78(%rsp),%rbp
  4bf875:	48 8b bc 24 80 00 00 	mov    0x80(%rsp),%rdi
  4bf87c:	00 
  4bf87d:	48 8b 84 24 88 00 00 	mov    0x88(%rsp),%rax
  4bf884:	00 
  4bf885:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf88a:	41 89 ed             	mov    %ebp,%r13d
  4bf88d:	41 c1 ed 08          	shr    $0x8,%r13d
  4bf891:	41 89 ee             	mov    %ebp,%r14d
  4bf894:	41 c1 ee 10          	shr    $0x10,%r14d
  4bf898:	48 89 2c 24          	mov    %rbp,(%rsp)
  4bf89c:	48 c1 ed 20          	shr    $0x20,%rbp
  4bf8a0:	e9 4b f8 ff ff       	jmp    4bf0f0 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x540>
  4bf8a5:	4c 8b 7c 24 10       	mov    0x10(%rsp),%r15
  4bf8aa:	4c 89 ff             	mov    %r15,%rdi
  4bf8ad:	e8 2e 0e e3 ff       	call   2f06e0 <quickjs_oxide::engine::value::primitive::JsString::first_unpaired_surrogate>
  4bf8b2:	48 83 f8 01          	cmp    $0x1,%rax
  4bf8b6:	0f 95 c2             	setne  %dl
  4bf8b9:	40 b7 02             	mov    $0x2,%dil
  4bf8bc:	48 8b 44 24 10       	mov    0x10(%rsp),%rax
  4bf8c1:	48 ff 08             	decq   (%rax)
  4bf8c4:	48 89 4c 24 08       	mov    %rcx,0x8(%rsp)
  4bf8c9:	75 7d                	jne    4bf948 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xd98>
  4bf8cb:	49 89 fe             	mov    %rdi,%r14
  4bf8ce:	48 8d 7c 24 40       	lea    0x40(%rsp),%rdi
  4bf8d3:	49 89 f5             	mov    %rsi,%r13
  4bf8d6:	89 d5                	mov    %edx,%ebp
  4bf8d8:	e8 f3 db be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bf8dd:	89 ea                	mov    %ebp,%edx
  4bf8df:	4c 89 ee             	mov    %r13,%rsi
  4bf8e2:	4c 89 f7             	mov    %r14,%rdi
  4bf8e5:	eb 61                	jmp    4bf948 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xd98>
  4bf8e7:	48 8b 6c 24 10       	mov    0x10(%rsp),%rbp
  4bf8ec:	48 89 6c 24 78       	mov    %rbp,0x78(%rsp)
  4bf8f1:	c6 44 24 70 06       	movb   $0x6,0x70(%rsp)
  4bf8f6:	48 8d bc 24 b8 01 00 	lea    0x1b8(%rsp),%rdi
  4bf8fd:	00 
  4bf8fe:	48 8d 54 24 70       	lea    0x70(%rsp),%rdx
  4bf903:	4c 89 f6             	mov    %r14,%rsi
  4bf906:	e8 b5 bd c2 ff       	call   eb6c0 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::into_jsvalue>
  4bf90b:	0f b6 84 24 b8 01 00 	movzbl 0x1b8(%rsp),%eax
  4bf912:	00 
  4bf913:	3c 0b                	cmp    $0xb,%al
  4bf915:	0f 85 9f 00 00 00    	jne    4bf9ba <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xe0a>
  4bf91b:	0f b6 bc 24 c0 01 00 	movzbl 0x1c0(%rsp),%edi
  4bf922:	00 
  4bf923:	0f b6 94 24 c1 01 00 	movzbl 0x1c1(%rsp),%edx
  4bf92a:	00 
  4bf92b:	44 0f b7 bc 24 c2 01 	movzwl 0x1c2(%rsp),%r15d
  4bf932:	00 00 
  4bf934:	8b b4 24 c4 01 00 00 	mov    0x1c4(%rsp),%esi
  4bf93b:	48 8b 84 24 c8 01 00 	mov    0x1c8(%rsp),%rax
  4bf942:	00 
  4bf943:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf948:	31 c9                	xor    %ecx,%ecx
  4bf94a:	e9 fc fa ff ff       	jmp    4bf44b <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x89b>
  4bf94f:	88 44 24 28          	mov    %al,0x28(%rsp)
  4bf953:	8b 84 24 b9 01 00 00 	mov    0x1b9(%rsp),%eax
  4bf95a:	8b 8c 24 bc 01 00 00 	mov    0x1bc(%rsp),%ecx
  4bf961:	89 4c 24 23          	mov    %ecx,0x23(%rsp)
  4bf965:	89 44 24 20          	mov    %eax,0x20(%rsp)
  4bf969:	0f b6 84 24 c0 01 00 	movzbl 0x1c0(%rsp),%eax
  4bf970:	00 
  4bf971:	48 89 04 24          	mov    %rax,(%rsp)
  4bf975:	44 0f b6 ac 24 c1 01 	movzbl 0x1c1(%rsp),%r13d
  4bf97c:	00 00 
  4bf97e:	44 0f b7 b4 24 c2 01 	movzwl 0x1c2(%rsp),%r14d
  4bf985:	00 00 
  4bf987:	8b ac 24 c4 01 00 00 	mov    0x1c4(%rsp),%ebp
  4bf98e:	8b bc 24 c8 01 00 00 	mov    0x1c8(%rsp),%edi
  4bf995:	8b b4 24 cc 01 00 00 	mov    0x1cc(%rsp),%esi
  4bf99c:	48 8b 84 24 d0 01 00 	mov    0x1d0(%rsp),%rax
  4bf9a3:	00 
  4bf9a4:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf9a9:	89 fa                	mov    %edi,%edx
  4bf9ab:	c1 ea 08             	shr    $0x8,%edx
  4bf9ae:	41 89 ff             	mov    %edi,%r15d
  4bf9b1:	41 c1 ef 10          	shr    $0x10,%r15d
  4bf9b5:	e9 e8 01 00 00       	jmp    4bfba2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xff2>
  4bf9ba:	88 44 24 28          	mov    %al,0x28(%rsp)
  4bf9be:	8b 84 24 b9 01 00 00 	mov    0x1b9(%rsp),%eax
  4bf9c5:	8b 8c 24 bc 01 00 00 	mov    0x1bc(%rsp),%ecx
  4bf9cc:	89 4c 24 23          	mov    %ecx,0x23(%rsp)
  4bf9d0:	89 44 24 20          	mov    %eax,0x20(%rsp)
  4bf9d4:	0f b6 84 24 c0 01 00 	movzbl 0x1c0(%rsp),%eax
  4bf9db:	00 
  4bf9dc:	48 89 04 24          	mov    %rax,(%rsp)
  4bf9e0:	44 0f b6 ac 24 c1 01 	movzbl 0x1c1(%rsp),%r13d
  4bf9e7:	00 00 
  4bf9e9:	44 0f b7 b4 24 c2 01 	movzwl 0x1c2(%rsp),%r14d
  4bf9f0:	00 00 
  4bf9f2:	8b ac 24 c4 01 00 00 	mov    0x1c4(%rsp),%ebp
  4bf9f9:	8b bc 24 c8 01 00 00 	mov    0x1c8(%rsp),%edi
  4bfa00:	8b b4 24 cc 01 00 00 	mov    0x1cc(%rsp),%esi
  4bfa07:	48 8b 84 24 d0 01 00 	mov    0x1d0(%rsp),%rax
  4bfa0e:	00 
  4bfa0f:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bfa14:	89 fa                	mov    %edi,%edx
  4bfa16:	c1 ea 08             	shr    $0x8,%edx
  4bfa19:	41 89 ff             	mov    %edi,%r15d
  4bfa1c:	41 c1 ef 10          	shr    $0x10,%r15d
  4bfa20:	e9 de f6 ff ff       	jmp    4bf103 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x553>
  4bfa25:	48 8b 44 24 50       	mov    0x50(%rsp),%rax
  4bfa2a:	48 89 44 24 18       	mov    %rax,0x18(%rsp)
  4bfa2f:	4d 85 f6             	test   %r14,%r14
  4bfa32:	0f 84 89 00 00 00    	je     4bfac1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf11>
  4bfa38:	4c 8d 64 24 70       	lea    0x70(%rsp),%r12
  4bfa3d:	48 8d ac 24 a0 01 00 	lea    0x1a0(%rsp),%rbp
  4bfa44:	00 
  4bfa45:	4c 8b 2d f4 d3 34 00 	mov    0x34d3f4(%rip),%r13        # 80ce40 <free@GLIBC_2.2.5>
  4bfa4c:	4c 8b 34 24          	mov    (%rsp),%r14
  4bfa50:	eb 1b                	jmp    4bfa6d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xebd>
  4bfa52:	4c 89 e7             	mov    %r12,%rdi
  4bfa55:	41 ff d5             	call   *%r13
  4bfa58:	49 89 ec             	mov    %rbp,%r12
  4bfa5b:	48 8d ac 24 a0 01 00 	lea    0x1a0(%rsp),%rbp
  4bfa62:	00 
  4bfa63:	49 83 c6 10          	add    $0x10,%r14
  4bfa67:	49 83 c7 f0          	add    $0xfffffffffffffff0,%r15
  4bfa6b:	74 54                	je     4bfac1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf11>
  4bfa6d:	41 0f 10 06          	movups (%r14),%xmm0
  4bfa71:	0f 29 84 24 a0 01 00 	movaps %xmm0,0x1a0(%rsp)
  4bfa78:	00 
  4bfa79:	80 bc 24 a0 01 00 00 	cmpb   $0xa,0x1a0(%rsp)
  4bfa80:	0a 
  4bfa81:	74 3e                	je     4bfac1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf11>
  4bfa83:	4c 89 e7             	mov    %r12,%rdi
  4bfa86:	48 8b 74 24 30       	mov    0x30(%rsp),%rsi
  4bfa8b:	48 89 ea             	mov    %rbp,%rdx
  4bfa8e:	e8 8d 9c c1 ff       	call   d9720 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::release_jsvalue>
  4bfa93:	80 7c 24 70 06       	cmpb   $0x6,0x70(%rsp)
  4bfa98:	75 c9                	jne    4bfa63 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xeb3>
  4bfa9a:	4c 89 e5             	mov    %r12,%rbp
  4bfa9d:	4c 8b 64 24 78       	mov    0x78(%rsp),%r12
  4bfaa2:	49 83 7c 24 28 00    	cmpq   $0x0,0x28(%r12)
  4bfaa8:	74 08                	je     4bfab2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf02>
  4bfaaa:	49 8b 7c 24 30       	mov    0x30(%r12),%rdi
  4bfaaf:	41 ff d5             	call   *%r13
  4bfab2:	49 8b 7c 24 40       	mov    0x40(%r12),%rdi
  4bfab7:	48 85 ff             	test   %rdi,%rdi
  4bfaba:	74 96                	je     4bfa52 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xea2>
  4bfabc:	41 ff d5             	call   *%r13
  4bfabf:	eb 91                	jmp    4bfa52 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xea2>
  4bfac1:	48 83 7c 24 18 00    	cmpq   $0x0,0x18(%rsp)
  4bfac7:	74 0a                	je     4bfad3 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf23>
  4bfac9:	48 8b 3c 24          	mov    (%rsp),%rdi
  4bfacd:	ff 15 6d d3 34 00    	call   *0x34d36d(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bfad3:	0f 10 84 24 d8 02 00 	movups 0x2d8(%rsp),%xmm0
  4bfada:	00 
  4bfadb:	0f 10 8c 24 e8 02 00 	movups 0x2e8(%rsp),%xmm1
  4bfae2:	00 
  4bfae3:	e9 be f8 ff ff       	jmp    4bf3a6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7f6>
  4bfae8:	48 8b 44 24 50       	mov    0x50(%rsp),%rax
  4bfaed:	48 89 44 24 30       	mov    %rax,0x30(%rsp)
  4bfaf2:	4c 8b 6c 24 58       	mov    0x58(%rsp),%r13
  4bfaf7:	e9 90 f6 ff ff       	jmp    4bf18c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5dc>
  4bfafc:	72 76                	jb     4bfb74 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfc4>
  4bfafe:	48 c7 04 24 00 00 00 	movq   $0x0,(%rsp)
  4bfb05:	00 
  4bfb06:	eb 6c                	jmp    4bfb74 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xfc4>
  4bfb08:	0f b6 7c 24 71       	movzbl 0x71(%rsp),%edi
  4bfb0d:	e8 0e 5d c5 ff       	call   115820 <quickjs_oxide::engine::value::primitive::<impl core::convert::From<quickjs_oxide::engine::value::primitive::JsStringError> for quickjs_oxide::engine::api::error::Error>::from>
  4bfb12:	41 89 c5             	mov    %eax,%r13d
  4bfb15:	41 c1 ed 08          	shr    $0x8,%r13d
  4bfb19:	41 89 c6             	mov    %eax,%r14d
  4bfb1c:	41 c1 ee 10          	shr    $0x10,%r14d
  4bfb20:	48 89 04 24          	mov    %rax,(%rsp)
  4bfb24:	48 89 c5             	mov    %rax,%rbp
  4bfb27:	48 c1 ed 20          	shr    $0x20,%rbp
  4bfb2b:	48 8b 44 24 08       	mov    0x8(%rsp),%rax
  4bfb30:	48 ff 08             	decq   (%rax)
  4bfb33:	c6 44 24 28 06       	movb   $0x6,0x28(%rsp)
  4bfb38:	75 12                	jne    4bfb4c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf9c>
  4bfb3a:	48 8d bc 24 b8 01 00 	lea    0x1b8(%rsp),%rdi
  4bfb41:	00 
  4bfb42:	4c 8b 7c 24 10       	mov    0x10(%rsp),%r15
  4bfb47:	e8 84 d9 be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bfb4c:	eb 54                	jmp    4bfba2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xff2>
  4bfb4e:	48 89 6c 24 08       	mov    %rbp,0x8(%rsp)
  4bfb53:	0f b6 54 24 79       	movzbl 0x79(%rsp),%edx
  4bfb58:	44 0f b7 7c 24 7a    	movzwl 0x7a(%rsp),%r15d
  4bfb5e:	8b 74 24 7c          	mov    0x7c(%rsp),%esi
  4bfb62:	e9 d1 fb ff ff       	jmp    4bf738 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xb88>
  4bfb67:	b1 05                	mov    $0x5,%cl
  4bfb69:	48 89 0c 24          	mov    %rcx,(%rsp)
  4bfb6d:	48 8d 3d ea 2e 1e 00 	lea    0x1e2eea(%rip),%rdi        # 6a2a5e <num_bigint::biguint::convert::get_radix_base::BASES+0x27cae>
  4bfb74:	89 fa                	mov    %edi,%edx
  4bfb76:	c1 ea 08             	shr    $0x8,%edx
  4bfb79:	41 89 ff             	mov    %edi,%r15d
  4bfb7c:	41 c1 ef 10          	shr    $0x10,%r15d
  4bfb80:	48 89 fe             	mov    %rdi,%rsi
  4bfb83:	48 c1 ee 20          	shr    $0x20,%rsi
  4bfb87:	49 89 46 28          	mov    %rax,0x28(%r14)
  4bfb8b:	c6 44 24 28 08       	movb   $0x8,0x28(%rsp)
  4bfb90:	b8 30 00 00 00       	mov    $0x30,%eax
  4bfb95:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bfb9a:	41 b5 05             	mov    $0x5,%r13b
  4bfb9d:	66 41 be 06 00       	mov    $0x6,%r14w
  4bfba2:	48 8b 44 24 10       	mov    0x10(%rsp),%rax
  4bfba7:	48 ff 08             	decq   (%rax)
  4bfbaa:	75 2c                	jne    4bfbd8 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1028>
  4bfbac:	48 89 7c 24 10       	mov    %rdi,0x10(%rsp)
  4bfbb1:	48 8d 7c 24 40       	lea    0x40(%rsp),%rdi
  4bfbb6:	48 89 b4 24 b0 01 00 	mov    %rsi,0x1b0(%rsp)
  4bfbbd:	00 
  4bfbbe:	89 54 24 6c          	mov    %edx,0x6c(%rsp)
  4bfbc2:	e8 09 d9 be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bfbc7:	8b 54 24 6c          	mov    0x6c(%rsp),%edx
  4bfbcb:	48 8b b4 24 b0 01 00 	mov    0x1b0(%rsp),%rsi
  4bfbd2:	00 
  4bfbd3:	48 8b 7c 24 10       	mov    0x10(%rsp),%rdi
  4bfbd8:	80 7c 24 28 0b       	cmpb   $0xb,0x28(%rsp)
  4bfbdd:	0f 85 20 f5 ff ff    	jne    4bf103 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x553>
  4bfbe3:	e9 35 f2 ff ff       	jmp    4bee1d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x26d>
  4bfbe8:	bf 08 00 00 00       	mov    $0x8,%edi
  4bfbed:	be 10 00 00 00       	mov    $0x10,%esi
  4bfbf2:	e8 e5 7f b9 ff       	call   57bdc <alloc::alloc::handle_alloc_error>
  4bfbf7:	0f b6 44 24 71       	movzbl 0x71(%rsp),%eax
  4bfbfc:	88 84 24 b8 01 00 00 	mov    %al,0x1b8(%rsp)
  4bfc03:	48 8d 3d 76 db 1e 00 	lea    0x1edb76(%rip),%rdi        # 6ad780 <num_bigint::biguint::convert::get_radix_base::BASES+0x329d0>
  4bfc0a:	48 8d 0d 67 ca 30 00 	lea    0x30ca67(%rip),%rcx        # 7cc678 <__do_global_dtors_aux_fini_array_entry+0x1dc0>
  4bfc11:	4c 8d 05 88 13 32 00 	lea    0x321388(%rip),%r8        # 7e0fa0 <__do_global_dtors_aux_fini_array_entry+0x166e8>
  4bfc18:	48 8d 94 24 b8 01 00 	lea    0x1b8(%rsp),%rdx
  4bfc1f:	00 
  4bfc20:	be 36 00 00 00       	mov    $0x36,%esi
  4bfc25:	e8 f6 86 b9 ff       	call   58320 <core::result::unwrap_failed>
  4bfc2a:	eb 47                	jmp    4bfc73 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10c3>
  4bfc2c:	bf 08 00 00 00       	mov    $0x8,%edi
  4bfc31:	be 40 00 00 00       	mov    $0x40,%esi
  4bfc36:	e8 a1 7f b9 ff       	call   57bdc <alloc::alloc::handle_alloc_error>
  4bfc3b:	eb 36                	jmp    4bfc73 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10c3>
  4bfc3d:	48 8d 15 94 ce 30 00 	lea    0x30ce94(%rip),%rdx        # 7ccad8 <__do_global_dtors_aux_fini_array_entry+0x2220>
  4bfc44:	be 00 01 00 00       	mov    $0x100,%esi
  4bfc49:	4c 89 f7             	mov    %r14,%rdi
  4bfc4c:	e8 01 82 b9 ff       	call   57e52 <core::panicking::panic_bounds_check>
  4bfc51:	48 8d 15 f8 51 31 00 	lea    0x3151f8(%rip),%rdx        # 7d4e50 <__do_global_dtors_aux_fini_array_entry+0xa598>
  4bfc58:	4c 89 f7             	mov    %r14,%rdi
  4bfc5b:	e8 f0 82 b9 ff       	call   57f50 <core::slice::index::slice_end_index_len_fail>
  4bfc60:	eb 11                	jmp    4bfc73 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10c3>
  4bfc62:	48 8d 3d 47 52 31 00 	lea    0x315247(%rip),%rdi        # 7d4eb0 <__do_global_dtors_aux_fini_array_entry+0xa5f8>
  4bfc69:	4c 8b 7c 24 10       	mov    0x10(%rsp),%r15
  4bfc6e:	e8 4d 86 b9 ff       	call   582c0 <core::cell::panic_already_mutably_borrowed>
  4bfc73:	0f 0b                	ud2
  4bfc75:	eb 3b                	jmp    4bfcb2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1102>
  4bfc77:	48 89 c3             	mov    %rax,%rbx
  4bfc7a:	48 89 6c 24 40       	mov    %rbp,0x40(%rsp)
  4bfc7f:	48 89 6c 24 10       	mov    %rbp,0x10(%rsp)
  4bfc84:	eb 2f                	jmp    4bfcb5 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1105>
  4bfc86:	48 89 c3             	mov    %rax,%rbx
  4bfc89:	48 83 7c 24 18 00    	cmpq   $0x0,0x18(%rsp)
  4bfc8f:	74 0a                	je     4bfc9b <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x10eb>
  4bfc91:	48 8b 3c 24          	mov    (%rsp),%rdi
  4bfc95:	ff 15 a5 d1 34 00    	call   *0x34d1a5(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bfc9b:	48 8d bc 24 d8 02 00 	lea    0x2d8(%rsp),%rdi
  4bfca2:	00 
  4bfca3:	e8 68 b6 c3 ff       	call   fb310 <core::ptr::drop_in_place<quickjs_oxide::engine::api::runtime_error::RuntimeError>>
  4bfca8:	48 89 df             	mov    %rbx,%rdi
  4bfcab:	e8 80 73 b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bfcb0:	eb 21                	jmp    4bfcd3 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1123>
  4bfcb2:	48 89 c3             	mov    %rax,%rbx
  4bfcb5:	48 8b 44 24 08       	mov    0x8(%rsp),%rax
  4bfcba:	48 ff 08             	decq   (%rax)
  4bfcbd:	75 17                	jne    4bfcd6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1126>
  4bfcbf:	48 8d bc 24 b8 01 00 	lea    0x1b8(%rsp),%rdi
  4bfcc6:	00 
  4bfcc7:	e8 04 d8 be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bfccc:	eb 08                	jmp    4bfcd6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1126>
  4bfcce:	4c 89 7c 24 10       	mov    %r15,0x10(%rsp)
  4bfcd3:	48 89 c3             	mov    %rax,%rbx
  4bfcd6:	48 8b 44 24 10       	mov    0x10(%rsp),%rax
  4bfcdb:	48 ff 08             	decq   (%rax)
  4bfcde:	0f 85 a4 00 00 00    	jne    4bfd88 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11d8>
  4bfce4:	48 8d 7c 24 40       	lea    0x40(%rsp),%rdi
  4bfce9:	e8 e2 d7 be ff       	call   ad4d0 <alloc::rc::Rc<T,A>::drop_slow>
  4bfcee:	e9 95 00 00 00       	jmp    4bfd88 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11d8>
  4bfcf3:	e8 07 88 b9 ff       	call   584ff <core::panicking::panic_in_cleanup>
  4bfcf8:	48 89 c3             	mov    %rax,%rbx
  4bfcfb:	48 83 7c 24 50 00    	cmpq   $0x0,0x50(%rsp)
  4bfd01:	0f 84 81 00 00 00    	je     4bfd88 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11d8>
  4bfd07:	48 8b 44 24 58       	mov    0x58(%rsp),%rax
  4bfd0c:	48 89 04 24          	mov    %rax,(%rsp)
  4bfd10:	eb 7e                	jmp    4bfd90 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11e0>
  4bfd12:	48 89 c3             	mov    %rax,%rbx
  4bfd15:	4c 89 ef             	mov    %r13,%rdi
  4bfd18:	ff 15 22 d1 34 00    	call   *0x34d122(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bfd1e:	48 89 df             	mov    %rbx,%rdi
  4bfd21:	e8 0a 73 b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bfd26:	48 89 c3             	mov    %rax,%rbx
  4bfd29:	eb 55                	jmp    4bfd80 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11d0>
  4bfd2b:	48 89 c3             	mov    %rax,%rbx
  4bfd2e:	48 83 7c 24 30 00    	cmpq   $0x0,0x30(%rsp)
  4bfd34:	74 0a                	je     4bfd40 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1190>
  4bfd36:	48 8b 3c 24          	mov    (%rsp),%rdi
  4bfd3a:	ff 15 00 d1 34 00    	call   *0x34d100(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bfd40:	48 8d bc 24 f8 02 00 	lea    0x2f8(%rsp),%rdi
  4bfd47:	00 
  4bfd48:	e8 c3 b5 c3 ff       	call   fb310 <core::ptr::drop_in_place<quickjs_oxide::engine::api::runtime_error::RuntimeError>>
  4bfd4d:	48 89 df             	mov    %rbx,%rdi
  4bfd50:	e8 db 72 b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bfd55:	48 89 c3             	mov    %rax,%rbx
  4bfd58:	eb 36                	jmp    4bfd90 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11e0>
  4bfd5a:	48 89 c3             	mov    %rax,%rbx
  4bfd5d:	48 8d 7c 24 70       	lea    0x70(%rsp),%rdi
  4bfd62:	e8 c9 6b d3 ff       	call   1f6930 <core::ptr::drop_in_place<quickjs_oxide::engine::builtins::primitive::text::ScalarTextResumeState>>
  4bfd67:	eb 1f                	jmp    4bfd88 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11d8>
  4bfd69:	e8 91 87 b9 ff       	call   584ff <core::panicking::panic_in_cleanup>
  4bfd6e:	48 89 c3             	mov    %rax,%rbx
  4bfd71:	49 ff 0f             	decq   (%r15)
  4bfd74:	75 0a                	jne    4bfd80 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11d0>
  4bfd76:	48 8b 7c 24 18       	mov    0x18(%rsp),%rdi
  4bfd7b:	e8 c0 42 c0 ff       	call   c4040 <alloc::rc::Rc<T,A>::drop_slow>
  4bfd80:	48 83 7c 24 30 00    	cmpq   $0x0,0x30(%rsp)
  4bfd86:	75 08                	jne    4bfd90 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x11e0>
  4bfd88:	48 89 df             	mov    %rbx,%rdi
  4bfd8b:	e8 a0 72 b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bfd90:	48 8b 3c 24          	mov    (%rsp),%rdi
  4bfd94:	ff 15 a6 d0 34 00    	call   *0x34d0a6(%rip)        # 80ce40 <free@GLIBC_2.2.5>
  4bfd9a:	48 89 df             	mov    %rbx,%rdi
  4bfd9d:	e8 8e 72 b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bfda2:	e8 58 87 b9 ff       	call   584ff <core::panicking::panic_in_cleanup>
