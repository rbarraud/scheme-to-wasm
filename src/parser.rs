use crate::common::{BinOp, Expr, ExprKind, Type};
use im_rc::Vector;

#[derive(Clone, Debug)]
pub struct ParseError(String);

impl From<&str> for ParseError {
    fn from(message: &str) -> Self {
        ParseError(String::from(message))
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "ParseError: {}", self.0)
    }
}

// allows other errors to wrap this one
// see https://doc.rust-lang.org/rust-by-example/error/multiple_error_types/define_error_type.html
impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

//
// Helper functions
//

pub fn parse_type(annotation: &lexpr::Value) -> Result<Type, ParseError> {
    match annotation {
        lexpr::Value::Symbol(val) => match val.as_ref() {
            "int" => Ok(Type::Int),
            "bool" => Ok(Type::Bool),
            "string" => Ok(Type::Str),
            "unknown" => Ok(Type::Unknown),
            val => {
                let mut iter = val.chars();
                let first = iter.next();
                if first == Some('T') {
                    let rest_str: String = iter.collect();
                    match rest_str.parse::<u64>() {
                        Ok(num) => Ok(Type::TypeVar(num)),
                        Err(e) => Err(ParseError(format!("ParseError: {}", e))),
                    }
                } else {
                    Err(ParseError::from(
                        "Type annotation not recognized as a valid type.",
                    ))
                }
            }
        },
        lexpr::Value::Cons(_) => {
            let lst_vec = match annotation.to_vec() {
                Some(vec) => vec,
                None => return Err(ParseError::from("Type annotation is not a valid list.")),
            };
            // ensure that the function annotation has at least -> and a return type as elements
            if lst_vec.is_empty() {
                return Err(ParseError::from("Type annotation is missing values."));
            }
            match lst_vec[0].as_symbol() {
                Some("->") => parse_func_annotation(lst_vec),
                Some("list") => parse_list_annotation(lst_vec),
                Some("tuple") => parse_tuple_annotation(lst_vec),
                Some("exists") => parse_exists_annotation(lst_vec),
                _ => Err(ParseError::from(
                    r#"Type annotation does not have "->", "tuple", or "list" as first symbol."#,
                )),
            }
        }
        _ => Err(ParseError::from(
            "Type annotation is invalid or is missing.",
        )),
    }
}

fn parse_func_annotation(lst_vec: Vec<lexpr::Value>) -> Result<Type, ParseError> {
    if lst_vec.len() < 2 {
        return Err(ParseError::from(
            "Type annotation for function is missing values.",
        ));
    }
    let input_types: Result<Vec<Type>, ParseError> = lst_vec[1..(lst_vec.len() - 1)]
        .iter()
        .map(|val| parse_type(val))
        .collect();
    let input_types_unwrapped = match input_types {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    let return_type = match parse_type(&lst_vec[lst_vec.len() - 1]) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    Ok(Type::Func(
        Vector::from(input_types_unwrapped),
        Box::from(return_type),
    ))
}

fn parse_list_annotation(lst_vec: Vec<lexpr::Value>) -> Result<Type, ParseError> {
    if lst_vec.len() != 2 {
        return Err(ParseError::from(
            "Type annotation for list has incorrect number of values.",
        ));
    }
    let lst_type = match parse_type(&lst_vec[1]) {
        Ok(typ) => typ,
        Err(e) => return Err(e),
    };
    Ok(Type::List(Box::from(lst_type)))
}

fn parse_tuple_annotation(lst_vec: Vec<lexpr::Value>) -> Result<Type, ParseError> {
    let tuple_types: Result<Vec<Type>, ParseError> = lst_vec[1..(lst_vec.len())]
        .iter()
        .map(|val| parse_type(val))
        .collect();
    let tuple_types_unwrapped = match tuple_types {
        Ok(val) => val,
        Err(e) => return Err(e),
    };
    Ok(Type::Tuple(Vector::from(tuple_types_unwrapped)))
}

fn parse_exists_annotation(lst_vec: Vec<lexpr::Value>) -> Result<Type, ParseError> {
    if lst_vec.len() != 3 {
        return Err(ParseError::from(
            "Type annotation for existential type has incorrect number of values.",
        ));
    }
    let type_var = match lst_vec[1].as_name() {
                        Some(val) => {
                            if !val.starts_with('T') {
                                return Err(ParseError::from("Type variable for existential type is not of the form T0, T1, etc."));
                            }
                            match val[1..].parse::<u64>() {
                            Ok(num) => num,
                            Err(e) => return Err(ParseError(format!("ParseError: {}", e))),
                        }},
                        None => return Err(ParseError::from(
                            "Type annotation for existential type does not have a valid type variable in its first argument.",
                        )),
                    };
    let lst_type = match parse_type(&lst_vec[2]) {
        Ok(typ) => typ,
        Err(e) => return Err(e),
    };
    Ok(Type::Exists(type_var, Box::from(lst_type)))
}

fn unwrap_lambda_args(args: &lexpr::Value) -> Result<Vector<(String, Type)>, ParseError> {
    let arg_list = match args.to_vec() {
        Some(vec) => vec,
        None => {
            return Err(ParseError::from(
                "Lambda arguments are not in a valid list.",
            ))
        }
    };
    arg_list
        .iter()
        .map(|arg| {
            // [x : int] as a vec
            let arg_vec = match arg.to_vec() {
                Some(vec) => vec,
                None => return Err(ParseError::from("Lambda argument is not a valid list.")),
            };
            if arg_vec.len() != 3 {
                return Err(ParseError::from(
                    "Lambda argument is missing values or contains extra values.",
                ));
            }

            // check there is a separator
            let separator = match arg_vec[1].as_symbol() {
                Some(val) => val,
                None => {
                    return Err(ParseError::from(
                        "Lambda argument does not contain the correct : separator.",
                    ))
                }
            };
            if separator != ":" {
                return Err(ParseError::from(
                    "Lambda argument does not contain the correct : separator.",
                ));
            }

            let arg_name = match arg_vec[0].as_symbol() {
                Some(val) => val,
                None => {
                    return Err(ParseError::from(
                        "Lambda argument does not have a valid name.",
                    ))
                }
            };
            let arg_type = match parse_type(&arg_vec[2]) {
                Ok(typ) => typ,
                Err(e) => return Err(e),
            };
            Ok((String::from(arg_name), arg_type))
        })
        .collect()
}

//
// Parsing functions
//

fn parse_array(exps: &[lexpr::Value]) -> Result<Vector<Expr>, ParseError> {
    exps.iter().map(|exp| parse(exp)).collect()
}

fn parse_binop(op: &str, rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() != 2 {
        return Err(ParseError::from(
            "Binary operator has incorrect number of sub-expressions.",
        ));
    }
    let exp1 = match parse(&rest[0]) {
        Ok(val) => Box::from(val),
        Err(e) => return Err(e),
    };
    let exp2 = match parse(&rest[1]) {
        Ok(val) => Box::from(val),
        Err(e) => return Err(e),
    };
    let operator = match op {
        "and" => BinOp::And,
        "or" => BinOp::Or,
        "+" => BinOp::Add,
        "-" => BinOp::Subtract,
        "*" => BinOp::Multiply,
        "/" => BinOp::Divide,
        "<" => BinOp::LessThan,
        ">" => BinOp::GreaterThan,
        "<=" => BinOp::LessOrEqual,
        ">=" => BinOp::GreaterOrEqual,
        "=" => BinOp::EqualTo,
        "concat" => BinOp::Concat,
        _ => return Err(ParseError::from("Unrecognized binary operator.")),
    };
    Ok(Expr::new(ExprKind::Binop(operator, exp1, exp2)))
}

fn parse_if(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 3 {
        parse(&rest[0]).and_then(|predicate| {
            parse(&rest[1]).and_then(|consequent| {
                parse(&rest[2]).and_then(|alternate| {
                    Ok(Expr::new(ExprKind::If(
                        Box::from(predicate),
                        Box::from(consequent),
                        Box::from(alternate),
                    )))
                })
            })
        })
    } else {
        Err(ParseError::from(
            "If expression has incorrect number of arguments.",
        ))
    }
}

fn parse_let(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() != 2 {
        return Err(ParseError::from(
            "Let expression has incorrect number of arguments.",
        ));
    }
    // Assert that the bindings is a proper list
    let bindings = match rest[0].to_vec() {
        Some(vec) => vec,
        None => {
            return Err(ParseError::from(
                "Let expression bindings are not in a proper list.",
            ))
        }
    };

    let parsed_bindings: Result<Vector<(String, Expr)>, ParseError> = Vector::from(bindings)
        .iter()
        .map(|binding| {
            let binding_vec = match binding.to_vec() {
                Some(vec) => vec,
                None => return Err(ParseError::from("Let binding is not a valid list.")),
            };
            if binding_vec.len() != 2 {
                return Err(ParseError::from(
                    "Let binding is missing values or contains extra values.",
                ));
            }

            binding_vec[0]
                .as_symbol()
                .ok_or_else(|| ParseError::from("Let binding does not have a valid name."))
                .and_then(|binding_name| {
                    parse(&binding_vec[1])
                        .and_then(|binding_val| Ok((String::from(binding_name), binding_val)))
                })
        })
        .collect();
    parsed_bindings.and_then(|bindings_vec| {
        parse(&rest[1])
            .and_then(|body_expr| Ok(Expr::new(ExprKind::Let(bindings_vec, Box::from(body_expr)))))
    })
}

fn parse_lambda(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() != 4 {
        return Err(ParseError::from(
            "Lambda expression has incorrect number of arguments. Perhaps you are missing the return type?",
        ));
    }
    let args = match unwrap_lambda_args(&rest[0]) {
        Ok(val) => val,
        Err(e) => return Err(e),
    };

    // check there is a separator
    let separator = match rest[1].as_symbol() {
        Some(val) => val,
        None => return Err(ParseError::from("Lambda expression does not have a separator between the arguments list and return type.")),
    };
    if separator != ":" {
        return Err(ParseError::from("Lambda expression does not have the correct separator : between the arguments list and return type."));
    }

    // get the (annotated) return type
    let ret_type = match parse_type(&rest[2]) {
        Ok(typ) => typ,
        Err(e) => return Err(e),
    };
    let body = match parse(&rest[3]) {
        Ok(typ) => typ,
        Err(e) => return Err(e),
    };

    Ok(Expr::new(ExprKind::Lambda(args, ret_type, Box::from(body))))
}

fn parse_make_env(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    let parsed_bindings: Result<Vector<(String, Expr)>, ParseError> = rest
        .iter()
        .map(|binding| {
            let binding_vec = match binding.to_vec() {
                Some(vec) => vec,
                None => return Err(ParseError::from("Make-env binding is not a valid list.")),
            };
            if binding_vec.len() != 2 {
                return Err(ParseError::from(
                    "Make-env binding is missing values or contains extra values.",
                ));
            }

            binding_vec[0]
                .as_symbol()
                .ok_or_else(|| ParseError::from("Make-env binding does not have a valid name."))
                .and_then(|binding_name| {
                    parse(&binding_vec[1])
                        .and_then(|binding_val| Ok((String::from(binding_name), binding_val)))
                })
        })
        .collect();
    parsed_bindings.and_then(|bindings_vec| Ok(Expr::new(ExprKind::Env(bindings_vec))))
}

fn parse_get_env(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 2 {
        parse(&rest[0]).and_then(|env| {
            rest[1]
                .as_symbol()
                .ok_or_else(|| ParseError::from("Env-ref key is not a valid identifier."))
                .and_then(|key| {
                    Ok(Expr::new(ExprKind::EnvGet(
                        Box::from(env),
                        String::from(key),
                    )))
                })
        })
    } else {
        Err(ParseError::from(
            "Env-ref expression has incorrect number of arguments.",
        ))
    }
}

fn parse_begin(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if !rest.is_empty() {
        match parse_array(&rest) {
            Ok(exps) => Ok(Expr::new(ExprKind::Begin(exps))),
            Err(e) => Err(e),
        }
    } else {
        Err(ParseError::from("Begin expression has no arguments."))
    }
}

fn parse_set_bang(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 2 {
        rest[0]
            .as_symbol()
            .ok_or_else(|| {
                ParseError::from("Set expression does not have a symbol as its first argument.")
            })
            .and_then(|var| {
                parse(&rest[1]).and_then(|expr| {
                    Ok(Expr::new(ExprKind::Set(String::from(var), Box::from(expr))))
                })
            })
    } else {
        Err(ParseError::from(
            "Set expression has incorrect number of arguments.",
        ))
    }
}

// This is parsed separately from other binary operators since it has a type
// which is not statically determined, so it would require adding extra
// complexity to the binary operator type checking function
fn parse_cons(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 2 {
        parse(&rest[0]).and_then(|arg1| {
            parse(&rest[1])
                .and_then(|arg2| Ok(Expr::new(ExprKind::Cons(Box::from(arg1), Box::from(arg2)))))
        })
    } else {
        Err(ParseError::from(
            "Cons expression has incorrect number of arguments.",
        ))
    }
}

fn parse_car(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 1 {
        parse(&rest[0]).and_then(|exp| Ok(Expr::new(ExprKind::Car(Box::from(exp)))))
    } else {
        Err(ParseError::from(
            "Car expression has incorrect number of arguments.",
        ))
    }
}

fn parse_cdr(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 1 {
        parse(&rest[0]).and_then(|exp| Ok(Expr::new(ExprKind::Cdr(Box::from(exp)))))
    } else {
        Err(ParseError::from(
            "Cdr expression has incorrect number of arguments.",
        ))
    }
}

fn parse_is_null(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 1 {
        parse(&rest[0]).and_then(|exp| Ok(Expr::new(ExprKind::IsNull(Box::from(exp)))))
    } else {
        Err(ParseError::from(
            "Null? expression has incorrect number of arguments.",
        ))
    }
}

fn parse_null(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 1 {
        match parse_type(&rest[0]) {
            Ok(typ) => Ok(Expr::new(ExprKind::Null(typ))),
            Err(e) => Err(e),
        }
    } else {
        Err(ParseError::from(
            "Null expression has incorrect number of arguments.",
        ))
    }
}

fn parse_func(first: &lexpr::Value, rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    let func = match parse(first) {
        Ok(exp) => exp,
        Err(e) => return Err(e),
    };
    let args = match parse_array(rest) {
        Ok(exps) => exps,
        Err(e) => return Err(e),
    };
    Ok(Expr::new(ExprKind::FnApp(Box::from(func), args)))
}

fn parse_make_tuple(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 3 {
        let vals = match rest[0].to_vec() {
            Some(val) => match parse_array(&val) {
                Ok(arr) => arr,
                Err(e) => return Err(e),
            },
            None => {
                return Err(ParseError::from(
                    "First argument in make-tuple expression is not a list.",
                ))
            }
        };
        // check there is a separator
        let separator = match rest[1].as_symbol() {
            Some(val) => val,
            None => return Err(ParseError::from("Make-tuple expression does not have a separator between the arguments list and return type.")),
        };
        if separator != ":" {
            return Err(ParseError::from("Make-tuple expression does not have the correct separator : between the arguments list and return type."));
        }

        let typs_vec_unparsed: Vec<lexpr::Value> = match rest[2].to_vec() {
            Some(val) => val,
            None => {
                return Err(ParseError::from(
                    "Make-tuple expression does not have a proper list of types.",
                ))
            }
        };
        let typs_vec: Vector<Type> = match typs_vec_unparsed
            .iter()
            .map(|typ| parse_type(typ))
            .collect()
        {
            Ok(vals) => vals,
            Err(e) => return Err(e),
        };

        Ok(Expr::new(ExprKind::Tuple(vals, typs_vec)))
    } else {
        Err(ParseError::from(
            "Make-tuple expression has incorrect number of arguments.",
        ))
    }
}

fn parse_get_tuple(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 2 {
        parse(&rest[0]).and_then(|env| {
            parse(&rest[1]).and_then(|key| {
                Ok(Expr::new(ExprKind::TupleGet(
                    Box::from(env),
                    Box::from(key),
                )))
            })
        })
    } else {
        Err(ParseError::from(
            "get-n expression has incorrect number of arguments.",
        ))
    }
}

fn parse_pack(rest: &[lexpr::Value]) -> Result<Expr, ParseError> {
    if rest.len() == 3 {
        parse(&rest[0]).and_then(|exp| {
            parse_type(&rest[1]).and_then(|type_var| {
                parse_type(&rest[2]).and_then(|exist| {
                    Ok(Expr::new(ExprKind::Pack(Box::from(exp), type_var, exist)))
                })
            })
        })
    } else {
        Err(ParseError::from(
            "get-n expression has incorrect number of arguments.",
        ))
    }
}

pub fn parse(value: &lexpr::Value) -> Result<Expr, ParseError> {
    match value {
        lexpr::Value::Number(x) => match x.as_i64() {
            Some(val) => Ok(Expr::new(ExprKind::Num(val))),
            None => Err(ParseError::from(
                "Invalid number found (must be a 64-bit integer).",
            )),
        },
        lexpr::Value::Bool(x) => Ok(Expr::new(ExprKind::Bool(*x))),
        lexpr::Value::String(x) => Ok(Expr::new(ExprKind::Str((*x).to_string()))),
        lexpr::Value::Cons(_) => {
            let lst = match value.to_vec() {
                Some(vec) => vec,
                None => return Err(ParseError::from("Cons expression is not a valid list.")),
            };
            // our language currently does not assign () to any meaning
            if lst.is_empty() {
                return Err(ParseError::from("Empty list found."));
            }
            let lst_parts = lst.split_at(1);
            let first = &(lst_parts.0)[0];
            let rest = lst_parts.1;

            match first.as_symbol() {
                Some(val) => match val {
                    "and" | "or" | "+" | "*" | "-" | "/" | ">" | "<" | ">=" | "<=" | "="
                    | "concat" => parse_binop(val, &rest),
                    "if" => parse_if(&rest),
                    "let" => parse_let(&rest),
                    "lambda" => parse_lambda(&rest),
                    "make-env" => parse_make_env(&rest),
                    "env-ref" => parse_get_env(&rest),
                    "begin" => parse_begin(&rest),
                    "set!" => parse_set_bang(&rest),
                    "cons" => parse_cons(&rest),
                    "car" => parse_car(&rest),
                    "cdr" => parse_cdr(&rest),
                    "null?" => parse_is_null(&rest),
                    "null" => parse_null(&rest),
                    "make-tuple" => parse_make_tuple(&rest),
                    "get-nth" => parse_get_tuple(&rest),
                    "pack" => parse_pack(&rest),
                    _ => parse_func(&first, &rest),
                },
                None => parse_func(&first, &rest),
            }
        }
        lexpr::Value::Symbol(x) => match &x[..] {
            "true" => Ok(Expr::new(ExprKind::Bool(true))),
            "false" => Ok(Expr::new(ExprKind::Bool(false))),
            symbol => Ok(Expr::new(ExprKind::Id(symbol.to_string()))),
        },
        _ => Err(ParseError::from("Unrecognized form of expression found.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use im_rc::vector;

    #[test]
    fn test_parse_type() {
        let exp = lexpr::from_str("int").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::Int);

        let exp = lexpr::from_str("bool").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::Bool);

        let exp = lexpr::from_str("string").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::Str);

        let exp = lexpr::from_str("T0").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::TypeVar(0));

        let exp = lexpr::from_str("T42").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::TypeVar(42));

        let exp = lexpr::from_str("(list int)").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::List(Box::from(Type::Int)));

        let exp = lexpr::from_str("(list (list int))").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::List(Box::from(Type::List(Box::from(Type::Int))))
        );

        let exp = lexpr::from_str("(tuple)").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::Tuple(vector![]));

        let exp = lexpr::from_str("(tuple int)").unwrap();
        assert_eq!(parse_type(&exp).unwrap(), Type::Tuple(vector![Type::Int]));

        let exp = lexpr::from_str("(tuple int string)").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::Tuple(vector![Type::Int, Type::Str])
        );

        let exp = lexpr::from_str("(-> int)").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::Func(vector![], Box::from(Type::Int))
        );

        let exp = lexpr::from_str("(-> int int)").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::Func(vector![Type::Int], Box::from(Type::Int))
        );

        let exp = lexpr::from_str("(-> string int bool)").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::Func(vector![Type::Str, Type::Int], Box::from(Type::Bool))
        );

        let exp = lexpr::from_str("(-> (-> int int bool) int int bool)").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::Func(
                vector![
                    Type::Func(vector![Type::Int, Type::Int], Box::from(Type::Bool)),
                    Type::Int,
                    Type::Int
                ],
                Box::from(Type::Bool)
            )
        );

        let exp = lexpr::from_str("(exists T0 (-> T0 bool))").unwrap();
        assert_eq!(
            parse_type(&exp).unwrap(),
            Type::Exists(
                0,
                Box::from(Type::Func(vector![Type::TypeVar(0)], Box::from(Type::Bool)))
            )
        );
    }
}
