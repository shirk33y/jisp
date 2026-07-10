use jisp_ir::Expr;
use jisp_types::Type;
use proc_macro2::TokenStream;
use quote::quote;

use super::EmitContext;
use crate::CodegenError;

impl<'a> EmitContext<'a> {
    pub(super) fn emit_native_intrinsic(
        &mut self,
        name: &str,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        if let Some(operator) = binary_intrinsic_operator(name) {
            return self.emit_binary_intrinsic(arguments, operator);
        }
        match name {
            "/" => self.emit_divide_intrinsic(arguments, expected),
            "//" => self.emit_floor_divide_intrinsic(arguments, expected),
            "%" => self.emit_modulo_intrinsic(arguments, expected),
            "math.abs" => self.emit_abs_intrinsic(arguments, expected),
            "math.min" => self.emit_binary_method_intrinsic(arguments, quote! { min }),
            "math.max" => self.emit_binary_method_intrinsic(arguments, quote! { max }),
            "math.pow" => self.emit_pow_intrinsic(arguments, expected),
            "math.sqrt" => self.emit_unary_method_intrinsic(arguments, quote! { sqrt }),
            "math.floor" => self.emit_unary_method_intrinsic(arguments, quote! { floor }),
            "math.ceil" => self.emit_unary_method_intrinsic(arguments, quote! { ceil }),
            "math.round" => self.emit_unary_method_intrinsic(arguments, quote! { round }),
            "str.cat" => self.emit_str_cat_intrinsic(arguments),
            "str.len" => self.emit_str_len_intrinsic(arguments),
            "str.join" => self.emit_str_join_intrinsic(arguments),
            "str.split" => self.emit_str_split_intrinsic(arguments),
            "str.trim" => self.emit_unary_string_intrinsic(arguments, quote! { trim }),
            "str.upper" => self.emit_unary_string_intrinsic(arguments, quote! { to_uppercase }),
            "str.lower" => self.emit_unary_string_intrinsic(arguments, quote! { to_lowercase }),
            "str.has" => self.emit_binary_string_predicate(arguments, quote! { contains }),
            "str.starts" => self.emit_binary_string_predicate(arguments, quote! { starts_with }),
            "str.ends" => self.emit_binary_string_predicate(arguments, quote! { ends_with }),
            "str.replace" => self.emit_str_replace_intrinsic(arguments),
            "list.len" => self.emit_list_len_intrinsic(arguments),
            "list.cat" => self.emit_list_cat_intrinsic(arguments),
            "list.rest" => self.emit_list_rest_intrinsic(arguments),
            "list.has" => self.emit_list_has_intrinsic(arguments),
            "list.prepend" => self.emit_list_prepend_intrinsic(arguments),
            "list.append" => self.emit_list_append_intrinsic(arguments),
            "obj.len" => self.emit_obj_len_intrinsic(arguments),
            "obj.has" => self.emit_obj_has_intrinsic(arguments),
            "obj.keys" => self.emit_obj_keys_intrinsic(arguments),
            "obj.set" => self.emit_obj_set_intrinsic(arguments, expected),
            "obj.del" => self.emit_obj_del_intrinsic(arguments, expected),
            "obj.values" => self.emit_obj_values_intrinsic(arguments),
            "obj.cat" => self.emit_obj_cat_intrinsic(arguments, expected),
            _ => Err(CodegenError::Unsupported("calls outside native module")),
        }
    }

    fn emit_binary_intrinsic(
        &mut self,
        arguments: &[Expr],
        operator: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, None)?;
        let right = self.emit_expr(right, None)?;
        Ok(quote! { (#left #operator #right) })
    }

    fn emit_divide_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, expected)?;
        let right = self.emit_expr(right, expected)?;
        match expected {
            Some(Type::Int) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                __jisp_left
                    .checked_div(__jisp_right)
                    .expect("jisp / integer division by zero or overflow")
            }}),
            Some(Type::Float) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                if __jisp_right == 0.0 {
                    panic!("jisp / division by zero");
                }
                __jisp_left / __jisp_right
            }}),
            _ => Err(CodegenError::Unsupported(
                "native / without expected numeric type",
            )),
        }
    }

    fn emit_floor_divide_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, expected)?;
        let right = self.emit_expr(right, expected)?;
        match expected {
            Some(Type::Int) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                __jisp_left
                    .checked_div_euclid(__jisp_right)
                    .expect("jisp // integer division by zero or overflow")
            }}),
            Some(Type::Float) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                if __jisp_right == 0.0 {
                    panic!("jisp // division by zero");
                }
                (__jisp_left / __jisp_right).floor()
            }}),
            _ => Err(CodegenError::Unsupported(
                "native // without expected numeric type",
            )),
        }
    }

    fn emit_modulo_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, expected)?;
        let right = self.emit_expr(right, expected)?;
        match expected {
            Some(Type::Int) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                __jisp_left
                    .checked_rem_euclid(__jisp_right)
                    .expect("jisp % integer modulo by zero or overflow")
            }}),
            Some(Type::Float) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                if __jisp_right == 0.0 {
                    panic!("jisp % modulo by zero");
                }
                __jisp_left.rem_euclid(__jisp_right)
            }}),
            _ => Err(CodegenError::Unsupported(
                "native % without expected numeric type",
            )),
        }
    }

    fn emit_abs_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, expected)?;
        match expected {
            Some(Type::Int) => Ok(quote! {{
                let __jisp_value = #value;
                __jisp_value
                    .checked_abs()
                    .expect("jisp math.abs integer overflow")
            }}),
            Some(Type::Float) => Ok(quote! { #value.abs() }),
            _ => Err(CodegenError::Unsupported(
                "math.abs without expected native numeric type",
            )),
        }
    }

    fn emit_unary_method_intrinsic(
        &mut self,
        arguments: &[Expr],
        method: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, None)?;
        Ok(quote! { #value.#method() })
    }

    fn emit_binary_method_intrinsic(
        &mut self,
        arguments: &[Expr],
        method: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, None)?;
        let right = self.emit_expr(right, None)?;
        Ok(quote! { #left.#method(#right) })
    }

    fn emit_pow_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [left, right] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let left = self.emit_expr(left, expected)?;
        let right = self.emit_expr(right, expected)?;
        match expected {
            Some(Type::Int) => Ok(quote! {{
                let __jisp_base = #left;
                let __jisp_exponent = #right;
                if __jisp_exponent < 0i64 {
                    panic!("jisp math.pow requires non-negative integer exponent");
                }
                __jisp_base
                    .checked_pow(__jisp_exponent as u32)
                    .expect("jisp math.pow integer overflow")
            }}),
            Some(Type::Float) => Ok(quote! { #left.powf(#right) }),
            _ => Err(CodegenError::Unsupported(
                "math.pow without expected native numeric type",
            )),
        }
    }

    fn emit_str_cat_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        if arguments.is_empty() {
            return Ok(quote! { String::new() });
        }
        let arguments = arguments
            .iter()
            .map(|argument| self.emit_expr(argument, Some(&Type::Str)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { [#(#arguments),*].concat() })
    }

    fn emit_str_len_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, Some(&Type::Str))?;
        Ok(quote! { #value.chars().count() as i64 })
    }

    fn emit_str_join_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [delimiter, list] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let delimiter = self.emit_expr(delimiter, Some(&Type::Str))?;
        let list_type = Type::List(Box::new(Type::Str));
        let list = self.emit_expr(list, Some(&list_type))?;
        Ok(quote! { #list.join(&#delimiter) })
    }

    fn emit_str_split_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [value, delimiter] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let value = self.emit_expr(value, Some(&Type::Str))?;
        let delimiter = self.emit_expr(delimiter, Some(&Type::Str))?;
        Ok(quote! { #value.split(&#delimiter).map(String::from).collect::<Vec<String>>() })
    }

    fn emit_unary_string_intrinsic(
        &mut self,
        arguments: &[Expr],
        method: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, Some(&Type::Str))?;
        Ok(quote! { #value.#method().to_owned() })
    }

    fn emit_binary_string_predicate(
        &mut self,
        arguments: &[Expr],
        method: TokenStream,
    ) -> Result<TokenStream, CodegenError> {
        let [value, needle] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let value = self.emit_expr(value, Some(&Type::Str))?;
        let needle = self.emit_expr(needle, Some(&Type::Str))?;
        Ok(quote! { #value.#method(&#needle) })
    }

    fn emit_str_replace_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [value, from, to] = arguments else {
            return Err(CodegenError::Unsupported("non-ternary native intrinsics"));
        };
        let value = self.emit_expr(value, Some(&Type::Str))?;
        let from = self.emit_expr(from, Some(&Type::Str))?;
        let to = self.emit_expr(to, Some(&Type::Str))?;
        Ok(quote! { #value.replace(&#from, &#to) })
    }

    fn emit_list_len_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, None)?;
        Ok(quote! { #value.len() as i64 })
    }

    fn emit_list_cat_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        if arguments.is_empty() {
            return Ok(quote! { Vec::new() });
        }
        let arguments = arguments
            .iter()
            .map(|argument| self.emit_expr(argument, None))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! { vec![#(#arguments),*].into_iter().flatten().collect::<Vec<_>>() })
    }

    fn emit_list_rest_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, None)?;
        Ok(quote! { #value.get(1usize..).unwrap_or_default().to_vec() })
    }

    fn emit_list_has_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [list, value] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let list = self.emit_expr(list, None)?;
        let value = self.emit_expr(value, None)?;
        Ok(quote! { #list.contains(&#value) })
    }

    fn emit_list_prepend_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [value, list] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let value = self.emit_expr(value, None)?;
        let list = self.emit_expr(list, None)?;
        Ok(quote! {{
            let mut __jisp_list = #list;
            __jisp_list.insert(0usize, #value);
            __jisp_list
        }})
    }

    fn emit_list_append_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [list, value] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let list = self.emit_expr(list, None)?;
        let value = self.emit_expr(value, None)?;
        Ok(quote! {{
            let mut __jisp_list = #list;
            __jisp_list.push(#value);
            __jisp_list
        }})
    }

    fn emit_obj_len_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [object] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let row = self.native_object_row(object)?;
        let len = row.fields.len() as i64;
        Ok(quote! { #len })
    }

    fn emit_obj_has_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [object, key] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let Some(key) = super::static_string_key(key) else {
            return Err(CodegenError::Unsupported("dynamic native object keys"));
        };
        let row = self.native_object_row(object)?;
        let has = row.fields.contains_key(&key);
        Ok(quote! { #has })
    }

    fn emit_obj_keys_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [object] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let row = self.native_object_row(object)?;
        let keys = row.fields.keys();
        Ok(quote! { vec![#(String::from(#keys)),*] })
    }

    fn emit_obj_set_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [object, key, value] = arguments else {
            return Err(CodegenError::Unsupported("non-ternary native intrinsics"));
        };
        let Some(key) = super::static_string_key(key) else {
            return Err(CodegenError::Unsupported("dynamic native object keys"));
        };
        let Some(Type::Object(row)) = expected else {
            return Err(CodegenError::Unsupported(
                "obj.set without expected object type",
            ));
        };
        let ident = self.object_types.ident_for_row(row)?;
        let object = self.emit_expr(object, None)?;
        let fields = row
            .fields
            .iter()
            .map(|(name, ty)| {
                let field = super::rust_ident(name);
                if name == &key {
                    let value = self.emit_expr(value, Some(ty))?;
                    Ok(quote! { #field: #value })
                } else {
                    Ok(quote! { #field: __jisp_object.#field.clone() })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(quote! {{
            let __jisp_object = #object;
            #ident { #(#fields),* }
        }})
    }

    fn emit_obj_del_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [object, key] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        if super::static_string_key(key).is_none() {
            return Err(CodegenError::Unsupported("dynamic native object keys"));
        }
        let Some(Type::Object(row)) = expected else {
            return Err(CodegenError::Unsupported(
                "obj.del without expected object type",
            ));
        };
        let ident = self.object_types.ident_for_row(row)?;
        let object = self.emit_expr(object, None)?;
        let fields = row
            .fields
            .keys()
            .map(|name| {
                let field = super::rust_ident(name);
                quote! { #field: __jisp_object.#field.clone() }
            })
            .collect::<Vec<_>>();
        Ok(quote! {{
            let __jisp_object = #object;
            #ident { #(#fields),* }
        }})
    }

    fn emit_obj_values_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [object] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let row = self.native_object_row(object)?;
        let object = self.emit_expr(object, None)?;
        let fields = row.fields.keys().map(|name| {
            let field = super::rust_ident(name);
            quote! { __jisp_object.#field.clone() }
        });
        Ok(quote! {{
            let __jisp_object = #object;
            vec![#(#fields),*]
        }})
    }

    fn emit_obj_cat_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let Some(Type::Object(row)) = expected else {
            return Err(CodegenError::Unsupported(
                "obj.cat without expected object type",
            ));
        };
        let ident = self.object_types.ident_for_row(row)?;
        let rows = arguments
            .iter()
            .map(|argument| self.native_object_row(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let objects = arguments
            .iter()
            .map(|argument| self.emit_expr(argument, None))
            .collect::<Result<Vec<_>, _>>()?;
        let fields = row
            .fields
            .keys()
            .map(|name| {
                let source = rows
                    .iter()
                    .enumerate()
                    .rev()
                    .find(|(_, row)| row.fields.contains_key(name))
                    .map(|(index, _)| format!("__jisp_object_{index}"))
                    .ok_or(CodegenError::Unsupported("obj.cat native field mismatch"))?;
                let source = super::rust_ident(&source);
                let field = super::rust_ident(name);
                Ok(quote! { #field: #source.#field.clone() })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bindings = objects.into_iter().enumerate().map(|(index, object)| {
            let name = super::rust_ident(&format!("__jisp_object_{index}"));
            quote! { let #name = #object; }
        });
        Ok(quote! {{
            #(#bindings)*
            #ident { #(#fields),* }
        }})
    }

    fn native_object_row(&self, expr: &Expr) -> Result<jisp_types::ObjectRow, CodegenError> {
        let jisp_ir::ExprKind::Name(name) = &expr.kind else {
            return Err(CodegenError::Unsupported(
                "native object helper arguments without known object rows",
            ));
        };
        let ty = self
            .locals
            .get(name)
            .and_then(Option::as_ref)
            .or_else(|| self.top_level_schemes.get(name).map(|scheme| &scheme.body));
        match ty {
            Some(Type::Object(row)) if row.rest.is_none() => Ok(row.clone()),
            Some(Type::Object(_)) => {
                Err(CodegenError::Unsupported("open object row type emission"))
            }
            _ => Err(CodegenError::Unsupported(
                "native object helper arguments without known object rows",
            )),
        }
    }
}

fn binary_intrinsic_operator(name: &str) -> Option<TokenStream> {
    match name {
        "+" => Some(quote! { + }),
        "-" => Some(quote! { - }),
        "*" => Some(quote! { * }),
        "=" => Some(quote! { == }),
        "<" => Some(quote! { < }),
        ">" => Some(quote! { > }),
        "<=" => Some(quote! { <= }),
        ">=" => Some(quote! { >= }),
        _ => None,
    }
}
