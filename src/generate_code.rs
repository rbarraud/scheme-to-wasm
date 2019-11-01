use crate::common::{generate_id, BinOp, Expr, ExprKind, Prog};
use crate::type_check::type_check;
use crate::types::Type;
use im_rc::Vector;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use std::iter::FromIterator;

#[derive(Clone, Debug)]
pub struct GenerateCodeError(String);

// Allows other errors to wrap this one
impl std::error::Error for GenerateCodeError {}

impl From<&str> for GenerateCodeError {
    fn from(message: &str) -> Self {
        GenerateCodeError(String::from(message))
    }
}

impl std::fmt::Display for GenerateCodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "GenerateCodeError: {}", self.0)
    }
}

// TODO: clean up / remove function if unneeded
pub fn generate_code_typ(typ: &Type) -> Result<TokenStream, GenerateCodeError> {
    match typ {
        Type::Int => Ok(quote! { IntVal }),
        Type::Bool => Ok(quote! { BoolVal }),
        Type::Str => Ok(quote! { StrVal }),
        Type::List(inner_typ) => Ok({
            let inner_code = generate_code_typ(inner_typ)?;
            quote! {
                ListVal<#inner_code>
            }
        }),
        Type::Func(typs, ret_typ) => unimplemented!(),
        Type::Tuple(typs) => unimplemented!(),
        Type::Record(fields) => unimplemented!(),
        Type::Exists(bound_var, inner_typ) => unimplemented!(),
        Type::TypeVar(x) => unimplemented!(),
        Type::Unknown => panic!("Trying to convert an unknown type to Rust!"),
    }
}

fn get_record_placeholder(fields: &Vector<(String, Expr)>) -> String {
    let fields_str = fields
        .iter()
        .map(|pair| format!(" ({} {{}})", pair.0))
        .collect::<String>();
    format!("(make-record{})", fields_str)
}

/// Struct representing a pair of inline code and the global code necessary
/// to support it.
///
/// Ex. When constructing a "record" expression, there will be inline code to
/// construct the actual Struct representing the record type, as well as
/// global code that defines the struct so it can be heap allocated etc.
#[derive(Default)]
pub struct CodeFragment {
    pub inline: TokenStream,
    pub global: TokenStream,
}

// TODO: refactor each ExprKind into separate function
pub fn generate_code_exp(exp: &Expr) -> Result<CodeFragment, GenerateCodeError> {
    match &*exp.kind {
        ExprKind::Binop(op, exp1, exp2) => Ok({
            let CodeFragment {
                inline: inline1,
                global: global1,
            } = generate_code_exp(exp1)?;
            let CodeFragment {
                inline: inline2,
                global: global2,
            } = generate_code_exp(exp2)?;
            let inline = match op {
                BinOp::Add => quote! { (#inline1 + #inline2) },
                BinOp::Subtract => quote! { (#inline1 - #inline2) },
                BinOp::Multiply => quote! { (#inline1 * #inline2) },
                BinOp::Divide => quote! { (#inline1 / #inline2) },
                BinOp::LessThan => quote! { BoolVal(#inline1 < #inline2) },
                BinOp::GreaterThan => quote! { BoolVal(#inline1 > #inline2) },
                BinOp::LessOrEqual => quote! { BoolVal(#inline1 <= #inline2) },
                BinOp::GreaterOrEqual => quote! { BoolVal(#inline1 >= #inline2) },
                BinOp::EqualTo => quote! { BoolVal(#inline1 == #inline2) },

                // && and || cannot be operator overloaded in Rust
                // so a special form, i.e. some native Rust syntax with short
                // circuiting behavior needs to be used
                BinOp::And => quote! {
                    { if { #inline1.0 } { #inline2 } else { BoolVal(false) } }
                },
                BinOp::Or => quote! {
                    { if { #inline1.0 } { BoolVal(true) } else { #inline2 } }
                },
                BinOp::Concat => quote! { concat(#inline1, #inline2) },
            };
            CodeFragment {
                inline,
                global: TokenStream::from_iter(vec![global1, global2]),
            }
        }),
        ExprKind::If(pred, cons, alt) => Ok({
            let CodeFragment {
                inline: pred_inline,
                global: pred_global,
            } = generate_code_exp(pred)?;
            let CodeFragment {
                inline: cons_inline,
                global: cons_global,
            } = generate_code_exp(cons)?;
            let CodeFragment {
                inline: alt_inline,
                global: alt_global,
            } = generate_code_exp(alt)?;
            let inline = quote! { {
                if { #pred_inline.0 } {
                    #cons_inline
                } else {
                    #alt_inline
                }
            } };
            CodeFragment {
                inline,
                global: TokenStream::from_iter(vec![pred_global, cons_global, alt_global]),
            }
        }),
        ExprKind::Let(bindings, body) => Ok({
            let mut globals = vec![];
            let binding_stmts = bindings
                .iter()
                .map(|bind| {
                    let CodeFragment {
                        inline: bind_inline,
                        global: bind_global,
                    } = generate_code_exp(&bind.1)?;
                    globals.push(bind_global);
                    let name = format_ident!("{}", bind.0);
                    Ok(quote! {
                        let #name = #bind_inline;
                    })
                })
                .collect::<Result<Vec<TokenStream>, GenerateCodeError>>()?;
            let CodeFragment {
                inline: body_inline,
                global: body_global,
            } = generate_code_exp(body)?;
            globals.push(body_global);
            let inline = quote! { {
                #(#binding_stmts)*
                { #body_inline }
            } };
            CodeFragment {
                inline,
                global: TokenStream::from_iter(globals),
            }
        }),
        ExprKind::Lambda(params, ret_type, body) => unimplemented!(),
        ExprKind::FnApp(func, args) => unimplemented!(),
        ExprKind::Record(bindings) => Ok({
            let mut globals = vec![];
            let fields = bindings
                .iter()
                .map(|bind| {
                    let CodeFragment {
                        inline: bind_inline,
                        global: bind_global,
                    } = generate_code_exp(&bind.1)?;
                    globals.push(bind_global);
                    let name = format_ident!("{}", bind.0);
                    Ok(quote! {
                        #name: #bind_inline,
                    })
                })
                .collect::<Result<Vec<TokenStream>, GenerateCodeError>>()?;
            let field_types = bindings
                .iter()
                .map(|bind| {
                    let field_typ = match type_check(&bind.1) {
                        Ok(val) => val,
                        Err(e) => {
                            return Err(GenerateCodeError::from(
                                format!("TypeCheckError: {}", e).as_str(),
                            ))
                        }
                    };
                    let field_typ_code = generate_code_typ(&field_typ)?;
                    let name = format_ident!("{}", bind.0);
                    Ok(quote! {
                        pub #name: #field_typ_code,
                    })
                })
                .collect::<Result<Vec<TokenStream>, GenerateCodeError>>()?;
            // let record_name = generate_record_name();
            let record_name = format_ident!("Record{}", generate_id());
            // TODO: refactor type_checker to annotate types
            let record_type = match type_check(&exp) {
                Ok(val) => val,
                Err(e) => {
                    return Err(GenerateCodeError::from(
                        format!("TypeCheckError: {}", e).as_str(),
                    ))
                }
            };
            let record_type_str = format!("{}", record_type);
            let record_placeholder = get_record_placeholder(bindings);
            let record_fields = bindings
                .iter()
                .map(|pair| format_ident!("{}", pair.0.clone()))
                .collect::<Vec<proc_macro2::Ident>>();
            let global = quote! {
                #[derive(Clone)]
                struct #record_name {
                    #(#field_types)*
                }

                impl DisplayType for #record_name {
                    fn fmt_type() -> String {
                        String::from(#record_type_str)
                    }
                }

                impl Display for #record_name {
                    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        write!(f, #record_placeholder, #(self.#record_fields),*)
                    }
                }
            };
            let inline = quote! { {
                #record_name {
                    #(#fields)*
                }
            } };
            CodeFragment { inline, global }
        }),
        ExprKind::RecordGet(record, key) => unimplemented!(),
        ExprKind::Begin(exps) => unimplemented!(),
        ExprKind::Set(var_name, exp) => unimplemented!(),
        ExprKind::Cons(car, cdr) => Ok({
            let CodeFragment {
                inline: car_inline,
                global: car_global,
            } = generate_code_exp(car)?;
            let CodeFragment {
                inline: cdr_inline,
                global: cdr_global,
            } = generate_code_exp(cdr)?;
            let inline = quote! {
                ListVal::Cons(#car_inline, Box::from(#cdr_inline))
            };
            CodeFragment {
                inline,
                global: TokenStream::from_iter(vec![car_global, cdr_global]),
            }
        }),
        ExprKind::Car(exp) => Ok({
            let CodeFragment {
                inline: exp_inline,
                global: exp_global,
            } = generate_code_exp(exp)?;
            let inline = quote! {
                #exp_inline.get_car()
            };
            CodeFragment {
                inline,
                global: exp_global,
            }
        }),
        ExprKind::Cdr(exp) => Ok({
            let CodeFragment {
                inline: exp_inline,
                global: exp_global,
            } = generate_code_exp(exp)?;
            let inline = quote! {
                #exp_inline.get_cdr()
            };
            CodeFragment {
                inline,
                global: exp_global,
            }
        }),
        ExprKind::IsNull(exp) => Ok({
            let CodeFragment {
                inline: exp_inline,
                global: exp_global,
            } = generate_code_exp(exp)?;
            let inline = quote! {
                #exp_inline.is_null()
            };
            CodeFragment {
                inline,
                global: exp_global,
            }
        }),
        ExprKind::Null(typ) => Ok({
            let _typ_code = generate_code_typ(typ)?;
            // Rust does not require us to parameterize ListVal<T> since it
            // can be inferred.
            let inline = quote! {
                ListVal::Null
            };
            CodeFragment {
                inline,
                ..Default::default()
            }
        }),
        ExprKind::Tuple(exps) => unimplemented!(),
        ExprKind::TupleGet(tup, key) => unimplemented!(),
        ExprKind::Pack(val, sub, exist) => unimplemented!(),
        ExprKind::Unpack(var, package, typ_sub, body) => unimplemented!(),
        ExprKind::Id(val) => Ok({
            let name = format_ident!("{}", val);
            let inline = quote! {
                #name
            };
            CodeFragment {
                inline,
                ..Default::default()
            }
        }),
        ExprKind::Num(val) => Ok({
            let inline = quote! {
                IntVal(#val)
            };
            CodeFragment {
                inline,
                ..Default::default()
            }
        }),
        ExprKind::Bool(val) => Ok({
            let inline = quote! {
                BoolVal(#val)
            };
            CodeFragment {
                inline,
                ..Default::default()
            }
        }),
        ExprKind::Str(val) => Ok({
            let inline = quote! {
                StrVal(String::from(#val))
            };
            CodeFragment {
                inline,
                ..Default::default()
            }
        }),
    }
}

// TODO: use prog.fns
pub fn generate_code_prog(prog: &Prog) -> Result<CodeFragment, GenerateCodeError> {
    generate_code_exp(&prog.exp)
}
