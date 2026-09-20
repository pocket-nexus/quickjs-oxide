//! Resident property operations end the short slot borrow before mutation/release.
use crate::engine::{
    api::{Error, runtime::Runtime},
    code::runtime::PublishedFunctionSnapshot,
    value::JsValue,
    vm::{exception::runtime_error_to_vm_error, stack::FrameTransaction},
};
#[derive(Clone, Copy)]
pub(super) enum Operation {
    Write(u32),
    ElementWrite,
    ElementRead(bool),
    Define(u32),
    Delete,
}
fn index(runtime: &Runtime, value: &JsValue) -> Option<u32> {
    match value {
        JsValue::Int(n) => u32::try_from(*n).ok(),
        JsValue::String(id) => {
            let text = super::super::numeric::string_payload(runtime, *id).ok()?;
            crate::engine::atom::AtomTable::canonical_array_index(&text)
        }
        _ => None,
    }
}
#[inline(never)]
pub(super) fn complete(
    runtime: &Runtime,
    executable: &PublishedFunctionSnapshot,
    pc: usize,
    transaction: &mut FrameTransaction<'_>,
    operation: Operation,
) -> Result<bool, Error> {
    if matches!(operation, Operation::ElementWrite) {
        let (base, key, value) = {
            let mut slots = transaction.slots();
            let value = slots.pop()?;
            let key = slots.pop()?;
            (slots.pop()?, key, value)
        };
        let handled = match index(runtime, &key) {
            Some(index) => runtime
                .try_dense_array_write_owned(&base, index, &value)
                .map_err(runtime_error_to_vm_error)?,
            None => false,
        };
        if !handled {
            let mut slots = transaction.slots();
            slots.push(base)?;
            slots.push(key)?;
            slots.push(value)?;
        }
        #[cfg(feature = "profiling")]
        if handled {
            crate::engine::api::profiling::record_owned_execution_event(
                "array_write_completed_in_run",
            );
        }
        return Ok(handled);
    }
    let (base, value) = {
        let mut slots = transaction.slots();
        let value = slots.pop()?;
        (slots.pop()?, value)
    };
    let mut result = None;
    let handled = match operation {
        Operation::Write(key) => runtime
            .try_property_ic_write_owned(&base, executable, pc, key, &value)
            .map_err(runtime_error_to_vm_error)?,
        Operation::Define(key) => runtime
            .try_define_field_owned(&base, executable, key, &value)
            .map_err(runtime_error_to_vm_error)?,
        Operation::Delete => {
            if matches!(
                value,
                JsValue::Int(_) | JsValue::String(_) | JsValue::Symbol(_)
            ) {
                let key = super::super::property_keys::canonical(runtime, &value)?;
                result = runtime
                    .try_delete_own_data(&base, &key)
                    .map_err(runtime_error_to_vm_error)?
                    .map(JsValue::Bool);
            }
            result.is_some()
        }
        Operation::ElementRead(_) => {
            result =
                index(runtime, &value).and_then(|index| runtime.try_dense_array_kept_read(&base, index));
            result.is_some()
        }
        Operation::ElementWrite => unreachable!(),
    };
    if !handled {
        let mut slots = transaction.slots();
        slots.push(base)?;
        slots.push(value)?;
        return Ok(false);
    }
    match operation {
        Operation::Define(_) => transaction.slots().push(base)?,
        Operation::ElementRead(keep_key) => {
            let mut slots = transaction.slots();
            slots.push(base)?;
            if keep_key {
                slots.push(value)?;
            }
            slots.push(result.expect("read result"))?;
        }
        Operation::Delete => transaction.slots().push(result.expect("delete result"))?,
        Operation::Write(_) | Operation::ElementWrite => {}
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(match operation {
        Operation::Write(_) => "property_write_completed_in_run",
        Operation::ElementWrite => "array_write_completed_in_run",
        Operation::ElementRead(_) => "array_kept_read_completed_in_run",
        Operation::Define(_) => "define_field_completed_in_run",
        Operation::Delete => "delete_completed_in_run",
    });
    Ok(true)
}

#[cfg(test)]
mod tests {
    use crate::engine::{api::runtime::Runtime, value::Value};
    #[test]
    fn resident_property_writes_preserve_descriptors_elements_and_references() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let result=context.eval(r#"(() => {
            function put(o,v) { o.x=v; }
            let a={x:0}, b={x:0}, value={alive:7};
            for(let i=0;i<5;i++){put(a,value);put(b,i);}
            if(a.x!==value || b.x!==4)return false;
            delete a.x; let calls=0;
            Object.defineProperty(a,'x',{set(v){calls+=v;},configurable:true});put(a,3);
            if(calls!==3)return false;
            delete a.x;Object.setPrototypeOf(a,{set x(v){calls+=v;}});put(a,4);
            if(calls!==7)return false;
            Object.freeze(b);put(b,42);if(b.x!==4)return false;
            function setElement(a,k,v){a[k]=v;}
            let arr=[1,2];setElement(arr,0,value);if(arr[0]!==value)return false;
            arr[1]++;if(arr[1]!==3)return false;
            Object.freeze(arr);setElement(arr,0,9);if(arr[0]!==value)return false;
            let lengths=[1,2,3];function len(a,n){a.length=n;}len(lengths,1);
            if(lengths.length!==1 || 1 in lengths)return false;
            let symbol=Symbol();let object={x:value,s:symbol};
            if(object.x!==value || object.s!==symbol || !delete object.x || 'x' in object)return false;
            let proxy=Proxy.revocable({x:0},{});put(proxy.proxy,1);proxy.revoke();
            try{put(proxy.proxy,2);return false;}catch(e){if(!(e instanceof TypeError))return false;}
            return true;
        })()"#).unwrap();
        assert_eq!(result, Value::Bool(true));
    }
}
