//! Conservative facts about consumed UTF-16 text. Assertions and references
//! deliberately lose information rather than making assumptions about captures.

use super::{Atom, CharacterRange, Expression, ModifierState, RegExpFlags, Term};
use crate::unicode::case::regexp_canonicalize;

const MAX_LITERAL: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FirstSet {
    ascii: [u64; 2],
    non_ascii: bool,
}

impl FirstSet {
    fn empty() -> Self {
        Self {
            ascii: [0; 2],
            non_ascii: false,
        }
    }

    fn any() -> Self {
        Self {
            ascii: [u64::MAX; 2],
            non_ascii: true,
        }
    }

    fn union(&mut self, other: &Self) {
        self.ascii[0] |= other.ascii[0];
        self.ascii[1] |= other.ascii[1];
        self.non_ascii |= other.non_ascii;
    }

    pub(crate) fn contains(&self, unit: u16) -> bool {
        if unit < 128 {
            self.ascii[usize::from(unit / 64)] & (1 << (unit % 64)) != 0
        } else {
            self.non_ascii
        }
    }

    fn ranges(ranges: &[CharacterRange], inverted: bool, ignore_case: bool, unicode: bool) -> Self {
        let mut set = Self::empty();
        for unit in 0..128u32 {
            let value = if ignore_case {
                regexp_canonicalize(unit, unicode)
            } else {
                unit
            };
            if ranges
                .iter()
                .any(|range| range.start <= value && value <= range.end)
                != inverted
            {
                set.ascii[(unit / 64) as usize] |= 1 << (unit % 64);
            }
        }
        // Case folding can bring non-ASCII input into an ASCII range (Kelvin
        // sign and long s in Unicode mode). Keep that half of the set open.
        set.non_ascii = inverted || ignore_case || ranges.iter().any(|range| range.end >= 128);
        set
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RequiredLiteral {
    pub(crate) units: Vec<u16>,
    pub(crate) max_prefix: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SearchPlan {
    pub(crate) anchored: bool,
    pub(crate) minimum: usize,
    pub(crate) first: FirstSet,
    pub(crate) required: Option<RequiredLiteral>,
    pub(crate) literal: Option<Vec<u16>>,
}

impl Default for SearchPlan {
    fn default() -> Self {
        Self {
            anchored: false,
            minimum: 0,
            first: FirstSet::any(),
            required: None,
            literal: None,
        }
    }
}

impl SearchPlan {
    pub(super) fn owned_bytes(&self) -> usize {
        (self
            .required
            .as_ref()
            .map_or(0, |required| required.units.capacity())
            + self.literal.as_ref().map_or(0, Vec::capacity))
            * size_of::<u16>()
    }

    pub(super) fn analyze(expression: &Expression, flags: RegExpFlags, no_captures: bool) -> Self {
        let facts = expression_facts(
            expression,
            ModifierState::from_flags(flags),
            flags.is_unicode(),
        );
        let literal = if no_captures { facts.exact } else { None };
        Self {
            anchored: facts.anchored,
            minimum: facts.minimum,
            first: if facts.minimum == 0 {
                FirstSet::any()
            } else {
                facts.first
            },
            required: if literal.is_some() {
                None
            } else {
                facts.required
            },
            literal,
        }
    }
}

struct Facts {
    minimum: usize,
    maximum: Option<usize>,
    first: FirstSet,
    required: Option<RequiredLiteral>,
    // Some only for assertion-free, case-sensitive, fixed consumed text.
    exact: Option<Vec<u16>>,
    anchored: bool,
}

impl Facts {
    fn empty() -> Self {
        Self {
            minimum: 0,
            maximum: Some(0),
            first: FirstSet::empty(),
            required: None,
            exact: Some(Vec::new()),
            anchored: false,
        }
    }

    fn choose_required(&mut self, required: RequiredLiteral) {
        if !required.units.is_empty()
            && self
                .required
                .as_ref()
                .is_none_or(|old| old.units.len() < required.units.len())
        {
            self.required = Some(required);
        }
    }
}

fn add(left: Option<usize>, right: Option<usize>) -> Option<usize> {
    left?.checked_add(right?)
}

fn append(target: &mut Vec<u16>, source: &[u16]) -> bool {
    if target.len().saturating_add(source.len()) > MAX_LITERAL {
        return false;
    }
    target.extend_from_slice(source);
    true
}

fn expression_facts(expression: &Expression, modifiers: ModifierState, unicode: bool) -> Facts {
    let mut alternatives = expression.alternatives.iter().map(|sequence| {
        let mut result = Facts::empty();
        let mut run = Vec::new();
        let mut run_prefix = Some(0);
        for term in sequence {
            let facts = term_facts(term, modifiers, unicode);
            if result.minimum == 0 {
                result.first.union(&facts.first);
            }
            if result.maximum == Some(0) {
                result.anchored |= facts.anchored;
            }
            if let Some(required) = facts.required {
                result.choose_required(RequiredLiteral {
                    units: required.units,
                    max_prefix: add(result.maximum, required.max_prefix),
                });
            }
            if let Some(exact) = &facts.exact {
                if run.is_empty() {
                    run_prefix = result.maximum;
                }
                if !append(&mut run, exact) {
                    run.clear();
                    run_prefix = result.maximum;
                    append(&mut run, exact);
                }
                result.choose_required(RequiredLiteral {
                    units: run.clone(),
                    max_prefix: run_prefix,
                });
            } else {
                run.clear();
            }
            result.exact = match (result.exact.take(), facts.exact) {
                (Some(mut left), Some(right)) => append(&mut left, &right).then_some(left),
                _ => None,
            };
            result.minimum = result.minimum.saturating_add(facts.minimum);
            result.maximum = add(result.maximum, facts.maximum);
        }
        result
    });
    let mut result = alternatives.next().unwrap_or_else(Facts::empty);
    for other in alternatives {
        result.minimum = result.minimum.min(other.minimum);
        result.maximum = result
            .maximum
            .zip(other.maximum)
            .map(|(left, right)| left.max(right));
        result.first.union(&other.first);
        result.anchored &= other.anchored;
        if result.exact != other.exact {
            result.exact = None;
        }
        result.required = match (result.required.take(), other.required) {
            (Some(left), Some(right)) if left.units == right.units => Some(RequiredLiteral {
                units: left.units,
                max_prefix: left
                    .max_prefix
                    .zip(right.max_prefix)
                    .map(|(left, right)| left.max(right)),
            }),
            _ => None,
        };
    }
    result
}

fn term_facts(term: &Term, modifiers: ModifierState, unicode: bool) -> Facts {
    let mut result = atom_facts(&term.atom, modifiers, unicode);
    let Some(quantifier) = term.quantifier else {
        return result;
    };
    if quantifier.maximum == Some(0) {
        return Facts::empty();
    }
    result.minimum = result.minimum.saturating_mul(quantifier.minimum as usize);
    result.maximum = if result.maximum == Some(0) {
        Some(0)
    } else {
        result
            .maximum
            .zip(quantifier.maximum)
            .and_then(|(size, count)| size.checked_mul(count as usize))
    };
    result.exact = result.exact.and_then(|units| {
        let count = quantifier.minimum as usize;
        if quantifier.maximum != Some(quantifier.minimum)
            || units.len().saturating_mul(count) > MAX_LITERAL
        {
            return None;
        }
        if units.is_empty() {
            Some(units)
        } else {
            Some(units.repeat(count))
        }
    });
    if quantifier.minimum == 0 {
        result.required = None;
        result.anchored = false;
    }
    if let Some(exact) = &result.exact {
        result.required = (!exact.is_empty()).then(|| RequiredLiteral {
            units: exact.clone(),
            max_prefix: Some(0),
        });
    }
    result
}

fn atom_facts(atom: &Atom, modifiers: ModifierState, unicode: bool) -> Facts {
    let mut result = Facts::empty();
    match atom {
        Atom::Literal(value) => {
            let canonical = if modifiers.ignore_case {
                regexp_canonicalize(*value, unicode)
            } else {
                *value
            };
            result.first = FirstSet::ranges(
                &[CharacterRange::new(canonical, canonical)],
                false,
                modifiers.ignore_case,
                unicode,
            );
            let units = if *value > 0xffff {
                vec![
                    ((*value - 0x10000) / 1024 + 0xd800) as u16,
                    ((*value - 0x10000) % 1024 + 0xdc00) as u16,
                ]
            } else {
                vec![*value as u16]
            };
            result.minimum = units.len();
            result.maximum = Some(units.len());
            result.exact =
                if modifiers.ignore_case || (unicode && (0xd800..=0xdfff).contains(value)) {
                    None
                } else {
                    Some(units.clone())
                };
            if !modifiers.ignore_case {
                result.required = Some(RequiredLiteral {
                    units,
                    max_prefix: Some(0),
                });
            }
        }
        Atom::Group {
            modifiers: local,
            expression,
            ..
        } => return expression_facts(expression, local.unwrap_or(modifiers), unicode),
        Atom::LineStart => {
            result.anchored = !modifiers.multiline;
            result.exact = None;
        }
        Atom::LineEnd | Atom::WordBoundary { .. } | Atom::LookAround { .. } => {
            result.exact = None;
        }
        Atom::BackReference { .. } => {
            result.first = FirstSet::any();
            result.maximum = None;
            result.exact = None;
        }
        Atom::Class(class) => {
            result.minimum = 1;
            result.maximum = Some(if unicode { 2 } else { 1 });
            result.exact = None;
            result.first = FirstSet::ranges(
                &class.ranges,
                class.inverted,
                modifiers.ignore_case,
                unicode,
            );
            for string in &class.strings {
                let length = string
                    .iter()
                    .map(|value| if *value > 0xffff { 2 } else { 1 })
                    .sum();
                result.minimum = result.minimum.min(length);
                result.maximum = result.maximum.map(|maximum| maximum.max(length));
                if let Some(value) = string.first() {
                    let canonical = if modifiers.ignore_case {
                        regexp_canonicalize(*value, unicode)
                    } else {
                        *value
                    };
                    result.first.union(&FirstSet::ranges(
                        &[CharacterRange::new(canonical, canonical)],
                        false,
                        modifiers.ignore_case,
                        unicode,
                    ));
                }
            }
        }
        Atom::Dot | Atom::Space { .. } => {
            result.minimum = 1;
            result.maximum = Some(if unicode { 2 } else { 1 });
            result.first = FirstSet::any();
            result.exact = None;
        }
    }
    result
}
