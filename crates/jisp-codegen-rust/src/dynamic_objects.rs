use jisp_ir::Expr;
use jisp_types::{ObjectRow, Type};
use proc_macro2::TokenStream;
use quote::quote;

use super::{result_arguments, EmitContext};
use crate::CodegenError;

impl<'a> EmitContext<'a> {
    pub(super) fn emit_dynamic_field(
        &mut self,
        object: &Expr,
        key: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let row = self.native_closed_object_row(object)?;
        let field_type = homogeneous_field_type(&row)?;
        if expected != Some(field_type) {
            return Err(CodegenError::Unsupported(
                "dynamic native field type mismatch",
            ));
        }
        let object = self.emit_expr(object, None)?;
        let key = self.emit_expr(key, Some(&Type::Str))?;
        let dispatch = emit_field_dispatch(
            &row,
            quote! { __jisp_object },
            None,
            quote! { panic!("jisp object has no key `{}`", __jisp_key) },
        );
        Ok(quote! {{
            let __jisp_object = #object;
            let __jisp_key = #key;
            #dispatch
        }})
    }

    pub(super) fn emit_dynamic_obj_get(
        &mut self,
        object: &Expr,
        key: &Expr,
        expected: Option<&Type>,
    ) -> Result<TokenStream, CodegenError> {
        let Some((ok_type, err_type)) = result_arguments(expected) else {
            return Err(CodegenError::Unsupported(
                "dynamic native obj.get without expected result type",
            ));
        };
        if err_type != &Type::Str {
            return Err(CodegenError::Unsupported(
                "dynamic native obj.get error type",
            ));
        }
        let row = self.native_closed_object_row(object)?;
        if homogeneous_field_type(&row)? != ok_type {
            return Err(CodegenError::Unsupported(
                "dynamic native obj.get value type mismatch",
            ));
        }
        let ok = self.enum_types.prelude_constructor("ok", expected)?.ok_or(
            CodegenError::Unsupported("dynamic native obj.get result type"),
        )?;
        let err = self
            .enum_types
            .prelude_constructor("err", expected)?
            .ok_or(CodegenError::Unsupported(
                "dynamic native obj.get result type",
            ))?;
        let object = self.emit_expr(object, None)?;
        let key = self.emit_expr(key, Some(&Type::Str))?;
        let ok_enum = &ok.enum_ident;
        let ok_variant = &ok.ident;
        let err_enum = &err.enum_ident;
        let err_variant = &err.ident;
        let dispatch = emit_field_dispatch(
            &row,
            quote! { __jisp_object },
            Some(quote! { #ok_enum::#ok_variant }),
            quote! { #err_enum::#err_variant(format!("object has no key `{}`", __jisp_key)) },
        );
        Ok(quote! {{
            let __jisp_object = #object;
            let __jisp_key = #key;
            #dispatch
        }})
    }

    pub(super) fn emit_dynamic_obj_has(
        &mut self,
        object: &Expr,
        key: &Expr,
    ) -> Result<TokenStream, CodegenError> {
        let row = self.native_closed_object_row(object)?;
        let object = self.emit_expr(object, None)?;
        let key = self.emit_expr(key, Some(&Type::Str))?;
        let keys = row.fields.keys();
        Ok(quote! {{
            let _ = #object;
            let __jisp_key = #key;
            false #(|| __jisp_key == #keys)*
        }})
    }
}

fn homogeneous_field_type(row: &ObjectRow) -> Result<&Type, CodegenError> {
    let Some(field_type) = row.fields.values().next() else {
        return Err(CodegenError::Unsupported(
            "dynamic native access on empty object",
        ));
    };
    if row.fields.values().all(|candidate| candidate == field_type) {
        Ok(field_type)
    } else {
        Err(CodegenError::Unsupported(
            "dynamic native access on heterogeneous object",
        ))
    }
}

fn emit_field_dispatch(
    row: &ObjectRow,
    object: TokenStream,
    success: Option<TokenStream>,
    missing: TokenStream,
) -> TokenStream {
    row.fields.keys().rev().fold(missing, |otherwise, key| {
        let field = super::rust_ident(key);
        let value = match &success {
            Some(success) => quote! { #success(#object.#field) },
            None => quote! { #object.#field },
        };
        quote! {
            if __jisp_key == #key {
                #value
            } else {
                #otherwise
            }
        }
    })
}
