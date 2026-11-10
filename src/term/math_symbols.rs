//! Character tables for converting LaTeX math to Unicode text.

/// The character a symbol command stands for, such as `alpha` for `α`.
pub fn symbol(name: &str) -> Option<&'static str> {
    greek(name)
        .or_else(|| operator(name))
        .or_else(|| other(name))
}

fn greek(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" => "ϵ",
        "varepsilon" => "ε",
        "zeta" => "ζ",
        "eta" => "η",
        "theta" => "θ",
        "vartheta" => "ϑ",
        "iota" => "ι",
        "kappa" => "κ",
        "lambda" => "λ",
        "mu" => "μ",
        "nu" => "ν",
        "xi" => "ξ",
        "omicron" => "ο",
        "pi" => "π",
        "varpi" => "ϖ",
        "rho" => "ρ",
        "varrho" => "ϱ",
        "sigma" => "σ",
        "varsigma" => "ς",
        "tau" => "τ",
        "upsilon" => "υ",
        "phi" => "ϕ",
        "varphi" => "φ",
        "chi" => "χ",
        "psi" => "ψ",
        "omega" => "ω",
        "Gamma" => "Γ",
        "Delta" => "Δ",
        "Theta" => "Θ",
        "Lambda" => "Λ",
        "Xi" => "Ξ",
        "Pi" => "Π",
        "Sigma" => "Σ",
        "Upsilon" => "Υ",
        "Phi" => "Φ",
        "Psi" => "Ψ",
        "Omega" => "Ω",
        _ => return None,
    })
}

fn operator(name: &str) -> Option<&'static str> {
    Some(match name {
        "times" => "×",
        "cdot" => "⋅",
        "div" => "÷",
        "pm" => "±",
        "mp" => "∓",
        "ast" => "∗",
        "star" => "⋆",
        "circ" => "∘",
        "bullet" => "∙",
        "oplus" => "⊕",
        "ominus" => "⊖",
        "otimes" => "⊗",
        "odot" => "⊙",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "sim" => "∼",
        "simeq" => "≃",
        "equiv" => "≡",
        "cong" => "≅",
        "propto" => "∝",
        "ll" => "≪",
        "gg" => "≫",
        "prec" => "≺",
        "succ" => "≻",
        "coloneqq" | "coloneq" => "≔",
        "triangleq" => "≜",
        "in" => "∈",
        "notin" => "∉",
        "ni" => "∋",
        "subset" => "⊂",
        "subseteq" => "⊆",
        "supset" => "⊃",
        "supseteq" => "⊇",
        "cup" => "∪",
        "cap" => "∩",
        "bigcup" => "⋃",
        "bigcap" => "⋂",
        "setminus" => "∖",
        "emptyset" | "varnothing" => "∅",
        "forall" => "∀",
        "exists" => "∃",
        "nexists" => "∄",
        "neg" | "lnot" => "¬",
        "land" | "wedge" => "∧",
        "lor" | "vee" => "∨",
        "to" | "rightarrow" => "→",
        "gets" | "leftarrow" => "←",
        "leftrightarrow" => "↔",
        "Rightarrow" => "⇒",
        "Leftarrow" => "⇐",
        "Leftrightarrow" => "⇔",
        "implies" | "Longrightarrow" => "⟹",
        "impliedby" | "Longleftarrow" => "⟸",
        "iff" | "Longleftrightarrow" => "⟺",
        "longrightarrow" => "⟶",
        "longleftarrow" => "⟵",
        "mapsto" => "↦",
        "uparrow" => "↑",
        "downarrow" => "↓",
        "sum" => "∑",
        "prod" => "∏",
        "coprod" => "∐",
        "int" => "∫",
        "iint" => "∬",
        "iiint" => "∭",
        "oint" => "∮",
        _ => return None,
    })
}

fn other(name: &str) -> Option<&'static str> {
    Some(match name {
        "infty" => "∞",
        "partial" => "∂",
        "nabla" => "∇",
        "ldots" | "dots" | "dotsc" | "dotsb" => "…",
        "cdots" => "⋯",
        "vdots" => "⋮",
        "ddots" => "⋱",
        "prime" => "′",
        "angle" => "∠",
        "perp" | "bot" => "⊥",
        "top" => "⊤",
        "parallel" => "∥",
        "mid" => "∣",
        "ell" => "ℓ",
        "hbar" => "ℏ",
        "Re" => "ℜ",
        "Im" => "ℑ",
        "aleph" => "ℵ",
        "triangle" => "△",
        "square" | "Box" => "□",
        "checkmark" => "✓",
        "dagger" => "†",
        "ddagger" => "‡",
        "S" => "§",
        "degree" => "°",
        "vert" | "lvert" | "rvert" => "|",
        "Vert" | "lVert" | "rVert" => "‖",
        "langle" => "⟨",
        "rangle" => "⟩",
        "lceil" => "⌈",
        "rceil" => "⌉",
        "lfloor" => "⌊",
        "rfloor" => "⌋",
        "lbrace" => "{",
        "rbrace" => "}",
        "lbrack" => "[",
        "rbrack" => "]",
        "backslash" => "\\",
        _ => return None,
    })
}

/// Relations, binary operators and arrows, which take spaces around them.
pub fn is_operator(name: &str) -> bool {
    operator(name).is_some()
}

/// Delimiters that open or close a pair: no space after an opener or
/// before a closer.
pub fn opens(name: &str) -> bool {
    matches!(name, "langle" | "lceil" | "lfloor" | "lvert" | "lVert")
}

pub fn closes(name: &str) -> bool {
    matches!(name, "rangle" | "rceil" | "rfloor" | "rvert" | "rVert")
}

/// Operator names printed upright as words.
pub fn is_function(name: &str) -> bool {
    matches!(
        name,
        "sin"
            | "cos"
            | "tan"
            | "cot"
            | "sec"
            | "csc"
            | "arcsin"
            | "arccos"
            | "arctan"
            | "sinh"
            | "cosh"
            | "tanh"
            | "log"
            | "ln"
            | "lg"
            | "exp"
            | "max"
            | "min"
            | "sup"
            | "inf"
            | "arg"
            | "lim"
            | "limsup"
            | "liminf"
            | "det"
            | "dim"
            | "ker"
            | "deg"
            | "gcd"
            | "hom"
            | "Pr"
    )
}

/// Combining mark an accent command puts over its argument.
pub fn accent(name: &str) -> Option<char> {
    Some(match name {
        "hat" | "widehat" => '\u{302}',
        "tilde" | "widetilde" => '\u{303}',
        "bar" => '\u{304}',
        "overline" => '\u{305}',
        "breve" => '\u{306}',
        "dot" => '\u{307}',
        "ddot" => '\u{308}',
        "check" => '\u{30c}',
        "acute" => '\u{301}',
        "grave" => '\u{300}',
        "vec" | "overrightarrow" => '\u{20d7}',
        "underline" => '\u{332}',
        _ => return None,
    })
}

pub fn is_combining(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36f}' | '\u{20d0}'..='\u{20ff}')
}

pub fn superscript(c: char) -> Option<char> {
    let digits = "⁰¹²³⁴⁵⁶⁷⁸⁹";
    if let Some(d) = c.to_digit(10) {
        return digits.chars().nth(d as usize);
    }
    Some(match c {
        '+' => '⁺',
        '-' | '−' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'a' => 'ᵃ',
        'b' => 'ᵇ',
        'c' => 'ᶜ',
        'd' => 'ᵈ',
        'e' => 'ᵉ',
        'f' => 'ᶠ',
        'g' => 'ᵍ',
        'h' => 'ʰ',
        'i' => 'ⁱ',
        'j' => 'ʲ',
        'k' => 'ᵏ',
        'l' => 'ˡ',
        'm' => 'ᵐ',
        'n' => 'ⁿ',
        'o' => 'ᵒ',
        'p' => 'ᵖ',
        'r' => 'ʳ',
        's' => 'ˢ',
        't' => 'ᵗ',
        'u' => 'ᵘ',
        'v' => 'ᵛ',
        'w' => 'ʷ',
        'x' => 'ˣ',
        'y' => 'ʸ',
        'z' => 'ᶻ',
        'A' => 'ᴬ',
        'B' => 'ᴮ',
        'D' => 'ᴰ',
        'E' => 'ᴱ',
        'G' => 'ᴳ',
        'H' => 'ᴴ',
        'I' => 'ᴵ',
        'J' => 'ᴶ',
        'K' => 'ᴷ',
        'L' => 'ᴸ',
        'M' => 'ᴹ',
        'N' => 'ᴺ',
        'O' => 'ᴼ',
        'P' => 'ᴾ',
        'R' => 'ᴿ',
        'T' | '⊤' => 'ᵀ',
        'U' => 'ᵁ',
        'V' => 'ⱽ',
        'W' => 'ᵂ',
        'β' => 'ᵝ',
        'γ' => 'ᵞ',
        'δ' => 'ᵟ',
        'θ' => 'ᶿ',
        'φ' | 'ϕ' => 'ᵠ',
        'χ' => 'ᵡ',
        '∘' => '°',
        '′' => '′',
        '*' | '∗' => '*',
        _ => return None,
    })
}

pub fn subscript(c: char) -> Option<char> {
    let digits = "₀₁₂₃₄₅₆₇₈₉";
    if let Some(d) = c.to_digit(10) {
        return digits.chars().nth(d as usize);
    }
    Some(match c {
        '+' => '₊',
        '-' | '−' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'a' => 'ₐ',
        'e' => 'ₑ',
        'h' => 'ₕ',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'k' => 'ₖ',
        'l' => 'ₗ',
        'm' => 'ₘ',
        'n' => 'ₙ',
        'o' => 'ₒ',
        'p' => 'ₚ',
        'r' => 'ᵣ',
        's' => 'ₛ',
        't' => 'ₜ',
        'u' => 'ᵤ',
        'v' => 'ᵥ',
        'x' => 'ₓ',
        'β' => 'ᵦ',
        'γ' => 'ᵧ',
        'ρ' => 'ᵨ',
        'φ' | 'ϕ' => 'ᵩ',
        'χ' => 'ᵪ',
        _ => return None,
    })
}

/// `\mathbb` letters and digits.
pub fn double_struck(c: char) -> Option<char> {
    let special = match c {
        'C' => Some('ℂ'),
        'H' => Some('ℍ'),
        'N' => Some('ℕ'),
        'P' => Some('ℙ'),
        'Q' => Some('ℚ'),
        'R' => Some('ℝ'),
        'Z' => Some('ℤ'),
        _ => None,
    };
    special.or_else(|| offset(c, 0x1d538, 0x1d552, Some(0x1d7d8)))
}

/// `\mathcal` and `\mathscr` letters.
pub fn script(c: char) -> Option<char> {
    let special = match c {
        'B' => Some('ℬ'),
        'E' => Some('ℰ'),
        'F' => Some('ℱ'),
        'H' => Some('ℋ'),
        'I' => Some('ℐ'),
        'L' => Some('ℒ'),
        'M' => Some('ℳ'),
        'R' => Some('ℛ'),
        'e' => Some('ℯ'),
        'g' => Some('ℊ'),
        'o' => Some('ℴ'),
        _ => None,
    };
    special.or_else(|| offset(c, 0x1d49c, 0x1d4b6, None))
}

/// A letter or digit in a Mathematical Alphanumeric Symbols alphabet.
fn offset(c: char, upper: u32, lower: u32, digits: Option<u32>) -> Option<char> {
    let base = match c {
        'A'..='Z' => upper + (c as u32 - 'A' as u32),
        'a'..='z' => lower + (c as u32 - 'a' as u32),
        '0'..='9' => digits? + (c as u32 - '0' as u32),
        _ => return None,
    };
    char::from_u32(base)
}
