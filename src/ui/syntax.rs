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
    parsing::{SyntaxReference, SyntaxSet},
};

fn syntax_set() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(two_face::syntax::extra_no_newlines)
}

fn file_syntax<'a>(
    syntaxes: &'a SyntaxSet,
    filename: &str,
    lines: &[DLine],
) -> &'a SyntaxReference {
    let path = Path::new(filename);
    let basename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    // Match full names (Dockerfile, .bashrc), then longest suffixes (html.j2 before j2).
    std::iter::successors(Some(basename), |name| {
        name.split_once('.').map(|(_, suffix)| suffix)
    })
    .find_map(|name| syntaxes.find_syntax_by_extension(name))
    .or_else(|| {
        // Use diff content: deleted files and repository-relative paths may not exist on disk.
        let first_line = lines
            .iter()
            .find(|line| line.new_no == Some(1))
            .or_else(|| lines.iter().find(|line| line.old_no == Some(1)))?;
        let body = first_line
            .text
            .strip_prefix(['+', '-', ' '])
            .unwrap_or(&first_line.text);
        syntaxes.find_syntax_by_first_line(body)
    })
    .unwrap_or_else(|| syntaxes.find_syntax_plain_text())
}

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
    let syntaxes = syntax_set();
    let syntax = file_syntax(syntaxes, filename, lines);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_detection_and_tokenization() {
        let syntaxes = syntax_set();
        for (filename, expected) in [
            ("src/app.js", "JavaScript (Babel)"),
            ("src/app.jsx", "JavaScript (Babel)"),
            ("src/app.mjs", "JavaScript (Babel)"),
            ("src/app.cjs", "JavaScript (Babel)"),
            ("src/app.ts", "TypeScript"),
            ("src/app.tsx", "TypeScriptReact"),
            ("src/app.mts", "TypeScript"),
            ("src/app.cts", "TypeScript"),
            ("src/app.TSX", "TypeScriptReact"),
            ("app.py", "Python"),
            ("app.rs", "Rust"),
            ("app.go", "Go"),
            ("App.java", "Java"),
            ("app.c", "C"),
            ("app.cpp", "C++"),
            ("App.cs", "C#"),
            ("app.rb", "Ruby"),
            ("app.php", "PHP"),
            ("app.swift", "Swift"),
            ("app.kt", "Kotlin"),
            ("app.dart", "Dart"),
            ("app.vue", "Vue Component"),
            ("app.svelte", "Svelte"),
            ("index.html", "HTML"),
            ("styles.css", "CSS"),
            ("styles.scss", "SCSS"),
            ("query.sql", "SQL"),
            ("config.json", "JSON"),
            ("config.yml", "YAML"),
            ("config.toml", "TOML"),
            ("bin/script.sh", "Bourne Again Shell (bash)"),
            ("bin/script.ps1", "PowerShell"),
            ("build/Dockerfile", "Dockerfile"),
            ("build/Makefile", "Makefile"),
            ("build/CMakeLists.txt", "CMake"),
            ("config/.bashrc", "Bourne Again Shell (bash)"),
            ("config/.env.local", "DotENV"),
            ("config/.gitconfig", "Git Config"),
            ("Cargo.lock", "TOML"),
            ("go.mod", "Gomod"),
            ("templates/page.html.j2", "HTML (Jinja2)"),
            ("src/schema.pb.txt", "Protocol Buffer (TEXT)"),
            ("unknown.freezy-unknown", "Plain Text"),
        ] {
            assert_eq!(
                file_syntax(syntaxes, filename, &[]).name,
                expected,
                "{filename}"
            );
        }

        let source = [DLine::plain(
            "+const message = \"hello\";".into(),
            DKind::Add,
        )];
        for extension in ["js", "jsx", "mjs", "cjs", "ts", "tsx", "mts", "cts"] {
            let colored = highlight(&source, &format!("app.{extension}"));
            assert_eq!(colored[0].to_string(), "const message = \"hello\";");
            for (token, foreground) in [
                ("const", Color::Rgb(203, 166, 247)),
                ("hello", Color::Rgb(166, 227, 161)),
            ] {
                assert!(
                    colored[0].spans.iter().any(|span| {
                        span.content.contains(token) && span.style.fg == Some(foreground)
                    }),
                    "{extension}: {token} must be colored"
                );
            }
        }

        // Shebangs come from real line 1, including removed files, never a later hunk.
        for (shebang, expected) in [
            ("#!/usr/bin/env python3", "Python"),
            ("#!/bin/bash", "Bourne Again Shell (bash)"),
            ("#!/usr/bin/env node", "JavaScript (Babel)"),
        ] {
            for kind in [DKind::Add, DKind::Del] {
                let mut line = DLine {
                    text: format!("{}{shebang}", if kind == DKind::Add { '+' } else { '-' }),
                    kind,
                    old_no: (kind == DKind::Del).then_some(1),
                    new_no: (kind == DKind::Add).then_some(1),
                };
                assert_eq!(
                    file_syntax(syntaxes, "bin/run", std::slice::from_ref(&line)).name,
                    expected
                );
                line.old_no = line.old_no.map(|_| 50);
                line.new_no = line.new_no.map(|_| 50);
                assert_eq!(file_syntax(syntaxes, "bin/run", &[line]).name, "Plain Text");
            }
        }

        let plain = highlight(&source, "unknown.freezy-unknown");
        assert_eq!(plain[0].to_string(), "const message = \"hello\";");
        assert!(plain[0]
            .spans
            .iter()
            .all(|span| span.style.fg == Some(TEXT)));
    }
}
