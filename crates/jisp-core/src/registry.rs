#[derive(Clone, Copy, Debug)]
pub struct SpecialFormSpec {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub min_args: usize,
    pub max_args: Option<usize>,
    pub top_level: bool,
    pub summary: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct UiElementSpec {
    pub name: &'static str,
    pub summary: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub struct UiDirectiveSpec {
    pub name: &'static str,
    pub summary: &'static str,
}

pub const UI_ELEMENTS: &[UiElementSpec] = &[
    UiElementSpec {
        name: "a",
        summary: "HTML anchor element.",
    },
    UiElementSpec {
        name: "article",
        summary: "HTML article element.",
    },
    UiElementSpec {
        name: "aside",
        summary: "HTML aside element.",
    },
    UiElementSpec {
        name: "button",
        summary: "HTML button element.",
    },
    UiElementSpec {
        name: "div",
        summary: "HTML generic container.",
    },
    UiElementSpec {
        name: "footer",
        summary: "HTML footer element.",
    },
    UiElementSpec {
        name: "form",
        summary: "HTML form element.",
    },
    UiElementSpec {
        name: "h1",
        summary: "HTML heading level 1.",
    },
    UiElementSpec {
        name: "h2",
        summary: "HTML heading level 2.",
    },
    UiElementSpec {
        name: "h3",
        summary: "HTML heading level 3.",
    },
    UiElementSpec {
        name: "header",
        summary: "HTML header element.",
    },
    UiElementSpec {
        name: "img",
        summary: "HTML image element.",
    },
    UiElementSpec {
        name: "input",
        summary: "HTML input element.",
    },
    UiElementSpec {
        name: "label",
        summary: "HTML label element.",
    },
    UiElementSpec {
        name: "li",
        summary: "HTML list item.",
    },
    UiElementSpec {
        name: "main",
        summary: "HTML main element.",
    },
    UiElementSpec {
        name: "nav",
        summary: "HTML navigation element.",
    },
    UiElementSpec {
        name: "ol",
        summary: "HTML ordered list.",
    },
    UiElementSpec {
        name: "option",
        summary: "HTML option element.",
    },
    UiElementSpec {
        name: "p",
        summary: "HTML paragraph element.",
    },
    UiElementSpec {
        name: "section",
        summary: "HTML section element.",
    },
    UiElementSpec {
        name: "select",
        summary: "HTML select element.",
    },
    UiElementSpec {
        name: "span",
        summary: "HTML inline container.",
    },
    UiElementSpec {
        name: "strong",
        summary: "HTML strong emphasis element.",
    },
    UiElementSpec {
        name: "textarea",
        summary: "HTML textarea element.",
    },
    UiElementSpec {
        name: "ul",
        summary: "HTML unordered list.",
    },
];

pub const UI_DIRECTIVES: &[UiDirectiveSpec] = &[
    UiDirectiveSpec {
        name: "attr",
        summary: "Set an explicit HTML attribute on a UI element.",
    },
    UiDirectiveSpec {
        name: "prop",
        summary: "Set a renderer property on a UI element.",
    },
    UiDirectiveSpec {
        name: "class",
        summary: "Enable one or more utility classes on a UI element.",
    },
    UiDirectiveSpec {
        name: "class-if",
        summary: "Conditionally enable a utility class on a UI element.",
    },
    UiDirectiveSpec {
        name: "on",
        summary: "Attach an event handler for an interactive UI host.",
    },
    UiDirectiveSpec {
        name: "key",
        summary: "Attach a reconciliation identity to a UI element.",
    },
    UiDirectiveSpec {
        name: "for",
        summary: "Repeat a UI child for each value in a collection.",
    },
];

pub const SPECIAL_FORMS: &[SpecialFormSpec] = &[
    SpecialFormSpec {
        name: "def",
        aliases: &[],
        min_args: 2,
        max_args: Some(2),
        top_level: true,
        summary: "Define a private module binding.",
    },
    SpecialFormSpec {
        name: "export",
        aliases: &[],
        min_args: 1,
        max_args: Some(2),
        top_level: true,
        summary: "Export an existing binding or define and export a binding.",
    },
    SpecialFormSpec {
        name: "import",
        aliases: &[],
        min_args: 1,
        max_args: Some(2),
        top_level: true,
        summary: "Import a module, optionally under an alias.",
    },
    SpecialFormSpec {
        name: "macro-import",
        aliases: &[],
        min_args: 1,
        max_args: Some(2),
        top_level: true,
        summary: "Reserved explicit compile-time macro import form.",
    },
    SpecialFormSpec {
        name: "type",
        aliases: &[],
        min_args: 2,
        max_args: None,
        top_level: true,
        summary: "Define an algebraic data type.",
    },
    SpecialFormSpec {
        name: "component",
        aliases: &[],
        min_args: 3,
        max_args: Some(3),
        top_level: true,
        summary: "Define a UI component over renderer-neutral structural nodes.",
    },
    SpecialFormSpec {
        name: "fn",
        aliases: &[],
        min_args: 2,
        max_args: None,
        top_level: false,
        summary: "Create a closure.",
    },
    SpecialFormSpec {
        name: "let",
        aliases: &[],
        min_args: 2,
        max_args: None,
        top_level: false,
        summary: "Create sequential lexical bindings.",
    },
    SpecialFormSpec {
        name: "do",
        aliases: &[],
        min_args: 1,
        max_args: None,
        top_level: false,
        summary: "Evaluate expressions from left to right.",
    },
    SpecialFormSpec {
        name: "if",
        aliases: &[],
        min_args: 2,
        max_args: Some(3),
        top_level: false,
        summary: "Conditional expression.",
    },
    SpecialFormSpec {
        name: "case",
        aliases: &[],
        min_args: 2,
        max_args: None,
        top_level: false,
        summary: "Exhaustive pattern matching.",
    },
    SpecialFormSpec {
        name: "use",
        aliases: &[],
        min_args: 3,
        max_args: None,
        top_level: false,
        summary: "Pass the remaining body as the final callback argument.",
    },
    SpecialFormSpec {
        name: "text",
        aliases: &[],
        min_args: 1,
        max_args: Some(1),
        top_level: false,
        summary: "Create a structural UI text node inside a component.",
    },
    SpecialFormSpec {
        name: "quote",
        aliases: &[],
        min_args: 1,
        max_args: Some(1),
        top_level: false,
        summary: "Return syntax as data.",
    },
    SpecialFormSpec {
        name: "quasiquote",
        aliases: &["`"],
        min_args: 1,
        max_args: Some(1),
        top_level: false,
        summary: "Quote syntax while permitting unquote forms.",
    },
    SpecialFormSpec {
        name: "unquote",
        aliases: &[","],
        min_args: 1,
        max_args: Some(1),
        top_level: false,
        summary: "Evaluate inside quasiquote or string templates.",
    },
    SpecialFormSpec {
        name: "unquote-splicing",
        aliases: &[",@"],
        min_args: 1,
        max_args: Some(1),
        top_level: false,
        summary: "Splice a list inside quasiquote or string templates.",
    },
    SpecialFormSpec {
        name: "macro",
        aliases: &["~"],
        min_args: 1,
        max_args: Some(1),
        top_level: false,
        summary: "Mark a compile-time transformer.",
    },
];

pub fn special_form(name: &str) -> Option<&'static SpecialFormSpec> {
    SPECIAL_FORMS
        .iter()
        .find(|form| form.name == name || form.aliases.contains(&name))
}

pub fn ui_element(name: &str) -> Option<&'static UiElementSpec> {
    UI_ELEMENTS.iter().find(|element| element.name == name)
}

pub fn ui_directive(name: &str) -> Option<&'static UiDirectiveSpec> {
    UI_DIRECTIVES
        .iter()
        .find(|directive| directive.name == name)
}
