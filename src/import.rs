//! Loading the participant list from .txt / .csv.

/// UTF-8 (with or without BOM), UTF-16 LE/BE with BOM, otherwise Windows-1251 (what Excel saves on Russian Windows).
pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&units);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        return String::from_utf16_lossy(&units);
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => bytes.iter().map(|&b| cp1251(b)).collect(),
    }
}

fn cp1251(b: u8) -> char {
    match b {
        0x00..=0x7F => b as char,
        0xC0..=0xFF => char::from_u32(0x0410 + (b as u32 - 0xC0)).unwrap_or('?'),
        0xA8 => 'Ё',
        0xB8 => 'ё',
        0xA0 => ' ',
        0x96 => '–',
        0x97 => '—',
        0xAB => '«',
        0xBB => '»',
        0xB9 => '№',
        0x93 | 0x94 => '"',
        0x91 | 0x92 => '\'',
        0x85 => '…',
        0xAA => 'Є',
        0xBA => 'є',
        0xAF => 'Ї',
        0xBF => 'ї',
        0xA5 => 'Ґ',
        0xB4 => 'ґ',
        0xB2 => 'І',
        0xB3 => 'і',
        _ => '?',
    }
}

/// In .txt the whole line, in .csv the first non-empty field of the line.
pub fn parse(text: &str, csv: bool) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let value = if csv {
                let sep = [';', '\t', ','].into_iter().find(|&c| line.contains(c));
                match sep {
                    Some(sep) => line.split(sep).map(|f| f.trim().trim_matches('"').trim()).find(|f| !f.is_empty())?,
                    None => line.trim_matches('"').trim(),
                }
            } else {
                line
            };
            (!value.is_empty()).then(|| value.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_cp1251() {
        assert_eq!(decode(&[0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2]), "Привет");
    }

    #[test]
    fn decodes_utf8_bom() {
        assert_eq!(decode("\u{feff}Аня".as_bytes()), "Аня");
    }

    #[test]
    fn parses_csv_first_column() {
        let v = parse("\"Иван\";25\n\n;Пётр;30\nОля,1", true);
        assert_eq!(v, vec!["Иван", "Пётр", "Оля"]);
    }

    #[test]
    fn parses_txt_lines() {
        assert_eq!(parse("  a b \r\n\r\nc", false), vec!["a b", "c"]);
    }
}
