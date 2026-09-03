//! Tokenize on the Git worker, once per diff; painting never runs a parser.

use crate::{
    model::{DKind, DLine},
    theme::TEXT,
};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use std::{path::Path, sync::OnceLock};
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SyntaxColor, StyleModifier, Theme, ThemeItem},
    parsing::SyntaxSet,
};

fn syntax_theme() -> &'static Theme {
    static THEME: OnceLock<Theme> = OnceLock::new();
    THEME.get_or_init(|| {
        let color = |r, g, b| SyntaxColor { r, g, b, a: 255 };
        let mut theme = Theme::default();
        theme.settings.foreground = Some(color(205, 214, 244));
        for (scope, (r, g, b)) in [
            ("comment", (108, 112, 134)),
            ("keyword, storage", (203, 166, 247)),
            ("string", (166, 227, 161)),
            ("constant.numeric, constant.language", (250, 179, 135)),
            ("entity.name.function, support.function", (137, 180, 250)),
            (
                "entity.name.type, entity.name.class, support.type, support.class",
                (249, 226, 175),
            ),
            ("variable.parameter", (235, 160, 172)),
            ("keyword.operator", (137, 220, 235)),
            ("punctuation", (147, 153, 178)),
            ("entity.name.tag", (203, 166, 247)),
            ("entity.other.attribute-name", (249, 226, 175)),
            ("markup.heading", (137, 180, 250)),
        ] {
            theme.scopes.push(ThemeItem {
                scope: scope.parse().expect("static syntax scope"),
                style: StyleModifier {
                    foreground: Some(color(r, g, b)),
                    ..Default::default()
                },
            });
        }
        theme
    })
}

pub fn highlight(lines: &[DLine], filename: &str) -> Vec<Line<'static>> {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    let syntaxes = SYNTAXES.get_or_init(SyntaxSet::load_defaults_nonewlines);
    let extension = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let syntax = syntaxes
        .find_syntax_by_extension(extension)
        .unwrap_or_else(|| syntaxes.find_syntax_plain_text());
    let mut old = HighlightLines::new(syntax, syntax_theme());
    let mut new = HighlightLines::new(syntax, syntax_theme());
    lines
        .iter()
        .map(|line| {
            let body = line
                .text
                .strip_prefix(['+', '-', ' '])
                .unwrap_or(&line.text);
            if matches!(line.kind, DKind::File | DKind::Hunk | DKind::Gap) {
                return Line::default();
            }
            let tokens = match line.kind {
                DKind::Del => old.highlight_line(body, syntaxes),
                DKind::Ctx => {
                    let _ = old.highlight_line(body, syntaxes);
                    new.highlight_line(body, syntaxes)
                }
                _ => new.highlight_line(body, syntaxes),
            };
            Line::from(
                tokens
                    .map(|tokens| {
                        tokens
                            .into_iter()
                            .map(|(style, text)| {
                                let color = style.foreground;
                                Span::styled(
                                    text.to_owned(),
                                    Style::default().fg(Color::Rgb(color.r, color.g, color.b)),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|_| {
                        vec![Span::styled(body.to_owned(), Style::default().fg(TEXT))]
                    }),
            )
        })
        .collect()
}
