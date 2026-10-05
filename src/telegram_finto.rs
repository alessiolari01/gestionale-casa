//! Un Telegram finto, solo per i test.
//!
//! Nato col collaudo di `9307a33` (29 settembre 2026): i difetti peggiori di
//! quel giro stavano nel **flusso** dei messaggi — un prezzo scritto male
//! che rimetteva il bot ad aspettare una quantità, `/start` letto come
//! quantità, una ricerca rimasta viva dietro al menù principale — e i test
//! non potevano vederli, perché ogni gestore ha bisogno di un bot che parli
//! con Telegram. Qui c'è un server HTTP locale che risponde come l'API di
//! Telegram e si ricorda cosa gli è stato chiesto, così una prova può
//! ripetere i passi del collaudo e controllare sia i dati sia i testi.
//!
//! Risponde solo a quello che serve: i metodi che mandano o modificano un
//! messaggio ricevono un messaggio nuovo con un `message_id` crescente, tutti
//! gli altri `true`.

use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc, Mutex,
};

use serde_json::{json, Value};
use sqlx::SqlitePool;
use teloxide::types::Message;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

use crate::context_bot::{ContextBot, ImproveContextStore};

pub const CHAT: i64 = 4242;

#[derive(Debug, Clone)]
pub struct Chiamata {
    pub metodo: String,
    pub corpo: Value,
}

pub struct TelegramFinto {
    url: url::Url,
    chiamate: Arc<Mutex<Vec<Chiamata>>>,
}

impl TelegramFinto {
    pub async fn avvia() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("porta locale");
        let indirizzo = listener.local_addr().expect("indirizzo locale");
        let chiamate = Arc::new(Mutex::new(Vec::new()));
        let prossimo_id = Arc::new(AtomicI32::new(1000));
        {
            let chiamate = chiamate.clone();
            let prossimo_id = prossimo_id.clone();
            tokio::spawn(async move {
                while let Ok((socket, _)) = listener.accept().await {
                    tokio::spawn(servi(socket, chiamate.clone(), prossimo_id.clone()));
                }
            });
        }
        Self {
            url: url::Url::parse(&format!("http://{indirizzo}/")).expect("url"),
            chiamate,
        }
    }

    pub fn bot(&self, pool: &SqlitePool) -> ContextBot {
        let telegram = teloxide::Bot::new("123456:PROVA").set_api_url(self.url.clone());
        ContextBot::new(telegram, ImproveContextStore::default(), pool.clone())
    }

    pub fn chiamate(&self) -> Vec<Chiamata> {
        self.chiamate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// I testi mandati o riscritti, nell'ordine.
    pub fn testi(&self) -> Vec<String> {
        self.chiamate()
            .into_iter()
            .filter_map(|chiamata| {
                chiamata
                    .corpo
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect()
    }

    pub fn ultimo_testo(&self) -> String {
        self.testi().pop().unwrap_or_default()
    }

    /// I messaggi del bot che sono stati cancellati, nell'ordine.
    pub fn cancellati(&self) -> Vec<i64> {
        self.chiamate()
            .into_iter()
            .filter(|chiamata| chiamata.metodo.eq_ignore_ascii_case("deleteMessage"))
            .filter_map(|chiamata| chiamata.corpo.get("message_id").and_then(Value::as_i64))
            .collect()
    }

    /// Tutti i messaggi cancellati, uno per volta (`deleteMessage`) o in
    /// blocco (`deleteMessages`), nell'ordine.
    pub fn tutti_i_cancellati(&self) -> Vec<i64> {
        self.chiamate()
            .into_iter()
            .flat_map(|chiamata| {
                let metodo = chiamata.metodo.to_ascii_lowercase();
                if metodo == "deletemessage" {
                    chiamata
                        .corpo
                        .get("message_id")
                        .and_then(Value::as_i64)
                        .into_iter()
                        .collect::<Vec<_>>()
                } else if metodo == "deletemessages" {
                    chiamata
                        .corpo
                        .get("message_ids")
                        .and_then(Value::as_array)
                        .map(|ids| ids.iter().filter_map(Value::as_i64).collect())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            })
            .collect()
    }

    /// Le etichette dei pulsanti dell'ultimo messaggio che ne aveva.
    pub fn ultimi_pulsanti(&self) -> Vec<String> {
        self.chiamate()
            .into_iter()
            .rev()
            .find_map(|chiamata| {
                chiamata
                    .corpo
                    .get("reply_markup")
                    .and_then(|markup| markup.get("inline_keyboard"))
                    .and_then(Value::as_array)
                    .cloned()
            })
            .unwrap_or_default()
            .iter()
            .flat_map(|riga| riga.as_array().cloned().unwrap_or_default())
            .filter_map(|pulsante| {
                pulsante
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect()
    }
}

/// Un messaggio di testo scritto dall'utente.
pub fn messaggio(testo: &str) -> Message {
    messaggio_con_id(testo, 1)
}

/// Un messaggio dell'utente con un id scelto: in una chat privata gli id
/// sono consecutivi fra utente e bot, e `/clear` ci conta.
pub fn messaggio_con_id(testo: &str, message_id: i32) -> Message {
    serde_json::from_value(json!({
        "message_id": message_id,
        "date": 1_759_000_000,
        "chat": { "id": CHAT, "type": "private", "first_name": "Alessio" },
        "from": { "id": CHAT, "is_bot": false, "first_name": "Alessio" },
        "text": testo,
    }))
    .expect("messaggio di prova")
}

async fn servi(
    mut socket: TcpStream,
    chiamate: Arc<Mutex<Vec<Chiamata>>>,
    prossimo_id: Arc<AtomicI32>,
) {
    let mut buffer = Vec::new();
    loop {
        // Legge fino alla fine delle intestazioni.
        let fine_intestazioni = loop {
            if let Some(posizione) = buffer.windows(4).position(|w| w == b"\r\n\r\n") {
                break posizione + 4;
            }
            let mut pezzo = [0u8; 4096];
            match socket.read(&mut pezzo).await {
                Ok(0) | Err(_) => return,
                Ok(letti) => buffer.extend_from_slice(&pezzo[..letti]),
            }
        };
        let intestazioni = String::from_utf8_lossy(&buffer[..fine_intestazioni]).to_string();
        let lunghezza = intestazioni
            .lines()
            .find_map(|riga| {
                let (nome, valore) = riga.split_once(':')?;
                nome.eq_ignore_ascii_case("content-length")
                    .then(|| valore.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0);
        while buffer.len() < fine_intestazioni + lunghezza {
            let mut pezzo = [0u8; 4096];
            match socket.read(&mut pezzo).await {
                Ok(0) | Err(_) => return,
                Ok(letti) => buffer.extend_from_slice(&pezzo[..letti]),
            }
        }
        let corpo_grezzo = buffer[fine_intestazioni..fine_intestazioni + lunghezza].to_vec();
        buffer.drain(..fine_intestazioni + lunghezza);

        let metodo = intestazioni
            .split_whitespace()
            .nth(1)
            .and_then(|percorso| percorso.rsplit('/').next())
            .unwrap_or_default()
            .to_string();
        let corpo: Value = serde_json::from_slice(&corpo_grezzo).unwrap_or(Value::Null);
        let risultato = risposta(&metodo, &corpo, &prossimo_id);
        chiamate
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(Chiamata { metodo, corpo });

        let testo = json!({ "ok": true, "result": risultato }).to_string();
        let risposta_http = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{testo}",
            testo.len()
        );
        if socket.write_all(risposta_http.as_bytes()).await.is_err() {
            return;
        }
    }
}

fn risposta(metodo: &str, corpo: &Value, prossimo_id: &AtomicI32) -> Value {
    let metodo = metodo.to_ascii_lowercase();
    let manda_un_messaggio = metodo.starts_with("send") || metodo.starts_with("edit");
    if !manda_un_messaggio {
        return Value::Bool(true);
    }
    let chat_id = corpo.get("chat_id").and_then(Value::as_i64).unwrap_or(CHAT);
    json!({
        "message_id": prossimo_id.fetch_add(1, Ordering::SeqCst),
        "date": 1_759_000_000,
        "chat": { "id": chat_id, "type": "private", "first_name": "Alessio" },
        "from": { "id": 1, "is_bot": true, "first_name": "Gestionale" },
        "text": corpo.get("text").and_then(Value::as_str).unwrap_or(""),
    })
}
