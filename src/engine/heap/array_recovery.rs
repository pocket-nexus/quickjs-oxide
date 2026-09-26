use super::*;

impl Heap {
    /// Move a complete slow Array's default indexed data properties back into
    /// its contiguous payload. `indexed_slots` is the source shape's *physical*
    /// slot for each index in ascending numeric order. The runtime constructs
    /// that mapping while holding the same state borrow; this heap checks the
    /// immediate-integer key and all storage preconditions again before moving
    /// any owner.
    ///
    /// The replacement shape must contain precisely the non-index properties
    /// in their logical insertion order. Slow Arrays commonly have dictionary
    /// shapes, whose physical slot order can differ after a deletion.
    /// Allocation failure before publication returns `Ok(None)` with the
    /// source layout and all owners unchanged. `Some(cleanup)` means the move
    /// committed; an error from subsequent shape finalization still propagates.
    pub(crate) fn recover_array_dense_shape(
        &mut self,
        id: ObjectId,
        shape: ShapeId,
        indexed_slots: &[usize],
    ) -> Result<Option<HeapCleanup>, HeapError> {
        let (previous_shape, named_indices) = {
            let object = self.object(id)?;
            if !matches!(object.payload, ObjectPayload::Array { dense: None }) {
                return Err(HeapError::Invariant("dense recovery requires a slow Array"));
            }
            let source = self.shape(object.shape)?;
            let replacement = self.shape(shape)?;
            self.validate_property_layout(object.shape, &object.slots)?;
            if !source.dictionary_layout_is_valid() || replacement.is_dictionary() {
                return Err(HeapError::Invariant(
                    "dense recovery requires valid source and ordinary replacement shapes",
                ));
            }
            if indexed_slots.is_empty()
                || indexed_slots.len() > u32::MAX as usize
                || indexed_slots.len() > source.entries().len()
                || replacement.prototype() != source.prototype()
                || replacement.entries().len() != source.entries().len() - indexed_slots.len()
            {
                return Err(HeapError::Invariant(
                    "dense recovery shape or indexed count is invalid",
                ));
            }

            let mut named_indices = Vec::new();
            if named_indices
                .try_reserve_exact(replacement.entries().len())
                .is_err()
            {
                return Ok(None);
            }
            // A shape has unique atoms. Matching each immediate index to its
            // numeric position in `indexed_slots` proves the mapping is unique;
            // the expected named count below proves no index was omitted.
            // Validate both kinds in logical order, including dictionary order,
            // without an extra selected-slot bitmap or a second shape scan.
            for slot_index in source.ordered_indices() {
                let entry = &source.entries()[slot_index];
                if let Some(index) = entry.atom.immediate_integer() {
                    if indexed_slots.get(index as usize) != Some(&slot_index)
                        || entry.flags != PropertyFlags::data(true, true, true)
                        || !matches!(
                            object.slots.get(slot_index),
                            Some(PropertySlot::Data(value)) if is_map_storable_value(value)
                        )
                    {
                        return Err(HeapError::Invariant(
                            "dense recovery index is not default own data at its mapped slot",
                        ));
                    }
                } else {
                    if replacement.entries().get(named_indices.len()) != Some(entry) {
                        return Err(HeapError::Invariant(
                            "dense recovery changed named property order or flags",
                        ));
                    }
                    named_indices.push(slot_index);
                }
            }
            if named_indices.len() != replacement.entries().len() {
                return Err(HeapError::Invariant(
                    "dense recovery omitted named properties",
                ));
            }
            (object.shape, named_indices)
        };

        // Complete every fallible allocation before retaining the new shape
        // and publishing the representation change. Property values and their
        // object/String/BigInt/Symbol edges are moved, never duplicated.
        let mut dense = Vec::new();
        if dense.try_reserve_exact(indexed_slots.len()).is_err() {
            return Ok(None);
        }
        let mut named = Vec::new();
        if named.try_reserve_exact(named_indices.len()).is_err() {
            return Ok(None);
        }
        self.retain_shape(shape)?;

        self.invalidate_property_layout(id);
        let object = self
            .object_mut(id)
            .expect("authenticated Array disappeared during dense recovery");
        let mut old_slots = std::mem::take(&mut object.slots);
        for &slot_index in indexed_slots {
            let slot = std::mem::replace(
                old_slots
                    .get_mut(slot_index)
                    .expect("validated index slot disappeared during dense recovery"),
                PropertySlot::Data(RawValue::Undefined),
            );
            let PropertySlot::Data(value) = slot else {
                unreachable!("validated index changed storage during dense recovery")
            };
            dense.push(value);
        }
        for slot_index in named_indices {
            named.push(std::mem::replace(
                old_slots
                    .get_mut(slot_index)
                    .expect("validated named slot disappeared during dense recovery"),
                PropertySlot::Data(RawValue::Undefined),
            ));
        }
        object.shape = shape;
        object.slots = Slots::from_vec(named);
        let ObjectPayload::Array { dense: payload } = &mut object.payload else {
            unreachable!("validated Array changed class during dense recovery")
        };
        *payload = Some(dense);

        self.release_and_drain(RawId::Shape(previous_shape))
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::object::shape::ShapeEntry;

    const DATA: PropertyFlags = PropertyFlags::data(true, true, true);

    fn entry(atom: AtomIdx) -> ShapeEntry {
        ShapeEntry { atom, flags: DATA }
    }

    fn index(value: u32) -> AtomIdx {
        AtomIdx::from_immediate_integer(value).unwrap()
    }

    fn slow_array(heap: &mut Heap, shape: ShapeId, slots: Vec<PropertySlot>) -> ObjectId {
        let mut object = ObjectData::array(shape, slots);
        object.payload = ObjectPayload::Array { dense: None };
        heap.allocate_object(object).unwrap()
    }

    #[test]
    fn recovery_moves_edges_and_keeps_dictionary_named_order() {
        let mut heap = Heap::new();
        let target_shape = heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap();
        let target = heap
            .allocate_object(ObjectData::ordinary(target_shape, Vec::new()))
            .unwrap();

        // The removed name swaps the last physical slot to position one.
        // Logical named order remains name_b, name_c.
        let name_removed = AtomIdx::from_raw(1);
        let name_b = AtomIdx::from_raw(2);
        let name_c = AtomIdx::from_raw(3);
        let mut source = Shape::new(
            None,
            [
                entry(index(1)),
                entry(name_removed),
                entry(index(0)),
                entry(name_b),
                entry(name_c),
            ],
        )
        .unwrap();
        source.enable_dictionary();
        source.remove_dictionary_property(name_removed).unwrap();
        assert_eq!(
            source
                .entries()
                .iter()
                .map(|entry| entry.atom)
                .collect::<Vec<_>>(),
            [index(1), name_c, index(0), name_b]
        );
        let source = heap.allocate_shape(source).unwrap();
        let replacement = heap
            .allocate_shape(Shape::new(None, [entry(name_b), entry(name_c)]).unwrap())
            .unwrap();
        let array = slow_array(
            &mut heap,
            source,
            vec![
                PropertySlot::Data(RawValue::Object(target)),
                PropertySlot::Data(RawValue::Int(30)),
                PropertySlot::Data(RawValue::Int(0)),
                PropertySlot::Data(RawValue::Int(20)),
            ],
        );
        heap.replace_object_slot(array, 2, PropertySlot::Data(RawValue::Object(array)))
            .unwrap();
        let target_count = heap.object_strong_count(target).unwrap();
        let array_count = heap.object_strong_count(array).unwrap();

        heap.recover_array_dense_shape(array, replacement, &[2, 0])
            .unwrap()
            .expect("small recovery should be admitted");
        let object = heap.object(array).unwrap();
        assert_eq!(object.shape, replacement);
        assert_eq!(heap.array_dense_len(array), Ok(Some(2)));
        let ObjectPayload::Array { dense: Some(dense) } = &object.payload else {
            panic!("recovery did not publish the dense payload");
        };
        assert!(
            matches!(dense.as_slice(), [RawValue::Object(first), RawValue::Object(second)] if *first == array && *second == target)
        );
        assert!(matches!(
            object.slots.as_slice(),
            [
                PropertySlot::Data(RawValue::Int(20)),
                PropertySlot::Data(RawValue::Int(30))
            ]
        ));
        assert_eq!(heap.object_strong_count(target).unwrap(), target_count);
        assert_eq!(heap.object_strong_count(array).unwrap(), array_count);

        heap.release_shape(source).unwrap();
        heap.release_shape(replacement).unwrap();
        heap.release_object(array).unwrap();
        heap.run_gc_for_runtime_teardown().unwrap();
        assert_eq!(heap.object_strong_count(target), Ok(1));
        heap.release_object(target).unwrap();
        heap.release_shape(target_shape).unwrap();
        assert_eq!(heap.counts().live, 0);
    }

    #[test]
    fn invalid_mapping_and_descriptor_leave_slow_array_unchanged() {
        let mut heap = Heap::new();
        let source = heap
            .allocate_shape(Shape::new(None, [entry(index(1)), entry(index(0))]).unwrap())
            .unwrap();
        let replacement = heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap();
        let array = slow_array(
            &mut heap,
            source,
            vec![
                PropertySlot::Data(RawValue::Int(11)),
                PropertySlot::Data(RawValue::Int(22)),
            ],
        );
        let source_count = heap.shape_strong_count(source).unwrap();
        let replacement_count = heap.shape_strong_count(replacement).unwrap();
        for mapping in [&[0, 0][..], &[0, 1][..]] {
            assert!(
                heap.recover_array_dense_shape(array, replacement, mapping)
                    .is_err()
            );
            let object = heap.object(array).unwrap();
            assert_eq!(object.shape, source);
            assert!(matches!(
                object.payload,
                ObjectPayload::Array { dense: None }
            ));
            assert!(matches!(
                object.slots.as_slice(),
                [
                    PropertySlot::Data(RawValue::Int(11)),
                    PropertySlot::Data(RawValue::Int(22))
                ]
            ));
            assert_eq!(heap.shape_strong_count(source).unwrap(), source_count);
            assert_eq!(
                heap.shape_strong_count(replacement).unwrap(),
                replacement_count
            );
        }

        let special = heap
            .allocate_shape(
                Shape::new(
                    None,
                    [ShapeEntry {
                        atom: index(0),
                        flags: PropertyFlags::data(true, true, false),
                    }],
                )
                .unwrap(),
            )
            .unwrap();
        let special_array = slow_array(
            &mut heap,
            special,
            vec![PropertySlot::Data(RawValue::Int(1))],
        );
        assert!(
            heap.recover_array_dense_shape(special_array, replacement, &[0])
                .is_err()
        );
        assert!(matches!(
            heap.object(special_array).unwrap().payload,
            ObjectPayload::Array { dense: None }
        ));

        // The mapping names indices zero and one, but the source also has two.
        // The single-pass validation must reject that surplus index before
        // changing the shape or moving any value.
        let extra_source = heap
            .allocate_shape(
                Shape::new(None, [entry(index(2)), entry(index(1)), entry(index(0))]).unwrap(),
            )
            .unwrap();
        let extra_replacement = heap
            .allocate_shape(Shape::new(None, [entry(AtomIdx::from_raw(4))]).unwrap())
            .unwrap();
        let extra_array = slow_array(
            &mut heap,
            extra_source,
            vec![
                PropertySlot::Data(RawValue::Int(2)),
                PropertySlot::Data(RawValue::Int(1)),
                PropertySlot::Data(RawValue::Int(0)),
            ],
        );
        let source_count = heap.shape_strong_count(extra_source).unwrap();
        let replacement_count = heap.shape_strong_count(extra_replacement).unwrap();
        assert!(
            heap.recover_array_dense_shape(extra_array, extra_replacement, &[2, 1])
                .is_err()
        );
        let object = heap.object(extra_array).unwrap();
        assert_eq!(object.shape, extra_source);
        assert!(matches!(
            object.payload,
            ObjectPayload::Array { dense: None }
        ));
        assert!(matches!(
            object.slots.as_slice(),
            [
                PropertySlot::Data(RawValue::Int(2)),
                PropertySlot::Data(RawValue::Int(1)),
                PropertySlot::Data(RawValue::Int(0))
            ]
        ));
        assert_eq!(heap.shape_strong_count(extra_source), Ok(source_count));
        assert_eq!(
            heap.shape_strong_count(extra_replacement),
            Ok(replacement_count)
        );

        heap.release_object(extra_array).unwrap();
        heap.release_object(special_array).unwrap();
        heap.release_object(array).unwrap();
        heap.release_shape(extra_source).unwrap();
        heap.release_shape(extra_replacement).unwrap();
        heap.release_shape(special).unwrap();
        heap.release_shape(source).unwrap();
        heap.release_shape(replacement).unwrap();
        assert_eq!(heap.counts().live, 0);
    }
}
