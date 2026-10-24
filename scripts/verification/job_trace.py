"""Exact production job-root edges, with a closed field policy for every variant."""

from source import item, masked, method


def proof(source, value_code):
    declaration = item(source, r"pub\(crate\) enum Job\b")
    expected = """pub(crate) enum Job {
        Reaction { record: ObjectId, argument: Value, rejected: bool, },
        Thenable { promise: ObjectId, thenable: Value, then: Value, },
        Call { function: Value, this: Value, args: Vec<Value>, },
    }"""
    if "".join(masked(declaration).split()) != "".join(expected.split()):
        raise ValueError("Job fields changed; review every variant's tracing policy")
    declaration = declaration.replace("pub(crate) enum Job", "enum Job", 1)
    trace = method(item(source, r"impl Job\b"), "trace", """
        ensures final(stack)@ == old(stack)@ + self.roots(),
    """)
    specs = """
        spec fn roots(&self) -> Seq<Value> {
            match self {
                Job::Reaction { record, argument, rejected: _ } =>
                    seq![Value((TAG_OBJECT << TAG_SHIFT) | record.0 as u64), *argument],
                Job::Thenable { promise, thenable, then } =>
                    seq![Value((TAG_OBJECT << TAG_SHIFT) | promise.0 as u64), *thenable, *then],
                Job::Call { function, this, args } => seq![*function, *this] + args@,
            }
        }
    """
    marker = "pub fn main() {}"
    if value_code.count(marker) != 1:
        raise ValueError("job tracing proof lost its unique composition point")
    return value_code.replace(marker, declaration + "\nimpl Job {\n" + specs + trace + "\n}\n" + marker)
