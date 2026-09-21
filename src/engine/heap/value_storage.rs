use super::*;

impl Heap {
    /// Read one live string node payload.
    pub fn string(&self, id: StringId) -> Result<&JsString, HeapError> {
        match self.live_node(RawId::String(id))?.data {
            NodeData::String(ref value) => Ok(value),
            NodeData::Object(_)
            | NodeData::Shape(_)
            | NodeData::VarRef(_)
            | NodeData::Context(_)
            | NodeData::FunctionBytecode(_)
            | NodeData::BigInt(_) => Err(HeapError::Invariant(
                "typed string lookup reached another node payload",
            )),
        }
    }

    /// Mutable payload access requires a single owning arena edge. The
    /// JsString append kernel additionally authenticates its Rc buffer owner,
    /// covering public roots, constants, and payloads shared by distinct nodes.
    pub(crate) fn unique_string_mut(
        &mut self,
        id: StringId,
    ) -> Result<Option<&mut JsString>, HeapError> {
        if self.strong_count(RawId::String(id))? != 1 {
            return Ok(None);
        }
        match &mut self.live_node_mut(RawId::String(id))?.data {
            NodeData::String(value) => Ok(Some(value)),
            _ => Err(HeapError::Invariant(
                "typed string mutation reached another node payload",
            )),
        }
    }

    /// Trusted shared read for a live `StringId` held by an owning edge.
    #[inline]
    pub(crate) fn string_fast(&self, id: StringId) -> &JsString {
        match &self.live_node_fast(RawId::String(id)).data {
            NodeData::String(value) => value,
            _ => unreachable!("trusted string handle reached another node payload"),
        }
    }

    /// Replace a consumed operand's payload only when its arena edge is unique.
    /// Callers must own (not merely borrow) that edge and transfer it to the result.
    /// The old Rc payload may still be shared by public values or other nodes;
    /// replacing the payload does not mutate any such shared BigInt.
    pub(crate) fn unique_bigint_mut(
        &mut self,
        id: BigIntId,
    ) -> Result<Option<&mut JsBigInt>, HeapError> {
        if self.strong_count(RawId::BigInt(id))? != 1 {
            return Ok(None);
        }
        match &mut self.live_node_mut(RawId::BigInt(id))?.data {
            NodeData::BigInt(value) => Ok(Some(value)),
            _ => Err(HeapError::Invariant(
                "typed bigint mutation reached another node payload",
            )),
        }
    }

    /// Read one live BigInt node payload.
    pub fn bigint(&self, id: BigIntId) -> Result<&JsBigInt, HeapError> {
        match self.live_node(RawId::BigInt(id))?.data {
            NodeData::BigInt(ref value) => Ok(value),
            NodeData::Object(_)
            | NodeData::Shape(_)
            | NodeData::VarRef(_)
            | NodeData::Context(_)
            | NodeData::FunctionBytecode(_)
            | NodeData::String(_) => Err(HeapError::Invariant(
                "typed bigint lookup reached another node payload",
            )),
        }
    }
}
