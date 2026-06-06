//! `TeX`-source fixups for `LaTeXML`-produced math annotations.
//!
//! This module is deliberately the *only* home for heuristic rewriting — legacy
//! plain-`TeX` primitives, package-specific macros, and `LaTeXML` cruft. Keeping
//! it quarantined lets the structured converters ([`super::engine`], and any
//! future `MathML` walker) stay clean and declarative.
//!
//! Everything here is a pure `&str -> String` transform applied to the `TeX`
//! *before* it reaches a converter, so both the native-`Typst` path and the
//! image fallback benefit from the same normalization.

/// Tokens `LaTeXML` emits that carry no meaning in `Typst` block math.
const CRUFT: &[&str] = &[
    "\\displaystyle",
    "\\textstyle",
    "\\scriptstyle",
    "\\scriptscriptstyle",
    "\\leavevmode",
    "\\nobreak",
    "\\@add@centering",
    "\\centering",
];

/// Legacy font-switch primitives (declaration form: they affect the rest of the
/// enclosing group), mapped to modern `\math..{..}` commands.
fn font_switch(name: &str) -> Option<&'static str> {
    Some(match name {
        "rm" => "mathrm",
        "sf" => "mathsf",
        "bf" => "mathbf",
        "it" | "sl" => "mathit",
        "tt" => "mathtt",
        "cal" => "mathcal",
        _ => return None,
    })
}

/// Package macros `\cmd{arg}` rewritten to standard `TeX` so a downstream
/// converter handles the final mapping. `ARG` marks the argument slot.
///
/// `\bm`/`\boldsymbol` map to `\boldsymbol` (not `\mathbf`) on purpose: that
/// yields `Typst` `bold(..)` — bold *italic*, the intended vector look — whereas
/// `\mathbf` yields `upright(bold(..))`.
fn macro_template(name: &str) -> Option<&'static str> {
    Some(match name {
        "bm" | "boldsymbol" | "pmb" => "\\boldsymbol{ARG}",
        "ket" => "\\left|ARG\\right\\rangle",
        "bra" => "\\left\\langle ARG\\right|",
        "braket" => "\\left\\langle ARG\\right\\rangle",
        "absolutevalue" | "abs" => "\\left|ARG\\right|",
        "norm" => "\\left\\|ARG\\right\\|",
        _ => return None,
    })
}

/// Read a control sequence starting at `chars[i] == '\\'`. Returns the name
/// (without the backslash) and the index just past it. A control *word* is a run
/// of letters; a control *symbol* is a single non-letter.
fn read_cmd(chars: &[char], i: usize) -> (String, usize) {
    let mut j = i + 1;
    if j < chars.len() && chars[j].is_alphabetic() {
        let start = j;
        while j < chars.len() && chars[j].is_alphabetic() {
            j += 1;
        }
        (chars[start..j].iter().collect(), j)
    } else if j < chars.len() {
        (chars[j].to_string(), j + 1)
    } else {
        (String::new(), j)
    }
}

/// Given `chars[i] == '{'`, return the group's inner chars and the index just
/// past the matching `'}'`.
fn take_group(chars: &[char], i: usize) -> Option<(Vec<char>, usize)> {
    if chars.get(i) != Some(&'{') {
        return None;
    }
    let mut depth = 0usize;
    let mut inner = Vec::new();
    let mut j = i;
    while j < chars.len() {
        match chars[j] {
            '{' => {
                depth += 1;
                if depth > 1 {
                    inner.push('{');
                }
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((inner, j + 1));
                }
                inner.push('}');
            }
            c => inner.push(c),
        }
        j += 1;
    }
    None
}

/// Rewrite a top-level `\over` in this group's content to `\frac{..}{..}`.
fn split_over(chars: &[char]) -> Option<String> {
    let mut depth = 0i32;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' => depth += 1,
            '}' => depth -= 1,
            '\\' => {
                let (name, next) = read_cmd(chars, i);
                if depth == 0 && name == "over" {
                    let num = transform(&chars[..i]);
                    let den = transform(&chars[next..]);
                    return Some(format!("\\frac{{{}}}{{{}}}", num.trim(), den.trim()));
                }
                i = next;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Recursively rewrite the content of one group (or the whole input).
fn transform(chars: &[char]) -> String {
    if let Some(fr) = split_over(chars) {
        return fr;
    }
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                if let Some((inner, next)) = take_group(chars, i) {
                    out.push('{');
                    out.push_str(&transform(&inner));
                    out.push('}');
                    i = next;
                } else {
                    out.push('{');
                    i += 1;
                }
            }
            '\\' => {
                let (name, next) = read_cmd(chars, i);
                if let Some(modern) = font_switch(&name) {
                    // A font switch applies to the rest of the current group.
                    let rest = transform(&chars[next..]);
                    out.push('\\');
                    out.push_str(modern);
                    out.push('{');
                    out.push_str(rest.trim());
                    out.push('}');
                    return out;
                } else if let Some(tmpl) = macro_template(&name) {
                    let mut k = next;
                    while k < chars.len() && chars[k] == ' ' {
                        k += 1;
                    }
                    if let Some((arg, after)) = take_group(chars, k) {
                        out.push_str(&tmpl.replace("ARG", &transform(&arg)));
                        i = after;
                    } else {
                        out.push_str(&tmpl.replace("ARG", ""));
                        i = next;
                    }
                } else {
                    out.extend(&chars[i..next]);
                    i = next;
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// Normalize a `LaTeXML` math annotation into `TeX` a converter can handle:
/// strip cruft, rewrite legacy primitives, and expand known package macros.
pub fn normalize(tex: &str) -> String {
    let mut s = tex.to_string();
    for tok in CRUFT {
        s = s.replace(tok, " ");
    }
    s = s.replace("~{}", " ");
    transform(&s.chars().collect::<Vec<_>>())
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn strips_displaystyle_cruft() {
        assert_eq!(normalize("\\displaystyle x").trim(), "x");
    }

    #[test]
    fn rewrites_over_to_frac() {
        assert_eq!(normalize("{\\beta \\over 4}"), "{\\frac{\\beta}{4}}");
    }

    #[test]
    fn rewrites_legacy_font_switch() {
        // `{\rm nl}` becomes `\mathrm{nl}` inside the surrounding braces.
        assert_eq!(normalize("\\tau_{\\rm nl}"), "\\tau_{\\mathrm{nl}}");
    }

    #[test]
    fn bm_maps_to_boldsymbol_not_mathbf() {
        let out = normalize("\\bm{x}");
        assert!(out.contains("\\boldsymbol{x}"), "got {out:?}");
        assert!(!out.contains("\\mathbf"), "got {out:?}");
    }

    #[test]
    fn ket_expands_to_delimited_form() {
        assert_eq!(normalize("\\ket{\\psi}"), "\\left|\\psi\\right\\rangle");
    }
}
