use super::*;

impl Heap {
    /// Read one live string node payload.
    pub fn string(&self, id: StringId) -> Result<&JsString, HeapError> {
        match &self.live_leaf_slot(RawId::String(id))?.value {
            LeafValue::String(value) => Ok(value),
            LeafValue::BigInt(_) | LeafValue::Vacant | LeafValue::Retired => Err(
                HeapError::Invariant("typed string lookup reached another leaf payload"),
            ),
        }
    }

    /// Mutable payload access requires a single owning arena edge. The
    /// JsString append kernel additionally authenticates its Rc buffer owner,
    /// covering public roots, constants, and payloads shared by distinct nodes.
    ///
    /// Callers own the edge, so the count check and the borrow are trusted:
    /// debug builds assert the slot identity through `live_leaf_fast`.
    pub(crate) fn unique_string_mut(
        &mut self,
        id: StringId,
    ) -> Result<Option<&mut JsString>, HeapError> {
        if self.live_leaf_fast(RawId::String(id)).strong.get() != 1 {
            return Ok(None);
        }
        match &mut self.live_leaf_fast_mut(RawId::String(id)).value {
            LeafValue::String(value) => Ok(Some(value)),
            _ => Err(HeapError::Invariant(
                "typed string mutation reached another leaf payload",
            )),
        }
    }

    /// Trusted shared read for a live `StringId` held by an owning edge.
    #[inline]
    pub(crate) fn string_fast(&self, id: StringId) -> &JsString {
        match &self.live_leaf_fast(RawId::String(id)).value {
            LeafValue::String(value) => value,
            _ => unreachable!("trusted string handle reached another leaf payload"),
        }
    }

    /// Trusted shared read for a live `BigIntId` held by an owning edge.
    #[inline]
    pub(crate) fn bigint_fast(&self, id: BigIntId) -> &JsBigInt {
        match &self.live_leaf_fast(RawId::BigInt(id)).value {
            LeafValue::BigInt(value) => value,
            _ => unreachable!("trusted bigint handle reached another leaf payload"),
        }
    }

    /// Replace a consumed operand's payload only when its arena edge is unique.
    /// Callers must own (not merely borrow) that edge and transfer it to the result.
    /// The old Rc payload may still be shared by public values or other nodes;
    /// replacing the payload does not mutate any such shared BigInt.
    ///
    /// Callers own the edge, so the count check and the borrow are trusted:
    /// debug builds assert the slot identity through `live_leaf_fast`.
    pub(crate) fn unique_bigint_mut(
        &mut self,
        id: BigIntId,
    ) -> Result<Option<&mut JsBigInt>, HeapError> {
        if self.live_leaf_fast(RawId::BigInt(id)).strong.get() != 1 {
            return Ok(None);
        }
        match &mut self.live_leaf_fast_mut(RawId::BigInt(id)).value {
            LeafValue::BigInt(value) => Ok(Some(value)),
            _ => Err(HeapError::Invariant(
                "typed bigint mutation reached another leaf payload",
            )),
        }
    }

    /// Read one live BigInt node payload.
    pub fn bigint(&self, id: BigIntId) -> Result<&JsBigInt, HeapError> {
        match &self.live_leaf_slot(RawId::BigInt(id))?.value {
            LeafValue::BigInt(value) => Ok(value),
            LeafValue::String(_) | LeafValue::Vacant | LeafValue::Retired => Err(
                HeapError::Invariant("typed bigint lookup reached another leaf payload"),
            ),
        }
    }
}
