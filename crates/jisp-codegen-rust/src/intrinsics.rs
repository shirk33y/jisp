use jisp_ir::Expr;
use jisp_types::Type;
use proc_macro2::TokenStream;
use quote::quote;

use super::{result_arguments, EmitContext};
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
            "bigint" => self.emit_bigint_intrinsic(arguments),
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
            "str.slice" => self.emit_str_slice_intrinsic(arguments),
            "list.len" => self.emit_list_len_intrinsic(arguments),
            "list.get" => self.emit_list_get_intrinsic(arguments, expected),
            "list.cat" => self.emit_list_cat_intrinsic(arguments),
            "list.rest" => self.emit_list_rest_intrinsic(arguments),
            "list.slice" => self.emit_list_slice_intrinsic(arguments, expected),
            "list.map" => self.emit_list_map_intrinsic(arguments),
            "list.filter" => self.emit_list_filter_intrinsic(arguments),
            "list.fold" => self.emit_list_fold_intrinsic(arguments),
            "list.some" => self.emit_list_predicate_intrinsic(arguments, true),
            "list.every" => self.emit_list_predicate_intrinsic(arguments, false),
            "list.has" => self.emit_list_has_intrinsic(arguments),
            "list.prepend" => self.emit_list_prepend_intrinsic(arguments),
            "list.append" => self.emit_list_append_intrinsic(arguments),
            "obj.len" => self.emit_obj_len_intrinsic(arguments),
            "obj.has" => self.emit_obj_has_intrinsic(arguments),
            "obj.get" => self.emit_obj_get_intrinsic(arguments, expected),
            "obj.keys" => self.emit_obj_keys_intrinsic(arguments),
            "obj.set" => self.emit_obj_set_intrinsic(arguments, expected),
            "obj.del" => self.emit_obj_del_intrinsic(arguments, expected),
            "obj.values" => self.emit_obj_values_intrinsic(arguments),
            "obj.cat" => self.emit_obj_cat_intrinsic(arguments, expected),
            "result.try" => self.emit_result_try_intrinsic(arguments, expected),
            "result.map" => self.emit_result_map_intrinsic(arguments, expected),
            "result.map-err" => self.emit_result_map_err_intrinsic(arguments, expected),
            "result.recover" => self.emit_result_recover_intrinsic(arguments, expected),
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
            Some(Type::BigInt) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                if __jisp_right == ::num_bigint::BigInt::from(0i64) {
                    panic!("jisp / division by zero");
                }
                __jisp_left / __jisp_right
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
            Some(Type::BigInt) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                let __jisp_zero = ::num_bigint::BigInt::from(0i64);
                if __jisp_right == __jisp_zero {
                    panic!("jisp // division by zero");
                }
                let __jisp_quotient = &__jisp_left / &__jisp_right;
                let __jisp_remainder = &__jisp_left % &__jisp_right;
                if __jisp_remainder < __jisp_zero {
                    if __jisp_right > __jisp_zero {
                        __jisp_quotient - ::num_bigint::BigInt::from(1i64)
                    } else {
                        __jisp_quotient + ::num_bigint::BigInt::from(1i64)
                    }
                } else {
                    __jisp_quotient
                }
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
            Some(Type::BigInt) => Ok(quote! {{
                let __jisp_left = #left;
                let __jisp_right = #right;
                let __jisp_zero = ::num_bigint::BigInt::from(0i64);
                if __jisp_right == __jisp_zero {
                    panic!("jisp % modulo by zero");
                }
                let __jisp_remainder = &__jisp_left % &__jisp_right;
                if __jisp_remainder < __jisp_zero {
                    if __jisp_right > __jisp_zero {
                        __jisp_remainder + __jisp_right
                    } else {
                        __jisp_remainder - __jisp_right
                    }
                } else {
                    __jisp_remainder
                }
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
            Some(Type::BigInt) => Ok(quote! {{
                let __jisp_value = #value;
                if __jisp_value < ::num_bigint::BigInt::from(0i64) {
                    -__jisp_value
                } else {
                    __jisp_value
                }
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

    fn emit_str_slice_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [value, start, end] = arguments else {
            return Err(CodegenError::Unsupported("non-ternary native intrinsics"));
        };
        let result_type = result_type(Type::Str, Type::Str);
        let ok = self
            .enum_types
            .prelude_constructor("ok", Some(&result_type))?
            .ok_or(CodegenError::Unsupported("str.slice native result type"))?;
        let err = self
            .enum_types
            .prelude_constructor("err", Some(&result_type))?
            .ok_or(CodegenError::Unsupported("str.slice native result type"))?;
        let ok_enum = &ok.enum_ident;
        let ok_variant = &ok.ident;
        let err_enum = &err.enum_ident;
        let err_variant = &err.ident;
        let value = self.emit_expr(value, Some(&Type::Str))?;
        let start = self.emit_expr(start, Some(&Type::Int))?;
        let end = self.emit_expr(end, Some(&Type::Int))?;
        Ok(quote! {{
            let __jisp_value = #value;
            let __jisp_start = #start;
            let __jisp_end = #end;
            if __jisp_start < 0i64 || __jisp_end < 0i64 {
                #err_enum::#err_variant(String::from("string slice indices cannot be negative"))
            } else {
                let __jisp_start = __jisp_start as usize;
                let __jisp_end = __jisp_end as usize;
                let __jisp_chars = __jisp_value.chars().collect::<Vec<char>>();
                if __jisp_start > __jisp_end || __jisp_end > __jisp_chars.len() {
                    #err_enum::#err_variant(String::from("string slice is out of bounds"))
                } else {
                    #ok_enum::#ok_variant(__jisp_chars[__jisp_start..__jisp_end].iter().collect::<String>())
                }
            }
        }})
    }

    fn emit_list_len_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let value = self.emit_expr(value, None)?;
        Ok(quote! { #value.len() as i64 })
    }

    fn emit_list_get_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [list, index] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let Some((item_type, err_type)) = result_arguments(expected) else {
            return Err(CodegenError::Unsupported(
                "list.get without expected result type",
            ));
        };
        if err_type != &Type::Str {
            return Err(CodegenError::Unsupported("list.get native error type"));
        }
        let ok = self
            .enum_types
            .prelude_constructor("ok", expected)?
            .ok_or(CodegenError::Unsupported("list.get native result type"))?;
        let err = self
            .enum_types
            .prelude_constructor("err", expected)?
            .ok_or(CodegenError::Unsupported("list.get native result type"))?;
        let ok_enum = &ok.enum_ident;
        let ok_variant = &ok.ident;
        let err_enum = &err.enum_ident;
        let err_variant = &err.ident;
        let list_type = Type::List(Box::new(item_type.clone()));
        let list = self.emit_expr(list, Some(&list_type))?;
        let index = self.emit_expr(index, Some(&Type::Int))?;
        Ok(quote! {{
            let __jisp_list = #list;
            let __jisp_index = #index;
            if __jisp_index < 0i64 {
                #err_enum::#err_variant(String::from("list index cannot be negative"))
            } else {
                match __jisp_list.get(__jisp_index as usize) {
                    Some(__jisp_value) => #ok_enum::#ok_variant(__jisp_value.clone()),
                    None => #err_enum::#err_variant(String::from("list index is out of bounds")),
                }
            }
        }})
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

    fn emit_list_slice_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [list, start, end] = arguments else {
            return Err(CodegenError::Unsupported("non-ternary native intrinsics"));
        };
        let Some((ok_type, err_type)) = result_arguments(expected) else {
            return Err(CodegenError::Unsupported(
                "list.slice without expected result type",
            ));
        };
        let Type::List(item_type) = ok_type else {
            return Err(CodegenError::Unsupported("list.slice native ok type"));
        };
        if err_type != &Type::Str {
            return Err(CodegenError::Unsupported("list.slice native error type"));
        }
        let ok = self
            .enum_types
            .prelude_constructor("ok", expected)?
            .ok_or(CodegenError::Unsupported("list.slice native result type"))?;
        let err = self
            .enum_types
            .prelude_constructor("err", expected)?
            .ok_or(CodegenError::Unsupported("list.slice native result type"))?;
        let ok_enum = &ok.enum_ident;
        let ok_variant = &ok.ident;
        let err_enum = &err.enum_ident;
        let err_variant = &err.ident;
        let list_type = Type::List(Box::new((**item_type).clone()));
        let list = self.emit_expr(list, Some(&list_type))?;
        let start = self.emit_expr(start, Some(&Type::Int))?;
        let end = self.emit_expr(end, Some(&Type::Int))?;
        Ok(quote! {{
            let __jisp_list = #list;
            let __jisp_start = #start;
            let __jisp_end = #end;
            if __jisp_start < 0i64 || __jisp_end < 0i64 {
                #err_enum::#err_variant(String::from("list slice indices cannot be negative"))
            } else {
                let __jisp_start = __jisp_start as usize;
                let __jisp_end = __jisp_end as usize;
                match __jisp_list.get(__jisp_start..__jisp_end) {
                    Some(__jisp_value) => #ok_enum::#ok_variant(__jisp_value.to_vec()),
                    None => #err_enum::#err_variant(String::from("list slice is out of bounds")),
                }
            }
        }})
    }

    fn emit_list_map_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [callback, list] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native list.map"));
        };
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result: _,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [item_type] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported("native list.map callback arity"));
        };
        let callback = self.emit_expr(callback, Some(&callback_type))?;
        let list_type = Type::List(Box::new(item_type.clone()));
        let list = self.emit_expr(list, Some(&list_type))?;
        Ok(quote! {{
            let __jisp_callback = #callback;
            let __jisp_list = #list
                .into_iter()
                .map(|__jisp_value| __jisp_callback(__jisp_value))
                .collect::<Vec<_>>();
            __jisp_list
        }})
    }

    fn emit_list_filter_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [callback, list] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native list.filter"));
        };
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [item_type] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported(
                "native list.filter callback arity",
            ));
        };
        if result.as_ref() != &Type::Bool {
            return Err(CodegenError::Unsupported(
                "native list.filter callback result",
            ));
        }
        let callback = self.emit_expr(callback, Some(&callback_type))?;
        let list_type = Type::List(Box::new(item_type.clone()));
        let list = self.emit_expr(list, Some(&list_type))?;
        Ok(quote! {{
            let mut __jisp_result = Vec::new();
            for __jisp_value in #list {
                if (#callback)(__jisp_value.clone()) {
                    __jisp_result.push(__jisp_value);
                }
            }
            __jisp_result
        }})
    }

    fn emit_list_fold_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [callback, initial, list] = arguments else {
            return Err(CodegenError::Unsupported("non-ternary native list.fold"));
        };
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [accumulator_type, item_type] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported("native list.fold callback arity"));
        };
        if result.as_ref() != accumulator_type {
            return Err(CodegenError::Unsupported(
                "native list.fold callback result",
            ));
        }
        let callback = self.emit_expr(callback, Some(&callback_type))?;
        let initial = self.emit_expr(initial, Some(accumulator_type))?;
        let list_type = Type::List(Box::new(item_type.clone()));
        let list = self.emit_expr(list, Some(&list_type))?;
        Ok(quote! {{
            let __jisp_callback = #callback;
            #list
                .into_iter()
                .fold(#initial, |__jisp_accumulator, __jisp_value| {
                    __jisp_callback(__jisp_accumulator, __jisp_value)
                })
        }})
    }

    fn emit_list_predicate_intrinsic(
        &mut self,
        arguments: &[Expr],
        matches_any: bool,
    ) -> Result<TokenStream, CodegenError> {
        let [callback, list] = arguments else {
            return Err(CodegenError::Unsupported(
                "non-binary native list predicate",
            ));
        };
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [item_type] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported(
                "native list predicate callback arity",
            ));
        };
        if result.as_ref() != &Type::Bool {
            return Err(CodegenError::Unsupported(
                "native list predicate callback result",
            ));
        }
        let callback = self.emit_expr(callback, Some(&callback_type))?;
        let list_type = Type::List(Box::new(item_type.clone()));
        let list = self.emit_expr(list, Some(&list_type))?;
        let match_value = if matches_any {
            quote! { true }
        } else {
            quote! { false }
        };
        let default_value = if matches_any {
            quote! { false }
        } else {
            quote! { true }
        };
        let break_on_match = if matches_any {
            quote! { (#callback)(__jisp_value) }
        } else {
            quote! { !(#callback)(__jisp_value) }
        };
        Ok(quote! {{
            let mut __jisp_result = #default_value;
            for __jisp_value in #list {
                if #break_on_match {
                    __jisp_result = #match_value;
                    break;
                }
            }
            __jisp_result
        }})
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
        let row = self.native_closed_object_row(object)?;
        let len = row.fields.len() as i64;
        Ok(quote! { #len })
    }

    fn emit_obj_has_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [object, key] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native intrinsics"));
        };
        let Some(key) = super::static_string_key(key) else {
            return self.emit_dynamic_obj_has(object, key);
        };
        let row = self.native_closed_object_row(object)?;
        let has = row.fields.contains_key(&key);
        Ok(quote! { #has })
    }

    fn emit_obj_get_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [object, key] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native obj.get"));
        };
        let Some(key) = super::static_string_key(key) else {
            return self.emit_dynamic_obj_get(object, key, expected);
        };
        let row = self.native_closed_object_row(object)?;
        let Some(field_type) = row.fields.get(&key) else {
            return Err(CodegenError::Unsupported("obj.get missing static field"));
        };
        let result_type = result_type(field_type.clone(), Type::Str);
        if let Some(expected) = expected {
            if expected != &result_type {
                return Err(CodegenError::Unsupported(
                    "obj.get native value type mismatch",
                ));
            }
        }
        let ok = self
            .enum_types
            .prelude_constructor("ok", Some(&result_type))?
            .ok_or(CodegenError::Unsupported("obj.get native result type"))?;
        let object = self.emit_expr(object, None)?;
        let enum_ident = &ok.enum_ident;
        let ok_variant = &ok.ident;
        let field = super::rust_ident(&key);
        Ok(quote! {{
            let __jisp_object = #object;
            #enum_ident::#ok_variant(__jisp_object.#field.clone())
        }})
    }

    fn emit_obj_keys_intrinsic(&mut self, arguments: &[Expr]) -> Result<TokenStream, CodegenError> {
        let [object] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native intrinsics"));
        };
        let row = self.native_closed_object_row(object)?;
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
            return self.emit_dynamic_obj_set(object, key, value, expected);
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
            let __jisp_object = (#object).clone();
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
        let row = self.native_closed_object_row(object)?;
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
            .map(|argument| self.native_closed_object_row(argument))
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
}

fn result_type(ok: Type, err: Type) -> Type {
    Type::Named {
        name: "result".to_owned(),
        arguments: vec![ok, err],
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
