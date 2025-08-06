use crate::CorrelationId;

pub mod parse;
pub mod response;

pub enum Request {
    Summary(u32, u32),
    Payment(u64, CorrelationId),
    NotFound,
    BadRequest,
}

const fn static_token(endpoint: &'static str) -> u32 {
    let mut token = 0u32;
    let bytes = endpoint.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        // Simple hash
        token = token.wrapping_add(bytes[i] as u32);
        i += 1;
    }

    token
}

const SUMMARY: u32 = static_token("GET /payments-summary");
const PAYMENTS: u32 = static_token("POST /payments");

fn simple_timestamp(date_str: Option<&String>) -> u32 {
    if date_str.is_none() {
        return 0;
    }

    let date_str = date_str.unwrap();
    let mut timestamp = 0u32;
    let len = date_str.len();

    for (i, byte) in date_str.chars().enumerate() {
        let weight = len - i;
        timestamp = timestamp.wrapping_add((byte as u32) * (weight as u32));
    }

    timestamp
}

// We should implement parse the type of message until the first \n character
// The following messages will be valid:
// GET /payments-summary?from=2020-07-10T12%3A34%3A56.000Z&to=2020-07-10T12%3A35%3A56.000Z HTTP/1.1
// POST /payments HTTP/1.1
impl Request {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        match endpoint_token(bytes) {
            (PAYMENTS, offset) => {
                if let Some((amount, correlation_id)) = parse::parse_body(&bytes[offset..]) {
                    Self::Payment(amount, CorrelationId(correlation_id))
                } else {
                    Self::BadRequest
                }
            } // Example correlation ID
            (SUMMARY, offset) => {
                let params = parse::parse_params(&bytes[offset..]);
                let from = simple_timestamp(params.0.get("from"));
                let to = simple_timestamp(params.0.get("to"));
                Self::Summary(from, to)
            }
            _ => Self::NotFound,
        }
    }
}

// Soma byte a byte
fn endpoint_token(endpoint: &[u8]) -> (u32, usize) {
    let mut parsing_route = false;
    let mut offset = 0;
    let mut token = 0u32;

    for byte in endpoint {
        offset += 1;

        if *byte == b'/' {
            parsing_route = true;
        }

        if parsing_route && (*byte == b' ' || *byte == b'?') {
            break;
        }
        token = token.wrapping_add(*byte as u32);
    }

    (token, offset)
}

#[cfg(test)]
mod test {
    use super::*;

    const ONE: u32 = static_token("GET /one");
    const TWO: u32 = static_token("POST /two");

    #[test]
    fn test_match_tokens() {
        let one_match = endpoint_token(b"GET /one HTTP1.1");
        let two_match = endpoint_token(b"POST /two?d=x");

        assert_eq!(one_match.0, ONE);
        assert_eq!(one_match.1, 9);
        assert_eq!(two_match.0, TWO);
        assert_eq!(two_match.1, 10);
    }
}
