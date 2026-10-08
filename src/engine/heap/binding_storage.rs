use super::*;

impl Heap {
    /// Read one captured-variable cell. All functions holding the same
    /// `VarRefId` observe this shared value.
    pub fn var_ref(&self, id: VarRefId) -> Result<&VarRefData, HeapError> {
        Ok(&self.var_refs.live(id)?.data)
    }

    /// Trusted read of a cell that a live closure or frame edge keeps alive.
    /// Identity is checked only in debug and `checked-handles` builds.
    #[inline]
    pub(crate) fn var_ref_fast(&self, id: VarRefId) -> &VarRefData {
        &self.var_refs.live_fast(id).data
    }

    /// Read immutable executable data without promoting any raw cpool edges.
    pub fn function_bytecode(
        &self,
        id: FunctionBytecodeId,
    ) -> Result<&FunctionBytecodeData, HeapError> {
        match self.live_node(RawId::FunctionBytecode(id))?.data {
            NodeData::FunctionBytecode(ref bytecode) => Ok(bytecode),
            NodeData::Object(_) | NodeData::Context(_) => Err(HeapError::Invariant(
                "typed bytecode lookup reached another node payload",
            )),
        }
    }

    /// Checked bytecode read plus its strong count from the same slot.
    pub(crate) fn function_bytecode_with_strong(
        &self,
        id: FunctionBytecodeId,
    ) -> Result<(&FunctionBytecodeData, u32), HeapError> {
        let node = self.live_node(RawId::FunctionBytecode(id))?;
        match node.data {
            NodeData::FunctionBytecode(ref bytecode) => Ok((bytecode, node.strong.get())),
            NodeData::Object(_) | NodeData::Context(_) => Err(HeapError::Invariant(
                "typed bytecode lookup reached another node payload",
            )),
        }
    }

    /// Replace the value stored in a captured-variable cell transactionally.
    ///
    /// The new value's GC edge is retained before the old edge is detached.
    /// A symbol atom transfers to the heap on success; any atom owned by the
    /// previous value is returned in the cleanup.
    pub fn replace_var_ref_value(
        &mut self,
        id: VarRefId,
        replacement: RawValue,
    ) -> Result<HeapCleanup, HeapError> {
        let current = self.var_ref(id)?;
        validate_var_ref_value(
            current.kind,
            current.is_lexical,
            current.is_const,
            &replacement,
        )?;
        let new_edges = raw_value_edges(&replacement);
        self.retain_edges_transactionally(&new_edges)?;

        let previous = {
            let var_ref = self.var_ref_mut(id)?;
            std::mem::replace(&mut var_ref.value, replacement)
        };

        self.retire_var_ref_value(previous)
    }

    /// Retire a detached cell value, preserving the replacement path's
    /// ordering: enqueue its heap edge, drain all older zero work, then hand
    /// its atoms to the caller's cleanup. Even an immediate previous value
    /// drains older queued work. The caller already committed the exchange.
    pub(crate) fn retire_var_ref_value(
        &mut self,
        previous: RawValue,
    ) -> Result<HeapCleanup, HeapError> {
        let mut cleanup = HeapCleanup::default();
        cleanup.atoms.extend(raw_value_atom(&previous));
        for edge in raw_value_edges(&previous) {
            self.release_raw_no_drain(edge)?;
        }
        cleanup.merge(self.drain_zero_queue()?);
        Ok(cleanup)
    }

    /// Exchange two owned edges without a retain/release pair. The caller
    /// releases the returned previous edge after ending this storage borrow.
    /// Validation failure returns the unpublished replacement unchanged.
    pub(crate) fn replace_var_ref_value_owned(
        &mut self,
        id: VarRefId,
        replacement: RawValue,
    ) -> Result<RawValue, (HeapError, RawValue)> {
        let cell = match self.var_ref_mut(id) {
            Ok(cell) => cell,
            Err(error) => return Err((error, replacement)),
        };
        if let Err(error) =
            validate_var_ref_value(cell.kind, cell.is_lexical, cell.is_const, &replacement)
        {
            return Err((error, replacement));
        }
        Ok(std::mem::replace(&mut cell.value, replacement))
    }

    /// Update binding-mode metadata without disturbing the shared value or
    /// any of its retained GC edges.
    pub fn set_var_ref_metadata(
        &mut self,
        id: VarRefId,
        is_lexical: bool,
        is_const: bool,
        kind: ClosureVariableKind,
    ) -> Result<(), HeapError> {
        let var_ref = self.var_ref_mut(id)?;
        validate_var_ref_value(kind, is_lexical, is_const, &var_ref.value)?;
        var_ref.is_lexical = is_lexical;
        var_ref.is_const = is_const;
        var_ref.kind = kind;
        Ok(())
    }
}
