use jisp_ir::Expr;
use jisp_types::Type;
use proc_macro2::TokenStream;
use quote::quote;

use super::EmitContext;
use crate::CodegenError;

impl<'a> EmitContext<'a> {
    pub(super) fn emit_bigint_intrinsic(
        &mut self,
        arguments: &[Expr],
    ) -> Result<TokenStream, CodegenError> {
        let [value] = arguments else {
            return Err(CodegenError::Unsupported("non-unary native bigint"));
        };
        let value = self.emit_expr(value, Some(&Type::Str))?;
        Ok(quote! {{
            let __jisp_bigint_text = #value;
            ::num_bigint::BigInt::parse_bytes(__jisp_bigint_text.as_bytes(), 10)
                .expect("jisp invalid bigint literal")
        }})
    }
}
