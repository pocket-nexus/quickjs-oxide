
/home/eric/.cache/oxide-six-build-stack/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

00000000000690d0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value>:
   690d0:	push   %rbx
   690d1:	sub    $0x50,%rsp
   690d5:	movzbl 0x7a49f9(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
   690dc:	mov    $0x2d,%edi
   690e1:	call   *0x7a3d01(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
   690e7:	test   %rax,%rax
   690ea:	je     69196 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value+0xc6>
   690f0:	movups 0x648e99(%rip),%xmm0        # 6b1f90 <num_bigint::biguint::convert::get_radix_base::BASES+0x372e0>
   690f7:	movups %xmm0,0x1d(%rax)
   690fb:	movups 0x648e81(%rip),%xmm0        # 6b1f83 <num_bigint::biguint::convert::get_radix_base::BASES+0x372d3>
   69102:	movups %xmm0,0x10(%rax)
   69106:	movups 0x648e66(%rip),%xmm0        # 6b1f73 <num_bigint::biguint::convert::get_radix_base::BASES+0x372c3>
   6910d:	movups %xmm0,(%rax)
   69110:	movb   $0x5,0x48(%rsp)
   69115:	movq   $0x2d,0x28(%rsp)
   6911e:	mov    %rax,0x30(%rsp)
   69123:	movq   $0x2d,0x38(%rsp)
   6912c:	movq   $0x0,0x40(%rsp)
   69135:	movq   $0x0,(%rsp)
   6913d:	movzbl 0x7a4991(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
   69144:	mov    $0x50,%edi
   69149:	call   *0x7a3c99(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
   6914f:	test   %rax,%rax
   69152:	je     69185 <quickjs_oxide::engine::vm::stack::SlotStore::operand_push_replaces_live_value+0xb5>
   69154:	movups 0x40(%rsp),%xmm0
   69159:	movups %xmm0,0x40(%rax)
   6915d:	movups (%rsp),%xmm0
   69161:	movups 0x10(%rsp),%xmm1
   69166:	movups 0x20(%rsp),%xmm2
   6916b:	movups 0x30(%rsp),%xmm3
   69170:	movups %xmm3,0x30(%rax)
   69174:	movups %xmm2,0x20(%rax)
   69178:	movups %xmm1,0x10(%rax)
   6917c:	movups %xmm0,(%rax)
   6917f:	add    $0x50,%rsp
   69183:	pop    %rbx
   69184:	ret
   69185:	mov    $0x8,%edi
   6918a:	mov    $0x50,%esi
   6918f:	call   57bdc <alloc::alloc::handle_alloc_error>
   69194:	ud2
   69196:	mov    $0x1,%edi
   6919b:	mov    $0x2d,%esi
   691a0:	call   57bdc <alloc::alloc::handle_alloc_error>
   691a5:	mov    %rax,%rbx
   691a8:	mov    %rsp,%rdi
   691ab:	call   115b40 <core::ptr::drop_in_place<quickjs_oxide::engine::api::error::ErrorData>>
   691b0:	mov    %rbx,%rdi
   691b3:	call   57030 <_Unwind_Resume@plt>
