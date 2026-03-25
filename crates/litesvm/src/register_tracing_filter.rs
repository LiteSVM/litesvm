use {
    nom::{
        branch::alt,
        bytes::complete::{tag, take_while1},
        character::complete::multispace0,
        multi::separated_list1,
        sequence::delimited,
        IResult, Parser,
    },
    std::collections::HashMap,
};

// --- AST ---

#[derive(Debug, Clone)]
pub enum Op {
    Eq,
    Neq,
}

#[derive(Debug, Clone)]
pub struct Cond<'a> {
    field: &'a str,
    op: Op,
    value: &'a str,
}

#[derive(Debug, Clone)]
pub enum Expr<'a> {
    True, // always matches
    Cond(Cond<'a>),
    And(Vec<Expr<'a>>),
    Or(Vec<Expr<'a>>),
}

// --- Parser ---

/// Wraps a parser to strip leading and trailing whitespace.
fn ws<'a, F, O>(inner: F) -> impl Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>
where
    F: Parser<&'a str, Output = O, Error = nom::error::Error<&'a str>>,
{
    delimited(multispace0, inner, multispace0)
}

/// Parses a field name or value: alphanumeric, `_`, `-`, `.` (for example - dot
/// for struct.field).
fn ident(input: &str) -> IResult<&str, &str> {
    ws(take_while1(|c: char| {
        c.is_alphanumeric() || "_-.".contains(c)
    }))
    .parse(input)
}

/// Parses a comparison operator: `==` or `!=`.
fn op(input: &str) -> IResult<&str, Op> {
    ws(alt((tag("!=").map(|_| Op::Neq), tag("==").map(|_| Op::Eq)))).parse(input)
}

/// Parses a single condition: `field op value`.
fn cond(input: &str) -> IResult<&str, Expr<'_>> {
    (ident, op, ident)
        .map(|(field, op, value)| Expr::Cond(Cond { field, op, value }))
        .parse(input)
}

/// Parses an atomic expression: a parenthesized group or a single condition.
fn factor(input: &str) -> IResult<&str, Expr<'_>> {
    alt((delimited(ws(tag("(")), expr_inner, ws(tag(")"))), cond)).parse(input)
}

/// Parses one or more factors joined by `&&`.
fn and_expr(input: &str) -> IResult<&str, Expr<'_>> {
    separated_list1(ws(tag("&&")), factor)
        .map(|mut xs| {
            if xs.len() == 1 {
                xs.remove(0)
            } else {
                Expr::And(xs)
            }
        })
        .parse(input)
}

/// Parses one or more and-expressions joined by `||`.
fn or_expr(input: &str) -> IResult<&str, Expr<'_>> {
    separated_list1(ws(tag("||")), and_expr)
        .map(|mut xs| {
            if xs.len() == 1 {
                xs.remove(0)
            } else {
                Expr::Or(xs)
            }
        })
        .parse(input)
}

/// Returns `Expr::True` on empty input, otherwise delegates to `or_expr`.
fn expr_inner(input: &str) -> IResult<&str, Expr<'_>> {
    let (rest, _) = multispace0(input)?;
    if rest.is_empty() {
        return Ok((rest, Expr::True));
    }
    or_expr(input)
}

/// Parses a filter expression string into an AST.
/// Returns `Ok(Expr::True)` if the input is empty (matches everything).
/// Returns `Err` if the input is malformed or has trailing garbage.
/// Supports syntax: `field == value`, `field != value`, `&&`, `||`, and `()`
/// for grouping.
/// Example: `program_id == A || (program_id == B || program_id != C)`
pub fn expr(input: &str) -> Result<Expr<'_>, String> {
    match expr_inner(input) {
        Ok(("", ast)) => Ok(ast),
        Ok((rest, _)) => Err(format!("unexpected trailing input: '{}'", rest.trim())),
        Err(e) => Err(e.to_string()),
    }
}

// --- Evaluator (multi-value: field -> Vec<String>) ---

/// Evaluates a parsed expression against a row of data.
/// Each field maps to a vector of string values (multi-value support).
/// For `==`: returns true if the field contains the value.
/// For `!=`: returns true if ALL field values are different from the value.
/// For `&&`: detects contradictory `==` on the same field (e.g.
/// `field == A && field == B` where A != B) and short-circuits to false.
pub fn eval(expr: &Expr, row: &HashMap<&str, Vec<String>>) -> bool {
    match expr {
        Expr::True => true,
        Expr::Cond(c) => {
            let vals = row.get(c.field).map(Vec::as_slice).unwrap_or(&[]);
            match c.op {
                Op::Eq => vals.contains(&c.value.to_string()),
                Op::Neq => vals.iter().all(|v| *v != c.value),
            }
        }
        Expr::And(xs) => {
            let mut eq_by_field: HashMap<&str, &str> = HashMap::new();
            for x in xs {
                if let Expr::Cond(c) = x {
                    if matches!(c.op, Op::Eq) {
                        if let Some(prev) = eq_by_field.get(c.field) {
                            if *prev != c.value {
                                return false;
                            }
                        }
                        eq_by_field.insert(c.field, c.value);
                    }
                }
            }
            xs.iter().all(|e| eval(e, row))
        }
        Expr::Or(xs) => xs.iter().any(|e| eval(e, row)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real Solana program IDs for testing
    const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
    const SYSTEM_PROGRAM: &str = "11111111111111111111111111111111";
    const SPL_MEMO: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";

    #[test]
    fn test_empty_filter_matches_all() {
        let ast = expr("").unwrap();
        let row = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row));
    }

    #[test]
    fn test_simple_equality() {
        let filter = format!("program_id == {}", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        let row = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row));

        let row_no_match = HashMap::from([("program_id", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_simple_inequality() {
        let filter = format!("program_id != {}", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        let row = HashMap::from([("program_id", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row));

        let row_no_match = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_and_expression() {
        let filter = format!(
            "program_id == {} && program_id != {}",
            TOKEN_PROGRAM, SYSTEM_PROGRAM
        );
        let ast = expr(&filter).unwrap();
        let row_match = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row_match));

        let row_no_match = HashMap::from([("program_id", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_and_expression2() {
        let filter = format!(
            "program_id == {} && program_id == {}",
            TOKEN_PROGRAM, SYSTEM_PROGRAM
        );
        let ast = expr(&filter).unwrap();
        let row_no_match = HashMap::from([(
            "program_id",
            vec![TOKEN_PROGRAM.to_string(), SYSTEM_PROGRAM.to_string()],
        )]);
        // can't equal both in the same time
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_or_expression() {
        let filter = format!(
            "program_id == {} || program_id == {}",
            TOKEN_PROGRAM, SYSTEM_PROGRAM
        );
        let ast = expr(&filter).unwrap();
        let row_match1 = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row_match1));

        let row_match2 = HashMap::from([("program_id", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row_match2));

        let row_no_match = HashMap::from([("program_id", vec![SPL_MEMO.to_string()])]);
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_parentheses_grouping() {
        let filter = format!(
            "program_id == {} || (program_id == {} && program_id != {})",
            TOKEN_PROGRAM, SYSTEM_PROGRAM, SPL_MEMO
        );
        let ast = expr(&filter).unwrap();
        let row_match1 = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row_match1));

        let row_match2 = HashMap::from([("program_id", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row_match2));

        let row_no_match = HashMap::from([("program_id", vec![SPL_MEMO.to_string()])]);
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_multi_value_equality() {
        let filter = format!("program_id == {}", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        let row = HashMap::from([(
            "program_id",
            vec![SYSTEM_PROGRAM.to_string(), TOKEN_PROGRAM.to_string()],
        )]);
        assert!(eval(&ast, &row));
    }

    #[test]
    fn test_multi_value_inequality() {
        let filter = format!("program_id != {}", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        // All values must be different from TOKEN_PROGRAM
        let row_match = HashMap::from([(
            "program_id",
            vec![SYSTEM_PROGRAM.to_string(), SPL_MEMO.to_string()],
        )]);
        assert!(eval(&ast, &row_match));

        // One value equals TOKEN_PROGRAM, so inequality fails
        let row_no_match = HashMap::from([(
            "program_id",
            vec![SYSTEM_PROGRAM.to_string(), TOKEN_PROGRAM.to_string()],
        )]);
        assert!(!eval(&ast, &row_no_match));
    }

    #[test]
    fn test_whitespace_handling() {
        let filter = format!("  program_id   ==   {}  ", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        let row = HashMap::from([("program_id", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row));
    }

    #[test]
    fn test_missing_field() {
        let filter = format!("program_id == {}", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        let row = HashMap::from([("other_field", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(!eval(&ast, &row));
    }

    #[test]
    fn test_nested_field() {
        let filter = format!("account.owner == {}", TOKEN_PROGRAM);
        let ast = expr(&filter).unwrap();
        let row = HashMap::from([("account.owner", vec![TOKEN_PROGRAM.to_string()])]);
        assert!(eval(&ast, &row));

        let row_no_match = HashMap::from([("account.owner", vec![SYSTEM_PROGRAM.to_string()])]);
        assert!(!eval(&ast, &row_no_match));
    }
}
