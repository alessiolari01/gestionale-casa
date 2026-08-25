//! Whitelist Telegram di bootstrap/emergenza.
//!
//! Dallo Step 7.2E l'autorizzazione ordinaria vive nel database: un account
//! Telegram approvato può usare il bot anche se il suo `chat_id` non compare in
//! `ALLOWED_CHAT_IDS`. La whitelist resta necessaria per inizializzare il primo
//! amministratore su un database nuovo e come canale di emergenza controllato.

/// Restituisce `true` se il `chat_id` è presente nella whitelist bootstrap.
pub fn is_authorized(chat_id: i64, allowed_chat_ids: &[i64]) -> bool {
    allowed_chat_ids.contains(&chat_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_bootstrap_autorizzata() {
        let allowed = vec![123_456_789, 987_654_321];

        assert!(is_authorized(123_456_789, &allowed));
    }

    #[test]
    fn chat_non_presente_nella_whitelist_bootstrap() {
        let allowed = vec![123_456_789, 987_654_321];

        assert!(!is_authorized(111_111_111, &allowed));
    }
}
