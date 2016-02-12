// Copyright 2015 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use Indent;
use rewrite::{Rewrite, RewriteContext};
use utils::{wrap_str, format_mutability, span_after};
use lists::{format_item_list, itemize_list};
use expr::{rewrite_unary_prefix, rewrite_pair, rewrite_tuple};
use types::rewrite_path;

use syntax::ast::{BindingMode, Pat, Pat_, FieldPat};

impl Rewrite for Pat {
    fn rewrite(&self, context: &RewriteContext, width: usize, offset: Indent) -> Option<String> {
        match self.node {
            Pat_::PatBox(ref pat) => rewrite_unary_prefix(context, "box ", &**pat, width, offset),
            Pat_::PatIdent(binding_mode, ident, ref sub_pat) => {
                let (prefix, mutability) = match binding_mode {
                    BindingMode::BindByRef(mutability) => ("ref ", mutability),
                    BindingMode::BindByValue(mutability) => ("", mutability),
                };
                let mut_infix = format_mutability(mutability);
                let id_str = ident.node.to_string();

                let sub_pat = match *sub_pat {
                    Some(ref p) => {
                        let width = try_opt!(width.checked_sub(prefix.len() + mut_infix.len() +
                                                               id_str.len()));
                        format!(" @ {}", try_opt!(p.rewrite(context, width, offset)))
                    }
                    None => "".to_owned(),
                };

                let result = format!("{}{}{}{}", prefix, mut_infix, id_str, sub_pat);
                wrap_str(result, context.config.max_width, width, offset)
            }
            Pat_::PatWild => {
                if 1 <= width {
                    Some("_".to_owned())
                } else {
                    None
                }
            }
            Pat_::PatQPath(ref q_self, ref path) => {
                rewrite_path(context, true, Some(q_self), path, width, offset)
            }
            Pat_::PatRange(ref lhs, ref rhs) => {
                rewrite_pair(&**lhs, &**rhs, "", "...", "", context, width, offset)
            }
            Pat_::PatRegion(ref pat, mutability) => {
                let prefix = format!("&{}", format_mutability(mutability));
                rewrite_unary_prefix(context, &prefix, &**pat, width, offset)
            }
            Pat_::PatTup(ref items) => {
                rewrite_tuple(context,
                              items.iter().map(|x| &**x),
                              self.span,
                              width,
                              offset)
            }
            Pat_::PatEnum(ref path, ref pat_vec) => {
                let path_str = try_opt!(rewrite_path(context, true, None, path, width, offset));

                match *pat_vec {
                    Some(ref pat_vec) => {
                        if pat_vec.is_empty() {
                            Some(path_str)
                        } else {
                            // 1 = (
                            let width = try_opt!(width.checked_sub(path_str.len() + 1));
                            let offset = offset + path_str.len() + 1;
                            let items = itemize_list(context.codemap,
                                                     pat_vec.iter(),
                                                     ")",
                                                     |item| item.span.lo,
                                                     |item| item.span.hi,
                                                     |item| item.rewrite(context, width, offset),
                                                     span_after(self.span, "(", context.codemap),
                                                     self.span.hi);
                            Some(format!("{}({})",
                                         path_str,
                                         try_opt!(format_item_list(items,
                                                                   width,
                                                                   offset,
                                                                   context.config))))
                        }
                    }
                    None => Some(format!("{}(..)", path_str)),
                }
            }
            Pat_::PatLit(ref expr) => expr.rewrite(context, width, offset),
            Pat_::PatVec(ref prefix, ref slice_pat, ref suffix) => {
                // Rewrite all the sub-patterns.
                let prefix = prefix.iter().map(|p| p.rewrite(context, width, offset));
                let slice_pat = slice_pat.as_ref().map(|p| {
                    Some(format!("{}..", try_opt!(p.rewrite(context, width, offset))))
                });
                let suffix = suffix.iter().map(|p| p.rewrite(context, width, offset));

                // Munge them together.
                let pats = prefix.chain(slice_pat.into_iter()).chain(suffix);

                // Check that all the rewrites succeeded, and if not return None.
                let (somes, nones) = pats.partition::<Vec<Option<String>>, _>(Option::is_some);
                if nones.len() > 0 {
                    return None;
                }

                // Unwrap all the sub-strings and join them with commas.
                let pats = somes.into_iter().map(|p| p.unwrap()).collect::<Vec<_>>().join(", ");
                Some(format!("[{}]", pats))
            }
            Pat_::PatStruct(ref path, ref fields, elipses) => {
                let path = try_opt!(rewrite_path(context, true, None, path, width, offset));

                let (elipses_str, terminator) = if elipses {
                    (", ..", "..")
                } else {
                    ("", "}")
                };

                let budget = try_opt!(width.checked_sub(path.len() + 5 + elipses_str.len()));
                // FIXME Using visual indenting, should use block or visual to match
                // struct lit preference (however, in practice I think it is rare
                // for struct patterns to be multi-line).
                let offset = offset + path.len() + 3;

                let items = itemize_list(context.codemap,
                                         fields.iter(),
                                         terminator,
                                         |f| f.span.lo,
                                         |f| f.span.hi,
                                         |f| f.node.rewrite(context, budget, offset),
                                         span_after(self.span, "{", context.codemap),
                                         self.span.hi);
                let mut field_string = try_opt!(format_item_list(items,
                                                                 budget,
                                                                 offset,
                                                                 context.config));
                if elipses {
                    if field_string.contains('\n') {
                        field_string.push_str(",\n");
                        field_string.push_str(&offset.to_string(context.config));
                        field_string.push_str("..");
                    } else {
                        if field_string.len() > 0 {
                            field_string.push_str(", ");
                        }
                        field_string.push_str("..");
                    }
                }

                if field_string.len() == 0 {
                    Some(format!("{} {{}}", path))
                } else {
                    Some(format!("{} {{ {} }}", path, field_string))
                }
            }
            // FIXME(#819) format pattern macros.
            Pat_::PatMac(..) => {
                wrap_str(context.snippet(self.span),
                         context.config.max_width,
                         width,
                         offset)
            }
        }
    }
}

impl Rewrite for FieldPat {
    fn rewrite(&self, context: &RewriteContext, width: usize, offset: Indent) -> Option<String> {
        let pat = self.pat.rewrite(context, width, offset);
        if self.is_shorthand {
            pat
        } else {
            Some(format!("{}: {}", self.ident.to_string(), try_opt!(pat)))
        }
    }
}
