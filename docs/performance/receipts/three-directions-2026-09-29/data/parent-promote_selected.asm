
/tmp/oxide-three-build-parent/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000573360 <quickjs_oxide::engine::vm::frames::NativeClassification::promote_selected>:
  573360:	55                   	push   %rbp
  573361:	41 57                	push   %r15
  573363:	41 56                	push   %r14
  573365:	41 55                	push   %r13
  573367:	41 54                	push   %r12
  573369:	53                   	push   %rbx
  57336a:	48 83 ec 48          	sub    $0x48,%rsp
  57336e:	49 89 f4             	mov    %rsi,%r12
  573371:	48 89 fb             	mov    %rdi,%rbx
  573374:	48 8b 46 10          	mov    0x10(%rsi),%rax
  573378:	8b 2e                	mov    (%rsi),%ebp
  57337a:	44 8b 76 04          	mov    0x4(%rsi),%r14d
  57337e:	f2 0f 10 46 08       	movsd  0x8(%rsi),%xmm0
  573383:	0f b6 56 18          	movzbl 0x18(%rsi),%edx
  573387:	8b 76 1c             	mov    0x1c(%rsi),%esi
  57338a:	48 8b 08             	mov    (%rax),%rcx
  57338d:	48 8b b9 18 0f 00 00 	mov    0xf18(%rcx),%rdi
  573394:	48 ff 01             	incq   (%rcx)
  573397:	0f 84 df 00 00 00    	je     57347c <quickjs_oxide::engine::vm::frames::NativeClassification::promote_selected+0x11c>
  57339d:	48 89 7c 24 28       	mov    %rdi,0x28(%rsp)
  5733a2:	89 74 24 0c          	mov    %esi,0xc(%rsp)
  5733a6:	88 54 24 03          	mov    %dl,0x3(%rsp)
  5733aa:	0f 29 44 24 10       	movaps %xmm0,0x10(%rsp)
  5733af:	4c 8b 38             	mov    (%rax),%r15
  5733b2:	48 8d 7c 24 30       	lea    0x30(%rsp),%rdi
  5733b7:	4c 89 fe             	mov    %r15,%rsi
  5733ba:	89 ea                	mov    %ebp,%edx
  5733bc:	44 89 f1             	mov    %r14d,%ecx
  5733bf:	e8 fc 8e b7 ff       	call   ec2c0 <quickjs_oxide::engine::heap::ownership::<impl quickjs_oxide::engine::api::runtime::Runtime>::retain_object_handle>
  5733c4:	44 0f b6 6c 24 30    	movzbl 0x30(%rsp),%r13d
  5733ca:	41 80 fd 06          	cmp    $0x6,%r13b
  5733ce:	75 4a                	jne    57341a <quickjs_oxide::engine::vm::frames::NativeClassification::promote_selected+0xba>
  5733d0:	49 83 c4 19          	add    $0x19,%r12
  5733d4:	41 0f b6 44 24 02    	movzbl 0x2(%r12),%eax
  5733da:	88 43 2b             	mov    %al,0x2b(%rbx)
  5733dd:	41 0f b7 04 24       	movzwl (%r12),%eax
  5733e2:	66 89 43 29          	mov    %ax,0x29(%rbx)
  5733e6:	4c 89 3b             	mov    %r15,(%rbx)
  5733e9:	89 6b 08             	mov    %ebp,0x8(%rbx)
  5733ec:	44 89 73 0c          	mov    %r14d,0xc(%rbx)
  5733f0:	89 6b 10             	mov    %ebp,0x10(%rbx)
  5733f3:	44 89 73 14          	mov    %r14d,0x14(%rbx)
  5733f7:	48 8b 44 24 28       	mov    0x28(%rsp),%rax
  5733fc:	48 89 43 18          	mov    %rax,0x18(%rbx)
  573400:	0f 28 44 24 10       	movaps 0x10(%rsp),%xmm0
  573405:	0f 13 43 20          	movlps %xmm0,0x20(%rbx)
  573409:	0f b6 44 24 03       	movzbl 0x3(%rsp),%eax
  57340e:	88 43 28             	mov    %al,0x28(%rbx)
  573411:	8b 44 24 0c          	mov    0xc(%rsp),%eax
  573415:	89 43 2c             	mov    %eax,0x2c(%rbx)
  573418:	eb 53                	jmp    57346d <quickjs_oxide::engine::vm::frames::NativeClassification::promote_selected+0x10d>
  57341a:	8b 44 24 31          	mov    0x31(%rsp),%eax
  57341e:	8b 4c 24 34          	mov    0x34(%rsp),%ecx
  573422:	89 4c 24 07          	mov    %ecx,0x7(%rsp)
  573426:	89 44 24 04          	mov    %eax,0x4(%rsp)
  57342a:	4c 8b 74 24 38       	mov    0x38(%rsp),%r14
  57342f:	f2 0f 10 44 24 40    	movsd  0x40(%rsp),%xmm0
  573435:	49 ff 0f             	decq   (%r15)
  573438:	75 12                	jne    57344c <quickjs_oxide::engine::vm::frames::NativeClassification::promote_selected+0xec>
  57343a:	4c 89 ff             	mov    %r15,%rdi
  57343d:	0f 29 44 24 10       	movaps %xmm0,0x10(%rsp)
  573442:	e8 f9 0b b5 ff       	call   c4040 <alloc::rc::Rc<T,A>::drop_slow>
  573447:	0f 28 44 24 10       	movaps 0x10(%rsp),%xmm0
  57344c:	c6 03 08             	movb   $0x8,(%rbx)
  57344f:	44 88 6b 08          	mov    %r13b,0x8(%rbx)
  573453:	8b 44 24 04          	mov    0x4(%rsp),%eax
  573457:	8b 4c 24 07          	mov    0x7(%rsp),%ecx
  57345b:	89 43 09             	mov    %eax,0x9(%rbx)
  57345e:	89 4b 0c             	mov    %ecx,0xc(%rbx)
  573461:	4c 89 73 10          	mov    %r14,0x10(%rbx)
  573465:	0f 13 43 18          	movlps %xmm0,0x18(%rbx)
  573469:	c6 43 2c 65          	movb   $0x65,0x2c(%rbx)
  57346d:	48 83 c4 48          	add    $0x48,%rsp
  573471:	5b                   	pop    %rbx
  573472:	41 5c                	pop    %r12
  573474:	41 5d                	pop    %r13
  573476:	41 5e                	pop    %r14
  573478:	41 5f                	pop    %r15
  57347a:	5d                   	pop    %rbp
  57347b:	c3                   	ret
  57347c:	0f 0b                	ud2
