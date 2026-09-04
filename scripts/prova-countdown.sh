#!/usr/bin/env bash
# Collaudo isolato del meccanismo di invio/edit, sotto-step 1 del punto 6
# del ciclo (docs/previsto/automazione-ciclo-sviluppo.md). Nessun deploy
# vero: solo la meccanica del messaggio che si aggiorna via edit, mai un
# messaggio nuovo -- provata sul chat reale dell'amministratore principale.
#
# Tick al secondo (deciso il 4 settembre 2026, dopo il primo collaudo: a
# 15s per tick il countdown non si vedeva scendere, e modificare non
# spamma la chat come inviare farebbe). tg_modifica gestisce da sola il
# limite di frequenza di Telegram se lo incontra.
#
# Il tempo rimanente si ricava da una scadenza fissa (ora + durata, in
# secondi epoch), non da un contatore decrementato a ogni giro (deciso il
# 4 settembre 2026, trovato rileggendo il codice: un tick che dura più di
# un secondo — un retry di rete, il processo sospeso — faceva restare
# indietro il numero mostrato rispetto al tempo vero, senza mai
# recuperare). Ricalcolando da `scadenza - ora_attuale` a ogni giro, un
# ritardo si vede come un salto nel numero mostrato invece che come una
# deriva silenziosa.
#
# Niente pin (deciso il 4 settembre 2026, dopo il secondo collaudo):
# fissare e poi eliminare il messaggio lascia in chat una notifica di
# sistema fantasma ("Gestionale_Bot pinned Deleted message") che non
# sparisce da sola e non è ripulibile via API. Un messaggio normale,
# aggiornato sempre sullo stesso id, basta.
#
# Uso:
#   ./scripts/prova-countdown.sh [secondi]     default 20

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
source ./scripts/telegram-api.sh

DURATA="${1:-20}"

echo "Leggo le credenziali dall'S9..."
tg_leggi_credenziali || exit 1

testo_tick() {
    echo "🧪 PROVA — countdown

Questo è un collaudo del meccanismo di invio/edit, non una manutenzione vera.
Nessun deploy in corso.

Prossimo tick tra ${1}s."
}

echo "Invio il messaggio di prova..."
ID="$(tg_invia "$(testo_tick "$DURATA")")"
if [ -z "$ID" ]; then
    echo "Invio fallito, mi fermo." >&2
    exit 1
fi
echo "Messaggio inviato: id=$ID"

SCADENZA=$(($(date +%s) + DURATA))
RESTANO="$DURATA"
while [ "$RESTANO" -gt 0 ]; do
    sleep 1
    RESTANO=$((SCADENZA - $(date +%s)))
    [ "$RESTANO" -lt 0 ] && RESTANO=0
    tg_modifica "$ID" "$(testo_tick "$RESTANO")" || exit 1
done

echo "Messaggio finale..."
tg_modifica "$ID" "🧪 PROVA — countdown

Collaudo completato: $((DURATA + 1)) modifiche sullo stesso messaggio
(id $ID), un tick al secondo, mai duplicato. Nessun deploy è avvenuto." || exit 1

echo "OK Collaudo countdown al secondo completato ($((DURATA + 1)) modifiche)."
echo "Messaggio id=$ID lasciato in chat: elimina con tg_elimina se non serve più."
