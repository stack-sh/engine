//! Canonical, comment-preserving formatter for Stack source.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use stack_compiler::ast::{
    self, DiagramMember, EdgeProperty, GroupMember, LayoutStatement, NodeProperty,
};
use stack_compiler::diagnostic::Diagnostic;
use stack_compiler::lossless::{Document as LosslessDocument, Token, TokenKind};

/// Result of formatting Stack source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOutput {
    /// Canonical source, absent when lexical or syntax errors prevent formatting.
    pub source: Option<String>,
    /// Compiler diagnostics for the original source.
    pub diagnostics: Vec<Diagnostic>,
}

/// Formats UTF-8 Stack source into its canonical representation.
pub fn format(source: &str) -> FormatOutput {
    let parsed = stack_compiler::parse(source);
    let document = match parsed.document {
        Some(document) => document,
        None => {
            return FormatOutput {
                source: None,
                diagnostics: parsed.diagnostics,
            };
        }
    };

    let lossless = stack_compiler::parse_lossless(source);
    let lossless = match lossless.document {
        Some(document) => document,
        None => {
            return FormatOutput {
                source: None,
                diagnostics: lossless.diagnostics,
            };
        }
    };

    let formatted = Formatter::new(&document, &lossless).format();
    FormatOutput {
        source: Some(formatted),
        diagnostics: stack_compiler::compile(source).diagnostics,
    }
}

/// Decodes and formats Stack source bytes into the canonical UTF-8 representation.
pub fn format_bytes(source: &[u8]) -> FormatOutput {
    match std::str::from_utf8(source) {
        Ok(source) => format(source),
        Err(_) => {
            let parsed = stack_compiler::parse_lossless_bytes(source);
            FormatOutput {
                source: None,
                diagnostics: parsed.diagnostics,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Separator {
    None,
    Space,
    Line,
    Blank,
}

struct Formatter<'source> {
    lossless: &'source LosslessDocument,
    breaks: BTreeMap<usize, Separator>,
}

impl<'source> Formatter<'source> {
    fn new(document: &ast::Document, lossless: &'source LosslessDocument) -> Self {
        Self {
            lossless,
            breaks: collect_breaks(document),
        }
    }

    fn format(&self) -> String {
        let mut writer = Writer::default();
        let mut previous = None;
        let mut comments = Vec::new();
        let mut depth = 0_usize;

        for token in self.lossless.tokens() {
            match &token.kind {
                TokenKind::Whitespace => continue,
                TokenKind::LineComment => {
                    comments.push(token);
                    continue;
                }
                TokenKind::End => {
                    emit_gap(
                        &mut writer,
                        previous,
                        token,
                        &comments,
                        Separator::None,
                        0,
                        0,
                    );
                    break;
                }
                _ => {}
            }

            if matches!(token.kind, TokenKind::RightBrace) {
                depth = depth.saturating_sub(1);
            }
            let token_indent = depth * 2;
            let comment_indent = if matches!(token.kind, TokenKind::RightBrace) {
                (depth + 1) * 2
            } else {
                token_indent
            };
            let separator = self.separator(previous, token);
            let effective_indent = emit_gap(
                &mut writer,
                previous,
                token,
                &comments,
                separator,
                token_indent,
                comment_indent,
            );
            writer.indent(effective_indent);
            writer.token(token);

            if matches!(token.kind, TokenKind::LeftBrace) {
                depth += 1;
            }
            previous = Some(token);
            comments.clear();
        }

        writer.finish()
    }

    fn separator(&self, previous: Option<&Token>, current: &Token) -> Separator {
        let Some(previous) = previous else {
            return Separator::None;
        };

        if matches!(current.kind, TokenKind::RightBrace)
            || matches!(previous.kind, TokenKind::LeftBrace)
        {
            Separator::Line
        } else if let Some(separator) = self.breaks.get(&previous.span.end.byte_offset) {
            *separator
        } else if matches!(
            current.kind,
            TokenKind::RightBracket | TokenKind::Comma | TokenKind::Dot
        ) || matches!(previous.kind, TokenKind::LeftBracket | TokenKind::Dot)
        {
            Separator::None
        } else {
            Separator::Space
        }
    }
}

fn emit_gap(
    writer: &mut Writer,
    previous: Option<&Token>,
    current: &Token,
    comments: &[&Token],
    separator: Separator,
    token_indent: usize,
    comment_indent: usize,
) -> usize {
    if comments.is_empty() {
        writer.separator(separator);
        return token_indent;
    }

    let has_trailing =
        previous.is_some_and(|previous| previous.span.end.line == comments[0].span.start.line);
    let own_start = usize::from(has_trailing);

    if has_trailing {
        writer.separator(Separator::Space);
        writer.raw(&comments[0].text);
    }

    let own_comments = &comments[own_start..];
    if own_comments.is_empty() {
        separator_after_comment(writer, current, separator);
    } else {
        let before_comments = if matches!(current.kind, TokenKind::End) && previous.is_some() {
            Separator::Blank
        } else if separator == Separator::Space {
            Separator::Line
        } else {
            separator
        };
        writer.separator(before_comments);

        let own_indent = if separator == Separator::Space {
            token_indent + 2
        } else {
            comment_indent
        };
        for comment in own_comments {
            writer.indent(own_indent);
            writer.raw(&comment.text);
            writer.separator(Separator::Line);
        }
    }

    if separator == Separator::Space && !matches!(current.kind, TokenKind::End) {
        token_indent + 2
    } else {
        token_indent
    }
}

fn separator_after_comment(writer: &mut Writer, current: &Token, separator: Separator) {
    if matches!(current.kind, TokenKind::End) || separator == Separator::Space {
        writer.separator(Separator::Line);
    } else {
        writer.separator(separator);
    }
}

fn collect_breaks(document: &ast::Document) -> BTreeMap<usize, Separator> {
    let mut breaks = BTreeMap::new();
    mark_break(
        &mut breaks,
        document.version.span.end.byte_offset,
        Separator::Blank,
    );
    for member in &document.diagram.members {
        collect_diagram_member(member, &mut breaks);
        mark_break(
            &mut breaks,
            diagram_member_span(member).end.byte_offset,
            Separator::Blank,
        );
    }
    breaks
}

fn collect_diagram_member(member: &DiagramMember, breaks: &mut BTreeMap<usize, Separator>) {
    match member {
        DiagramMember::Node(node) => collect_node(node, breaks),
        DiagramMember::Group(group) => collect_group(group, breaks),
        DiagramMember::Edge(edge) => collect_edge(edge, breaks),
        DiagramMember::Layout(layout) => collect_layout(layout, breaks),
        DiagramMember::Theme(_) => {}
    }
}

fn collect_group(group: &ast::Group, breaks: &mut BTreeMap<usize, Separator>) {
    for member in &group.members {
        match member {
            GroupMember::Node(node) => collect_node(node, breaks),
            GroupMember::Group(group) => collect_group(group, breaks),
            GroupMember::Layout(layout) => collect_layout(layout, breaks),
        }
        mark_break(
            breaks,
            group_member_span(member).end.byte_offset,
            Separator::Blank,
        );
    }
}

fn collect_node(node: &ast::Node, breaks: &mut BTreeMap<usize, Separator>) {
    for property in &node.properties {
        let span = match property {
            NodeProperty::Kind(value) | NodeProperty::Icon(value) | NodeProperty::Detail(value) => {
                value.span
            }
        };
        mark_break(breaks, span.end.byte_offset, Separator::Line);
    }
}

fn collect_edge(edge: &ast::Edge, breaks: &mut BTreeMap<usize, Separator>) {
    for property in &edge.properties {
        let span = match property {
            EdgeProperty::Kind(value) => value.span,
        };
        mark_break(breaks, span.end.byte_offset, Separator::Line);
    }
}

fn collect_layout(layout: &ast::Layout, breaks: &mut BTreeMap<usize, Separator>) {
    for statement in &layout.statements {
        let span = match statement {
            LayoutStatement::Direction(value) => value.span,
            LayoutStatement::RankSame(list) | LayoutStatement::Order(list) => list.span,
        };
        mark_break(breaks, span.end.byte_offset, Separator::Line);
    }
}

fn mark_break(breaks: &mut BTreeMap<usize, Separator>, offset: usize, separator: Separator) {
    breaks
        .entry(offset)
        .and_modify(|existing| *existing = (*existing).max(separator))
        .or_insert(separator);
}

fn diagram_member_span(member: &DiagramMember) -> stack_compiler::diagnostic::Span {
    match member {
        DiagramMember::Node(node) => node.span,
        DiagramMember::Group(group) => group.span,
        DiagramMember::Edge(edge) => edge.span,
        DiagramMember::Theme(theme) => theme.span,
        DiagramMember::Layout(layout) => layout.span,
    }
}

fn group_member_span(member: &GroupMember) -> stack_compiler::diagnostic::Span {
    match member {
        GroupMember::Node(node) => node.span,
        GroupMember::Group(group) => group.span,
        GroupMember::Layout(layout) => layout.span,
    }
}

#[derive(Default)]
struct Writer {
    output: String,
}

impl Writer {
    fn separator(&mut self, separator: Separator) {
        match separator {
            Separator::None => {}
            Separator::Space => {
                if !self.output.is_empty() && !self.output.ends_with([' ', '\n']) {
                    self.output.push(' ');
                }
            }
            Separator::Line => self.ensure_newlines(1),
            Separator::Blank => self.ensure_newlines(2),
        }
    }

    fn ensure_newlines(&mut self, count: usize) {
        let existing = self
            .output
            .as_bytes()
            .iter()
            .rev()
            .take_while(|byte| **byte == b'\n')
            .count();
        for _ in existing..count {
            self.output.push('\n');
        }
    }

    fn indent(&mut self, spaces: usize) {
        if self.output.is_empty() || self.output.ends_with('\n') {
            for _ in 0..spaces {
                self.output.push(' ');
            }
        }
    }

    fn token(&mut self, token: &Token) {
        if let TokenKind::String(value) = &token.kind {
            self.output.push('"');
            for character in value.chars() {
                match character {
                    '"' => self.output.push_str("\\\""),
                    '\\' => self.output.push_str("\\\\"),
                    _ => self.output.push(character),
                }
            }
            self.output.push('"');
        } else {
            self.raw(&token.text);
        }
    }

    fn raw(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn finish(mut self) -> String {
        while self.output.ends_with("\n\n") {
            self.output.pop();
        }
        if !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.output
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{format, format_bytes};

    #[test]
    fn formats_all_constructs_comments_and_strings() {
        let source = concat!(
            "// leading\r\n",
            "stack 1 . 0// version\r\n",
            "diagram \"\\u56F3\"{\r\n",
            "group services \"Services\"{\r\n",
            "// nested\r\n",
            "node api \"API\"{detail \"quote: \\u0022 slash: \\\\\" icon \"service\" kind service}// node\r\n",
            "node worker \"Worker\"\r\n",
            "layout {order[api,worker] direction down}\r\n",
            "}\r\n",
            "theme dark\r\n",
            "layout {order[services,client] rank same[services,client]}\r\n",
            "node client \"Client\"\r\n",
            "edge client->api \"HTTPS\"{kind request}\r\n",
            "}\r\n",
        );
        let expected = concat!(
            "// leading\n",
            "stack 1.0 // version\n",
            "\n",
            "diagram \"図\" {\n",
            "  group services \"Services\" {\n",
            "    // nested\n",
            "    node api \"API\" {\n",
            "      detail \"quote: \\\" slash: \\\\\"\n",
            "      icon \"service\"\n",
            "      kind service\n",
            "    } // node\n",
            "\n",
            "    node worker \"Worker\"\n",
            "\n",
            "    layout {\n",
            "      order [api, worker]\n",
            "      direction down\n",
            "    }\n",
            "  }\n",
            "\n",
            "  theme dark\n",
            "\n",
            "  layout {\n",
            "    order [services, client]\n",
            "    rank same [services, client]\n",
            "  }\n",
            "\n",
            "  node client \"Client\"\n",
            "\n",
            "  edge client -> api \"HTTPS\" {\n",
            "    kind request\n",
            "  }\n",
            "}\n",
        );

        let output = format(source);
        assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
        assert_eq!(output.source.as_deref(), Some(expected));
        assert_eq!(format(expected).source, Some(expected.to_owned()));
        assert_eq!(format_bytes(source.as_bytes()).source, output.source);
    }

    #[test]
    fn preserves_comment_token_gaps_that_force_continuations() {
        let source = concat!(
            "stack 1.0\n",
            "diagram \"Comments\" {\n",
            "  node // identifier\n",
            "    api \"API\"\n",
            "  // before close\n",
            "}\n",
            "\n",
            "// final\n",
        );
        let expected = concat!(
            "stack 1.0\n",
            "\n",
            "diagram \"Comments\" {\n",
            "  node // identifier\n",
            "    api \"API\"\n",
            "  // before close\n",
            "}\n",
            "\n",
            "// final\n",
        );

        assert_eq!(format(source).source.as_deref(), Some(expected));
    }

    #[test]
    fn semantic_errors_remain_formattable_with_the_same_codes() {
        let source = concat!(
            "stack 1.0 diagram \"Invalid\"{",
            "node api \"First\" node api \"Second\" ",
            "edge api->missing}",
        );
        let before = diagnostic_codes(source);
        let output = format(source);
        assert!(output.source.is_some());
        let Some(formatted) = output.source else {
            return;
        };
        assert_eq!(before, diagnostic_codes(&formatted));
        assert_eq!(
            output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<BTreeSet<_>>(),
            before
        );
    }

    #[test]
    fn rejects_lexical_syntax_and_encoding_errors_without_output() {
        for source in [
            "\u{feff}stack 1.0",
            "stack 1.0 diagram \"Incomplete\" {",
            "stack 1.0 diagram \"Bad escape\" { node api \"\\n\" }",
        ] {
            let output = format(source);
            assert!(output.source.is_none());
            assert!(!output.diagnostics.is_empty());
        }

        let encoding = format_bytes(b"stack 1.0\n\xff");
        assert!(encoding.source.is_none());
        assert_eq!(encoding.diagnostics[0].code, "STK1001");
    }

    fn diagnostic_codes(source: &str) -> BTreeSet<&'static str> {
        stack_compiler::compile(source)
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect()
    }
}
