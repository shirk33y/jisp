
export type Fn = {(): any; _MACRO?: number; _BIND?: Ast};
export type Scalar = string | boolean | number | null | Fn | Env;
export type Ast = Array<Ast | Scalar>;
export type AstNode = Scalar | Ast;
export interface Env {
  [bind: string]: any;
}
export type Exprs = Array<any>;