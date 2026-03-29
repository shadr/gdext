use crate::{
    context::Context,
    models::domain::{ApiView, Class, ClassLike, Function, TyName},
};

pub fn import_class_docs(class: &Class, ctx: &Context, view: &ApiView) -> String {
    let doc = &class.description;

    let result = replace_simple_tags(doc);
    let result = replace_type_links(&result, ctx);
    let result = replace_method_links(&result, class, ctx, view);

    result
}

fn replace_simple_tags(str: &str) -> String {
    // replace \n with \n\n everywhere except codeblock tags
    let re = regex::RegexBuilder::new(
        r#"(\[codeblocks?( lang=.*?)?\](?:.|\n)*?\[\/codeblocks?\])|(\n)"#,
    )
    .build()
    .unwrap();
    let result = re.replace_all(&str, "$1$3$3");

    // replace bold tags
    let re = regex::RegexBuilder::new(r#"\[b\](.*?)\[\/b\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "**$1**");

    // replace italic tags
    let re = regex::RegexBuilder::new(r#"\[i\](.*?)\[\/i\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "*$1*");

    // replace code tags
    let re = regex::RegexBuilder::new(r#"\[code( skip-lint)?\](.*?)\[\/code\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "`$2`");

    // replace kbd tags
    let re = regex::RegexBuilder::new(r#"\[kbd\](.*?)\[\/kbd\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "`$1`");

    // replace url tags
    let re = regex::RegexBuilder::new(r#"\[url=(.*?)\](.*?)\[\/url\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "[$2]($1)");

    // replace codeblocks tags
    let re = regex::RegexBuilder::new(r#"\[codeblocks\]([\s\S]*?)\[\/codeblocks\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "$1");

    // replace codeblock tags
    let re = regex::RegexBuilder::new(r#"\[codeblock\]([\s\S]*?)\[\/codeblock\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "```gdscript$1```");

    // replace codeblock lang tags
    let re = regex::RegexBuilder::new(r#"\[codeblock lang=(.*?)\]([\s\S]*?)\[\/codeblock\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "```$1$2```");

    // replace gdscript tags
    let re = regex::RegexBuilder::new(r#"\[gdscript\]([\s\S]*?)\[\/gdscript\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "```gdscript$1```");

    // replace csharp tags
    let re = regex::RegexBuilder::new(r#"\[csharp\]([\s\S]*?)\[\/csharp\]"#)
        .build()
        .unwrap();
    let result = re.replace_all(&result, "```csharp$1```");

    result.to_string()
}

fn replace_type_links(doc: &str, ctx: &Context) -> String {
    let mut result = String::new();
    let re = regex::RegexBuilder::new(r#"\[([a-zA-Z0-9]+?)\]"#)
        .build()
        .unwrap();
    let mut previous = 0;
    const IGNORED_NAMES: &[&str] = &["Thread", "Mutex", "int"];
    for captures in re.captures_iter(&doc) {
        let whole_match = captures.get(0).unwrap();
        let start = whole_match.start();
        let end = whole_match.end();
        if doc[end..].starts_with("(http") {
            continue;
        }
        let class_name = captures.get(1).unwrap();
        let class_name = class_name.as_str();
        result.push_str(&doc[previous..start]);
        // if we encounter an ignored name then we insert it without any links or formatting
        if IGNORED_NAMES.contains(&class_name) {
            result.push_str(&class_name);
        } else {
            let is_builtin = ctx.is_builtin(&class_name);
            let class_name = crate::conv::to_pascal_case(class_name);
            result.push_str("[");
            result.push_str(&class_name);
            if is_builtin {
                result.push_str("][crate::builtin::");
            } else {
                result.push_str("][crate::classes::");
            }
            result.push_str(&class_name);
            result.push_str("]");
        }
        previous = end;
    }
    result.push_str(&doc[previous..]);
    result
}

fn replace_method_links(doc: &str, class: &Class, ctx: &Context, view: &ApiView) -> String {
    // replace method links
    let mut result = String::new();
    let re = regex::RegexBuilder::new(r#"\[method (([a-zA-Z0-9@]+?)\.)?([a-zA-Z0-9_]+?)\]"#)
        .build()
        .unwrap();
    let mut previous = 0;
    for captures in re.captures_iter(&doc) {
        let whole_match = captures.get(0).unwrap();
        let start = whole_match.start();
        let end = whole_match.end();
        if doc[end..].starts_with("(http") {
            continue;
        }
        result.push_str(&doc[previous..start]);

        let mut class_containing_method = class;

        let mut is_builtin = false;
        let mut is_global = false;
        if let Some(capture) = captures.get(2) {
            let name = capture.as_str();
            is_builtin = ctx.is_builtin(&name);
            if name == "@GDScript" {
                // TODO: link to a gdscript builtin function (load, preload, char, ord etc.)
                result.push_str(whole_match.as_str());
                previous = end;
                continue;
            }
            is_global = name == "@GlobalScope";
            if !is_global {
                if let Some(engine_class) = view.try_get_engine_class(&TyName::from_godot(name)) {
                    class_containing_method = engine_class;
                }
            }
        }

        let godot_method_name = captures.get(3).unwrap();
        let godot_method_name = godot_method_name.as_str();

        let is_virtual = if let Some(method) = class_containing_method
            .methods
            .iter()
            .find(|method| method.godot_name() == godot_method_name)
        {
            method.is_virtual()
        } else {
            false
        };

        let mut rust_method_name = godot_method_name.trim_start_matches("_");

        if rust_method_name == "typeof" {
            rust_method_name = "typeof_";
        }

        result.push_str("[");
        result.push_str(&rust_method_name);
        result.push_str("][`");

        let class_containing_method_name = class_containing_method.name().rust_ty.to_string();

        // TODO: temporary fix for special methods, find a better way of hadling these
        if rust_method_name == "is_instance_id_valid" {
            result.push_str("crate::obj::InstanceId::lookup_validity`]");
        } else if let Some(mapped_gd_method) =
            map_gd_method_name(&class_containing_method_name, &rust_method_name)
        {
            result.push_str("crate::obj::Gd::");
            result.push_str(mapped_gd_method);
            result.push_str("`]");
        } else {
            if is_global {
                result.push_str("crate::global");
            } else {
                if is_builtin {
                    result.push_str("crate::builtin::");
                } else {
                    result.push_str("crate::classes::");
                }

                let name = if is_virtual {
                    class_containing_method.name().virtual_trait_name()
                } else {
                    class_containing_method_name
                };
                result.push_str(&name);
            }
            result.push_str("::");
            result.push_str(&rust_method_name);
            result.push_str("`]");
        }

        previous = end;
    }
    result.push_str(&doc[previous..]);

    result
}

fn map_gd_method_name<'a, 'b>(class_name: &'a str, method_name: &'a str) -> Option<&'b str> {
    match (class_name, method_name) {
        ("Object", "free") => Some("free"),
        ("Object", "get_instance_id") => Some("instance_id"),
        (_, "instance_from_id") => Some("from_instance_id"),
        (_, "is_instance_valid") => Some("is_instance_valid"),
        _ => None,
    }
}
