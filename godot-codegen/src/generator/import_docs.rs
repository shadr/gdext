/*
 * Copyright (c) godot-rust; Bromeon and contributors.
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use std::fmt::Write;

use crate::context::Context;
use crate::models::domain::{ApiView, Class, ClassLike, Function, TyName};
use crate::{special_cases, util};

pub fn import_class_docs(
    description: &str,
    class: &Class,
    ctx: &Context,
    view: &ApiView,
) -> String {
    let mut result = replace_simple_tags(description, view);
    result = replace_type_links(&result, class, ctx, view);
    result = replace_method_links(&result, class, ctx, view);
    result = replace_unimplemented_links(&result, view);

    result
}

fn replace_unimplemented_links(str: &str, view: &ApiView) -> String {
    view.regexes()
        .unimplemented_links
        .replace_all(str, "\\$0")
        .to_string()
}

fn replace_simple_tags(str: &str, view: &ApiView) -> String {
    // Replace \n with \n\n everywhere except codeblock tags.
    let result = view.regexes().newlines.replace_all(str, "$1$3$3");

    // Replace bold tags.
    let result = view.regexes().bold_tags.replace_all(&result, "**$1**");

    // Replace italic tags.
    let result = view.regexes().italic_tags.replace_all(&result, "*$1*");

    // Replace code tags.
    let result = view.regexes().code_tags.replace_all(&result, "`$2`");

    // Replace kbd tags.
    let result = view.regexes().kbd_tags.replace_all(&result, "`$1`");

    // Replace url tags.
    let result = view.regexes().url_tags.replace_all(&result, "[$2]($1)");

    // Replace codeblocks tags.
    let result = view.regexes().codeblocks_tags.replace_all(&result, "$1");

    // Replace codeblock tags.
    let result = view
        .regexes()
        .codeblock_tags
        .replace_all(&result, "```gdscript$1```");

    // Replace codeblock lang tags.
    let result = view
        .regexes()
        .codeblock_lang_tags
        .replace_all(&result, "```$1$2```");

    // Replace gdscript tags.
    let result = view
        .regexes()
        .gdscript_tags
        .replace_all(&result, "```gdscript$1```");

    // Replace csharp tags.
    let result = view
        .regexes()
        .csharp_tags
        .replace_all(&result, "```csharp$1```");

    result.to_string()
}

fn replace_type_links(doc: &str, class: &Class, ctx: &Context, view: &ApiView) -> String {
    let mut result = String::new();
    let mut previous = 0;
    for captures in view.regexes().type_links.captures_iter(doc) {
        let whole_match = captures.get(0).unwrap();
        let start = whole_match.start();
        let end = whole_match.end();
        if doc[end..].starts_with("(http") {
            continue;
        }
        let class_name = captures.get(1).unwrap();
        let class_name = class_name.as_str();
        result.push_str(&doc[previous..start]);

        // If we encounter a deleted or primitive type, or an ignored link, we insert it without any links or formatting.
        if special_cases::is_godot_type_deleted(class_name)
            || matches_primitive_type(class_name)
            || matches_ignored_links(class_name)
        {
            write!(result, "{class_name}").unwrap();
        } else {
            let path = get_class_rust_path(class_name, ctx);
            let current_class_name = class.name().rust_ty.to_string();

            // If a link points to the current class, then do not create a link tag in Markdown to reduce noise.
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

fn matches_primitive_type(class: &str) -> bool {
    matches!(class, "int" | "float" | "bool")
}

fn matches_ignored_links(class: &str) -> bool {
    // We don't have a single place to point @GDScript to.
    class == "@GDScript"
}

fn replace_method_links(doc: &str, class: &Class, ctx: &Context, view: &ApiView) -> String {
    let mut result = String::new();
    let mut previous = 0;

    for captures in view.regexes().method_links.captures_iter(doc) {
        let whole_match = captures.get(0).unwrap();
        let start = whole_match.start();
        let end = whole_match.end();
        if doc[end..].starts_with("(http") {
            continue;
        }
        result.push_str(&doc[previous..start]);
        let method_path = captures.get(1).unwrap().as_str();

        if let Some(method_path) = convert_to_method_path(method_path, class, ctx, view) {
            let (_, method_name) = method_path
                .rsplit_once("::")
                .expect("rsplit_once should return a method name");
            write!(result, "[{method_name}][`{method_path}`]").unwrap();
        } else {
            write!(result, "\\{}", whole_match.as_str()).unwrap();
        }

        previous = end;
    }
    result.push_str(&doc[previous..]);

    result
}

fn convert_to_method_path(
    class_method: &str,
    class: &Class,
    ctx: &Context,
    view: &ApiView,
) -> Option<String> {
    let (godot_class, godot_method) =
        if let Some((class_name, method_name)) = class_method.split_once('.') {
            (class_name, method_name)
        } else {
            (class.name().godot_ty.as_str(), class_method)
        };

    let godot_method = util::safe_ident(godot_method).to_string();

    let mut ret = None;
    if matches_hardcoded_method(godot_class, &godot_method, &mut ret) {
        return ret;
    }

    if let Some(class) = view.try_get_engine_class(&TyName::from_godot(godot_class))
        && let Some(method) = class
            .methods
            .iter()
            .find(|method| method.godot_name() == godot_method)
    {
        let godot_method_name = godot_method.trim_start_matches("_");
        if method.is_private() {
            return None;
        }
        if method.is_virtual() {
            if class.is_final {
                // Final classes don't have an associated trait with virtual methods.
                return None;
            } else {
                return Some(format!(
                    "crate::classes::{}::{}",
                    class.name().virtual_trait_name(),
                    godot_method_name
                ));
            }
        }
    }

    let godot_method_name = godot_method.trim_start_matches("_");
    let rust_class_path = get_class_rust_path(godot_class, ctx);
    Some(format!("{}::{}", rust_class_path, godot_method_name))
}

fn matches_hardcoded_method(
    godot_class: &str,
    godot_method: &str,
    ret: &mut Option<String>,
) -> bool {
    match (godot_class, godot_method) {
        ("Object", "free") => {
            *ret = Some("crate::obj::Gd::free".to_string());
            true
        }
        ("Object", "get_instance_id") => {
            *ret = Some("crate::obj::Gd::instance_id".to_string());
            true
        }
        ("Object", "notification") => {
            *ret = Some("crate::classes::Object::notify".to_string());
            true
        }
        ("Object", "_notification") => {
            *ret = Some("crate::classes::IObject::on_notification".to_string());
            true
        }
        ("GDScript", "new") => {
            *ret = Some("crate::obj::NewGd::new_gd".to_string());
            true
        }
        ("@GlobalScope", "instance_from_id") => {
            *ret = Some("crate::obj::Gd::from_instance_id".to_string());
            true
        }
        ("@GlobalScope", "is_instance_valid") => {
            *ret = Some("crate::obj::Gd::is_instance_valid".to_string());
            true
        }
        ("@GDScript", "load") => {
            *ret = Some("crate::tools::load".to_string());
            true
        }
        ("@GDScript", "save") => {
            *ret = Some("crate::tools::save".to_string());
            true
        }
        // ("String", _) => {
        //     *ret = Some(format!("crate::builtin::GString::{}", godot_method));
        //     true
        // }
        ("@GlobalScope", _) => {
            *ret = Some(format!("crate::global::{}", godot_method));
            true
        }
        ("@GDScript", _) => {
            *ret = None;
            true
        }
        _ => false,
    }
}

fn convert_builtin_types(type_name: &str) -> Option<String> {
    match type_name {
        "String" => Some("crate::builtin::GString".to_string()),
        "Array" => Some("crate::builtin::Array".to_string()),
        "Dictionary" => Some("crate::builtin::Dictionary".to_string()),
        _ => None,
    }
}

fn get_class_rust_path(godot_class_name: &str, ctx: &Context) -> String {
    if let Some(hardcoded_builtin_type) = convert_builtin_types(godot_class_name) {
        return hardcoded_builtin_type;
    }

    let is_builtin = ctx.is_builtin(godot_class_name);
    let rust_class_name = crate::conv::to_pascal_case(godot_class_name);
    if is_builtin {
        format!("crate::builtin::{}", rust_class_name)
    } else {
        format!("crate::classes::{}", rust_class_name)
    }
}
