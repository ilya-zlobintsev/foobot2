/// Encode implementation credits:
/// https://gist.github.com/GaZaTu/427d7d2a6d7974c1acfe4eaced36ac87
/// https://gist.github.com/Nerixyz/080eac37f39512cb49bc7041f02078d4

pub fn encode(text: &str) -> String {
    let mut prev_char_was_invalid = false;
    let mut code = String::with_capacity(text.len() * 6);
    for decoded_char in text.trim().chars() {
        if !code.is_empty() && !prev_char_was_invalid {
            code.push(' ');
        }
        let ascii = u32::from(decoded_char);
        if !decoded_char.is_ascii() {
            prev_char_was_invalid = true;
            code.push(decoded_char);
            continue;
        }
        if decoded_char == ' ' && prev_char_was_invalid {
            prev_char_was_invalid = false;
            continue;
        }

        code.push(if (ascii & 0b100_0000) != 0 { 'F' } else { 'f' });
        code.push(match (ascii >> 4) & 0b11 {
            0b00 => 'Ö',
            0b01 => 'ö',
            0b10 => 'O',
            _ /* 0b11 */ => 'o',
        });
        code.push(if (ascii & 0b000_1000) != 0 { 'R' } else { 'r' });
        code.push(if (ascii & 0b000_0100) != 0 { 'S' } else { 's' });
        code.push(if (ascii & 0b000_0010) != 0 { 'E' } else { 'e' });
        code.push(if (ascii & 0b000_0001) != 0 { 'N' } else { 'n' });

        prev_char_was_invalid = false;
    }
    code
}

#[cfg(test)]
mod tests {
    use super::encode;
    use pretty_assertions::assert_eq;

    #[test]
    fn encode_non_ascii() {
        let text = "Hello world! 🤓";
        let code = encode(text);

        assert_eq!(code, "FÖRsen FOrSeN FORSen FORSen FORSEN fOrsen ForSEN FORSEN ForsEn FORSen FOrSen fOrseN fOrsen 🤓");
    }
}
