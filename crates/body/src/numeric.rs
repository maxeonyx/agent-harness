//! Whether a table cell is a number: what sets its column right-aligned.

/// A number with what numbers carry: units, currency, signs, percentages,
/// ratios ("8.2 s", "14/14", "+612 −40", "$3", "72%"). Every word is a
/// number or a short unit after one.
pub fn is(cell: &str) -> bool {
    let mut words = cell.split_whitespace();
    let Some(first) = words.next() else {
        return false;
    };
    number(first) && words.all(|w| number(w) || unit(w))
}

fn number(word: &str) -> bool {
    let word = word.trim_start_matches(['+', '-', '−', '±', '~', '≈', '$', '€', '£', '¥']);
    let digits = word
        .find(|c: char| !(c.is_ascii_digit() || ".,/:".contains(c)))
        .unwrap_or(word.len());
    word.starts_with(|c: char| c.is_ascii_digit()) && unit(&word[digits..])
}

fn unit(rest: &str) -> bool {
    let rest = rest.trim_end_matches(['%', '×', 'x']);
    rest.chars().count() <= 3 && rest.chars().all(char::is_alphabetic)
}
