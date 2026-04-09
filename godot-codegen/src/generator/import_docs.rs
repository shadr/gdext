use std::fmt::Write;

use crate::{
    context::Context,
    models::domain::{ApiView, Class, ClassLike, Function, TyName},
};

pub fn import_class_docs(class: &Class, ctx: &Context, view: &ApiView) -> String {
    let doc = &class.description;

    let mut result = replace_simple_tags(doc);
    result = replace_type_links(&result, class, ctx);
    result = replace_method_links(&result, class, ctx, view);

    result
}

fn replace_simple_tags(str: &str) -> String {
    // replace \n with \n\n everywhere except codeblock tags
    let re = regex::RegexBuilder::new(
        r#"(\[codeblocks?( lang=.*?)?\](?:.|\n)*?\[\/codeblocks?\])|(\n)"#,
    )
    .build()
    .unwrap();
    let result = re.replace_all(str, "$1$3$3");

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

fn replace_type_links(doc: &str, class: &Class, ctx: &Context) -> String {
    let mut result = String::new();
    let re = regex::RegexBuilder::new(r#"\[([a-zA-Z0-9]+?)\]"#)
        .build()
        .unwrap();
    let mut previous = 0;
    const IGNORED_NAMES: &[&str] = &["Thread", "Mutex", "int"];
    for captures in re.captures_iter(doc) {
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
            write!(result, "{class_name}").unwrap();
        } else {
            let path = get_class_rust_path(class_name, ctx);
            let current_class_name = class.name().rust_ty.to_string();
            // if a link points to the current class then do not create a link tag in markdown to reduce noise
            if current_class_name == class_name {
                write!(result, "`{class_name}`").unwrap();
            } else {
                write!(result, "[{class_name}][{path}]").unwrap();
            }
        }
        previous = end;
    }
    result.push_str(&doc[previous..]);
    result
}

fn replace_method_links(doc: &str, class: &Class, ctx: &Context, view: &ApiView) -> String {
    let mut result = String::new();
    let re = regex::RegexBuilder::new(r#"\[method ((([a-zA-Z0-9@]+?)\.)?([a-zA-Z0-9_]+?))\]"#)
        .build()
        .unwrap();
    let mut previous = 0;

    for captures in re.captures_iter(doc) {
        let whole_match = captures.get(0).unwrap();
        let start = whole_match.start();
        let end = whole_match.end();
        if doc[end..].starts_with("(http") {
            continue;
        }
        result.push_str(&doc[previous..start]);
        let method_path = captures.get(1).unwrap().as_str();

        if let Some((method_path, method_name)) =
            convert_to_method_path(method_path, class, ctx, view)
        {
            write!(result, "[{method_name}][`{method_path}`]").unwrap();
        } else {
            write!(result, "{}", whole_match.as_str()).unwrap();
        }

        previous = end;
    }
    result.push_str(&doc[previous..]);

    result
}

fn convert_to_method_path<'a>(
    method: &'a str,
    class: &Class,
    ctx: &Context,
    view: &ApiView,
) -> Option<(String, &'a str)> {
    let godot_class_name;
    let mut godot_method_name;
    if method.contains(".") {
        let mut splitted = method.split('.');
        godot_class_name = splitted.next().unwrap();
        godot_method_name = splitted.next().unwrap();
    } else {
        godot_class_name = class.name().godot_ty.as_str();
        godot_method_name = method;
    }

    if godot_method_name == "typeof" {
        godot_method_name = "typeof_";
    }

    match (godot_class_name, godot_method_name) {
        ("Object", "free") => {
            return Some(("crate::obj::Gd::free".to_string(), "free"));
        }
        ("Object", "get_instance_id") => {
            return Some(("crate::obj::Gd::instance_id".to_string(), "instance_id"));
        }
        ("@GlobalScope", "instance_from_id") => {
            return Some((
                "crate::obj::Gd::from_instance_id".to_string(),
                "from_instance_id",
            ));
        }
        ("@GlobalScope", "is_instance_valid") => {
            return Some((
                "crate::obj::Gd::is_instance_valid".to_string(),
                "is_instance_valid",
            ));
        }
        ("@GDScript", "load") => {
            return Some(("crate::tools::load".to_string(), "load"));
        }
        ("@GDScript", "save") => {
            return Some(("crate::tools::save".to_string(), "save"));
        }
        ("String", _) => {
            return Some((
                format!("crate::builtin::GString::{}", godot_method_name),
                godot_method_name,
            ));
        }
        ("@GlobalScope", _) => {
            return Some((
                format!("crate::global::{}", godot_method_name),
                godot_method_name,
            ));
        }
        ("@GDScript", _) => {
            return None;
        }
        _ => (),
    }

    if let Some(class) = view.try_get_engine_class(&TyName::from_godot(godot_class_name))
        && let Some(method) = class
            .methods
            .iter()
            .find(|method| method.godot_name() == godot_method_name)
    {
        let godot_method_name = godot_method_name.trim_start_matches("_");
        if method.is_private() {
            return None;
        }
        if method.is_virtual() {
            if class.is_final {
                // Final classes doesn't have associated trait with virtual methods
                return None;
            } else {
                return Some((
                    format!(
                        "crate::classes::{}::{}",
                        class.name().virtual_trait_name(),
                        godot_method_name
                    ),
                    godot_method_name,
                ));
            }
        }
    }

    let godot_method_name = godot_method_name.trim_start_matches("_");
    let rust_class_path = get_class_rust_path(godot_class_name, ctx);
    Some((
        format!("{}::{}", rust_class_path, godot_method_name),
        godot_method_name,
    ))
}

fn get_class_rust_path(godot_class_name: &str, ctx: &Context) -> String {
    if godot_class_name == "String" {
        return "crate::builtin::GString".to_string();
    }

    let is_builtin = ctx.is_builtin(godot_class_name);
    let rust_class_name = crate::conv::to_pascal_case(godot_class_name);
    if is_builtin {
        format!("crate::builtin::{}", rust_class_name)
    } else {
        format!("crate::classes::{}", rust_class_name)
    }
}
