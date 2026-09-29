
/tmp/oxide-six-build-baseline/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000222250 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index>:
  222250:	push   %rbx
  222251:	sub    $0x50,%rsp
  222255:	mov    %rdx,%rax
  222258:	mov    0x40(%rdx),%rcx
  22225c:	mov    0x30(%rdx),%rdx
  222260:	mov    0x38(%rax),%rax
  222264:	xor    %r8d,%r8d
  222267:	sub    %rdx,%rax
  22226a:	cmovae %rax,%r8
  22226e:	cmp    %r8,%rcx
  222271:	jae    222298 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x48>
  222273:	add    %rcx,%rdx
  222276:	cmp    %rsi,%rdx
  222279:	jae    222407 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x1b7>
  22227f:	mov    %rdx,%rax
  222282:	shl    $0x4,%rax
  222286:	cmpb   $0xe,(%rdi,%rax,1)
  22228a:	jne    22232f <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0xdf>
  222290:	xor    %eax,%eax
  222292:	add    $0x50,%rsp
  222296:	pop    %rbx
  222297:	ret
  222298:	movzbl 0x5eb836(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
  22229f:	mov    $0x2d,%edi
  2222a4:	call   *0x5eab3e(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
  2222aa:	test   %rax,%rax
  2222ad:	je     2223f8 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x1a8>
  2222b3:	movups 0x48fdd6(%rip),%xmm0        # 6b2090 <num_bigint::biguint::convert::get_radix_base::BASES+0x372e0>
  2222ba:	movups %xmm0,0x1d(%rax)
  2222be:	movups 0x48fdbe(%rip),%xmm0        # 6b2083 <num_bigint::biguint::convert::get_radix_base::BASES+0x372d3>
  2222c5:	movups %xmm0,0x10(%rax)
  2222c9:	movups 0x48fda3(%rip),%xmm0        # 6b2073 <num_bigint::biguint::convert::get_radix_base::BASES+0x372c3>
  2222d0:	movups %xmm0,(%rax)
  2222d3:	movb   $0x5,0x48(%rsp)
  2222d8:	movq   $0x2d,0x28(%rsp)
  2222e1:	mov    %rax,0x30(%rsp)
  2222e6:	movq   $0x2d,0x38(%rsp)
  2222ef:	movq   $0x0,0x40(%rsp)
  2222f8:	movq   $0x0,(%rsp)
  222300:	movzbl 0x5eb7ce(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
  222307:	mov    $0x50,%edi
  22230c:	call   *0x5eaad6(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
  222312:	test   %rax,%rax
  222315:	jne    2223ae <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x15e>
  22231b:	mov    $0x8,%edi
  222320:	mov    $0x50,%esi
  222325:	call   57bdc <alloc::alloc::handle_alloc_error>
  22232a:	jmp    2223f6 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x1a6>
  22232f:	movzbl 0x5eb79f(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
  222336:	mov    $0x2d,%edi
  22233b:	call   *0x5eaaa7(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
  222341:	test   %rax,%rax
  222344:	je     2223f8 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x1a8>
  22234a:	movups 0x48fd12(%rip),%xmm0        # 6b2063 <num_bigint::biguint::convert::get_radix_base::BASES+0x372b3>
  222351:	movups %xmm0,0x1d(%rax)
  222355:	movups 0x48fcfa(%rip),%xmm0        # 6b2056 <num_bigint::biguint::convert::get_radix_base::BASES+0x372a6>
  22235c:	movups %xmm0,0x10(%rax)
  222360:	movups 0x48fcdf(%rip),%xmm0        # 6b2046 <num_bigint::biguint::convert::get_radix_base::BASES+0x37296>
  222367:	movups %xmm0,(%rax)
  22236a:	movb   $0x5,0x48(%rsp)
  22236f:	movq   $0x2d,0x28(%rsp)
  222378:	mov    %rax,0x30(%rsp)
  22237d:	movq   $0x2d,0x38(%rsp)
  222386:	movq   $0x0,0x40(%rsp)
  22238f:	movq   $0x0,(%rsp)
  222397:	movzbl 0x5eb737(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
  22239e:	mov    $0x50,%edi
  2223a3:	call   *0x5eaa3f(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
  2223a9:	test   %rax,%rax
  2223ac:	je     2223e7 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x197>
  2223ae:	mov    %rax,%rdx
  2223b1:	movups 0x40(%rsp),%xmm0
  2223b6:	movups %xmm0,0x40(%rax)
  2223ba:	movups (%rsp),%xmm0
  2223be:	movups 0x10(%rsp),%xmm1
  2223c3:	movups 0x20(%rsp),%xmm2
  2223c8:	movups 0x30(%rsp),%xmm3
  2223cd:	movups %xmm3,0x30(%rax)
  2223d1:	movups %xmm2,0x20(%rax)
  2223d5:	movups %xmm1,0x10(%rax)
  2223d9:	movups %xmm0,(%rax)
  2223dc:	mov    $0x1,%eax
  2223e1:	add    $0x50,%rsp
  2223e5:	pop    %rbx
  2223e6:	ret
  2223e7:	mov    $0x8,%edi
  2223ec:	mov    $0x50,%esi
  2223f1:	call   57bdc <alloc::alloc::handle_alloc_error>
  2223f6:	ud2
  2223f8:	mov    $0x1,%edi
  2223fd:	mov    $0x2d,%esi
  222402:	call   57bdc <alloc::alloc::handle_alloc_error>
  222407:	lea    0x5c0dca(%rip),%rax        # 7e31d8 <__do_global_dtors_aux_fini_array_entry+0x18920>
  22240e:	mov    %rdx,%rdi
  222411:	mov    %rax,%rdx
  222414:	call   57e52 <core::panicking::panic_bounds_check>
  222419:	jmp    22241b <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_index+0x1cb>
  22241b:	mov    %rax,%rbx
  22241e:	mov    %rsp,%rdi
  222421:	call   115960 <core::ptr::drop_in_place<quickjs_oxide::engine::api::error::ErrorData>>
  222426:	mov    %rbx,%rdi
  222429:	call   57030 <_Unwind_Resume@plt>
