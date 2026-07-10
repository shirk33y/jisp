use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeVar(pub u32);

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Var(TypeVar),
    Never,
    Null,
    Bool,
    Int,
    Float,
    Str,
    List(Box<Type>),
    Object(ObjectRow),
    Function {
        parameters: Vec<Type>,
        rest: Option<Box<Type>>,
        result: Box<Type>,
    },
    Named {
        name: String,
        arguments: Vec<Type>,
    },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct ObjectRow {
    pub fields: BTreeMap<String, Type>,
    pub rest: Option<TypeVar>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scheme {
    pub variables: Vec<TypeVar>,
    pub body: Type,
}

impl Scheme {
    pub fn mono(body: Type) -> Self {
        Self {
            variables: vec![],
            body,
        }
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::Var(var) => write!(f, "t{}", var.0),
            Type::Never => f.write_str("never"),
            Type::Null => f.write_str("null"),
            Type::Bool => f.write_str("bool"),
            Type::Int => f.write_str("int"),
            Type::Float => f.write_str("float"),
            Type::Str => f.write_str("str"),
            Type::List(item) => write!(f, "(list {item})"),
            Type::Object(row) => {
                f.write_str("{")?;
                for (index, (name, ty)) in row.fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{name}: {ty}")?;
                }
                if let Some(rest) = row.rest {
                    write!(f, " | t{}", rest.0)?;
                }
                f.write_str("}")
            }
            Type::Function {
                parameters,
                rest,
                result,
            } => {
                f.write_str("(fn (")?;
                for (index, parameter) in parameters.iter().enumerate() {
                    if index > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{parameter}")?;
                }
                if let Some(rest) = rest {
                    if !parameters.is_empty() {
                        f.write_str(" ")?;
                    }
                    write!(f, "...{rest}")?;
                }
                write!(f, ") {result})")
            }
            Type::Named { name, arguments } if arguments.is_empty() => f.write_str(name),
            Type::Named { name, arguments } => {
                write!(f, "({name}")?;
                for argument in arguments {
                    write!(f, " {argument}")?;
                }
                f.write_str(")")
            }
        }
    }
}
