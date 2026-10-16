//! The global functions: qjs's `print`, `parseInt`, `parseFloat`, `isNaN`
//! and `isFinite`.

use std::io::Write;

use super::to_string_id;
use crate::value::Value;
use crate::vm::native::CallSite;
use crate::vm::number::number;
use crate::vm::{Cx, Output, Throw, ops, radix};

/// qjs's `print`: the arguments' strings joined by a space, then a newline.
pub(super) fn print(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let mut line = String::new();
    for index in 0..site.argc {
        if index > 0 {
            line.push(' ');
        }
        let argument = site.arg(cx, index);
        line.push_str(&ops::display(cx, argument)?);
    }
    line.push('\n');
    match &mut cx.host.output {
        Output::Stdout => {
            let mut stdout = std::io::stdout().lock();
            let _ = stdout.write_all(line.as_bytes());
            let _ = stdout.flush();
        }
        Output::Capture(text) => text.push_str(&line),
    }
    Ok(Value::UNDEFINED)
}

/// `parseInt(string, radix)`: `ToString`, then `ToInt32` of the radix.
pub(super) fn parse_int(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let input = site.arg(cx, 0);
    let input = to_string_id(cx, input)?;
    let radix = site.arg(cx, 1);
    let radix = match radix.as_int32() {
        Some(radix) => radix,
        None => crate::vm::number::to_int32(ops::to_number(cx, radix)?),
    };
    let units: Vec<u16> = cx.heap.string(input).repr.units().collect();
    Ok(number(radix::parse_int(&units, radix)))
}

pub(super) fn parse_float(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let input = site.arg(cx, 0);
    let input = to_string_id(cx, input)?;
    let units: Vec<u16> = cx.heap.string(input).repr.units().collect();
    Ok(number(radix::parse_float(&units)))
}

pub(super) fn is_nan(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    Ok(Value::bool(ops::to_number(cx, value)?.is_nan()))
}

pub(super) fn is_finite(cx: &mut Cx<'_>, site: CallSite) -> Result<Value, Throw> {
    let value = site.arg(cx, 0);
    Ok(Value::bool(ops::to_number(cx, value)?.is_finite()))
}
