//! Number-only execution of publication-authenticated canonical spans.
use super::{Error, Instruction, RunSlots};
use crate::engine::code::fusion::UpdateLocal;
use crate::engine::value::number::operations::Number;

pub(super) fn update_local(
    slots: &mut RunSlots<'_>,
    index: u16,
    update: UpdateLocal,
) -> Result<bool, Error> {
    if !slots.update_number_local(index, |previous| {
        let next = previous.update(update.increment);
        let result = (!update.discard).then_some(if update.postfix { previous } else { next });
        (next, result)
    })? {
        return Ok(false);
    }
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event(if update.discard {
        "fusion.UpdateLocalDiscard"
    } else if update.postfix {
        "fusion.UpdateLocalPostfix"
    } else {
        "fusion.UpdateLocalPrefix"
    });
    Ok(true)
}

/// The comparison semantics shared by the stack-consuming `CompareBranch`
/// span and the non-consuming local span. NaN and signed-zero behavior is the
/// float comparison itself; the caller has already proven both operands are
/// numbers.
#[inline(always)]
fn compare_numbers(instruction: &Instruction, left: Number, right: Number) -> bool {
    let (left, right) = (left.float(), right.float());
    match instruction {
        Instruction::Lt => left < right,
        Instruction::Lte => left <= right,
        Instruction::Gt => left > right,
        Instruction::Gte => left >= right,
        Instruction::Eq | Instruction::StrictEq => left == right,
        Instruction::Neq | Instruction::StrictNeq => left != right,
        _ => unreachable!("comparison opcode was validated before the transaction"),
    }
}

/// Execute an S1 `producer(a); producer(b); cmp; If*; [Goto]` span. Both
/// bindings are read non-owningly; on any guard miss nothing has changed and
/// the caller re-runs the canonical span start. `code` begins at the span's
/// first PC, `instructions` is its authenticated length (4 or 5), and `pc`
/// supplies the no-`Goto` fallthrough.
#[inline]
pub(super) fn local_compare_branch(
    slots: &RunSlots<'_>,
    code: &[Instruction],
    pc: usize,
    instructions: usize,
) -> Option<usize> {
    let (left, right) = match (code.first()?, code.get(1)?) {
        (
            Instruction::GetLocal(left) | Instruction::GetLocalCheck(left),
            Instruction::GetLocal(right) | Instruction::GetLocalCheck(right),
        ) => (
            slots.immediate_local(*left)?,
            slots.immediate_local(*right)?,
        ),
        (
            Instruction::GetLocal(left) | Instruction::GetLocalCheck(left),
            Instruction::GetArg(right),
        ) => (
            slots.immediate_local(*left)?,
            slots.immediate_parameter(*right)?,
        ),
        _ => return None,
    };
    let (target, when) = match code.get(3)? {
        Instruction::IfTrue(target) => (*target as usize, true),
        Instruction::IfFalse(target) => (*target as usize, false),
        _ => return None,
    };
    let taken = compare_numbers(code.get(2)?, left, right) == when;
    Some(if taken {
        target
    } else if instructions == 5 {
        match code.get(4)? {
            Instruction::Goto(next) => *next as usize,
            _ => return None,
        }
    } else {
        pc + 4
    })
}

// Keep the number-pair comparison inlined into `run`: without the hint the
// inlining decision flips between builds and the comparison degrades into a
// two-level call chain through `consume_number_pair`.
#[inline]
pub(super) fn compare_branch(
    slots: &mut RunSlots<'_>,
    instruction: &Instruction,
    branch: &Instruction,
) -> Result<Option<usize>, Error> {
    if !matches!(
        instruction,
        Instruction::Lt
            | Instruction::Lte
            | Instruction::Gt
            | Instruction::Gte
            | Instruction::Eq
            | Instruction::StrictEq
            | Instruction::Neq
            | Instruction::StrictNeq
    ) {
        return Err(Error::internal("invalid authenticated comparison span"));
    }
    let (target, when) = match branch {
        Instruction::IfTrue(target) => (*target as usize, true),
        Instruction::IfFalse(target) => (*target as usize, false),
        _ => return Err(Error::internal("invalid authenticated comparison branch")),
    };
    let Some(result) = slots.consume_number_pair(|left, right| {
        let (left, right) = (left.float(), right.float());
        match instruction {
            Instruction::Lt => left < right,
            Instruction::Lte => left <= right,
            Instruction::Gt => left > right,
            Instruction::Gte => left >= right,
            Instruction::Eq | Instruction::StrictEq => left == right,
            Instruction::Neq | Instruction::StrictNeq => left != right,
            _ => unreachable!("comparison opcode was validated before the transaction"),
        }
    })?
    else {
        return Ok(None);
    };
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("fusion.CompareBranch");
    // usize::MAX cannot be an instruction PC: executable allocation bounds
    // require the instruction array's byte length to fit isize::MAX.
    Ok(Some(if result == when { target } else { usize::MAX }))
}

/// Count the original logical operations, including intermediate stack depths.
/// This remains the authoritative span weight for future instruction budgets;
/// currently neither canonical run nor these spans has an instruction budget.
#[cfg(feature = "profiling")]
pub(super) fn record_span(code: &[Instruction], mut depth: usize) {
    for instruction in code {
        crate::engine::api::profiling::record_owned_instruction(depth);
        let effect = instruction.stack_contract();
        depth = depth - effect.popped + effect.pushed;
    }
}

#[cfg(all(test, feature = "profiling"))]
mod tests {
    use crate::engine::api::profiling::CostProfile;
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn number_spans_execute_all_update_results_and_comparison_directions() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let profile = CostProfile::start();
        assert_eq!(context.eval("(()=>{ let n=1; let a=++n; let b=n++; n++; --n; for(let i=0;i<4;i++){n++;} return a===2 && b===2 && n===7 && !(NaN<0) && (-0===0); })()").unwrap(), Value::Bool(true));
        let costs = profile.snapshot();
        for event in [
            "fusion.UpdateLocalPrefix",
            "fusion.UpdateLocalPostfix",
            "fusion.UpdateLocalDiscard",
            "fusion.CompareBranch",
        ] {
            assert!(
                costs
                    .owned_execution_events
                    .get(event)
                    .copied()
                    .unwrap_or(0)
                    > 0,
                "missing {event}: {costs:?}"
            );
        }
    }

    #[test]
    fn guarded_fallback_keeps_coercion_const_tdz_and_capture_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let log=[];
            let n={valueOf(){log.push('convert'); return 4;}};
            let previous=n++;
            let big=2n; let old=big++;
            let text='7'; ++text;
            let captured=1; function read(){return captured;} captured++;
            let constant=false; try { const c={valueOf(){log.push('const');return 1;}}; c++; } catch(e){constant=e instanceof TypeError;}
            let tdz=false; try { z++; let z; } catch(e){tdz=e instanceof ReferenceError;}
            let compared={valueOf(){log.push('compare'); return 1;}};
            if(compared<2)log.push('branch');
            return previous===4 && n===5 && old===2n && big===3n && text===8 && read()===2 && constant && tdz && log.join(',')==='convert,const,compare,branch';
        })()"#).unwrap(), Value::Bool(true));
    }
    #[test]
    fn signed_zero_overflow_and_postfix_preserve_numeric_representation() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let negativeZero=-0; let previous=negativeZero++;
            let positiveZero=0; let positivePrevious=positiveZero--;
            let maximum=2147483647; let maximumPrevious=maximum++;
            let minimum=-2147483648; let minimumPrevious=minimum--;
            let infinity=Infinity; let infinityPrevious=infinity++;
            let nan=NaN; let nanPrevious=nan--;
            return Object.is(previous,-0) && negativeZero===1 && Object.is(positivePrevious,0)
                && positiveZero===-1 && maximumPrevious===2147483647 && maximum===2147483648
                && minimumPrevious===-2147483648 && minimum===-2147483649
                && infinityPrevious===Infinity && infinity===Infinity
                && Number.isNaN(nanPrevious) && Number.isNaN(nan)
                && !(nan<0) && !(nan>=0);
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn conversion_side_effects_and_resume_boundaries_keep_canonical_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let x={valueOf(){x=99; return 3;}}; let old=x++;
            let y={valueOf(){y=77; throw 42;}}; let caught=false;
            try {y++;} catch(error){caught=error===42;}
            let add=1; function change(){add=10;return 2;} add+=change();
            let read=1; let sum=read+(read=2);
            function* values(){let n=0;try {yield n++;}finally{++n;}return n++;}
            let iterator=values();let first=iterator.next();let second=iterator.next();
            let final=0;try{final++;throw 1;}catch(error){++final;}finally{final++;}
            return old===3 && x===4 && caught && y===77 && add===3 && sum===3 && read===2
                && first.value===0 && !first.done && second.value===2 && second.done && final===3;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}

#[cfg(test)]
mod local_compare_tests {
    use crate::engine::api::{Runtime, Value};

    #[test]
    fn every_operator_and_edge_value_matches_canonical_results() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"((n)=>{
            let a=0; for (let i=0;i<n;i++) a++;
            let b=0; for (let i=n;i>0;i--) b++;
            let c=0; for (let i=0;i<=n;i++) c++;
            let d=0; for (let i=n;i>=0;i--) d++;
            let e=0; for (let i=0;i!=n;i++) e++;
            let base=0; let f=0; for (let i=0;i==base;i++) f++;
            let g=0; for (let i=0;i!==n;i++) g++;
            let h=0; for (let i=0;i===base;i++) h++;
            let localBound=3; let lb=0; for (let i=0;i<localBound;i++) lb++;
            let nan=NaN; let zeros=0; for (let i=0;i<nan;i++) zeros++;
            let negative=-0; let negativeBody=0; for (let i=0;i<negative;i++) negativeBody++;
            return a===4&&b===4&&c===5&&d===5&&e===4&&f===1&&g===4&&h===1&&lb===3
                && zeros===0 && negativeBody===0;
        })(4)"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn coercion_capture_tdz_and_bigint_stay_canonical() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let conversions=0;
            let limit={valueOf(){conversions++; return 3;}};
            let body=0; for (let i=0;i<limit;i++) body++;
            let text='0'; let textBody=0; for (let i=0;i<text;i++) textBody++;
            let captured=0; function peek(){return captured;}
            let three=3; for (;captured<three;) captured++;
            let tdz=false;
            try { if (later<1) {} } catch(e) { tdz=e instanceof ReferenceError; }
            let later=1;
            let big=0; for (let i=0n;i<3n;i++) big++;
            return conversions===4 && body===3 && textBody===0
                && captured===3 && peek()===3 && tdz && big===3;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
    }
}
