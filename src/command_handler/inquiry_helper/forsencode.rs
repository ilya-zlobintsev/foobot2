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

pub fn decode(code: &str) -> Result<String, String> {
    let mut text = String::with_capacity(code.len() / 6);

    for codeword in code.trim().split_whitespace() {
        match decode_codeword(codeword) {
            Ok(c) => text.push(c),
            Err(err) => {
                // Only check for non-ascii characters when the codeword couldn't be decoded
                // This will be faster when there isn't a lot of interpolated text since it avoids iterating over the string before decoding
                // Otherwise it will be slower
                if codeword.chars().any(|c| !c.is_ascii()) {
                    text.push_str(codeword)
                } else {
                    return Err(err);
                }
            }
        }
    }

    Ok(text)
}

fn decode_codeword(word: &str) -> Result<char, String> {
    let mut ascii = 0b000_000_u32;
    let mut i = 6;

    for c in word.chars() {
        let bit = match c {
            'f' | 'r' | 's' | 'e' | 'n' => 0b0,
            'F' | 'R' | 'S' | 'E' | 'N' => 0b1,
            'o' => {
                i -= 1;
                0b11
            }
            'O' => {
                i -= 1;
                0b10
            }
            'ö' => {
                i -= 1;
                0b01
            }
            'Ö' => {
                i -= 1;
                0b00
            }
            _ => return Err(format!("Unexpected character in codeword: {c}")),
        };

        ascii |= bit << i;
        i -= 1;
    }

    char::from_u32(ascii).ok_or_else(|| format!("Could not decode character from {ascii}"))
}

#[cfg(test)]
mod tests {
    use super::{decode, decode_codeword, encode};
    use pretty_assertions::assert_eq;

    #[test]
    fn encode_non_ascii() {
        let text = "Hello world! 🤓";
        let code = encode(text);

        assert_eq!(code, "FÖRsen FOrSeN FORSen FORSen FORSEN fOrsen ForSEN FORSEN ForsEn FORSen FOrSen fOrseN fOrsen 🤓");
    }

    #[test]
    fn decode_a() {
        assert_eq!(decode_codeword("FOrseN").unwrap(), 'a');
    }

    #[test]
    fn decode_0() {
        assert_eq!(decode_codeword("forsen").unwrap(), '0');
    }

    #[test]
    fn decode_basic() {
        let code =
            "FÖRsen FOrSeN FORSen FORSen FORSEN fOrsen ForSEN FORSEN ForsEn FORSen FOrSen fOrseN";
        let text = decode(code).unwrap();
        assert_eq!(text, "Hello world!");
    }

    #[test]
    fn decode_interpolated() {
        let code = "FÖRsen FOrSeN FORSen FORSen FORSEN fOrsen ForSEN FORSEN ForsEn FORSen FOrSen fOrseN fOrsen 🤓";
        let text = decode(code).unwrap();
        assert_eq!(text, "Hello world! 🤓");
    }
}
