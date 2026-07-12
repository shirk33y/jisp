use jisp_ir::Expr;
use jisp_types::Type;
use proc_macro2::TokenStream;
use quote::quote;

use super::{result_arguments, EmitContext};
use crate::CodegenError;

impl<'a> EmitContext<'a> {
    pub(super) fn emit_result_map_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [value, callback] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native result.map"));
        };
        let input = self.native_result_type(value)?.clone();
        let output = native_result_type(expected)?.clone();
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [parameter] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported(
                "native result.map callback arity",
            ));
        };
        let (input_ok, input_err) =
            result_arguments(Some(&input)).expect("native_result_type returns a result type");
        let (output_ok, output_err) =
            result_arguments(Some(&output)).expect("native_result_type returns a result type");
        if parameter != input_ok || result.as_ref() != output_ok || input_err != output_err {
            return Err(CodegenError::Unsupported("native result.map callback type"));
        }
        self.emit_result_transform(value, callback, &input, &output, true, false)
    }

    pub(super) fn emit_result_map_err_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [value, callback] = arguments else {
            return Err(CodegenError::Unsupported(
                "non-binary native result.map-err",
            ));
        };
        let input = self.native_result_type(value)?.clone();
        let output = native_result_type(expected)?.clone();
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [parameter] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported(
                "native result.map-err callback arity",
            ));
        };
        let (input_ok, input_err) =
            result_arguments(Some(&input)).expect("native_result_type returns a result type");
        let (output_ok, output_err) =
            result_arguments(Some(&output)).expect("native_result_type returns a result type");
        if parameter != input_err || result.as_ref() != output_err || input_ok != output_ok {
            return Err(CodegenError::Unsupported(
                "native result.map-err callback type",
            ));
        }
        self.emit_result_transform(value, callback, &input, &output, false, false)
    }

    pub(super) fn emit_result_try_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [value, callback] = arguments else {
            return Err(CodegenError::Unsupported("non-binary native result.try"));
        };
        let input = self.native_result_type(value)?.clone();
        let output = native_result_type(expected)?.clone();
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [parameter] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported(
                "native result.try callback arity",
            ));
        };
        let (input_ok, input_err) =
            result_arguments(Some(&input)).expect("native_result_type returns a result type");
        let (_, output_err) =
            result_arguments(Some(&output)).expect("native_result_type returns a result type");
        if parameter != input_ok || result.as_ref() != &output || input_err != output_err {
            return Err(CodegenError::Unsupported("native result.try callback type"));
        }
        self.emit_result_transform(value, callback, &input, &output, true, true)
    }

    pub(super) fn emit_result_recover_intrinsic(
        &mut self,
        arguments: &[Expr],
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let [value, callback] = arguments else {
            return Err(CodegenError::Unsupported(
                "non-binary native result.recover",
            ));
        };
        let input = self.native_result_type(value)?.clone();
        let output = native_result_type(expected)?.clone();
        let callback_type = self.native_callback_type(callback)?;
        let Type::Function {
            parameters,
            rest: None,
            result,
        } = &callback_type
        else {
            unreachable!("native_callback_type only returns fixed-arity functions");
        };
        let [parameter] = parameters.as_slice() else {
            return Err(CodegenError::Unsupported(
                "native result.recover callback arity",
            ));
        };
        let (input_ok, input_err) =
            result_arguments(Some(&input)).expect("native_result_type returns a result type");
        let (output_ok, _) =
            result_arguments(Some(&output)).expect("native_result_type returns a result type");
        if parameter != input_err || result.as_ref() != &output || input_ok != output_ok {
            return Err(CodegenError::Unsupported(
                "native result.recover callback type",
            ));
        }
        self.emit_result_transform(value, callback, &input, &output, false, true)
    }

    fn emit_result_transform(
        &mut self,
        value: &Expr,
        callback: &Expr,
        input: &Type,
        output: &Type,
        transform_ok: bool,
        callback_returns_result: bool,
    ) -> Result<TokenStream, CodegenError> {
        let input_ok = self
            .enum_types
            .prelude_constructor("ok", Some(input))?
            .ok_or(CodegenError::Unsupported("native result input type"))?;
        let input_err = self
            .enum_types
            .prelude_constructor("err", Some(input))?
            .ok_or(CodegenError::Unsupported("native result input type"))?;
        let output_ok = self
            .enum_types
            .prelude_constructor("ok", Some(output))?
            .ok_or(CodegenError::Unsupported("native result output type"))?;
        let output_err = self
            .enum_types
            .prelude_constructor("err", Some(output))?
            .ok_or(CodegenError::Unsupported("native result output type"))?;
        let value = self.emit_expr(value, Some(input))?;
        let callback_type = self.native_callback_type(callback)?;
        let callback = self.emit_expr(callback, Some(&callback_type))?;
        let input_ok_enum = &input_ok.enum_ident;
        let input_ok_variant = &input_ok.ident;
        let input_err_enum = &input_err.enum_ident;
        let input_err_variant = &input_err.ident;
        let output_ok_enum = &output_ok.enum_ident;
        let output_ok_variant = &output_ok.ident;
        let output_err_enum = &output_err.enum_ident;
        let output_err_variant = &output_err.ident;
        let ok_arm = if transform_ok && callback_returns_result {
            quote! { #input_ok_enum::#input_ok_variant(value) => #callback(value) }
        } else if transform_ok {
            quote! { #input_ok_enum::#input_ok_variant(value) => #output_ok_enum::#output_ok_variant(#callback(value)) }
        } else {
            quote! { #input_ok_enum::#input_ok_variant(value) => #output_ok_enum::#output_ok_variant(value) }
        };
        let err_arm = if !transform_ok && callback_returns_result {
            quote! { #input_err_enum::#input_err_variant(error) => #callback(error) }
        } else if !transform_ok {
            quote! { #input_err_enum::#input_err_variant(error) => #output_err_enum::#output_err_variant(#callback(error)) }
        } else {
            quote! { #input_err_enum::#input_err_variant(error) => #output_err_enum::#output_err_variant(error) }
        };
        Ok(quote! {{
            match #value {
                #ok_arm,
                #err_arm,
            }
        }})
    }

    fn native_result_type(&self, expr: &Expr) -> Result<&Type, CodegenError> {
        self.expression_type(expr)
            .filter(|ty| result_arguments(Some(ty)).is_some())
            .ok_or(CodegenError::Unsupported("native result value type"))
    }
}

fn native_result_type(expected: Option<&Type>) -> Result<&Type, CodegenError> {
    expected
        .filter(|ty| result_arguments(Some(ty)).is_some())
        .ok_or(CodegenError::Unsupported("native result value type"))
}
