
/tmp/oxide-three-build-combined/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000573b10 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked>:
  573b10:	55                   	push   %rbp
  573b11:	41 57                	push   %r15
  573b13:	41 56                	push   %r14
  573b15:	41 55                	push   %r13
  573b17:	41 54                	push   %r12
  573b19:	53                   	push   %rbx
  573b1a:	48 83 ec 28          	sub    $0x28,%rsp
  573b1e:	80 39 09             	cmpb   $0x9,(%rcx)
  573b21:	75 60                	jne    573b83 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x73>
  573b23:	44 8b 69 04          	mov    0x4(%rcx),%r13d
  573b27:	8b 59 08             	mov    0x8(%rcx),%ebx
  573b2a:	48 8b 8e 18 0f 00 00 	mov    0xf18(%rsi),%rcx
  573b31:	48 8b 02             	mov    (%rdx),%rax
  573b34:	41 be 02 00 00 00    	mov    $0x2,%r14d
  573b3a:	48 3b 88 18 0f 00 00 	cmp    0xf18(%rax),%rcx
  573b41:	75 63                	jne    573ba6 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x96>
  573b43:	44 3b 6a 08          	cmp    0x8(%rdx),%r13d
  573b47:	75 5d                	jne    573ba6 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x96>
  573b49:	3b 5a 0c             	cmp    0xc(%rdx),%ebx
  573b4c:	75 21                	jne    573b6f <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x5f>
  573b4e:	44 8b 72 10          	mov    0x10(%rdx),%r14d
  573b52:	44 8b 62 1c          	mov    0x1c(%rdx),%r12d
  573b56:	f2 0f 10 42 14       	movsd  0x14(%rdx),%xmm0
  573b5b:	0f b6 4a 22          	movzbl 0x22(%rdx),%ecx
  573b5f:	88 4c 24 02          	mov    %cl,0x2(%rsp)
  573b63:	0f b7 4a 20          	movzwl 0x20(%rdx),%ecx
  573b67:	66 89 0c 24          	mov    %cx,(%rsp)
  573b6b:	0f b6 6a 23          	movzbl 0x23(%rdx),%ebp
  573b6f:	48 ff 08             	decq   (%rax)
  573b72:	74 37                	je     573bab <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x9b>
  573b74:	41 83 fe 02          	cmp    $0x2,%r14d
  573b78:	75 59                	jne    573bd3 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0xc3>
  573b7a:	c6 47 1c 65          	movb   $0x65,0x1c(%rdi)
  573b7e:	e9 96 00 00 00       	jmp    573c19 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x109>
  573b83:	c6 47 1c 65          	movb   $0x65,0x1c(%rdi)
  573b87:	48 8b 3a             	mov    (%rdx),%rdi
  573b8a:	48 ff 0f             	decq   (%rdi)
  573b8d:	0f 85 86 00 00 00    	jne    573c19 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x109>
  573b93:	48 83 c4 28          	add    $0x28,%rsp
  573b97:	5b                   	pop    %rbx
  573b98:	41 5c                	pop    %r12
  573b9a:	41 5d                	pop    %r13
  573b9c:	41 5e                	pop    %r14
  573b9e:	41 5f                	pop    %r15
  573ba0:	5d                   	pop    %rbp
  573ba1:	e9 9a 04 b5 ff       	jmp    c4040 <alloc::rc::Rc<T,A>::drop_slow>
  573ba6:	48 ff 08             	decq   (%rax)
  573ba9:	75 c9                	jne    573b74 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x64>
  573bab:	48 89 7c 24 08       	mov    %rdi,0x8(%rsp)
  573bb0:	48 89 c7             	mov    %rax,%rdi
  573bb3:	49 89 f7             	mov    %rsi,%r15
  573bb6:	0f 29 44 24 10       	movaps %xmm0,0x10(%rsp)
  573bbb:	e8 80 04 b5 ff       	call   c4040 <alloc::rc::Rc<T,A>::drop_slow>
  573bc0:	0f 28 44 24 10       	movaps 0x10(%rsp),%xmm0
  573bc5:	4c 89 fe             	mov    %r15,%rsi
  573bc8:	48 8b 7c 24 08       	mov    0x8(%rsp),%rdi
  573bcd:	41 83 fe 02          	cmp    $0x2,%r14d
  573bd1:	74 a7                	je     573b7a <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x6a>
  573bd3:	0f b6 44 24 02       	movzbl 0x2(%rsp),%eax
  573bd8:	88 44 24 06          	mov    %al,0x6(%rsp)
  573bdc:	0f b7 04 24          	movzwl (%rsp),%eax
  573be0:	66 89 44 24 04       	mov    %ax,0x4(%rsp)
  573be5:	41 f6 c6 01          	test   $0x1,%r14b
  573be9:	74 3d                	je     573c28 <quickjs_oxide::engine::vm::frames::NativeClassification::classify_linked+0x118>
  573beb:	48 8b 86 18 0f 00 00 	mov    0xf18(%rsi),%rax
  573bf2:	0f b6 4c 24 06       	movzbl 0x6(%rsp),%ecx
  573bf7:	88 4f 1b             	mov    %cl,0x1b(%rdi)
  573bfa:	0f b7 4c 24 04       	movzwl 0x4(%rsp),%ecx
  573bff:	66 89 4f 19          	mov    %cx,0x19(%rdi)
  573c03:	44 89 2f             	mov    %r13d,(%rdi)
  573c06:	89 5f 04             	mov    %ebx,0x4(%rdi)
  573c09:	48 89 47 08          	mov    %rax,0x8(%rdi)
  573c0d:	0f 13 47 10          	movlps %xmm0,0x10(%rdi)
  573c11:	40 88 6f 18          	mov    %bpl,0x18(%rdi)
  573c15:	44 89 67 1c          	mov    %r12d,0x1c(%rdi)
  573c19:	48 83 c4 28          	add    $0x28,%rsp
  573c1d:	5b                   	pop    %rbx
  573c1e:	41 5c                	pop    %r12
  573c20:	41 5d                	pop    %r13
  573c22:	41 5e                	pop    %r14
  573c24:	41 5f                	pop    %r15
  573c26:	5d                   	pop    %rbp
  573c27:	c3                   	ret
  573c28:	48 8d 3d e2 e6 13 00 	lea    0x13e6e2(%rip),%rdi        # 6b2311 <num_bigint::biguint::convert::get_radix_base::BASES+0x37561>
  573c2f:	48 8d 15 ea f8 26 00 	lea    0x26f8ea(%rip),%rdx        # 7e3520 <__do_global_dtors_aux_fini_array_entry+0x18c68>
  573c36:	be 15 00 00 00       	mov    $0x15,%esi
  573c3b:	e8 60 47 ae ff       	call   583a0 <core::option::expect_failed>
