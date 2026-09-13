from pathlib import Path
import tempfile
import unittest

from binary_object.context import ScanContext
from binary_object.rules import ordinary_properties, source_setup

ROOT = Path(__file__).resolve().parents[4]


class OrdinaryPropertyContracts(unittest.TestCase):
    def scan(self, edits=()):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in ordinary_properties.FILES:
                source = (ROOT / relative).read_text()
                for path, before, after in edits:
                    if path == relative:
                        self.assertIn(before, source)
                        source = source.replace(before, after)
                target = root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(source)
            context = ScanContext(root)
            source_setup.check(context)
            ordinary_properties.check(context)
            return context.errors

    def test_current_contracts(self):
        self.assertEqual(self.scan(), [])

    def test_bad_boundaries_are_rejected(self):
        storage, ordinary, dispatch, runtime, heap, access, proxy_get, proxy_method, proxy_own, proxy_boolean, descriptor, proxy_call, ordinary_set, proxy_set, proxy_define, array_length, number, typed_element, typed_write, proxy_prototype, builtin_prototype, object_builtin = ordinary_properties.FILES
        mutations = [
            (builtin_prototype, "let target = arguments", "runtime.internal_get_prototype_of(); let target = arguments"),
            (object_builtin, "if self.is_proxy_object(object)? {", "self.internal_set_prototype_of(); if self.is_proxy_object(object)? {"),
            (proxy_prototype, "let name = match &kind {", "runtime.internal_get_prototype_of(); let name = match &kind {"),
            (proxy_prototype, "if value != prototype {", "runtime.internal_set_prototype_of(); if value != prototype {"),
            (proxy_boolean, "let step = MethodStep::start(runtime, realm, object, name)?;", "runtime.internal_prevent_extensions(); let step = MethodStep::start(runtime, realm, object, name)?;"),
            (proxy_boolean, "let (rooted, key, deleting) = match self.phase {", "runtime.internal_delete_property(); let (rooted, key, deleting) = match self.phase {"),
            (access, 'self.validate_value_domain(base, "delete base")?;', 'self.native_to_property_key(); self.validate_value_domain(base, "delete base")?;'),
            (typed_element, "Ok(match step {", "runtime.native_to_bigint(); Ok(match step {"),
            (typed_write, "let result = match result {", "runtime.typed_array_convert_element(); let result = match result {"),
            (dispatch, "let same_receiver = matches!(receiver,", "self.typed_array_convert_element(); let same_receiver = matches!(receiver,"),
            (array_length, "Ok(match value {", "runtime.native_to_number(); Ok(match value {"),
            (number, "Ok(match step {", "runtime.to_primitive(); Ok(match step {"),
            (ordinary_set, "let _operation = runtime.operation();", "runtime.internal_set(); let _operation = runtime.operation();"),
            (proxy_set, "let key_value = runtime.property_key_value(&key)?;", "runtime.proxy_set(); let key_value = runtime.property_key_value(&key)?;"),
            (proxy_define, "let key_value = runtime.property_key_value(&key)?;", "runtime.internal_define_own_property(); let key_value = runtime.property_key_value(&key)?;"),
            (proxy_call, "let guard = ProxyMethodStackGuard::enter(runtime);", "runtime.call_proxy(); let guard = ProxyMethodStackGuard::enter(runtime);"),
            (dispatch, "PreparedHas::Proxy(current.clone())", "PreparedHas::Complete(false)"),
            (proxy_method, "let key = runtime.intern_property_key(name)?;", "runtime.internal_get(); let key = runtime.intern_property_key(name)?;"),
            (proxy_own, "let key_value = runtime.property_key_value(&key)?;", "runtime.internal_get_own_property(); let key_value = runtime.property_key_value(&key)?;"),
            (proxy_boolean, "let result = runtime.value_to_boolean(&value)?;", "runtime.internal_is_extensible(); let result = runtime.value_to_boolean(&value)?;"),
            (descriptor, "let key = runtime.intern_property_key(name)?;", "runtime.internal_has_property(); let key = runtime.intern_property_key(name)?;"),
            (proxy_get, "runtime.validate_object_and_key(&proxy, &key)?;", "runtime.internal_get(); runtime.validate_object_and_key(&proxy, &key)?;"),
            (storage, "struct OwnSlot", "pub(crate) struct OwnSlot"),
            # Replace every occurrence to simulate removal of the shared class gate.
            (storage, "ObjectKind::Ordinary", "ObjectKind::ModuleNamespace"),
            (storage, "fn locate(", "fn bad() { self.call_internal(); } fn locate("),
            (dispatch, "impl Runtime {", "fn ordinary_set_fast_path_available() {} impl Runtime {"),
            (ordinary_set, "runtime.validate_value_domain(&value,", "runtime.skip_domain(&value,"),
            (ordinary_set, "rejected_object.as_ref().unwrap_or(&receiver)", "&receiver"),
            (ordinary, "use crate::engine::object::ordinary_storage::ReadProbe;", "self.call_internal(); use crate::engine::object::ordinary_storage::ReadProbe;"),
            (access, 'self.validate_value_domain(&receiver,', 'self.internal_get(); self.validate_value_domain(&receiver,'),
            (runtime, "if !failure.published", "if failure.published"),
            (heap, ".retain_edges_transactionally(&new_edges)", ".skip_retain(&new_edges)"),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                self.assertTrue(self.scan([mutation]))
