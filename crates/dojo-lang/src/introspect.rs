use std::collections::HashMap;

use cairo_lang_defs::patcher::RewriteNode;
use cairo_lang_defs::plugin::PluginDiagnostic;
use cairo_lang_syntax::node::ast::{Expr, ItemEnum, ItemStruct, OptionTypeClause};
use cairo_lang_syntax::node::db::SyntaxGroup;
use cairo_lang_syntax::node::helpers::QueryAttrs;
use cairo_lang_syntax::node::{Terminal, TypedSyntaxNode};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use dojo_world::manifest::Member;
use itertools::Itertools;

#[derive(Clone, Default)]
struct TypeIntrospection(usize, Vec<usize>);

fn primitive_type_introspection() -> HashMap<String, TypeIntrospection> {
    HashMap::from([
        ("felt252".into(), TypeIntrospection(1, vec![251])),
        ("bool".into(), TypeIntrospection(1, vec![1])),
        ("u8".into(), TypeIntrospection(1, vec![8])),
        ("u16".into(), TypeIntrospection(1, vec![16])),
        ("u32".into(), TypeIntrospection(1, vec![32])),
        ("u64".into(), TypeIntrospection(1, vec![64])),
        ("u128".into(), TypeIntrospection(1, vec![128])),
        ("u256".into(), TypeIntrospection(2, vec![128, 128])),
        ("usize".into(), TypeIntrospection(1, vec![32])),
        ("ContractAddress".into(), TypeIntrospection(1, vec![251])),
        ("ClassHash".into(), TypeIntrospection(1, vec![251])),
    ])
}

/// A handler for Dojo code derives Introspect for a struct
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_struct(db: &dyn SyntaxGroup, struct_ast: ItemStruct) -> RewriteNode {
    let name = struct_ast.name(db).text(db).into();

    let mut member_types: Vec<String> = vec![];
    let primitive_sizes = primitive_type_introspection();

    let members: Vec<_> = struct_ast
        .members(db)
        .elements(db)
        .iter()
        .map(|member| {
            let key = member.has_attr(db, "key");
            let ty = member.type_clause(db).ty(db).as_syntax_node().get_text(db).trim().to_string();
            let name = member.name(db).text(db).to_string();
            let mut attrs = vec![];
            if key {
                attrs.push("'key'");
            }

            if primitive_sizes.get(&ty).is_some() {
                // It's a primitive type
                member_types.push(format!(
                    "
                    dojo::database::schema::serialize_member(@dojo::database::schema::Member {{
                        name: '{name}',
                        ty: dojo::database::schema::Ty::Primitive('{ty}'),
                        attrs: array![{}].span()
                    }})\n",
                    attrs.join(","),
                ));
            } else {
                // It's a custom struct/enum
                member_types.push(format!(
                    "
                    dojo::database::schema::serialize_member(@dojo::database::schema::Member {{
                        name: '{name}',
                        ty: dojo::database::schema::SchemaIntrospection::<{ty}>::ty(),
                        attrs: array![{}].span()
                    }})\n",
                    attrs.join(","),
                ));
            }

            Member { name, ty, key }
        })
        .collect::<_>();
    drop(primitive_sizes);

    let type_ty = format!(
        "
        dojo::database::schema::Ty::Struct(dojo::database::schema::Struct {{
            name: '{name}',
            attrs: array![].span(),
            children: array![{}].span()
        }})",
        member_types.join(",\n")
    );

    handle_introspect_internal(db, name, vec![], 0, type_ty, members)
}

/// A handler for Dojo code derives Introspect for an enum
/// Parameters:
/// * db: The semantic database.
/// * struct_ast: The AST of the struct.
/// Returns:
/// * A RewriteNode containing the generated code.
pub fn handle_introspect_enum(
    db: &dyn SyntaxGroup,
    diagnostics: &mut Vec<PluginDiagnostic>,
    enum_ast: ItemEnum,
) -> RewriteNode {
    let name = enum_ast.name(db).text(db).into();
    let variant_type = enum_ast.variants(db).elements(db).first().unwrap().type_clause(db);
    let variant_type_text = variant_type.as_syntax_node().get_text(db);
    let variant_type_text = variant_type_text.trim();
    let mut variant_type_arr = vec![];

    if let OptionTypeClause::TypeClause(types_tuple) = variant_type {
        if let Expr::Tuple(paren_list) = types_tuple.ty(db) {
            let args = (*paren_list.expressions(db)).elements(db);
            args.iter().for_each(|arg| {
                let ty_name = arg.as_syntax_node().get_text(db);
                variant_type_arr.push((
                    // Not using Ty right now, but still keeping it for later.
                    format!(
                        "dojo::database::schema::serialize_member_type(
                            @dojo::database::schema::Ty::Primitive('{}')
                        )",
                        ty_name
                    ),
                    ty_name,
                ));
            });
        } else if let Expr::Path(type_path) = types_tuple.ty(db) {
            let ty_name = type_path.as_syntax_node().get_text(db);
            variant_type_arr.push((
                // Not using Ty right now, but still keeping it for later.
                format!(
                    "dojo::database::schema::serialize_member_type(
                        @dojo::database::schema::SchemaIntrospection::<{}>::ty()
                    )",
                    ty_name
                ),
                ty_name,
            ));
        } else {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: types_tuple.stable_ptr().0,
                message: "Only tuple and type paths are supported.".to_string(),
            });
        }
    }

    let members: Vec<_> = variant_type_arr
        .iter()
        .map(|(_, ty)| Member { name: ty.into(), ty: ty.into(), key: false })
        .collect_vec();

    let mut arms_ty: Vec<String> = vec![];

    // Add diagnostics for different Typeclauses.
    enum_ast.variants(db).elements(db).iter().for_each(|member| {
        let member_name = member.name(db).text(db);
        let member_type = member.type_clause(db).as_syntax_node();
        let member_type_text = member_type.get_text(db);
        if member_type_text.trim() != variant_type_text.trim() {
            diagnostics.push(PluginDiagnostic {
                stable_ptr: member_type.stable_ptr(),
                message: format!("Enum arms need to have same type - {}.", variant_type_text),
            });
        }

        // @TODO: Prepare type struct
        arms_ty.push(format!(
            "
            (
                '{member_name}',
                dojo::database::schema::serialize_member_type(
                @dojo::database::schema::Ty::Tuple(array![{}].span()))
            )",
            if !variant_type_arr.is_empty() {
                let ty_cairo: Vec<_> =
                    variant_type_arr.iter().map(|(ty_cairo, _)| ty_cairo.to_string()).collect();
                // format!("'{}'", &ty_cairo.join("', '"))
                ty_cairo.join(",\n")
            } else {
                "".to_string()
            }
        ));
    });

    let type_ty = format!(
        "
        dojo::database::schema::Ty::Enum(
            dojo::database::schema::Enum {{
                name: 'Direction',
                attrs: array![].span(),
                children: array![
                {}
                ].span()
            }}
        )",
        arms_ty.join(",\n")
    );
    // Enums have 1 size and 8 bit layout by default
    let layout = vec![RewriteNode::Text("layout.append(8);\n".into())];
    let size_precompute = 1;
    handle_introspect_internal(db, name, layout, size_precompute, type_ty, members)
}

fn handle_introspect_internal(
    _db: &dyn SyntaxGroup,
    name: String,
    mut layout: Vec<RewriteNode>,
    mut size_precompute: usize,
    type_ty: String,
    members: Vec<Member>,
) -> RewriteNode {
    let mut size = vec![];
    let primitive_sizes = primitive_type_introspection();

    members.iter().for_each(|m| {
        let primitive_intro = primitive_sizes.get(&m.ty);
        let mut attrs = vec![];

        if let Some(p_ty) = primitive_intro {
            // It's a primitive type
            if m.key {
                attrs.push("'key'");
            } else {
                size_precompute += p_ty.0;
                p_ty.1.iter().for_each(|l| {
                    layout.push(RewriteNode::Text(format!("layout.append({});\n", l)))
                });
            }
        } else {
            // It's a custom type
            if m.key {
                attrs.push("'key'");
            } else {
                size.push(format!(
                    "dojo::database::schema::SchemaIntrospection::<{}>::size()",
                    m.ty,
                ));
                layout.push(RewriteNode::Text(format!(
                    "dojo::database::schema::SchemaIntrospection::<{}>::layout(ref layout);\n",
                    m.ty
                )));
            }
        }
    });

    if size_precompute > 0 {
        size.push(format!("{}", size_precompute));
    }

    RewriteNode::interpolate_patched(
        "
        impl $name$SchemaIntrospection of dojo::database::schema::SchemaIntrospection<$name$> {
            
            #[inline(always)]
            fn size() -> usize {
                $size$
            }

            #[inline(always)]
            fn layout(ref layout: Array<u8>) {
                $layout$
            }

            #[inline(always)]
            fn ty() -> dojo::database::schema::Ty {
                $ty$
            }
        }
        ",
        &UnorderedHashMap::from([
            ("name".to_string(), RewriteNode::Text(name)),
            ("size".to_string(), RewriteNode::Text(size.join(" + "))),
            ("layout".to_string(), RewriteNode::new_modified(layout)),
            ("ty".to_string(), RewriteNode::Text(type_ty)),
        ]),
    )
}