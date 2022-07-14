/// Implementation credit to GaZaTu
/// https://gist.github.com/GaZaTu/427d7d2a6d7974c1acfe4eaced36ac87
static CODEWORD: &str = "forsen";
static CODEWORD_BIT_SIZE: usize = CODEWORD.len() + 1;
static CODEWORD_LIMIT: usize = usize::pow(2, CODEWORD_BIT_SIZE as u32);

pub fn encode(text: &str) -> String {
    let mut prev_char_was_invalid = false;

    let mut code = String::with_capacity(text.len() * CODEWORD.len());

    for decoded_char in text.trim().chars() {
        if !code.is_empty() && !prev_char_was_invalid {
            code.push(' ');
        }

        let ascii = decoded_char as usize;
        if ascii >= CODEWORD_LIMIT {
            prev_char_was_invalid = true;
            code.push(decoded_char);
            continue;
        }

        if decoded_char == ' ' && prev_char_was_invalid {
            prev_char_was_invalid = false;
            continue;
        }

        let ascii_bit_string = format!("{:07b}", ascii);

        let mut bit: usize = 0;

        for cc in CODEWORD.chars() {
            if cc == 'o' {
                let state = &ascii_bit_string[bit..bit + 2];

                let new_char = match state {
                    "00" => 'Ö',
                    "01" => 'ö',
                    "10" => 'O',
                    "11" => 'o',
                    _ => panic!("fdm"),
                };
                code.push(new_char);

                bit += 2;
                continue;
            }

            let upper = ascii_bit_string
                .chars()
                .nth(bit)
                .unwrap()
                .to_digit(2)
                .unwrap();
            let new_char = if upper != 0 {
                cc.to_ascii_uppercase()
            } else {
                cc.to_ascii_lowercase()
            };
            code.push(new_char);

            bit += 1;
        }

        prev_char_was_invalid = false;
    }

    code
}
