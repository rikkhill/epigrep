use crate::model::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for ParseError {}

// Known Phase 1 limitations: this tiny parser splits predicate lists on `,` and
// locates transitions by scanning for `->`/`-[` outside `[...]`. It is not
// quote-aware, so attribute values containing `]`, `,`, or `->`
// (e.g. `name == "a,b"`) will mis-parse. A proper lexer is deferred; the AST and
// builder API, not this surface, define the supported semantics.
pub fn parse_pattern(input: &str) -> Result<Pattern, ParseError> {
    let mut rest = input.trim();
    if rest.is_empty() {
        return Err(ParseError::new("pattern is empty"));
    }

    let first_atom_text = take_until_transition(rest);
    let mut steps = vec![Step::first(parse_atom(first_atom_text)?)];
    rest = rest[first_atom_text.len()..].trim_start();

    while !rest.is_empty() {
        let (transition, after_transition) = parse_transition(rest)?;
        rest = after_transition.trim_start();

        let atom_text = take_until_transition(rest);
        if atom_text.trim().is_empty() {
            return Err(ParseError::new("transition is missing a following atom"));
        }
        steps.push(Step::then(parse_atom(atom_text)?, transition));
        rest = rest[atom_text.len()..].trim_start();
    }

    Ok(Pattern::sequence(steps))
}

fn take_until_transition(input: &str) -> &str {
    let bracket_depths = input
        .char_indices()
        .scan(0_i32, |depth, (index, character)| {
            match character {
                '[' => *depth += 1,
                ']' => *depth -= 1,
                _ => {}
            }
            Some((index, *depth))
        });

    for (index, depth) in bracket_depths {
        if depth == 0 && input[index..].starts_with("->") {
            return input[..index].trim_end();
        }
        if depth == 0 && input[index..].starts_with("-[") {
            return input[..index].trim_end();
        }
    }

    input.trim_end()
}

fn parse_transition(input: &str) -> Result<(Transition, &str), ParseError> {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix("->") {
        return Ok((Transition::any(), rest));
    }

    let Some(rest) = input.strip_prefix("-[") else {
        return Err(ParseError::new("expected transition `->` or `-[...] ->`"));
    };
    let Some((body, after_body)) = rest.split_once("]->") else {
        return Err(ParseError::new(
            "transition constraint is missing closing `]->`",
        ));
    };

    let mut transition = Transition::any();
    for constraint in body
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if let Some(max_elapsed) = constraint.strip_prefix("<=") {
            transition = transition.within(parse_timestamp(max_elapsed.trim())?);
        } else if let Some(absent_atom) = constraint.strip_prefix("no ") {
            transition = transition.with_absence(parse_atom(absent_atom.trim())?);
        } else {
            return Err(ParseError::new(format!(
                "unsupported transition constraint `{constraint}`"
            )));
        }
    }

    Ok((transition, after_body))
}

fn parse_atom(input: &str) -> Result<Atom, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError::new("atom is empty"));
    }

    let Some(bracket_start) = input.find('[') else {
        return Ok(Atom::event_type(input));
    };
    let Some(bracket_end) = input.rfind(']') else {
        return Err(ParseError::new(
            "atom predicate list is missing closing `]`",
        ));
    };
    if bracket_end != input.len() - 1 {
        return Err(ParseError::new("unexpected text after atom predicate list"));
    }

    let event_type = input[..bracket_start].trim();
    if event_type.is_empty() {
        return Err(ParseError::new("atom event type is empty"));
    }

    let mut atom = Atom::event_type(event_type);
    let predicates = input[bracket_start + 1..bracket_end].trim();
    if predicates.is_empty() {
        return Ok(atom);
    }

    for predicate in predicates
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        atom = parse_atom_clause(atom, predicate)?;
    }

    Ok(atom)
}

fn parse_atom_clause(atom: Atom, clause: &str) -> Result<Atom, ParseError> {
    if let Some((attribute, binding)) = clause.split_once(" as ") {
        return Ok(atom.with_capture(Capture::new(
            normalize_identifier(binding)?,
            attribute.trim(),
        )));
    }

    let (attribute, operator, value) = parse_comparison(clause)?;
    if let Some(binding) = value.strip_prefix('$') {
        return Ok(atom.with_reference_predicate(ReferencePredicate::new(
            attribute,
            operator,
            binding.trim(),
        )));
    }

    Ok(atom.with_predicate(Predicate::new(attribute, operator, parse_value(value)?)))
}

fn parse_comparison(input: &str) -> Result<(&str, ComparisonOperator, &str), ParseError> {
    for (token, operator) in [
        ("==", ComparisonOperator::Eq),
        ("!=", ComparisonOperator::NotEq),
        (">=", ComparisonOperator::Gte),
        ("<=", ComparisonOperator::Lte),
        (">", ComparisonOperator::Gt),
        ("<", ComparisonOperator::Lt),
    ] {
        if let Some((left, right)) = input.split_once(token) {
            let attribute = left.trim();
            let value = right.trim();
            if attribute.is_empty() || value.is_empty() {
                return Err(ParseError::new(
                    "comparison must have an attribute and value",
                ));
            }
            return Ok((attribute, operator, value));
        }
    }

    Err(ParseError::new(format!(
        "unsupported atom predicate `{input}`"
    )))
}

fn parse_value(input: &str) -> Result<Value, ParseError> {
    let input = input.trim();
    if let Some(value) = input
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        return Ok(Value::String(value.to_owned()));
    }
    if input == "true" {
        return Ok(Value::Bool(true));
    }
    if input == "false" {
        return Ok(Value::Bool(false));
    }
    if input == "null" {
        return Ok(Value::Null);
    }
    if input.contains('.') {
        return input
            .parse::<f64>()
            .map(Value::Float)
            .map_err(|_| ParseError::new(format!("invalid float literal `{input}`")));
    }
    input
        .parse::<i64>()
        .map(Value::Int)
        .map_err(|_| ParseError::new(format!("invalid literal `{input}`")))
}

fn parse_timestamp(input: &str) -> Result<Timestamp, ParseError> {
    input
        .parse::<Timestamp>()
        .map_err(|_| ParseError::new(format!("invalid timestamp literal `{input}`")))
}

fn normalize_identifier(input: &str) -> Result<&str, ParseError> {
    let input = input.trim();
    let Some(identifier) = input.strip_prefix('$') else {
        return Err(ParseError::new("capture binding must start with `$`"));
    };
    if identifier.is_empty() {
        return Err(ParseError::new("capture binding name is empty"));
    }
    Ok(identifier)
}
