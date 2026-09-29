
/home/eric/.cache/oxide-six-build-stack/release/qjs:     file format elf64-x86-64


Disassembly of section .text:

0000000000068fe0 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded>:
   68fe0:	push   %rbx
   68fe1:	sub    $0x50,%rsp
   68fe5:	movzbl 0x7a4ae9(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
   68fec:	mov    $0x2d,%edi
   68ff1:	call   *0x7a3df1(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
   68ff7:	test   %rax,%rax
   68ffa:	je     690a6 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded+0xc6>
   69000:	movups 0x648f5c(%rip),%xmm0        # 6b1f63 <num_bigint::biguint::convert::get_radix_base::BASES+0x372b3>
   69007:	movups %xmm0,0x1d(%rax)
   6900b:	movups 0x648f44(%rip),%xmm0        # 6b1f56 <num_bigint::biguint::convert::get_radix_base::BASES+0x372a6>
   69012:	movups %xmm0,0x10(%rax)
   69016:	movups 0x648f29(%rip),%xmm0        # 6b1f46 <num_bigint::biguint::convert::get_radix_base::BASES+0x37296>
   6901d:	movups %xmm0,(%rax)
   69020:	movb   $0x5,0x48(%rsp)
   69025:	movq   $0x2d,0x28(%rsp)
   6902e:	mov    %rax,0x30(%rsp)
   69033:	movq   $0x2d,0x38(%rsp)
   6903c:	movq   $0x0,0x40(%rsp)
   69045:	movq   $0x0,(%rsp)
   6904d:	movzbl 0x7a4a81(%rip),%eax        # 80dad5 <__rust_no_alloc_shim_is_unstable>
   69054:	mov    $0x50,%edi
   69059:	call   *0x7a3d89(%rip)        # 80cde8 <malloc@GLIBC_2.2.5>
   6905f:	test   %rax,%rax
   69062:	je     69095 <quickjs_oxide::engine::vm::stack::SlotStore::operand_stack_capacity_exceeded+0xb5>
   69064:	movups 0x40(%rsp),%xmm0
   69069:	movups %xmm0,0x40(%rax)
   6906d:	movups (%rsp),%xmm0
   69071:	movups 0x10(%rsp),%xmm1
   69076:	movups 0x20(%rsp),%xmm2
   6907b:	movups 0x30(%rsp),%xmm3
   69080:	movups %xmm3,0x30(%rax)
   69084:	movups %xmm2,0x20(%rax)
   69088:	movups %xmm1,0x10(%rax)
   6908c:	movups %xmm0,(%rax)
   6908f:	add    $0x50,%rsp
   69093:	pop    %rbx
   69094:	ret
   69095:	mov    $0x8,%edi
   6909a:	mov    $0x50,%esi
   6909f:	call   57bdc <alloc::alloc::handle_alloc_error>
   690a4:	ud2
   690a6:	mov    $0x1,%edi
   690ab:	mov    $0x2d,%esi
   690b0:	call   57bdc <alloc::alloc::handle_alloc_error>
   690b5:	mov    %rax,%rbx
   690b8:	mov    %rsp,%rdi
   690bb:	call   115b40 <core::ptr::drop_in_place<quickjs_oxide::engine::api::error::ErrorData>>
   690c0:	mov    %rbx,%rdi
   690c3:	call   57030 <_Unwind_Resume@plt>
