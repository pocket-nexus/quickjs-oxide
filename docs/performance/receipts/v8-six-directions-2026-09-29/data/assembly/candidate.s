	.file	"candidate.b0b3432c87c9c02e-cgu.0"
	.section	.text.int_payload,"ax",@progbits
	.globl	int_payload
	.p2align	4
	.type	int_payload,@function
int_payload:
	.cfi_startproc
	movl	%edi, %eax
	retq
.Lfunc_end0:
	.size	int_payload, .Lfunc_end0-int_payload
	.cfi_endproc

	.section	.rodata.cst8,"aM",@progbits,8
	.p2align	3, 0x0
.LCPI1_0:
	.quad	0x41f0000000000000
.LCPI1_1:
	.quad	0x41e0000000000000
.LCPI1_2:
	.quad	0xc1f0000000000000
.LCPI1_3:
	.quad	0xc1e0000000000000
.LCPI1_4:
	.quad	0x41dfffffffc00000
	.section	.text.dynamic_payload,"ax",@progbits
	.globl	dynamic_payload
	.p2align	4
	.type	dynamic_payload,@function
dynamic_payload:
	.cfi_startproc
	pushq	%rax
	.cfi_def_cfa_offset 16
	testl	%edi, %edi
	je	.LBB1_6
	movq	%xmm0, %rax
	movabsq	$9223372036854775807, %rcx
	andq	%rax, %rcx
	movabsq	$9218868437227405312, %rax
	cmpq	%rax, %rcx
	setg	%al
	sete	%dl
	testq	%rcx, %rcx
	sete	%cl
	orb	%dl, %cl
	xorl	%esi, %esi
	orb	%al, %cl
	je	.LBB1_2
.LBB1_6:
	movl	%esi, %eax
	popq	%rcx
	.cfi_def_cfa_offset 8
	retq
.LBB1_2:
	.cfi_def_cfa_offset 16
	callq	*trunc@GOTPCREL(%rip)
	movsd	.LCPI1_0(%rip), %xmm1
	callq	*fmod@GOTPCREL(%rip)
	movsd	.LCPI1_0(%rip), %xmm3
	addsd	%xmm0, %xmm3
	xorpd	%xmm2, %xmm2
	movapd	%xmm0, %xmm1
	cmpltsd	%xmm2, %xmm1
	andpd	%xmm1, %xmm3
	andnpd	%xmm0, %xmm1
	orpd	%xmm3, %xmm1
	ucomisd	.LCPI1_1(%rip), %xmm1
	jae	.LBB1_5
	movapd	%xmm1, %xmm0
	maxsd	.LCPI1_3(%rip), %xmm0
	minsd	.LCPI1_4(%rip), %xmm0
	cvttsd2si	%xmm0, %ecx
	xorl	%eax, %eax
	ucomisd	%xmm1, %xmm1
	cmovnpl	%ecx, %eax
	popq	%rcx
	.cfi_def_cfa_offset 8
	retq
.LBB1_5:
	.cfi_def_cfa_offset 16
	movsd	.LCPI1_2(%rip), %xmm0
	addsd	%xmm1, %xmm0
	xorl	%eax, %eax
	ucomisd	%xmm0, %xmm0
	maxsd	.LCPI1_3(%rip), %xmm0
	minsd	.LCPI1_4(%rip), %xmm0
	cvttsd2si	%xmm0, %ecx
	cmovnpl	%ecx, %eax
	popq	%rcx
	.cfi_def_cfa_offset 8
	retq
.Lfunc_end1:
	.size	dynamic_payload, .Lfunc_end1-dynamic_payload
	.cfi_endproc

	.ident	"rustc version 1.88.0 (6b00bc388 2025-06-23)"
	.section	".note.GNU-stack","",@progbits
