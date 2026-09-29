
/tmp/oxide-three-build-parent/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

00000000004be8e0 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start>:
  4be8e0:	55                   	push   %rbp
  4be8e1:	41 57                	push   %r15
  4be8e3:	41 56                	push   %r14
  4be8e5:	41 55                	push   %r13
  4be8e7:	41 54                	push   %r12
  4be8e9:	53                   	push   %rbx
  4be8ea:	48 81 ec f8 02 00 00 	sub    $0x2f8,%rsp
  4be8f1:	48 89 fb             	mov    %rdi,%rbx
  4be8f4:	4c 8b b4 24 30 03 00 	mov    0x330(%rsp),%r14
  4be8fb:	00 
  4be8fc:	49 83 3e 00          	cmpq   $0x0,(%r14)
  4be900:	74 23                	je     4be925 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x45>
  4be902:	c6 43 08 04          	movb   $0x4,0x8(%rbx)
  4be906:	48 8d 05 26 eb 1c 00 	lea    0x1ceb26(%rip),%rax        # 68d433 <num_bigint::biguint::convert::get_radix_base::BASES+0x137a3>
  4be90d:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4be911:	48 c7 43 18 30 00 00 	movq   $0x30,0x18(%rbx)
  4be918:	00 
  4be919:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  4be920:	e9 f8 03 00 00       	jmp    4bed1d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x43d>
  4be925:	49 89 f7             	mov    %rsi,%r15
  4be928:	41 80 7e 08 02       	cmpb   $0x2,0x8(%r14)
  4be92d:	0f 83 15 01 00 00    	jae    4bea48 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x168>
  4be933:	41 89 d4             	mov    %edx,%r12d
  4be936:	89 cd                	mov    %ecx,%ebp
  4be938:	48 8d bc 24 90 00 00 	lea    0x90(%rsp),%rdi
  4be93f:	00 
  4be940:	45 31 f6             	xor    %r14d,%r14d
  4be943:	ba 08 01 00 00       	mov    $0x108,%edx
  4be948:	31 f6                	xor    %esi,%esi
  4be94a:	ff 15 08 c4 34 00    	call   *0x34c408(%rip)        # 80ad58 <memset@GLIBC_2.2.5>
  4be950:	b8 01 00 00 00       	mov    $0x1,%eax
  4be955:	48 8d 0d dd c6 1c 00 	lea    0x1cc6dd(%rip),%rcx        # 68b039 <num_bigint::biguint::convert::get_radix_base::BASES+0x113a9>
  4be95c:	49 81 fe ff 00 00 00 	cmp    $0xff,%r14
  4be963:	74 72                	je     4be9d7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf7>
  4be965:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4be96c:	00 00 00 00 
  4be970:	0f 87 7a 07 00 00    	ja     4bf0f0 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x810>
  4be976:	0f b6 54 08 ff       	movzbl -0x1(%rax,%rcx,1),%edx
  4be97b:	42 88 94 34 90 00 00 	mov    %dl,0x90(%rsp,%r14,1)
  4be982:	00 
  4be983:	4c 8b b4 24 90 01 00 	mov    0x190(%rsp),%r14
  4be98a:	00 
  4be98b:	49 ff c6             	inc    %r14
  4be98e:	4c 89 b4 24 90 01 00 	mov    %r14,0x190(%rsp)
  4be995:	00 
  4be996:	48 83 f8 1f          	cmp    $0x1f,%rax
  4be99a:	74 3b                	je     4be9d7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf7>
  4be99c:	49 81 fe ff 00 00 00 	cmp    $0xff,%r14
  4be9a3:	74 32                	je     4be9d7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0xf7>
  4be9a5:	0f 87 45 07 00 00    	ja     4bf0f0 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x810>
  4be9ab:	0f b6 14 08          	movzbl (%rax,%rcx,1),%edx
  4be9af:	42 88 94 34 90 00 00 	mov    %dl,0x90(%rsp,%r14,1)
  4be9b6:	00 
  4be9b7:	4c 8b b4 24 90 01 00 	mov    0x190(%rsp),%r14
  4be9be:	00 
  4be9bf:	49 ff c6             	inc    %r14
  4be9c2:	4c 89 b4 24 90 01 00 	mov    %r14,0x190(%rsp)
  4be9c9:	00 
  4be9ca:	48 83 c0 02          	add    $0x2,%rax
  4be9ce:	49 81 fe ff 00 00 00 	cmp    $0xff,%r14
  4be9d5:	75 99                	jne    4be970 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x90>
  4be9d7:	4c 8d b4 24 f0 01 00 	lea    0x1f0(%rsp),%r14
  4be9de:	00 
  4be9df:	48 8d b4 24 90 00 00 	lea    0x90(%rsp),%rsi
  4be9e6:	00 
  4be9e7:	ba 08 01 00 00       	mov    $0x108,%edx
  4be9ec:	4c 89 f7             	mov    %r14,%rdi
  4be9ef:	ff 15 eb c5 34 00    	call   *0x34c5eb(%rip)        # 80afe0 <memcpy@GLIBC_2.14>
  4be9f5:	48 8d 7c 24 50       	lea    0x50(%rsp),%rdi
  4be9fa:	4c 89 fe             	mov    %r15,%rsi
  4be9fd:	44 89 e2             	mov    %r12d,%edx
  4bea00:	89 e9                	mov    %ebp,%ecx
  4bea02:	41 b8 04 00 00 00    	mov    $0x4,%r8d
  4bea08:	4d 89 f1             	mov    %r14,%r9
  4bea0b:	e8 20 9e c6 ff       	call   128830 <quickjs_oxide::engine::builtins::error::construction::<impl quickjs_oxide::engine::api::runtime::Runtime>::new_native_error_from_message_jsvalue>
  4bea10:	0f b6 4c 24 50       	movzbl 0x50(%rsp),%ecx
  4bea15:	80 f9 0b             	cmp    $0xb,%cl
  4bea18:	0f 85 37 01 00 00    	jne    4beb55 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x275>
  4bea1e:	0f 10 44 24 58       	movups 0x58(%rsp),%xmm0
  4bea23:	0f 11 84 24 97 00 00 	movups %xmm0,0x97(%rsp)
  4bea2a:	00 
  4bea2b:	0f 11 43 18          	movups %xmm0,0x18(%rbx)
  4bea2f:	48 c7 43 08 00 00 00 	movq   $0x0,0x8(%rbx)
  4bea36:	00 
  4bea37:	b8 01 00 00 00       	mov    $0x1,%eax
  4bea3c:	ba 10 00 00 00       	mov    $0x10,%edx
  4bea41:	31 c9                	xor    %ecx,%ecx
  4bea43:	e9 51 01 00 00       	jmp    4beb99 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2b9>
  4bea48:	45 89 c5             	mov    %r8d,%r13d
  4bea4b:	44 89 4c 24 48       	mov    %r9d,0x48(%rsp)
  4bea50:	89 54 24 4c          	mov    %edx,0x4c(%rsp)
  4bea54:	89 4c 24 20          	mov    %ecx,0x20(%rsp)
  4bea58:	4c 8b a4 24 38 03 00 	mov    0x338(%rsp),%r12
  4bea5f:	00 
  4bea60:	41 80 fd 03          	cmp    $0x3,%r13b
  4bea64:	75 5c                	jne    4beac2 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1e2>
  4bea66:	48 c7 44 24 40 00 00 	movq   $0x0,0x40(%rsp)
  4bea6d:	00 00 
  4bea6f:	49 8b 6c 24 18       	mov    0x18(%r12),%rbp
  4bea74:	48 85 ed             	test   %rbp,%rbp
  4bea77:	0f 84 28 01 00 00    	je     4beba5 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2c5>
  4bea7d:	48 89 ea             	mov    %rbp,%rdx
  4bea80:	48 c1 e2 04          	shl    $0x4,%rdx
  4bea84:	48 89 e8             	mov    %rbp,%rax
  4bea87:	48 c1 e8 3c          	shr    $0x3c,%rax
  4bea8b:	0f 95 c0             	setne  %al
  4bea8e:	48 b9 f8 ff ff ff ff 	movabs $0x7ffffffffffffff8,%rcx
  4bea95:	ff ff 7f 
  4bea98:	48 39 ca             	cmp    %rcx,%rdx
  4bea9b:	0f 97 c1             	seta   %cl
  4bea9e:	08 c1                	or     %al,%cl
  4beaa0:	0f 84 e4 03 00 00    	je     4bee8a <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x5aa>
  4beaa6:	c6 43 08 04          	movb   $0x4,0x8(%rbx)
  4beaaa:	48 8d 05 39 e9 1c 00 	lea    0x1ce939(%rip),%rax        # 68d3ea <num_bigint::biguint::convert::get_radix_base::BASES+0x1375a>
  4beab1:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4beab5:	48 c7 43 18 24 00 00 	movq   $0x24,0x18(%rbx)
  4beabc:	00 
  4beabd:	e9 57 fe ff ff       	jmp    4be919 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x39>
  4beac2:	0f b6 05 0c d0 34 00 	movzbl 0x34d00c(%rip),%eax        # 80bad5 <__rust_no_alloc_shim_is_unstable>
  4beac9:	bf 10 00 00 00       	mov    $0x10,%edi
  4beace:	ff 15 14 c3 34 00    	call   *0x34c314(%rip)        # 80ade8 <malloc@GLIBC_2.2.5>
  4bead4:	48 85 c0             	test   %rax,%rax
  4bead7:	0f 84 bb 05 00 00    	je     4bf098 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7b8>
  4beadd:	48 89 c5             	mov    %rax,%rbp
  4beae0:	49 83 7c 24 10 00    	cmpq   $0x0,0x10(%r12)
  4beae6:	0f 84 cc 00 00 00    	je     4bebb8 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2d8>
  4beaec:	49 8b 54 24 08       	mov    0x8(%r12),%rdx
  4beaf1:	49 8b 37             	mov    (%r15),%rsi
  4beaf4:	48 8d bc 24 90 00 00 	lea    0x90(%rsp),%rdi
  4beafb:	00 
  4beafc:	e8 0f ab c3 ff       	call   f9610 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::dup_jsvalue>
  4beb01:	0f b6 84 24 90 00 00 	movzbl 0x90(%rsp),%eax
  4beb08:	00 
  4beb09:	3c 0b                	cmp    $0xb,%al
  4beb0b:	0f 85 f2 02 00 00    	jne    4bee03 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x523>
  4beb11:	0f b6 84 24 98 00 00 	movzbl 0x98(%rsp),%eax
  4beb18:	00 
  4beb19:	48 8b 8c 24 99 00 00 	mov    0x99(%rsp),%rcx
  4beb20:	00 
  4beb21:	48 89 8c 24 f0 01 00 	mov    %rcx,0x1f0(%rsp)
  4beb28:	00 
  4beb29:	48 8b 8c 24 a0 00 00 	mov    0xa0(%rsp),%rcx
  4beb30:	00 
  4beb31:	48 89 8c 24 f7 01 00 	mov    %rcx,0x1f7(%rsp)
  4beb38:	00 
  4beb39:	48 8b 8c 24 f0 01 00 	mov    0x1f0(%rsp),%rcx
  4beb40:	00 
  4beb41:	48 8b 94 24 f7 01 00 	mov    0x1f7(%rsp),%rdx
  4beb48:	00 
  4beb49:	48 89 4c 24 50       	mov    %rcx,0x50(%rsp)
  4beb4e:	48 89 54 24 57       	mov    %rdx,0x57(%rsp)
  4beb53:	eb 65                	jmp    4bebba <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2da>
  4beb55:	48 8b 44 24 60       	mov    0x60(%rsp),%rax
  4beb5a:	48 89 84 24 9f 00 00 	mov    %rax,0x9f(%rsp)
  4beb61:	00 
  4beb62:	0f 10 44 24 51       	movups 0x51(%rsp),%xmm0
  4beb67:	0f 29 84 24 90 00 00 	movaps %xmm0,0x90(%rsp)
  4beb6e:	00 
  4beb6f:	48 8b 44 24 68       	mov    0x68(%rsp),%rax
  4beb74:	48 8b 94 24 9f 00 00 	mov    0x9f(%rsp),%rdx
  4beb7b:	00 
  4beb7c:	48 89 53 18          	mov    %rdx,0x18(%rbx)
  4beb80:	0f 28 84 24 90 00 00 	movaps 0x90(%rsp),%xmm0
  4beb87:	00 
  4beb88:	0f 11 43 09          	movups %xmm0,0x9(%rbx)
  4beb8c:	88 4b 08             	mov    %cl,0x8(%rbx)
  4beb8f:	b9 01 00 00 00       	mov    $0x1,%ecx
  4beb94:	ba 20 00 00 00       	mov    $0x20,%edx
  4beb99:	48 89 04 13          	mov    %rax,(%rbx,%rdx,1)
  4beb9d:	48 89 0b             	mov    %rcx,(%rbx)
  4beba0:	e9 78 01 00 00       	jmp    4bed1d <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x43d>
  4beba5:	bd 08 00 00 00       	mov    $0x8,%ebp
  4bebaa:	45 31 e4             	xor    %r12d,%r12d
  4bebad:	48 c7 44 24 10 00 00 	movq   $0x0,0x10(%rsp)
  4bebb4:	00 00 
  4bebb6:	eb 27                	jmp    4bebdf <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2ff>
  4bebb8:	31 c0                	xor    %eax,%eax
  4bebba:	88 45 00             	mov    %al,0x0(%rbp)
  4bebbd:	48 8b 44 24 50       	mov    0x50(%rsp),%rax
  4bebc2:	48 8b 4c 24 57       	mov    0x57(%rsp),%rcx
  4bebc7:	48 89 45 01          	mov    %rax,0x1(%rbp)
  4bebcb:	48 89 4d 08          	mov    %rcx,0x8(%rbp)
  4bebcf:	41 bc 01 00 00 00    	mov    $0x1,%r12d
  4bebd5:	b8 01 00 00 00       	mov    $0x1,%eax
  4bebda:	48 89 44 24 10       	mov    %rax,0x10(%rsp)
  4bebdf:	49 83 c6 08          	add    $0x8,%r14
  4bebe3:	4d 8b 3f             	mov    (%r15),%r15
  4bebe6:	48 8d bc 24 d0 01 00 	lea    0x1d0(%rsp),%rdi
  4bebed:	00 
  4bebee:	4c 89 fe             	mov    %r15,%rsi
  4bebf1:	4c 89 f2             	mov    %r14,%rdx
  4bebf4:	48 89 6c 24 08       	mov    %rbp,0x8(%rsp)
  4bebf9:	e8 12 aa c3 ff       	call   f9610 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::dup_jsvalue>
  4bebfe:	80 bc 24 d0 01 00 00 	cmpb   $0xb,0x1d0(%rsp)
  4bec05:	0b 
  4bec06:	4c 89 7c 24 18       	mov    %r15,0x18(%rsp)
  4bec0b:	0f 85 1e 01 00 00    	jne    4bed2f <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x44f>
  4bec11:	49 ff 07             	incq   (%r15)
  4bec14:	0f 84 f9 04 00 00    	je     4bf113 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x833>
  4bec1a:	48 8d bc 24 90 00 00 	lea    0x90(%rsp),%rdi
  4bec21:	00 
  4bec22:	be 01 00 00 00       	mov    $0x1,%esi
  4bec27:	31 d2                	xor    %edx,%edx
  4bec29:	e8 12 3c c5 ff       	call   112840 <quickjs_oxide::engine::value::primitive::JsString::try_from_utf8>
  4bec2e:	80 bc 24 90 00 00 00 	cmpb   $0x1,0x90(%rsp)
  4bec35:	01 
  4bec36:	0f 84 6b 04 00 00    	je     4bf0a7 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7c7>
  4bec3c:	48 8b 84 24 98 00 00 	mov    0x98(%rsp),%rax
  4bec43:	00 
  4bec44:	49 c1 e4 04          	shl    $0x4,%r12
  4bec48:	49 01 ec             	add    %rbp,%r12
  4bec4b:	48 89 ac 24 a0 00 00 	mov    %rbp,0xa0(%rsp)
  4bec52:	00 
  4bec53:	48 8b 4c 24 10       	mov    0x10(%rsp),%rcx
  4bec58:	48 89 8c 24 b0 00 00 	mov    %rcx,0xb0(%rsp)
  4bec5f:	00 
  4bec60:	48 89 ac 24 a8 00 00 	mov    %rbp,0xa8(%rsp)
  4bec67:	00 
  4bec68:	4c 89 a4 24 b8 00 00 	mov    %r12,0xb8(%rsp)
  4bec6f:	00 
  4bec70:	4c 89 bc 24 90 00 00 	mov    %r15,0x90(%rsp)
  4bec77:	00 
  4bec78:	8b 4c 24 4c          	mov    0x4c(%rsp),%ecx
  4bec7c:	89 8c 24 c0 00 00 00 	mov    %ecx,0xc0(%rsp)
  4bec83:	8b 4c 24 20          	mov    0x20(%rsp),%ecx
  4bec87:	89 8c 24 c4 00 00 00 	mov    %ecx,0xc4(%rsp)
  4bec8e:	44 88 ac 24 c8 00 00 	mov    %r13b,0xc8(%rsp)
  4bec95:	00 
  4bec96:	8b 4c 24 48          	mov    0x48(%rsp),%ecx
  4bec9a:	88 8c 24 c9 00 00 00 	mov    %cl,0xc9(%rsp)
  4beca1:	c6 84 24 ca 00 00 00 	movb   $0x0,0xca(%rsp)
  4beca8:	00 
  4beca9:	48 89 84 24 98 00 00 	mov    %rax,0x98(%rsp)
  4becb0:	00 
  4becb1:	0f b6 05 1d ce 34 00 	movzbl 0x34ce1d(%rip),%eax        # 80bad5 <__rust_no_alloc_shim_is_unstable>
  4becb8:	bf 40 00 00 00       	mov    $0x40,%edi
  4becbd:	ff 15 25 c1 34 00    	call   *0x34c125(%rip)        # 80ade8 <malloc@GLIBC_2.2.5>
  4becc3:	48 85 c0             	test   %rax,%rax
  4becc6:	0f 84 13 04 00 00    	je     4bf0df <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7ff>
  4beccc:	48 8d 8c 24 d8 01 00 	lea    0x1d8(%rsp),%rcx
  4becd3:	00 
  4becd4:	0f 10 84 24 90 00 00 	movups 0x90(%rsp),%xmm0
  4becdb:	00 
  4becdc:	0f 10 8c 24 a0 00 00 	movups 0xa0(%rsp),%xmm1
  4bece3:	00 
  4bece4:	0f 10 94 24 b0 00 00 	movups 0xb0(%rsp),%xmm2
  4beceb:	00 
  4becec:	0f 10 9c 24 c0 00 00 	movups 0xc0(%rsp),%xmm3
  4becf3:	00 
  4becf4:	0f 11 58 30          	movups %xmm3,0x30(%rax)
  4becf8:	0f 11 50 20          	movups %xmm2,0x20(%rax)
  4becfc:	0f 11 48 10          	movups %xmm1,0x10(%rax)
  4bed00:	0f 11 00             	movups %xmm0,(%rax)
  4bed03:	0f 10 01             	movups (%rcx),%xmm0
  4bed06:	0f 11 43 18          	movups %xmm0,0x18(%rbx)
  4bed0a:	48 c7 43 08 01 00 00 	movq   $0x1,0x8(%rbx)
  4bed11:	00 
  4bed12:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4bed16:	48 c7 03 00 00 00 00 	movq   $0x0,(%rbx)
  4bed1d:	48 81 c4 f8 02 00 00 	add    $0x2f8,%rsp
  4bed24:	5b                   	pop    %rbx
  4bed25:	41 5c                	pop    %r12
  4bed27:	41 5d                	pop    %r13
  4bed29:	41 5e                	pop    %r14
  4bed2b:	41 5f                	pop    %r15
  4bed2d:	5d                   	pop    %rbp
  4bed2e:	c3                   	ret
  4bed2f:	4d 85 e4             	test   %r12,%r12
  4bed32:	0f 84 9d 00 00 00    	je     4bedd5 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x4f5>
  4bed38:	49 c1 e4 04          	shl    $0x4,%r12
  4bed3c:	45 31 f6             	xor    %r14d,%r14d
  4bed3f:	4c 8d ac 24 90 00 00 	lea    0x90(%rsp),%r13
  4bed46:	00 
  4bed47:	eb 23                	jmp    4bed6c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x48c>
  4bed49:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)
  4bed50:	4c 89 ef             	mov    %r13,%rdi
  4bed53:	41 ff d7             	call   *%r15
  4bed56:	49 89 ed             	mov    %rbp,%r13
  4bed59:	48 8b 6c 24 08       	mov    0x8(%rsp),%rbp
  4bed5e:	4c 8b 7c 24 18       	mov    0x18(%rsp),%r15
  4bed63:	49 83 c6 10          	add    $0x10,%r14
  4bed67:	4d 39 f4             	cmp    %r14,%r12
  4bed6a:	74 69                	je     4bedd5 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x4f5>
  4bed6c:	42 0f 10 44 35 00    	movups 0x0(%rbp,%r14,1),%xmm0
  4bed72:	0f 29 84 24 a0 01 00 	movaps %xmm0,0x1a0(%rsp)
  4bed79:	00 
  4bed7a:	80 bc 24 a0 01 00 00 	cmpb   $0xa,0x1a0(%rsp)
  4bed81:	0a 
  4bed82:	74 51                	je     4bedd5 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x4f5>
  4bed84:	4c 89 ef             	mov    %r13,%rdi
  4bed87:	4c 89 fe             	mov    %r15,%rsi
  4bed8a:	48 8d 94 24 a0 01 00 	lea    0x1a0(%rsp),%rdx
  4bed91:	00 
  4bed92:	e8 89 a9 c1 ff       	call   d9720 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::release_jsvalue>
  4bed97:	80 bc 24 90 00 00 00 	cmpb   $0x6,0x90(%rsp)
  4bed9e:	06 
  4bed9f:	75 c2                	jne    4bed63 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x483>
  4beda1:	4c 89 ed             	mov    %r13,%rbp
  4beda4:	4c 8b ac 24 98 00 00 	mov    0x98(%rsp),%r13
  4bedab:	00 
  4bedac:	49 83 7d 28 00       	cmpq   $0x0,0x28(%r13)
  4bedb1:	74 0a                	je     4bedbd <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x4dd>
  4bedb3:	49 8b 7d 30          	mov    0x30(%r13),%rdi
  4bedb7:	ff 15 83 c0 34 00    	call   *0x34c083(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bedbd:	49 8b 7d 40          	mov    0x40(%r13),%rdi
  4bedc1:	48 85 ff             	test   %rdi,%rdi
  4bedc4:	4c 8b 3d 75 c0 34 00 	mov    0x34c075(%rip),%r15        # 80ae40 <free@GLIBC_2.2.5>
  4bedcb:	74 83                	je     4bed50 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x470>
  4bedcd:	41 ff d7             	call   *%r15
  4bedd0:	e9 7b ff ff ff       	jmp    4bed50 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x470>
  4bedd5:	48 83 7c 24 10 00    	cmpq   $0x0,0x10(%rsp)
  4beddb:	74 09                	je     4bede6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x506>
  4beddd:	48 89 ef             	mov    %rbp,%rdi
  4bede0:	ff 15 5a c0 34 00    	call   *0x34c05a(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bede6:	0f 10 84 24 d0 01 00 	movups 0x1d0(%rsp),%xmm0
  4beded:	00 
  4bedee:	0f 10 8c 24 e0 01 00 	movups 0x1e0(%rsp),%xmm1
  4bedf5:	00 
  4bedf6:	0f 11 4b 18          	movups %xmm1,0x18(%rbx)
  4bedfa:	0f 11 43 08          	movups %xmm0,0x8(%rbx)
  4bedfe:	e9 16 fb ff ff       	jmp    4be919 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x39>
  4bee03:	8b 8c 24 91 00 00 00 	mov    0x91(%rsp),%ecx
  4bee0a:	8b 94 24 94 00 00 00 	mov    0x94(%rsp),%edx
  4bee11:	89 53 0c             	mov    %edx,0xc(%rbx)
  4bee14:	89 4b 09             	mov    %ecx,0x9(%rbx)
  4bee17:	0f b6 8c 24 98 00 00 	movzbl 0x98(%rsp),%ecx
  4bee1e:	00 
  4bee1f:	48 8b 94 24 99 00 00 	mov    0x99(%rsp),%rdx
  4bee26:	00 
  4bee27:	48 89 94 24 f0 01 00 	mov    %rdx,0x1f0(%rsp)
  4bee2e:	00 
  4bee2f:	48 8b 94 24 a0 00 00 	mov    0xa0(%rsp),%rdx
  4bee36:	00 
  4bee37:	48 89 94 24 f7 01 00 	mov    %rdx,0x1f7(%rsp)
  4bee3e:	00 
  4bee3f:	48 8b 94 24 a8 00 00 	mov    0xa8(%rsp),%rdx
  4bee46:	00 
  4bee47:	48 8b b4 24 f0 01 00 	mov    0x1f0(%rsp),%rsi
  4bee4e:	00 
  4bee4f:	48 8b bc 24 f7 01 00 	mov    0x1f7(%rsp),%rdi
  4bee56:	00 
  4bee57:	48 89 7b 18          	mov    %rdi,0x18(%rbx)
  4bee5b:	48 89 73 11          	mov    %rsi,0x11(%rbx)
  4bee5f:	88 43 08             	mov    %al,0x8(%rbx)
  4bee62:	88 4b 10             	mov    %cl,0x10(%rbx)
  4bee65:	48 89 53 20          	mov    %rdx,0x20(%rbx)
  4bee69:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  4bee70:	48 89 ef             	mov    %rbp,%rdi
  4bee73:	48 81 c4 f8 02 00 00 	add    $0x2f8,%rsp
  4bee7a:	5b                   	pop    %rbx
  4bee7b:	41 5c                	pop    %r12
  4bee7d:	41 5d                	pop    %r13
  4bee7f:	41 5e                	pop    %r14
  4bee81:	41 5f                	pop    %r15
  4bee83:	5d                   	pop    %rbp
  4bee84:	ff 25 b6 bf 34 00    	jmp    *0x34bfb6(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bee8a:	48 c7 84 24 98 00 00 	movq   $0x0,0x98(%rsp)
  4bee91:	00 00 00 00 00 
  4bee96:	48 8d bc 24 f0 01 00 	lea    0x1f0(%rsp),%rdi
  4bee9d:	00 
  4bee9e:	48 8d 8c 24 90 00 00 	lea    0x90(%rsp),%rcx
  4beea5:	00 
  4beea6:	be 08 00 00 00       	mov    $0x8,%esi
  4beeab:	48 89 54 24 28       	mov    %rdx,0x28(%rsp)
  4beeb0:	e8 1b 9a b9 ff       	call   588d0 <alloc::raw_vec::finish_grow>
  4beeb5:	83 bc 24 f0 01 00 00 	cmpl   $0x1,0x1f0(%rsp)
  4beebc:	01 
  4beebd:	0f 84 e3 fb ff ff    	je     4beaa6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x1c6>
  4beec3:	48 8b 84 24 f8 01 00 	mov    0x1f8(%rsp),%rax
  4beeca:	00 
  4beecb:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4beed0:	48 89 44 24 38       	mov    %rax,0x38(%rsp)
  4beed5:	48 89 6c 24 30       	mov    %rbp,0x30(%rsp)
  4beeda:	49 8b 74 24 10       	mov    0x10(%r12),%rsi
  4beedf:	48 39 f5             	cmp    %rsi,%rbp
  4beee2:	0f 87 1c 02 00 00    	ja     4bf104 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x824>
  4beee8:	49 8b 6c 24 08       	mov    0x8(%r12),%rbp
  4beeed:	48 8b 54 24 28       	mov    0x28(%rsp),%rdx
  4beef2:	48 8d 04 2a          	lea    (%rdx,%rbp,1),%rax
  4beef6:	48 89 84 24 88 00 00 	mov    %rax,0x88(%rsp)
  4beefd:	00 
  4beefe:	48 8d 45 10          	lea    0x10(%rbp),%rax
  4bef02:	49 8b 0f             	mov    (%r15),%rcx
  4bef05:	48 89 4c 24 10       	mov    %rcx,0x10(%rsp)
  4bef0a:	48 83 c2 f0          	add    $0xfffffffffffffff0,%rdx
  4bef0e:	48 c1 ea 04          	shr    $0x4,%rdx
  4bef12:	48 ff c2             	inc    %rdx
  4bef15:	48 89 54 24 28       	mov    %rdx,0x28(%rsp)
  4bef1a:	48 c7 44 24 18 00 00 	movq   $0x0,0x18(%rsp)
  4bef21:	00 00 
  4bef23:	45 31 e4             	xor    %r12d,%r12d
  4bef26:	eb 48                	jmp    4bef70 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x690>
  4bef28:	48 8d 84 24 b8 01 00 	lea    0x1b8(%rsp),%rax
  4bef2f:	00 
  4bef30:	0f 10 00             	movups (%rax),%xmm0
  4bef33:	48 8b 44 24 08       	mov    0x8(%rsp),%rax
  4bef38:	48 8b 4c 24 18       	mov    0x18(%rsp),%rcx
  4bef3d:	0f 11 04 08          	movups %xmm0,(%rax,%rcx,1)
  4bef41:	49 ff c4             	inc    %r12
  4bef44:	4c 89 64 24 40       	mov    %r12,0x40(%rsp)
  4bef49:	31 c0                	xor    %eax,%eax
  4bef4b:	48 3b ac 24 88 00 00 	cmp    0x88(%rsp),%rbp
  4bef52:	00 
  4bef53:	0f 95 c0             	setne  %al
  4bef56:	c1 e0 04             	shl    $0x4,%eax
  4bef59:	48 01 e8             	add    %rbp,%rax
  4bef5c:	48 83 c1 10          	add    $0x10,%rcx
  4bef60:	48 89 4c 24 18       	mov    %rcx,0x18(%rsp)
  4bef65:	4c 39 64 24 28       	cmp    %r12,0x28(%rsp)
  4bef6a:	0f 84 14 01 00 00    	je     4bf084 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x7a4>
  4bef70:	48 89 ea             	mov    %rbp,%rdx
  4bef73:	48 89 c5             	mov    %rax,%rbp
  4bef76:	48 8d bc 24 b0 01 00 	lea    0x1b0(%rsp),%rdi
  4bef7d:	00 
  4bef7e:	48 8b 74 24 10       	mov    0x10(%rsp),%rsi
  4bef83:	e8 88 a6 c3 ff       	call   f9610 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::dup_jsvalue>
  4bef88:	80 bc 24 b0 01 00 00 	cmpb   $0xb,0x1b0(%rsp)
  4bef8f:	0b 
  4bef90:	75 27                	jne    4befb9 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x6d9>
  4bef92:	4c 3b 64 24 30       	cmp    0x30(%rsp),%r12
  4bef97:	75 8f                	jne    4bef28 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x648>
  4bef99:	48 8d 7c 24 30       	lea    0x30(%rsp),%rdi
  4bef9e:	48 8d 35 1b 3f 31 00 	lea    0x313f1b(%rip),%rsi        # 7d2ec0 <__do_global_dtors_aux_fini_array_entry+0xa5b0>
  4befa5:	e8 56 fe c1 ff       	call   dee00 <alloc::raw_vec::RawVec<T,A>::grow_one>
  4befaa:	48 8b 44 24 38       	mov    0x38(%rsp),%rax
  4befaf:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4befb4:	e9 6f ff ff ff       	jmp    4bef28 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x648>
  4befb9:	48 8b 44 24 30       	mov    0x30(%rsp),%rax
  4befbe:	48 89 44 24 20       	mov    %rax,0x20(%rsp)
  4befc3:	4d 85 e4             	test   %r12,%r12
  4befc6:	4c 8b 64 24 18       	mov    0x18(%rsp),%r12
  4befcb:	0f 84 8b 00 00 00    	je     4bf05c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x77c>
  4befd1:	4c 8d b4 24 90 00 00 	lea    0x90(%rsp),%r14
  4befd8:	00 
  4befd9:	4c 8d 7c 24 70       	lea    0x70(%rsp),%r15
  4befde:	48 8b 2d 5b be 34 00 	mov    0x34be5b(%rip),%rbp        # 80ae40 <free@GLIBC_2.2.5>
  4befe5:	4c 8b 6c 24 08       	mov    0x8(%rsp),%r13
  4befea:	eb 1a                	jmp    4bf006 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x726>
  4befec:	4c 89 e7             	mov    %r12,%rdi
  4befef:	ff d5                	call   *%rbp
  4beff1:	4d 89 f4             	mov    %r14,%r12
  4beff4:	4d 89 fe             	mov    %r15,%r14
  4beff7:	4c 8d 7c 24 70       	lea    0x70(%rsp),%r15
  4beffc:	49 83 c5 10          	add    $0x10,%r13
  4bf000:	49 83 c4 f0          	add    $0xfffffffffffffff0,%r12
  4bf004:	74 56                	je     4bf05c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x77c>
  4bf006:	41 0f 10 45 00       	movups 0x0(%r13),%xmm0
  4bf00b:	0f 29 44 24 70       	movaps %xmm0,0x70(%rsp)
  4bf010:	80 7c 24 70 0a       	cmpb   $0xa,0x70(%rsp)
  4bf015:	74 45                	je     4bf05c <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x77c>
  4bf017:	4c 89 f7             	mov    %r14,%rdi
  4bf01a:	48 8b 74 24 10       	mov    0x10(%rsp),%rsi
  4bf01f:	4c 89 fa             	mov    %r15,%rdx
  4bf022:	e8 f9 a6 c1 ff       	call   d9720 <quickjs_oxide::engine::value::js_value::<impl quickjs_oxide::engine::api::runtime::Runtime>::release_jsvalue>
  4bf027:	80 bc 24 90 00 00 00 	cmpb   $0x6,0x90(%rsp)
  4bf02e:	06 
  4bf02f:	75 cb                	jne    4beffc <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x71c>
  4bf031:	4d 89 f7             	mov    %r14,%r15
  4bf034:	4d 89 e6             	mov    %r12,%r14
  4bf037:	4c 8b a4 24 98 00 00 	mov    0x98(%rsp),%r12
  4bf03e:	00 
  4bf03f:	49 83 7c 24 28 00    	cmpq   $0x0,0x28(%r12)
  4bf045:	74 07                	je     4bf04e <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x76e>
  4bf047:	49 8b 7c 24 30       	mov    0x30(%r12),%rdi
  4bf04c:	ff d5                	call   *%rbp
  4bf04e:	49 8b 7c 24 40       	mov    0x40(%r12),%rdi
  4bf053:	48 85 ff             	test   %rdi,%rdi
  4bf056:	74 94                	je     4befec <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x70c>
  4bf058:	ff d5                	call   *%rbp
  4bf05a:	eb 90                	jmp    4befec <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x70c>
  4bf05c:	48 83 7c 24 20 00    	cmpq   $0x0,0x20(%rsp)
  4bf062:	74 0b                	je     4bf06f <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x78f>
  4bf064:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  4bf069:	ff 15 d1 bd 34 00    	call   *0x34bdd1(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bf06f:	0f 10 84 24 b0 01 00 	movups 0x1b0(%rsp),%xmm0
  4bf076:	00 
  4bf077:	0f 10 8c 24 c0 01 00 	movups 0x1c0(%rsp),%xmm1
  4bf07e:	00 
  4bf07f:	e9 72 fd ff ff       	jmp    4bedf6 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x516>
  4bf084:	48 8b 44 24 30       	mov    0x30(%rsp),%rax
  4bf089:	48 89 44 24 10       	mov    %rax,0x10(%rsp)
  4bf08e:	48 8b 6c 24 38       	mov    0x38(%rsp),%rbp
  4bf093:	e9 47 fb ff ff       	jmp    4bebdf <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x2ff>
  4bf098:	bf 08 00 00 00       	mov    $0x8,%edi
  4bf09d:	be 10 00 00 00       	mov    $0x10,%esi
  4bf0a2:	e8 35 8b b9 ff       	call   57bdc <alloc::alloc::handle_alloc_error>
  4bf0a7:	0f b6 84 24 91 00 00 	movzbl 0x91(%rsp),%eax
  4bf0ae:	00 
  4bf0af:	88 84 24 f0 01 00 00 	mov    %al,0x1f0(%rsp)
  4bf0b6:	48 8d 3d a3 d5 1e 00 	lea    0x1ed5a3(%rip),%rdi        # 6ac660 <num_bigint::biguint::convert::get_radix_base::BASES+0x329d0>
  4bf0bd:	48 8d 0d 0c b6 30 00 	lea    0x30b60c(%rip),%rcx        # 7ca6d0 <__do_global_dtors_aux_fini_array_entry+0x1dc0>
  4bf0c4:	4c 8d 05 fd fe 31 00 	lea    0x31fefd(%rip),%r8        # 7defc8 <__do_global_dtors_aux_fini_array_entry+0x166b8>
  4bf0cb:	48 8d 94 24 f0 01 00 	lea    0x1f0(%rsp),%rdx
  4bf0d2:	00 
  4bf0d3:	be 36 00 00 00       	mov    $0x36,%esi
  4bf0d8:	e8 43 92 b9 ff       	call   58320 <core::result::unwrap_failed>
  4bf0dd:	eb 34                	jmp    4bf113 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x833>
  4bf0df:	bf 08 00 00 00       	mov    $0x8,%edi
  4bf0e4:	be 40 00 00 00       	mov    $0x40,%esi
  4bf0e9:	e8 ee 8a b9 ff       	call   57bdc <alloc::alloc::handle_alloc_error>
  4bf0ee:	eb 23                	jmp    4bf113 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x833>
  4bf0f0:	48 8d 15 39 ba 30 00 	lea    0x30ba39(%rip),%rdx        # 7cab30 <__do_global_dtors_aux_fini_array_entry+0x2220>
  4bf0f7:	be 00 01 00 00       	mov    $0x100,%esi
  4bf0fc:	4c 89 f7             	mov    %r14,%rdi
  4bf0ff:	e8 4e 8d b9 ff       	call   57e52 <core::panicking::panic_bounds_check>
  4bf104:	48 8d 15 9d 3d 31 00 	lea    0x313d9d(%rip),%rdx        # 7d2ea8 <__do_global_dtors_aux_fini_array_entry+0xa598>
  4bf10b:	48 89 ef             	mov    %rbp,%rdi
  4bf10e:	e8 3d 8e b9 ff       	call   57f50 <core::slice::index::slice_end_index_len_fail>
  4bf113:	0f 0b                	ud2
  4bf115:	48 89 c3             	mov    %rax,%rbx
  4bf118:	48 83 7c 24 20 00    	cmpq   $0x0,0x20(%rsp)
  4bf11e:	74 0b                	je     4bf12b <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x84b>
  4bf120:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  4bf125:	ff 15 15 bd 34 00    	call   *0x34bd15(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bf12b:	48 8d bc 24 b0 01 00 	lea    0x1b0(%rsp),%rdi
  4bf132:	00 
  4bf133:	e8 d8 c1 c3 ff       	call   fb310 <core::ptr::drop_in_place<quickjs_oxide::engine::api::runtime_error::RuntimeError>>
  4bf138:	48 89 df             	mov    %rbx,%rdi
  4bf13b:	e8 f0 7e b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bf140:	48 89 c3             	mov    %rax,%rbx
  4bf143:	48 83 7c 24 30 00    	cmpq   $0x0,0x30(%rsp)
  4bf149:	0f 84 85 00 00 00    	je     4bf1d4 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8f4>
  4bf14f:	48 8b 44 24 38       	mov    0x38(%rsp),%rax
  4bf154:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4bf159:	eb 6e                	jmp    4bf1c9 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8e9>
  4bf15b:	48 89 c3             	mov    %rax,%rbx
  4bf15e:	48 89 ef             	mov    %rbp,%rdi
  4bf161:	eb 6b                	jmp    4bf1ce <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8ee>
  4bf163:	48 89 c3             	mov    %rax,%rbx
  4bf166:	eb 59                	jmp    4bf1c1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8e1>
  4bf168:	48 89 c3             	mov    %rax,%rbx
  4bf16b:	48 83 7c 24 10 00    	cmpq   $0x0,0x10(%rsp)
  4bf171:	74 0b                	je     4bf17e <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x89e>
  4bf173:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  4bf178:	ff 15 c2 bc 34 00    	call   *0x34bcc2(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bf17e:	48 8d bc 24 d0 01 00 	lea    0x1d0(%rsp),%rdi
  4bf185:	00 
  4bf186:	e8 85 c1 c3 ff       	call   fb310 <core::ptr::drop_in_place<quickjs_oxide::engine::api::runtime_error::RuntimeError>>
  4bf18b:	48 89 df             	mov    %rbx,%rdi
  4bf18e:	e8 9d 7e b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bf193:	48 89 c3             	mov    %rax,%rbx
  4bf196:	eb 31                	jmp    4bf1c9 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8e9>
  4bf198:	48 89 c3             	mov    %rax,%rbx
  4bf19b:	48 8d bc 24 90 00 00 	lea    0x90(%rsp),%rdi
  4bf1a2:	00 
  4bf1a3:	e8 e8 75 d3 ff       	call   1f6790 <core::ptr::drop_in_place<quickjs_oxide::engine::builtins::primitive::text::ScalarTextResumeState>>
  4bf1a8:	eb 2a                	jmp    4bf1d4 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8f4>
  4bf1aa:	e8 50 93 b9 ff       	call   584ff <core::panicking::panic_in_cleanup>
  4bf1af:	48 89 c3             	mov    %rax,%rbx
  4bf1b2:	49 ff 0f             	decq   (%r15)
  4bf1b5:	75 0a                	jne    4bf1c1 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8e1>
  4bf1b7:	48 8b 7c 24 18       	mov    0x18(%rsp),%rdi
  4bf1bc:	e8 7f 4e c0 ff       	call   c4040 <alloc::rc::Rc<T,A>::drop_slow>
  4bf1c1:	48 83 7c 24 10 00    	cmpq   $0x0,0x10(%rsp)
  4bf1c7:	74 0b                	je     4bf1d4 <quickjs_oxide::engine::builtins::primitive::text::ScalarTextStep::start+0x8f4>
  4bf1c9:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  4bf1ce:	ff 15 6c bc 34 00    	call   *0x34bc6c(%rip)        # 80ae40 <free@GLIBC_2.2.5>
  4bf1d4:	48 89 df             	mov    %rbx,%rdi
  4bf1d7:	e8 54 7e b9 ff       	call   57030 <_Unwind_Resume@plt>
  4bf1dc:	e8 1e 93 b9 ff       	call   584ff <core::panicking::panic_in_cleanup>
