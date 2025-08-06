use std::collections::HashMap;

pub fn parse_body(bytes: &[u8]) -> Option<(u64, [u8; 36])> {
    let mut amount = 0u64;
    let mut correlation_id = [0; 36];
    let mut header_end_found = false;
    let mut cursor = 0;
    let mut valid = (false, false);

    // procurar pelo fim dos headers \n\n ou \r\n\r\n
    for b in bytes {
        cursor += 1;
        if *b == b'\n' && bytes[cursor] == b'\n' {
            header_end_found = true;
            break;
        }

        if *b == b'\r' && bytes[cursor..cursor + 3] == *b"\n\r\n" {
            header_end_found = true;
            break;
        }
    }

    if !header_end_found {
        return None;
    }

    while cursor < bytes.len() {
        let b = &bytes[cursor];
        cursor += 1;

        if *b == b'"' {
            if bytes[cursor..cursor + 6].eq_ignore_ascii_case(b"amount") {
                cursor += 6;
                if let Some(value) = parse_float_as_u32_as_bytes(bytes, &mut cursor) {
                    valid.0 = true;
                    amount = value;
                }
            } else if bytes[cursor..cursor + 13].eq_ignore_ascii_case(b"correlationId") {
                cursor += 13;
                if let Some(value) = parse_string(bytes, &mut cursor) {
                    correlation_id.copy_from_slice(&value);
                    valid.1 = true;
                }
            }
        }

        if valid.0 && valid.1 {
            break;
        }
    }

    Some((amount, correlation_id))
}

// We will only handle 2 cents
#[inline(always)]
fn parse_float_as_u32_as_bytes(bytes: &[u8], cursor: &mut usize) -> Option<u64> {
    let mut step = 0;
    let mut value = String::with_capacity(10);
    let mut cents = String::with_capacity(2);

    while *cursor < bytes.len() {
        let mut b = &bytes[*cursor];
        *cursor += 1;

        if *b == b':' {
            step = 1;
            continue;
        } else if step == 0 {
            continue;
        }

        if *b == b'.' {
            step = 2;
            b = &bytes[*cursor];
            *cursor += 1;
        }

        if step == 2 && cents.len() == 2 {
            break;
        }

        if step == 1 && b.is_ascii_digit() {
            value.push(*b as char);
        } else if step == 2 && b.is_ascii_digit() {
            cents.push(*b as char);
        } else if step == 2 {
            break;
        }
    }

    let value = if let Ok(value) = value.parse::<u64>() {
        value
    } else {
        return None;
    };

    let cents = cents.parse::<u8>().unwrap_or(0);
    Some(value * 100 + cents as u64)
}

#[inline(always)]
fn parse_string(bytes: &[u8], cursor: &mut usize) -> Option<[u8; 36]> {
    let mut value = [0; 36];
    let mut index = 0;
    let chars = [b':', b'"'];
    let mut step = 0;

    while *cursor < bytes.len() && index < value.len() {
        let b = &bytes[*cursor];
        *cursor += 1;

        if step == chars.len() {
            if *b == b'"' {
                break;
            }

            value[index] = *b;
            index += 1;
        } else if *b == chars[step] {
            step += 1;
        }
    }

    if value.is_empty() { None } else { Some(value) }
}

// Parse that string from=2020-07-10T12%3A34%3A56.000Z&to=2020-07-10T12%3A35%3A56.000Z HTTP/1.1
// where it found '=' character sum all bytes until '&' character and do the same for next param
pub fn parse_params(bytes: &[u8]) -> (HashMap<String, String>, usize) {
    let mut params = HashMap::with_capacity(10);
    let mut offset = 0;
    let mut parse_key = true;
    let mut key = String::with_capacity(20);
    let mut value = String::with_capacity(20);

    for b in bytes {
        offset += 1;
        // end of route with parameters
        if *b == b' ' {
            break;
        }

        if *b == b'&' {
            params.insert(std::mem::take(&mut key), std::mem::take(&mut value));
            parse_key = true;
            continue;
        }

        if *b == b'=' {
            parse_key = false;
            continue;
        }

        if parse_key {
            key.push(*b as char);
        } else {
            value.push(*b as char);
        }
    }

    if !value.is_empty() {
        params.insert(std::mem::take(&mut key), std::mem::take(&mut value));
    }

    (params, offset)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_summary() {
        let params = b"from=2020-07-10T12%3A34%3A56.000Z&to=2020-07-10T12%3A35%3A56.000Z HTTP/1.1";
        let (params, offset) = parse_params(params);
        assert_eq!(offset, 66);
        assert_eq!(
            params.get("from"),
            Some(&"2020-07-10T12%3A34%3A56.000Z".to_string())
        );
        assert_eq!(
            params.get("to"),
            Some(&"2020-07-10T12%3A35%3A56.000Z".to_string())
        );
        let params = b"to=2020-07-10T12%3A35%3A56.000Z HTTP/1.1";
        let (params, offset) = parse_params(params);
        assert_eq!(params.get("from"), None);
        assert_eq!(offset, 32);
    }
}
