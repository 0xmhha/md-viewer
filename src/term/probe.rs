//! Ask the terminal whether it can draw images, how big a cell is and
//! what color text is. The answers decide whether display math is drawn.

use crate::math_image::Rgb;

/// What drawing images in this terminal needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Graphics {
    /// Size of one character cell in the pixels images are drawn in.
    pub cell_width: u32,
    pub cell_height: u32,
    /// Text color, used as the ink of math images.
    pub foreground: Rgb,
    /// Background color, if the terminal said. Diagrams put it under
    /// their labels.
    pub background: Option<Rgb>,
    /// Rows on the screen; a taller image could not be placed.
    pub screen_rows: u16,
}

/// Queries sent together: a kitty graphics support query, the text and
/// background colors (OSC 10, 11), the cell size in pixels (CSI 16 t),
/// and Primary Device Attributes last. Every terminal answers the last one, so its reply
/// marks the end of the answers without waiting for a timeout.
const QUERIES: &str = concat!(
    "\x1b_Gi=31,s=1,v=1,a=q,t=d,f=24;AAAA\x1b\\",
    "\x1b]10;?\x1b\\",
    "\x1b]11;?\x1b\\",
    "\x1b[16t",
    "\x1b[c",
);

/// What the terminal said in reply to `QUERIES`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Replies {
    kitty: bool,
    foreground: Option<Rgb>,
    background: Option<Rgb>,
    cell: Option<(u32, u32)>,
    /// The device attributes reply came: no more answers will follow.
    done: bool,
}

fn parse_replies(bytes: &[u8]) -> Replies {
    let text = String::from_utf8_lossy(bytes);
    Replies {
        kitty: text.contains("\x1b_Gi=31;OK"),
        foreground: text.split("\x1b]10;rgb:").nth(1).and_then(parse_osc_color),
        background: text.split("\x1b]11;rgb:").nth(1).and_then(parse_osc_color),
        cell: text.split("\x1b[6;").nth(1).and_then(parse_cell),
        done: device_attributes_seen(&text),
    }
}

/// `rrrr/gggg/bbbb` with 1 to 4 hex digits per channel, as xterm sends.
fn parse_osc_color(rest: &str) -> Option<Rgb> {
    let end = rest.find(['\x1b', '\x07']).unwrap_or(rest.len());
    let channels: Vec<u8> = rest[..end]
        .split('/')
        .map(|hex| {
            let value = u32::from_str_radix(hex, 16).ok()?;
            let max = 16u32.checked_pow(u32::try_from(hex.len()).ok()?)? - 1;
            (!hex.is_empty() && hex.len() <= 4)
                .then(|| u8::try_from(value * 255 / max).ok())
                .flatten()
        })
        .collect::<Option<_>>()?;
    match channels[..] {
        [r, g, b] => Some(Rgb(r, g, b)),
        _ => None,
    }
}

/// `height;widtht`, the rest of a `CSI 6 ; height ; width t` reply.
fn parse_cell(rest: &str) -> Option<(u32, u32)> {
    let (sizes, _) = rest.split_once('t')?;
    let (height, width) = sizes.split_once(';')?;
    let (height, width) = (height.parse().ok()?, width.parse().ok()?);
    (height > 0 && width > 0).then_some((width, height))
}

/// A `CSI ? ... c` reply.
fn device_attributes_seen(text: &str) -> bool {
    text.split("\x1b[?").skip(1).any(|rest| {
        rest.find(|c: char| !(c.is_ascii_digit() || c == ';'))
            .is_some_and(|i| rest[i..].starts_with('c'))
    })
}

/// Terminals where the queries are known not to work: no terminal, or a
/// multiplexer that would need its own passthrough wrapping.
fn queries_unsupported(term: Option<&str>, multiplexed: bool) -> bool {
    multiplexed || matches!(term, None | Some("" | "dumb" | "linux"))
}

/// Probe the controlling terminal. `None` when it cannot draw images or
/// does not say what color its text is.
pub fn detect() -> Option<Graphics> {
    let term = std::env::var("TERM").ok();
    let multiplexed = std::env::var_os("TMUX").is_some() || std::env::var_os("STY").is_some();
    if queries_unsupported(term.as_deref(), multiplexed) {
        return None;
    }
    let remote =
        std::env::var_os("SSH_TTY").is_some() || std::env::var_os("SSH_CONNECTION").is_some();
    platform::detect(reply_timeout(remote))
}

/// How long to wait for the terminal's answers. Over SSH they make a round
/// trip across the network, so the wait is longer there.
fn reply_timeout(remote: bool) -> std::time::Duration {
    std::time::Duration::from_millis(if remote { 1000 } else { 200 })
}

#[cfg(unix)]
mod platform {
    use std::time::{Duration, Instant};

    use rustix::fd::OwnedFd;
    use rustix::fs::{Mode, OFlags, open};
    use rustix::termios::{
        LocalModes, OptionalActions, QueueSelector, SpecialCodeIndex, Termios, tcflush, tcgetattr,
        tcgetpgrp, tcgetwinsize, tcsetattr,
    };

    use super::{Graphics, QUERIES, Replies, parse_replies};

    /// Restores the terminal mode when dropped, even on an early return.
    struct ModeGuard<'a> {
        tty: &'a OwnedFd,
        saved: Termios,
    }

    impl Drop for ModeGuard<'_> {
        fn drop(&mut self) {
            let _ = tcsetattr(self.tty, OptionalActions::Now, &self.saved);
        }
    }

    pub fn detect(timeout: Duration) -> Option<Graphics> {
        let tty = open(
            "/dev/tty",
            OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .ok()?;
        // A background job (`mdv x.md &`) that changes the terminal mode is
        // stopped by SIGTTOU, and its replies would go to whoever is in front.
        if tcgetpgrp(&tty).ok()? != rustix::process::getpgrp() {
            return None;
        }
        let replies = ask(&tty, timeout)?;
        let size = tcgetwinsize(&tty).ok()?;
        let (cell_width, cell_height) = replies.cell.or_else(|| {
            let (cols, rows) = (u32::from(size.ws_col), u32::from(size.ws_row));
            (cols > 0 && rows > 0 && size.ws_xpixel > 0 && size.ws_ypixel > 0).then(|| {
                (
                    u32::from(size.ws_xpixel) / cols,
                    u32::from(size.ws_ypixel) / rows,
                )
            })
        })?;
        (replies.kitty && cell_width > 0 && cell_height > 0).then_some(Graphics {
            cell_width,
            cell_height,
            foreground: replies.foreground?,
            background: replies.background,
            screen_rows: size.ws_row,
        })
    }

    /// Send the queries with echo, line buffering and signal keys off, and
    /// read the answers until the device attributes reply or the timeout.
    /// Ctrl-C while waiting is read as input instead of killing `mdv` before
    /// it restores the terminal mode.
    fn ask(tty: &OwnedFd, timeout: Duration) -> Option<Replies> {
        let saved = tcgetattr(tty).ok()?;
        let mut quiet = saved.clone();
        quiet
            .local_modes
            .remove(LocalModes::ICANON | LocalModes::ECHO | LocalModes::ISIG);
        // A read returns what has arrived, or nothing after 0.1 s. `poll`
        // would be simpler, but macOS does not support it on /dev/tty.
        quiet.special_codes[SpecialCodeIndex::VMIN] = 0;
        quiet.special_codes[SpecialCodeIndex::VTIME] = 1;
        tcsetattr(tty, OptionalActions::Now, &quiet).ok()?;
        let _guard = ModeGuard { tty, saved };
        // Keys typed before start-up are waiting for the shell; reading the
        // replies would swallow them. They are counted only now, because in
        // line mode an unfinished line does not count as input yet.
        if rustix::io::ioctl_fionread(tty).ok()? > 0 {
            return None;
        }

        write_all(tty, QUERIES.as_bytes())?;
        let deadline = Instant::now() + timeout;
        let mut received = Vec::new();
        let mut buf = [0u8; 256];
        while Instant::now() < deadline {
            let n = rustix::io::read(tty, &mut buf).ok()?;
            received.extend_from_slice(&buf[..n]);
            if n > 0 && parse_replies(&received).done {
                return Some(parse_replies(&received));
            }
        }
        // Answers that are still on the way would reach the shell once echo
        // is back on. Drop what has come so far; a reply arriving after this
        // can still leak, which only a much longer wait would prevent.
        let _ = tcflush(tty, QueueSelector::IFlush);
        Some(parse_replies(&received))
    }

    fn write_all(tty: &OwnedFd, mut bytes: &[u8]) -> Option<()> {
        while !bytes.is_empty() {
            let n = rustix::io::write(tty, bytes).ok()?;
            bytes = &bytes[n..];
        }
        Some(())
    }
}

#[cfg(not(unix))]
mod platform {
    pub fn detect(_timeout: std::time::Duration) -> Option<super::Graphics> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GHOSTTY: &[u8] =
        b"\x1b_Gi=31;OK\x1b\\\x1b]10;rgb:cdcd/d6d6/f4f4\x1b\\\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\\x1b[6;21;8t\x1b[?62;22c";

    #[test]
    fn parses_a_full_set_of_replies() {
        assert_eq!(
            parse_replies(GHOSTTY),
            Replies {
                kitty: true,
                foreground: Some(Rgb(0xcd, 0xd6, 0xf4)),
                background: Some(Rgb(0x1e, 0x1e, 0x2e)),
                cell: Some((8, 21)),
                done: true,
            }
        );
    }

    #[test]
    fn a_terminal_without_graphics_answers_only_some_queries() {
        let replies = parse_replies(b"\x1b]10;rgb:ff/ff/ff\x07\x1b[?1;2c");
        assert!(!replies.kitty);
        assert_eq!(replies.foreground, Some(Rgb(255, 255, 255)));
        assert_eq!(replies.cell, None);
        assert!(replies.done);
    }

    #[test]
    fn partial_input_is_not_done() {
        let half = &GHOSTTY[..GHOSTTY.len() - 3];
        assert!(!parse_replies(half).done);
        assert!(!parse_replies(b"\x1b[?62;2").done);
    }

    #[test]
    fn color_channels_of_any_width_scale_to_eight_bits() {
        assert_eq!(parse_osc_color("f/8/0\x1b\\"), Some(Rgb(255, 136, 0)));
        assert_eq!(parse_osc_color("ffff/0000/8080"), Some(Rgb(255, 0, 128)));
        assert_eq!(parse_osc_color("zz/00/00"), None);
        assert_eq!(parse_osc_color("ff/00"), None);
        assert_eq!(parse_osc_color("fffff/0/0"), None);
    }

    #[test]
    fn zero_cell_size_is_rejected() {
        assert_eq!(parse_cell("0;8t"), None);
        assert_eq!(parse_cell("21;8t"), Some((8, 21)));
    }

    #[test]
    fn remote_sessions_wait_longer() {
        assert!(reply_timeout(true) > reply_timeout(false));
    }

    #[test]
    fn multiplexers_and_dumb_terminals_are_not_probed() {
        assert!(queries_unsupported(Some("xterm-256color"), true));
        assert!(queries_unsupported(Some("dumb"), false));
        assert!(queries_unsupported(None, false));
        assert!(!queries_unsupported(Some("xterm-ghostty"), false));
    }
}
