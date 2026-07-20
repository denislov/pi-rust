//! Syntax highlighting — ported from `highlightCode`, `getLanguageFromPath`,
//! and `buildCliHighlightTheme` in `theme.ts`.
//!
//! Uses `syntect` (Sublime syntaxes) in place of TS's `cli-highlight`
//! (highlight.js). We build a `syntect::highlighting::Theme` whose scope
//! selectors map to the 9 theme syntax tokens, mirroring the hljs-scope ->
//! token mapping in `buildCliHighlightTheme`. `HighlightLines` then yields
//! per-region foreground colors already chosen from the active pi theme.

use std::sync::OnceLock;

use syntect::easy::HighlightLines;
use syntect::highlighting::{
    Color as SyntectColor, ScopeSelectors, StyleModifier, Theme as SyntectTheme, ThemeItem,
};
use syntect::parsing::SyntaxSet;

use super::{ResolvedColor, ResolvedTheme, ThemeColor};

/// Map a file path to a language identifier by extension, mirroring
/// `getLanguageFromPath`. Returns `None` for unknown/missing extensions.
#[cfg(test)]
pub fn get_language_from_path(path: &str) -> Option<&'static str> {
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    if ext == path {
        // No dot -> no extension (e.g. "Dockerfile").
        return None;
    }
    EXT_TO_LANG
        .iter()
        .copied()
        .find_map(|(e, lang)| if e == ext { Some(lang) } else { None })
}

/// Extension -> language table, ported verbatim from TS `getLanguageFromPath`.
#[cfg(test)]
const EXT_TO_LANG: &[(&str, &str)] = &[
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("js", "javascript"),
    ("jsx", "javascript"),
    ("mjs", "javascript"),
    ("cjs", "javascript"),
    ("py", "python"),
    ("rb", "ruby"),
    ("rs", "rust"),
    ("go", "go"),
    ("java", "java"),
    ("kt", "kotlin"),
    ("swift", "swift"),
    ("c", "c"),
    ("h", "c"),
    ("cpp", "cpp"),
    ("cc", "cpp"),
    ("cxx", "cpp"),
    ("hpp", "cpp"),
    ("cs", "csharp"),
    ("php", "php"),
    ("sh", "bash"),
    ("bash", "bash"),
    ("zsh", "bash"),
    ("fish", "fish"),
    ("ps1", "powershell"),
    ("sql", "sql"),
    ("html", "html"),
    ("htm", "html"),
    ("css", "css"),
    ("scss", "scss"),
    ("sass", "sass"),
    ("less", "less"),
    ("json", "json"),
    ("yaml", "yaml"),
    ("yml", "yaml"),
    ("toml", "toml"),
    ("xml", "xml"),
    ("md", "markdown"),
    ("markdown", "markdown"),
    ("lua", "lua"),
    ("perl", "perl"),
    ("r", "r"),
    ("scala", "scala"),
    ("clj", "clojure"),
    ("ex", "elixir"),
    ("exs", "elixir"),
    ("erl", "erlang"),
    ("hs", "haskell"),
    ("ml", "ocaml"),
    ("vim", "vim"),
    ("graphql", "graphql"),
    ("proto", "protobuf"),
    ("tf", "hcl"),
    ("hcl", "hcl"),
];

/// A loaded `SyntaxSet`. Cached for the process lifetime (loading parses
/// embedded syntax definitions, ~ms on first call).
fn syntax_set() -> &'static SyntaxSet {
    static SET: OnceLock<SyntaxSet> = OnceLock::new();
    SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// Resolve a language name (e.g. "rust", or a file extension like "rs") to a
/// syntect syntax reference.
fn syntax_for_language(lang: &str) -> Option<&'static syntect::parsing::SyntaxReference> {
    let set = syntax_set();
    set.find_syntax_by_extension(lang)
        .or_else(|| set.find_syntax_by_token(lang))
        .or_else(|| {
            set.syntaxes()
                .iter()
                .find(|s| s.name.eq_ignore_ascii_case(lang))
        })
}

/// Build a `syntect::Theme` whose scope selectors map to the 9 syntax tokens
/// of the active pi theme, mirroring TS `buildCliHighlightTheme`. Each
/// selector's foreground is the resolved theme color for the corresponding
/// token; the `text`/default token is the code-block fallback color.
fn build_syntect_theme(theme: &ResolvedTheme) -> SyntectTheme {
    let mut syntect_theme = SyntectTheme::default();

    let push = |t: &mut SyntectTheme, selector: &str, color: ResolvedColor| {
        if let Some(c) = syntect_color(color) {
            t.scopes.push(ThemeItem {
                scope: selector
                    .parse::<ScopeSelectors>()
                    .expect("valid scope selector"),
                style: StyleModifier {
                    foreground: Some(c),
                    background: None,
                    font_style: Default::default(),
                },
            });
        }
    };

    // (scope selector, token) pairs mirroring buildCliHighlightTheme.
    push(
        &mut syntect_theme,
        "comment, doctag",
        theme.fg(ThemeColor::SyntaxComment),
    );
    push(
        &mut syntect_theme,
        "string, regexp",
        theme.fg(ThemeColor::SyntaxString),
    );
    push(
        &mut syntect_theme,
        "constant.numeric, constant.language",
        theme.fg(ThemeColor::SyntaxNumber),
    );
    // `keyword` and all `storage` (including storage.type.function like
    // Rust's `fn`) map to the keyword token, matching hljs classification.
    push(
        &mut syntect_theme,
        "keyword, storage",
        theme.fg(ThemeColor::SyntaxKeyword),
    );
    push(
        &mut syntect_theme,
        "entity.name.function, support.function, variable.function",
        theme.fg(ThemeColor::SyntaxFunction),
    );
    push(
        &mut syntect_theme,
        "entity.name.class, entity.name.type, support.type",
        theme.fg(ThemeColor::SyntaxType),
    );
    push(
        &mut syntect_theme,
        "variable, entity.name.attribute, meta.parameter",
        theme.fg(ThemeColor::SyntaxVariable),
    );
    push(
        &mut syntect_theme,
        "keyword.operator",
        theme.fg(ThemeColor::SyntaxOperator),
    );
    push(
        &mut syntect_theme,
        "punctuation",
        theme.fg(ThemeColor::SyntaxPunctuation),
    );

    syntect_theme
}

fn syntect_color(color: ResolvedColor) -> Option<SyntectColor> {
    match color {
        ResolvedColor::Default => None,
        ResolvedColor::Hex(r, g, b) => Some(SyntectColor { r, g, b, a: 0xff }),
        ResolvedColor::Ansi256(_) => None, // syntect is RGB-only; 256 left default
    }
}

/// Highlight `code` for `lang`, returning one painted string per line using
/// the theme's syntax tokens. Mirrors `highlightCode`:
/// - unknown/empty language -> each line painted with `mdCodeBlock` (single color)
/// - parse error -> fall back to single-color lines
pub fn highlight_code(code: &str, lang: Option<&str>, theme: &ResolvedTheme) -> Vec<String> {
    let Some(lang) = lang.filter(|l| !l.is_empty()) else {
        return single_color_lines(code, theme);
    };
    let Some(syntax) = syntax_for_language(lang) else {
        return single_color_lines(code, theme);
    };

    let set = syntax_set();
    let syntect_theme = build_syntect_theme(theme);
    let mut highlighter = HighlightLines::new(syntax, &syntect_theme);
    let fallback = theme.fg(ThemeColor::MdCodeBlock);

    let mut out = Vec::new();
    for line in code.trim_end_matches('\n').split('\n') {
        match highlighter.highlight_line(line, set) {
            Ok(ranges) => {
                let mut painted = String::new();
                for (style, text) in ranges {
                    let color = syntect_color_to_resolved(style.foreground, fallback);
                    painted.push_str(&paint(text, color));
                }
                out.push(format!("   {painted}"));
            }
            Err(_) => {
                out.push(format!("   {}", paint(line, fallback)));
            }
        }
    }
    out
}

/// Convert a syntect foreground color back to a `ResolvedColor`. syntect uses
/// `{r:0,g:0,b:0,a:0}` for "no color" (transparent/default), which we treat as
/// the code-block fallback.
fn syntect_color_to_resolved(color: SyntectColor, fallback: ResolvedColor) -> ResolvedColor {
    if color.a == 0 {
        return fallback;
    }
    ResolvedColor::Hex(color.r, color.g, color.b)
}

fn single_color_lines(code: &str, theme: &ResolvedTheme) -> Vec<String> {
    let color = theme.fg(ThemeColor::MdCodeBlock);
    code.trim_end_matches('\n')
        .split('\n')
        .map(|line| format!("   {}", paint(line, color)))
        .collect()
}

/// Render `text` with the given resolved color as ANSI, using `pi-tui`'s
/// paint layer. `Default` leaves text uncolored.
fn paint(text: &str, color: ResolvedColor) -> String {
    use pi_tui::api::render::{Color, ColorLevel, Style, paint_with_level};
    let style = Style::fg(match color {
        ResolvedColor::Default => Color::Default,
        ResolvedColor::Hex(r, g, b) => Color::Rgb(r, g, b),
        ResolvedColor::Ansi256(n) => Color::Ansi256(n),
    });
    paint_with_level(text, &style, ColorLevel::TrueColor)
}
