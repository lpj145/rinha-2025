use crate::CorrelationId;

pub enum Message {
    Summary(u32, u32),
    Payment(u64, CorrelationId),
    Ack,
}

unsafe impl Send for Message {}
unsafe impl Sync for Message {}

impl Message {
    pub const SIZE: usize = 54;
    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        match self {
            Message::Summary(from, to) => {
                let mut bytes = [0; Self::SIZE];
                bytes[0] = b'@'; // Marker for Summary
                bytes[1..5].copy_from_slice(&from.to_be_bytes());
                bytes[5..9].copy_from_slice(&to.to_be_bytes());
                bytes[53] = 0x06; // ACK
                bytes
            }
            Message::Payment(amount, correlation_id) => {
                let mut bytes = [0; Self::SIZE];
                bytes[0] = b'$'; // Marker for Payment
                bytes[1..9].copy_from_slice(&amount.to_be_bytes());
                bytes[9..45].copy_from_slice(&correlation_id.0); // 36 bytes: 9 to 44
                bytes[53] = 0x06; // ACK no último byte
                bytes
            }
            Message::Ack => {
                [0x06; Self::SIZE] // Just ACK
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() {
            return Err("Empty bytes".into());
        }

        match bytes[0] {
            // Summary
            b'@' => {
                // Verificar se termina com 0x06 (ACK)
                if bytes.len() != Self::SIZE {
                    return Err(format!(
                        "Summary message doesn't have the correct size, got: {}, expected: {}",
                        bytes.len(),
                        Self::SIZE,
                    ));
                }

                // Extrair from (bytes 1-4)
                let from_bytes: [u8; 4] = bytes[1..5].try_into().unwrap_or([0; 4]);
                let from = u32::from_be_bytes(from_bytes);

                // Extrair to (bytes 5-8)
                let to_bytes: [u8; 4] = bytes[5..9].try_into().unwrap_or([0; 4]);
                let to = u32::from_be_bytes(to_bytes);

                Ok(Message::Summary(from, to))
            }
            // Payment
            b'$' => {
                // Verificar tamanho do pacote
                if bytes.len() != Self::SIZE {
                    return Err(format!(
                        "Payment message doesn't have the correct size, got: {}, expected: {}",
                        bytes.len(),
                        Self::SIZE
                    ));
                }

                let amount = u64::from_be_bytes([
                    bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7], bytes[8],
                ]);

                let correlation_id = CorrelationId(bytes[9..45].try_into().unwrap_or([0; 36]));

                Ok(Message::Payment(amount, correlation_id))
            }
            0x06 => {
                // ACK message
                Ok(Message::Ack)
            }
            _ => Err("Invalid message type marker".into()),
        }
    }
}

impl std::fmt::Debug for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Message::Summary(from, to) => write!(f, "Summary(from: {from}, to: {to})"),
            Message::Payment(amount, _) => {
                write!(f, "Payment(amount: {amount}, correlation_id: [u8; 36])",)
            }
            Message::Ack => write!(f, "Ack"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_to_bytes() {
        let message = Message::Summary(12345, 67890);
        let bytes = message.to_bytes();

        // Verificar estrutura: PACK_SIZE bytes com '@' no início e 0x06 no último byte
        assert_eq!(bytes.len(), Message::SIZE);
        assert_eq!(bytes[0], b'@');
        assert_eq!(bytes[53], 0x06); // ACK no último byte (posição 53)

        // Verificar valores
        let from = u64::from_be_bytes(bytes[1..9].try_into().unwrap());
        let to = u64::from_be_bytes(bytes[9..17].try_into().unwrap());
        assert_eq!(from, 12345);
        assert_eq!(to, 67890);
    }

    #[test]
    fn test_payment_to_bytes() {
        // Cria um correlation_id de 36 bytes
        let mut correlation_bytes = [0u8; 36];
        correlation_bytes[..16].copy_from_slice(b"1234567890abcdef");
        correlation_bytes[16..32].copy_from_slice(b"fedcba0987654321");
        correlation_bytes[32..36].copy_from_slice(b"wxyz");

        let correlation_id = CorrelationId(correlation_bytes);
        let message = Message::Payment(98765, correlation_id);
        let bytes = message.to_bytes();

        // Verificar estrutura: PACK_SIZE bytes com '$' no início e 0x06 no final
        assert_eq!(bytes.len(), Message::SIZE);
        assert_eq!(bytes[0], b'$');
        assert_eq!(bytes[53], 0x06); // ACK no último byte

        // Verificar amount
        let amount = u64::from_be_bytes(bytes[1..9].try_into().unwrap());
        assert_eq!(amount, 98765);

        // Verificar correlation_id (primeiros 16 bytes)
        let correlation_part = &bytes[9..25];
        assert_eq!(correlation_part, b"1234567890abcdef");

        // Verificar resto do correlation_id
        let correlation_part2 = &bytes[25..41];
        assert_eq!(correlation_part2, b"fedcba0987654321");

        let correlation_part3 = &bytes[41..45];
        assert_eq!(correlation_part3, b"wxyz");
    }
    #[test]
    fn test_ack_to_bytes() {
        let message = Message::Ack;
        let bytes = message.to_bytes();

        // Verificar que todos os bytes são 0x06
        assert_eq!(bytes.len(), Message::SIZE);
        for byte in bytes.iter() {
            assert_eq!(*byte, 0x06);
        }
    }

    #[test]
    fn test_summary_from_bytes() {
        // Criar bytes manualmente
        let mut bytes = [0u8; Message::SIZE];
        bytes[0] = b'@';
        bytes[1..9].copy_from_slice(&12345u64.to_be_bytes());
        bytes[9..17].copy_from_slice(&67890u64.to_be_bytes());
        bytes[53] = 0x06; // ACK no último byte

        let message = Message::from_bytes(&bytes).unwrap();

        match message {
            Message::Summary(from, to) => {
                assert_eq!(from, 12345);
                assert_eq!(to, 67890);
            }
            _ => panic!("Expected Summary message"),
        }
    }

    #[test]
    fn test_payment_from_bytes() {
        // Criar bytes manualmente
        let mut bytes = [0u8; Message::SIZE];
        bytes[0] = b'$';
        bytes[1..9].copy_from_slice(&98765u64.to_be_bytes());

        // Correlation ID de 36 bytes (só os primeiros 16 são usados no to_bytes)
        let mut correlation_bytes = [0u8; 36];
        correlation_bytes[..16].copy_from_slice(b"1234567890abcdef");
        bytes[9..45].copy_from_slice(&correlation_bytes); // bytes 9-44 (36 bytes)

        bytes[53] = 0x06; // último byte

        let message = Message::from_bytes(&bytes).unwrap();

        match message {
            Message::Payment(amount, correlation_id) => {
                assert_eq!(amount, 98765);
                assert_eq!(correlation_id.0.len(), 36);
                // Verificar os primeiros 16 bytes do correlation_id
                assert_eq!(&correlation_id.0[..16], b"1234567890abcdef");
            }
            _ => panic!("Expected Payment message"),
        }
    }

    #[test]
    fn test_ack_from_bytes() {
        let bytes = [0x06u8; Message::SIZE];
        let message = Message::from_bytes(&bytes).unwrap();

        match message {
            Message::Ack => {} // Sucesso
            _ => panic!("Expected Ack message"),
        }
    }

    #[test]
    fn test_roundtrip_summary() {
        let original = Message::Summary(12345, 67890);
        let bytes = original.to_bytes();
        let decoded = Message::from_bytes(&bytes).unwrap();

        match decoded {
            Message::Summary(from, to) => {
                assert_eq!(from, 12345);
                assert_eq!(to, 67890);
            }
            _ => panic!("Expected Summary message"),
        }
    }

    #[test]
    fn test_roundtrip_payment() {
        let mut correlation_bytes = [0u8; 36];
        correlation_bytes[..16].copy_from_slice(b"test-correlation");
        let original = Message::Payment(54321, CorrelationId(correlation_bytes));

        let bytes = original.to_bytes();
        let decoded = Message::from_bytes(&bytes).unwrap();

        match decoded {
            Message::Payment(amount, correlation_id) => {
                assert_eq!(amount, 54321);
                assert_eq!(&correlation_id.0[..16], b"test-correlation");
            }
            _ => panic!("Expected Payment message"),
        }
    }

    #[test]
    fn test_roundtrip_ack() {
        let original = Message::Ack;
        let bytes = original.to_bytes();
        let decoded = Message::from_bytes(&bytes).unwrap();

        match decoded {
            Message::Ack => {} // Sucesso
            _ => panic!("Expected Ack message"),
        }
    }

    #[test]
    fn test_invalid_message_type() {
        let mut bytes = [0u8; Message::SIZE];
        bytes[0] = b'X'; // Tipo inválido

        let result = Message::from_bytes(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Invalid message type marker");
    }

    #[test]
    fn test_empty_bytes() {
        let bytes = [];
        let result = Message::from_bytes(&bytes);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Empty bytes");
    }

    #[test]
    fn test_wrong_size_bytes() {
        let bytes = [b'@', 1, 2, 3]; // Tamanho incorreto
        let result = Message::from_bytes(&bytes);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("doesn't have the correct size")
        );
    }
}
