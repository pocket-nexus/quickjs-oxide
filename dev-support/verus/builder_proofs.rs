verus! {
impl CodeBuilder {
    pub closed spec fn instructions(&self) -> Seq<Insn> { self.code@ }
    pub closed spec fn exceptions(&self) -> Seq<Handler> { self.handlers@ }
}

pub open spec fn emitted(before: CodeBuilder, after: CodeBuilder, pc: u32, insn: Insn) -> bool {
    &&& pc == before.instructions().len()
    &&& after.instructions() == before.instructions().push(insn)
    &&& after.exceptions() == before.exceptions()
}
}
