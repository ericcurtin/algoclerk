// SPDX-License-Identifier: Apache-2.0
//! Syntax colours: maps the core's C++ highlighter classes ([`HlKind`]) to palette letters and splits a
//! text into per-line coloured spans for the editor, the brief and the code reveal.

use algoclerk_core::cpp::lexer::{HlKind, highlight};

/// Palette letter of a token class on a light (paper) background.
pub fn light(k: HlKind) -> char {
    match k {
        HlKind::Keyword => 'b',
        HlKind::Type => 'u',
        HlKind::StdName => 'G',
        HlKind::Number => 'o',
        HlKind::Str | HlKind::Char => 'r',
        HlKind::Comment => 'a',
        HlKind::Preproc => 'm',
        HlKind::Punct => 'k',
        HlKind::Ident => 'K',
    }
}

/// Palette letter of a token class on a dark background.
pub fn dark(k: HlKind) -> char {
    match k {
        HlKind::Keyword => 'o',
        HlKind::Type => 'c',
        HlKind::StdName => 's',
        HlKind::Number => 'y',
        HlKind::Str | HlKind::Char => 'l',
        HlKind::Comment => 'a',
        HlKind::Preproc => 'p',
        HlKind::Punct => 'A',
        HlKind::Ident => 'W',
    }
}

/// A coloured piece of one line.
pub type Piece = (String, char);

/// Split `text` into lines of coloured pieces that cover every character of the line (gaps get
/// `plain`). `colour` picks the letter of a class.
pub fn lines(text: &str, colour: fn(HlKind) -> char, plain: char) -> Vec<Vec<Piece>> {
    let spans = highlight(text);
    let mut out: Vec<Vec<Piece>> = Vec::new();
    let mut si = 0usize;
    let mut start = 0usize;
    for line in text.split('\n') {
        let end = start + line.len();
        let mut pieces: Vec<Piece> = Vec::new();
        let mut at = start;
        while si < spans.len() && (spans[si].0.end as usize) <= start {
            si += 1;
        }
        let mut sj = si;
        while sj < spans.len() && (spans[sj].0.start as usize) < end {
            let (sp, kind) = spans[sj];
            let a = (sp.start as usize).max(start);
            let b = (sp.end as usize).min(end);
            if a > at {
                pieces.push((text[at..a].to_string(), plain));
            }
            if b > a {
                pieces.push((text[a..b].to_string(), colour(kind)));
            }
            at = b.max(at);
            sj += 1;
        }
        if at < end {
            pieces.push((text[at..end].to_string(), plain));
        }
        out.push(pieces);
        start = end + 1;
    }
    out
}

/// Total characters of a line of pieces.
pub fn width(line: &[Piece]) -> usize {
    line.iter().map(|p| p.0.chars().count()).sum()
}

/// Soft-wrap one coloured line to `cols` columns. Continuation rows are indented by the line's own
/// indentation plus two spaces (at most half the width). Breaks go after a space or a comma when one is
/// close, otherwise in the middle of a word, so no character is ever dropped.
pub fn wrap_line(line: &[Piece], cols: usize) -> Vec<Vec<Piece>> {
    let cols = cols.max(4);
    let chars: Vec<(char, char)> = line
        .iter()
        .flat_map(|(t, c)| t.chars().map(move |ch| (ch, *c)))
        .collect();
    if chars.len() <= cols {
        return vec![line.to_vec()];
    }
    let lead = chars.iter().take_while(|(c, _)| *c == ' ').count();
    let indent = (lead + 2).min(cols / 2);
    let mut rows: Vec<Vec<(char, char)>> = Vec::new();
    let mut at = 0usize;
    let mut first = true;
    while at < chars.len() {
        let room = if first { cols } else { cols - indent };
        if chars.len() - at <= room {
            rows.push(chars[at..].to_vec());
            break;
        }
        let window = &chars[at..at + room];
        let cut = window
            .iter()
            .rposition(|(c, _)| *c == ' ' || *c == ',')
            .filter(|i| *i + 1 > room / 3)
            .map_or(room, |i| i + 1);
        rows.push(chars[at..at + cut].to_vec());
        at += cut;
        first = false;
    }
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let mut pieces: Vec<Piece> = Vec::new();
            if i > 0 {
                pieces.push((" ".repeat(indent), 'K'));
            }
            for (ch, c) in r {
                match pieces.last_mut() {
                    Some(last) if last.1 == *c => last.0.push(*ch),
                    _ => pieces.push((ch.to_string(), *c)),
                }
            }
            pieces
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pieces_cover_every_line_exactly() {
        let src = "#include <vector>\nint f(int a) { // hi\n    return a + 1; /* x\n y */ }\n";
        let ls = lines(src, light, 'K');
        for (i, l) in src.split('\n').enumerate() {
            let joined: String = ls[i].iter().map(|p| p.0.as_str()).collect();
            assert_eq!(joined, l, "line {i}");
        }
    }

    #[test]
    fn keywords_and_numbers_get_colours() {
        let ls = lines("return 42;", light, 'K');
        assert!(ls[0].iter().any(|p| p.0 == "return" && p.1 == 'b'));
        assert!(ls[0].iter().any(|p| p.0 == "42" && p.1 == 'o'));
    }
    #[test]
    fn wrapping_keeps_every_character() {
        let src = "    std::unordered_map<int, std::vector<int>> seen = buildTheIndexOfEverything(nums, target);";
        for cols in [8usize, 13, 20, 40] {
            let ls = lines(src, light, 'K');
            let rows = wrap_line(&ls[0], cols);
            assert!(rows.iter().all(|r| width(r) <= cols), "{cols}");
            let joined: String = rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let t: String = r.iter().map(|p| p.0.as_str()).collect();
                    if i == 0 {
                        t
                    } else {
                        t.trim_start().to_string()
                    }
                })
                .collect();
            assert_eq!(joined.replace(' ', ""), src.replace(' ', ""), "cols {cols}");
        }
        let short = lines("int a;", light, 'K');
        assert_eq!(wrap_line(&short[0], 20).len(), 1);
    }
}
