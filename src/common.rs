pub const MARGINFI_ACCOUNT_DISCRIMINATOR: [u8; 8] = [67, 178, 130, 109, 126, 114, 28, 42];
pub const MARGINFI_ACCOUNT_DISCRIMINATOR_LEN: usize = MARGINFI_ACCOUNT_DISCRIMINATOR.len();
pub const MARGINFI_BANK_DISCRIMINATOR: [u8; 8] = [142, 49, 166, 242, 50, 66, 97, 188];
pub const MARGINFI_BANK_DISCRIMINATOR_LEN: usize = MARGINFI_BANK_DISCRIMINATOR.len();

// TODO: Is there better home for Geysermessage and GeyserMessageType?
#[derive(Debug, PartialEq)]
pub enum MessageType {
    Clock,
    MarginfiAccount,
    Bank,
    Oracle,
}

pub fn get_marginfi_message_type(account_data: &[u8]) -> Option<MessageType> {
    if account_data.len() > MARGINFI_ACCOUNT_DISCRIMINATOR_LEN
        && account_data.starts_with(&MARGINFI_ACCOUNT_DISCRIMINATOR)
    {
        Some(MessageType::MarginfiAccount)
    } else if account_data.len() > MARGINFI_BANK_DISCRIMINATOR_LEN
        && account_data.starts_with(&MARGINFI_BANK_DISCRIMINATOR)
    {
        Some(MessageType::Bank)
    } else {
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_marginfi_account_message_type() {
        let mut data = MARGINFI_ACCOUNT_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(
            get_marginfi_message_type(&data),
            Some(MessageType::MarginfiAccount)
        );
    }

    #[test]
    fn test_get_marginfi_bank_message_type() {
        let mut data = MARGINFI_BANK_DISCRIMINATOR.to_vec();
        data.extend_from_slice(&[5, 6, 7, 8]);
        assert_eq!(get_marginfi_message_type(&data), Some(MessageType::Bank));
    }

    #[test]
    fn test_account_data_too_short() {
        let data = MARGINFI_ACCOUNT_DISCRIMINATOR[..4].to_vec();
        assert_eq!(get_marginfi_message_type(&data), None);

        let data = MARGINFI_BANK_DISCRIMINATOR[..4].to_vec();
        assert_eq!(get_marginfi_message_type(&data), None);
    }

    #[test]
    fn test_account_data_wrong_discriminator() {
        let data = vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9];
        assert_eq!(get_marginfi_message_type(&data), None);
    }

    #[test]
    fn test_account_data_exact_length_but_not_matching() {
        let data = vec![0; MARGINFI_ACCOUNT_DISCRIMINATOR_LEN];
        assert_eq!(get_marginfi_message_type(&data), None);

        let data = vec![0; MARGINFI_BANK_DISCRIMINATOR_LEN];
        assert_eq!(get_marginfi_message_type(&data), None);
    }

    #[test]
    fn test_account_data_starts_with_partial_discriminator() {
        let mut data = MARGINFI_ACCOUNT_DISCRIMINATOR[..4].to_vec();
        data.extend_from_slice(&[9, 9, 9, 9, 9, 9, 9, 9]);
        assert_eq!(get_marginfi_message_type(&data), None);
    }
}
