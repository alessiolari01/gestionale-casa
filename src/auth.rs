//! Whitelist delle chat autorizzate a usare il bot.
//!
//! In questa prima versione l'autorizzazione è basata sul `chat_id` di
//! Telegram. Una chat non presente in `ALLOWED_CHAT_IDS` viene ignorata e
//! nessun comando viene eseguito.

/// Restituisce `true` se il `chat_id` è presente nella whitelist.
pub fn is_authorized(chat_id: i64, allowed_chat_ids: &[i64]) -> bool {
    allowed_chat_ids.contains(&chat_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_autorizzata() {
        let allowed = vec![123_456_789, 987_654_321];

        assert!(is_authorized(123_456_789, &allowed));
    }

    #[test]
    fn chat_non_autorizzata() {
        let allowed = vec![123_456_789, 987_654_321];

        assert!(!is_authorized(111_111_111, &allowed));
    }
}
