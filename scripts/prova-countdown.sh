#!/usr/bin/env bash
# Collaudo isolato del meccanismo di pin/edit, sotto-step 1 del punto 6 del
# ciclo (docs/previsto/automazione-ciclo-sviluppo.md). Nessun deploy vero:
# solo la meccanica del messaggio pinnato che si aggiorna via edit, mai un
# messaggio nuovo -- provata sul chat reale dell'amministratore principale.
#
# Tick al secondo (deciso il 4 settembre 2026, dopo il primo collaudo: a
# 15s per tick il countdown non si vedeva scendere, e modificare non
# spamma la chat come inviare farebbe). tg_modifica gestisce da sola il
# limite di frequenza di Telegram se lo incontra.
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
    echo "🧪 PROVA — countdown pinnato

Questo è un collaudo del meccanismo di pin/edit, non una manutenzione vera.
Nessun deploy in corso.

Prossimo tick tra ${1}s."
}

echo "Invio e fisso il messaggio di prova..."
ID="$(tg_invia_e_fissa "$(testo_tick "$DURATA")")"
if [ -z "$ID" ]; then
    echo "Invio fallito, mi fermo." >&2
    exit 1
fi
echo "Messaggio pinnato: id=$ID"

RESTANO="$DURATA"
while [ "$RESTANO" -gt 0 ]; do
    sleep 1
    RESTANO=$((RESTANO - 1))
    tg_modifica "$ID" "$(testo_tick "$RESTANO")" || exit 1
done

echo "Messaggio finale..."
tg_modifica "$ID" "🧪 PROVA — countdown pinnato

Collaudo completato: $((DURATA + 1)) modifiche sullo stesso messaggio
(id $ID), un tick al secondo, mai duplicato. Nessun deploy è avvenuto." || exit 1

echo "Sblocco il messaggio..."
tg_sblocca "$ID"

echo "OK Collaudo countdown al secondo completato ($((DURATA + 1)) modifiche)."
