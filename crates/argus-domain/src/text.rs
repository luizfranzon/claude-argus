/// Maps a Latin diacritic to its plain ASCII base letter (case preserved),
/// e.g. `ã` -> `a`, `Ç` -> `C`. Covers the vowels plus `c`/`n`/`y`, the
/// consonants that commonly carry a diacritic in Portuguese and other Latin
/// European languages. Characters outside this table (including plain ASCII
/// letters) pass through unchanged.
fn strip_diacritic(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => 'A',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => 'E',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => 'I',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => 'O',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => 'U',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => 'C',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' => 'n',
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => 'N',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'Ý' | 'Ÿ' | 'Ŷ' => 'Y',
        other => other,
    }
}

/// Replaces every accented Latin character in `text` with its plain ASCII
/// base letter, preserving length and character positions 1:1 — every input
/// char maps to exactly one output char, so indices computed against the
/// stripped string stay valid against the original.
///
/// Used to make search (file-name fuzzy matching, content grep) accent
/// insensitive in both directions: a query with or without diacritics
/// matches text with or without diacritics.
pub fn strip_diacritics(text: &str) -> String {
    text.chars().map(strip_diacritic).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_lower_and_upper_accents() {
        assert_eq!(strip_diacritics("não"), "nao");
        assert_eq!(strip_diacritics("NÃO"), "NAO");
        assert_eq!(strip_diacritics("café"), "cafe");
    }

    #[test]
    fn leaves_plain_ascii_and_length_unchanged() {
        let input = "hello_world.rs";
        assert_eq!(strip_diacritics(input), input);
        assert_eq!(strip_diacritics("não").chars().count(), "não".chars().count());
    }
}
