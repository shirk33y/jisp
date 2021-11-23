// import * as log from "log";
// import * as _ from "lodash";
// import { sprintf } from "std/fmt/printf.ts";
// import { assert, AssertionError } from "std/testing/asserts.ts";

// import { AstNode, Env, Exprs, Fn } from "/ast.ts";
// import { UndefinedSymbolError } from "./utils.ts";
// import { arythmetics, constructors} from "./std.ts";
// import { _loop } from "./eval.ts";
// import *  as std from "./std.ts";

// export function minimal(env: Env) {
 

//   env = Object.assign(Object.create(env), {
//     eval: (a: AstNode) => _loop(a, env),
//     ...arythmetics,
//     ...constructors,
//     std
//   });

//   // Lib specific
//   return env;
// }



// import { assert } from "https://deno.land/std@0.114.0/testing/asserts.ts";
// import { Fn } from "./ast.ts";
// import { Exprs } from "./ast.ts";
// import { AstNode } from "./ast.ts";
// import { log } from "./std.ts";
// import { assertArray } from "./utils.ts";
// import { isString } from "./utils.ts";
// import { isArray } from "./utils.ts";

// class Env {
//   [symbol: string]: any; //index signature

//   constructor(env?: Env) {
//     if (env) {
//       Object.keys(env).forEach((key: string) => {
//         this[key] = env[key];
//       });
//     }
//   }

//   has(symbol: string): boolean {
//     return symbol in this;
//   }

//   get(symbol: string): any {
//     return this[symbol];
//   }

//   set(symbol: string, value: any) {
//     return Object.assign(Object.create(this), { [symbol]: value });
//   }

//   merge(env: Env) {}

//   eval(ast: AstNode) {
//     // list?
//     if (isArray(ast)) {
//       return ast.map((a: AstNode) => this.loop(a));
//     }
//     // symbol?
//     if (isString(ast)) {
//       return this.get(ast);
//     }

//     return ast; // ast unchanged
//   }

//   bind(ast: AstNode, exprs: Exprs) {
//     assertArray(ast);
//     let env = this;

//     ast?.some((a: any, i: number) => {
//       if (a === "&") {
//         return (env = this.set(ast[i + 1] as string, exprs.slice(i)));
//       } else {
//         return (env = this.set(a, exprs[i])), 0;
//       }
//     });

//     return env;
//   }

//   macroexpand(ast: AstNode) {
//     assertArray(ast);
//     const [a0, ...a$] = ast;
//     assert(typeof a0 === "string");

//     while (this.get(a0)?._MACRO) {
//       ast = this.get(a0)(a$);
//     }
//     return ast;
//   }

//   loop(ast: AstNode): AstNode {
//     while (true) {
//       log.debug(`eval: ${ast}`);
//       if (!isArray(ast)) return this.eval(ast);

//       // apply
//       ast = this.macroexpand(ast);
//       if (!isArray(ast)) return this.eval(ast);

//       const [fname, ...fargs] = ast;
//       const [a1, a2, a3] = fargs;

//       // switch (fname) {
//       //   // def my-symbol 42
//       //   case "def": {
//       //     const [name, value] = fargs;
//       //     assert(typeof name === "string");

//       //     return this.set(name, this.loop(value));
//       //   }
//     }
//   }
      
//     def(symbol: string, value: any) {
//       return this.set(symbol, this.loop(value))
//     } 
    
//        // fn [arg1 arg2 ... rest]
//         //   body
//     fn(args: string[], body: any) {

//           const f: Fn = (...a: any[]) => this.bind(args, a).loop(body);
//           f._AST = [body, this, args];

//           return f;
//         }

//         // let [foo 1 bar 2]
//         //   body
//         let(pairs: any[], body: any) {
//           let env = this

//           for (let i = 0; i < pairs.length; i++) {
//             if (i % 2) {
//               const k = pairs[i - 1];
//               assert(typeof k === "string");
//               const v = pairs[i];
//               env = this.set(k, this.loop(v));
//             }
//           }
//           ast = body;
//           break;
//         }

//         // if [= foo 123]
//         //   print "foo is 123"
//         //   do
//         //     print "foo is not 123"
//         //     print "so, i'm mad."
//         //
//         if(condition: boolean, t: any, f?: any) {

//           ast = this.loop(a1) ? a2 : a3;
//           break;
//         }

//         // multiple forms (for side-effects)
//         //
//         // do
//         //   fun1 1 2
//         //   fun2 "foo"
//         //
//         case "do":
//           _eval(ast.slice(1, ast.length - 1), env);
//           ast = ast[ast.length - 1];
//           break;

//         // quote (unevaluated)
//         case "str":
//         case "`":
//           return a1;

//         // mark as macro
//         case "~": {
//           const fn: Fn = _loop(a1, env) as Fn; // eval regular function
//           fn._MACRO = 1; // mark as macro
//           return fn;
//         }
// }