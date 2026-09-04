#!/usr/bin/env bash
# Primitive per pilotare Telegram via API diretta dall'agente, non dal bot
# sull'S9. Decisione del 3 settembre 2026 (docs/previsto/
# automazione-ciclo-sviluppo.md, punto 6): il messaggio di countdown/
# checklist deve continuare ad aggiornarsi anche quando il processo S9 e'
# fermo per lo swap, quindi non puo' dipendere dal bot in esecuzione in
# quel momento.
#
# Il repository e' pubblico: token e chat_id non devono MAI comparire come
# letterali in un file committato. Si leggono via SSH dal .env e dal
# database reali dell'S9 (tg_leggi_credenziali), e restano solo in variabili
# di shell per la durata dello script che le usa -- mai scritti su disco su
# questa macchina.
#
# Uso, da un altro script:
#   source scripts/telegram-api.sh
#   tg_leggi_credenziali          # imposta TG_TOKEN e TG_CHAT_ID
#   id=$(tg_invia "testo")
#   tg_modifica "$id" "nuovo testo"
#   tg_elimina "$id"

set -uo pipefail

# `curl --data-urlencode` su questa macchina (curl 8.21 / mingw-w64)
# corrompe i caratteri non-ASCII: una "e" accentata diventa U+FFFD prima
# ancora di essere codificata, e Telegram rifiuta la richiesta con "strings
# must be encoded in UTF-8". Trovato provando il countdown per davvero, non
# a tavolino. Si aggira codificando il testo in percent-encoding con Python
# (stessa scelta di rilevamento gia' in verifica-ci.sh e
# controlla-documenti.sh) e passandolo gia' pronto a curl con --data
# semplice, che non lo ritocca.
_TG_PYTHON=""
for candidato in python3 python; do
    if command -v "$candidato" >/dev/null 2>&1 && "$candidato" --version >/dev/null 2>&1; then
        _TG_PYTHON="$candidato"
        break
    fi
done
if [ -z "$_TG_PYTHON" ]; then
    echo "Serve python3 (o python) per codificare il testo, non trovato o non funzionante." >&2
    return 1 2>/dev/null || exit 1
fi

tg_urlencode() {
    "$_TG_PYTHON" -c "import urllib.parse, sys; print(urllib.parse.quote(sys.argv[1]))" "$1"
}

# Ogni chiamata a Telegram passa da qui: un countdown al secondo (deciso il
# 4 settembre 2026, dopo il primo collaudo) puo' incontrare sia un 429
# (limite di frequenza) sia un fallimento di connessione transitorio,
# incontrato per davvero collaudando ("Recv failure: Connection was
# reset"). `--retry` di curl copre entrambi da solo: riprova sugli errori
# di rete, e dal 7.66 rispetta l'header Retry-After che Telegram manda con
# un 429 -- non serve leggerlo a mano.
_tg_curl() {
    curl -sS --retry 4 --retry-all-errors --retry-delay 2 "$@"
}

# Legge token (dal .env dell'S9) e chat_id dell'amministratore principale
# (dal database reale dell'S9) in due variabili di shell. Una sola volta
# per esecuzione, non a ogni chiamata: una modifica per tick di countdown
# non deve aprire una connessione SSH in piu' del necessario.
tg_leggi_credenziali() {
    TG_TOKEN="$(ssh s9 "cd ~/gestionale-casa && grep '^TELOXIDE_TOKEN=' .env | cut -d= -f2-")"
    if [ -z "$TG_TOKEN" ]; then
        echo "Impossibile leggere TELOXIDE_TOKEN dall'S9." >&2
        return 1
    fi
    TG_CHAT_ID="$(ssh s9 "cd ~/gestionale-casa && sqlite3 data/db/gestionale.db \"SELECT at.chat_id FROM account_telegram at JOIN utenti u ON u.id = at.utente_id WHERE u.amministratore_principale = 1 LIMIT 1;\"")"
    if [ -z "$TG_CHAT_ID" ]; then
        echo "Impossibile leggere il chat_id dell'amministratore principale dall'S9." >&2
        return 1
    fi
    export TG_TOKEN TG_CHAT_ID
}

# Invia un messaggio. Stampa il message_id.
#
# Deciso il 4 settembre 2026, dopo il primo collaudo: niente pin. Fissare
# (pinChatMessage) e poi eliminare il messaggio lascia in chat una notifica
# di sistema fantasma ("Gestionale_Bot pinned Deleted message") che non
# sparisce da sola e non si può ripulire via API (i service message di
# pin/unpin non hanno un message_id restituito da queste chiamate). Un
# messaggio normale, aggiornato sempre sullo stesso id, basta: resta
# comunque l'unico messaggio che cambia, l'ordine cronologico della chat
# non ha bisogno di essere forzato.
tg_invia() {
    local testo risposta message_id
    testo="$(tg_urlencode "$1")"
    risposta="$(_tg_curl "https://api.telegram.org/bot${TG_TOKEN}/sendMessage" \
        --data "chat_id=${TG_CHAT_ID}" \
        --data "text=${testo}")"
    message_id="$(echo "$risposta" | grep -o '"message_id":[0-9]*' | head -1 | grep -o '[0-9]*')"
    if [ -z "$message_id" ]; then
        echo "Invio fallito: $risposta" >&2
        return 1
    fi
    echo "$message_id"
}

# Sostituisce il testo di un messaggio esistente. Mai un messaggio nuovo:
# e' la regola scritta nel ciclo di automazione.
tg_modifica() {
    local message_id="$1" testo risposta
    testo="$(tg_urlencode "$2")"
    risposta="$(_tg_curl "https://api.telegram.org/bot${TG_TOKEN}/editMessageText" \
        --data "chat_id=${TG_CHAT_ID}" \
        --data "message_id=${message_id}" \
        --data "text=${testo}")"
    if echo "$risposta" | grep -q '"ok":true'; then
        return 0
    fi
    # Il testo e' gia' quello (es. due tick identici di seguito): non e'
    # un errore da segnalare, il risultato voluto e' gia' li'.
    if echo "$risposta" | grep -q "message is not modified"; then
        return 0
    fi
    echo "Modifica fallita: $risposta" >&2
    return 1
}

# Elimina un messaggio. Serve per ripulire i messaggi di prova/collaudo
# dalla chat reale, non fa parte del ciclo di deploy in se'.
tg_elimina() {
    local message_id="$1" risposta
    risposta="$(_tg_curl "https://api.telegram.org/bot${TG_TOKEN}/deleteMessage" \
        --data "chat_id=${TG_CHAT_ID}" \
        --data "message_id=${message_id}")"
    if echo "$risposta" | grep -q '"ok":true'; then
        return 0
    fi
    echo "Eliminazione di $message_id fallita: $risposta" >&2
    return 1
}
