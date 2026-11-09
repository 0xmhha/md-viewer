//! ANSI styles. Colors use the terminal's 16-color palette so the output
//! follows the user's light or dark theme.

/// One of the 16 palette colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    BrightBlack,
}

impl Color {
    fn index(self) -> u8 {
        match self {
            Color::Black => 0,
            Color::Red => 1,
            Color::Green => 2,
            Color::Yellow => 3,
            Color::Blue => 4,
            Color::Magenta => 5,
            Color::Cyan => 6,
            Color::BrightBlack => 60,
        }
    }
}

/// Text attributes for one span. `Default` is unstyled text.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Style {
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub fg: Option<Color>,
    pub bg: Option<Color>,
    /// Target of an OSC 8 hyperlink.
    pub link: Option<String>,
}

impl Style {
    pub fn fg(color: Color) -> Self {
        Style {
            fg: Some(color),
            ..Style::default()
        }
    }

    pub fn bold(self) -> Self {
        Style { bold: true, ..self }
    }

    pub fn dim(self) -> Self {
        Style { dim: true, ..self }
    }

    pub fn italic(self) -> Self {
        Style {
            italic: true,
            ..self
        }
    }

    pub fn underline(self) -> Self {
        Style {
            underline: true,
            ..self
        }
    }

    pub fn strike(self) -> Self {
        Style {
            strike: true,
            ..self
        }
    }

    pub fn with_fg(self, color: Color) -> Self {
        Style {
            fg: Some(color),
            ..self
        }
    }

    pub fn with_bg(self, color: Color) -> Self {
        Style {
            bg: Some(color),
            ..self
        }
    }

    pub fn with_link(self, url: &str) -> Self {
        Style {
            link: Some(url.to_owned()),
            ..self
        }
    }

    fn sgr(&self) -> String {
        let mut codes: Vec<String> = [
            (self.bold, 1),
            (self.dim, 2),
            (self.italic, 3),
            (self.underline, 4),
            (self.strike, 9),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .map(|(_, code)| code.to_string())
        .collect();
        if let Some(c) = self.fg {
            codes.push((30 + c.index()).to_string());
        }
        if let Some(c) = self.bg {
            codes.push((40 + c.index()).to_string());
        }
        if codes.is_empty() {
            String::new()
        } else {
            format!("\x1b[{}m", codes.join(";"))
        }
    }
}

/// Wrap `text` in the escape sequences for `style`.
/// Control characters are replaced first, so a document cannot send its
/// own escape sequences to the terminal.
pub fn paint(text: &str, style: &Style, color: bool) -> String {
    let text = sanitize(text);
    if !color || text.is_empty() {
        return text;
    }
    let sgr = style.sgr();
    let body = if sgr.is_empty() {
        text.to_owned()
    } else {
        format!("{sgr}{text}\x1b[0m")
    };
    match &style.link {
        Some(url) => {
            let url = sanitize(url);
            format!("\x1b]8;;{url}\x1b\\{body}\x1b]8;;\x1b\\")
        }
        None => body,
    }
}

/// Replace what must not reach the terminal: control characters (escape
/// sequences) and bidirectional overrides (text that reads differently
/// from what it is). Tabs become spaces so widths stay countable.
pub fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '\n' => '\n',
            '\t' => ' ',
            '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' => char::REPLACEMENT_CHARACTER,
            c if c.is_control() => char::REPLACEMENT_CHARACTER,
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_mode_returns_text_unchanged() {
        assert_eq!(paint("x", &Style::fg(Color::Red).bold(), false), "x");
    }

    #[test]
    fn combines_attributes_into_one_sgr() {
        let s = Style::fg(Color::Black).bold().with_bg(Color::Yellow);
        assert_eq!(paint("x", &s, true), "\x1b[1;30;43mx\x1b[0m");
    }

    #[test]
    fn bright_black_uses_the_bright_range() {
        assert_eq!(
            paint("x", &Style::fg(Color::BrightBlack), true),
            "\x1b[90mx\x1b[0m"
        );
    }

    #[test]
    fn unstyled_text_has_no_escapes() {
        assert_eq!(paint("x", &Style::default(), true), "x");
    }

    #[test]
    fn control_characters_cannot_reach_the_terminal() {
        let out = paint("a\x1b]0;pwn\x07b", &Style::default().bold(), true);
        assert_eq!(out, "\x1b[1ma\u{fffd}]0;pwn\u{fffd}b\x1b[0m");
        assert_eq!(paint("\x1b[31m", &Style::default(), false), "\u{fffd}[31m");
    }

    #[test]
    fn link_wraps_in_osc8() {
        let s = Style::default().with_link("https://a");
        assert_eq!(
            paint("t", &s, true),
            "\x1b]8;;https://a\x1b\\t\x1b]8;;\x1b\\"
        );
    }
}
