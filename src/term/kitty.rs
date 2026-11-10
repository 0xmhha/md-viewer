//! Kitty graphics protocol: escape sequences that draw a PNG in place.

/// Base64 payload bytes per escape sequence; the protocol caps a chunk
/// at 4096.
const CHUNK: usize = 4096;

/// Escape sequences that draw `png` with its top-left corner at the
/// cursor. The cursor does not move (`C=1`) and the terminal sends no
/// reply (`q=2`), so nothing is left for the shell to read.
pub fn place(png: &[u8]) -> String {
    const HEAD: &str = "a=T,f=100,t=d,q=2,C=1";
    let payload = base64(png);
    // Base64 is ASCII, so splitting by bytes keeps characters whole.
    let chunks: Vec<&[u8]> = payload.as_bytes().chunks(CHUNK).collect();
    let mut out = String::with_capacity(payload.len() + chunks.len() * 16);
    let last = chunks.len().saturating_sub(1);
    for (i, chunk) in chunks.iter().enumerate() {
        let chunk = std::str::from_utf8(chunk).unwrap_or_default();
        let keys = match (i, chunks.len()) {
            (_, 1) => HEAD.to_owned(),
            (0, _) => format!("{HEAD},m=1"),
            (i, _) if i == last => "m=0".to_owned(),
            _ => "m=1".to_owned(),
        };
        out.push_str(&format!("\x1b_G{keys};{chunk}\x1b\\"));
    }
    if chunks.is_empty() {
        out.push_str(&format!("\x1b_G{HEAD};\x1b\\"));
    }
    out
}

fn base64(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for group in data.chunks(3) {
        let b = [
            group[0],
            *group.get(1).unwrap_or(&0),
            *group.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= group.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc_4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ];
        for (input, want) in cases {
            assert_eq!(base64(input.as_bytes()), want, "input {input:?}");
        }
        assert_eq!(base64(&[0xff, 0xfe, 0xfd]), "//79");
    }

    #[test]
    fn small_image_is_one_sequence() {
        let out = place(b"foo");
        assert_eq!(out, "\x1b_Ga=T,f=100,t=d,q=2,C=1;Zm9v\x1b\\");
    }

    #[test]
    fn large_image_is_split_into_chunks() {
        let data = vec![7u8; 7000]; // 9336 base64 bytes: three chunks
        let out = place(&data);
        let parts: Vec<&str> = out.split("\x1b\\").filter(|p| !p.is_empty()).collect();
        assert_eq!(parts.len(), 3, "{parts:?}");
        assert!(parts[0].starts_with("\x1b_Ga=T,f=100,t=d,q=2,C=1,m=1;"));
        assert!(parts[1].starts_with("\x1b_Gm=1;"));
        assert!(parts[2].starts_with("\x1b_Gm=0;"));
        let payload: String = parts
            .iter()
            .map(|p| p.split_once(';').expect("payload").1)
            .collect();
        assert_eq!(payload, base64(&data));
        assert!(
            parts
                .iter()
                .all(|p| p.split_once(';').unwrap().1.len() <= CHUNK)
        );
    }
}
