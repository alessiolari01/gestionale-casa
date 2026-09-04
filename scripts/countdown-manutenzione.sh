#!/usr/bin/env bash
# Sotto-step 5d del punto 6 del ciclo di automazione (deploy): countdown
# reale di manutenzione, usato dall'orchestrazione dello swap
# (scripts/deploy.sh). Stessa meccanica gia' collaudata in
# prova-countdown.sh -- scadenza fissa (secondi epoch), non un contatore
# decrementato a ogni giro, dopo il bug trovato e corretto il 4 settembre
# 2026 -- ma letta dal default configurato nella schermata admin
# 🚀 Distribuzione (tabella impostazioni_distribuzione) invece di un valore
# fisso da riga di comando, con un override opzionale per la scelta
# puntuale del singolo deploy (non ancora offerta da un'interfaccia: per
# ora solo --minuti).
#
# Uso:
#   ./scripts/countdown-manutenzione.sh [--minuti N]
#
#   Senza --minuti, legge il default dalla tabella
#   impostazioni_distribuzione sull'S9:
#     - tipo "subito":      un solo messaggio, nessuna attesa;
#     - tipo "countdown":   il countdown al secondo, come sempre;
#     - tipo "programmato": aspetta fino a quell'orario (nessun countdown
#                           visibile nel frattempo), poi un messaggio come
#                           "subito". Percorso meno collaudato degli altri
#                           due: il default operativo del progetto e'
#                           "countdown", verificato per davvero.
#
# Stampa l'id del messaggio finale su stdout (ultima riga), cosi' chi
# chiama puo' modificarlo o eliminarlo in seguito.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
source ./scripts/telegram-api.sh

MINUTI_OVERRIDE=""
if [ "${1:-}" = "--minuti" ]; then
    MINUTI_OVERRIDE="${2:-}"
fi

echo "Leggo le credenziali dall'S9..." >&2
tg_leggi_credenziali || exit 1

echo "Leggo il default di distribuzione dall'S9..." >&2
RIGA="$(ssh s9 "cd ~/gestionale-casa && sqlite3 -separator '|' data/db/gestionale.db \"SELECT tipo_default, minuti_countdown_default, orario_programmato_default FROM impostazioni_distribuzione WHERE id = 1;\"")"
if [ -z "$RIGA" ]; then
    echo "Impossibile leggere il default di distribuzione dall'S9." >&2
    exit 1
fi
TIPO="$(echo "$RIGA" | cut -d'|' -f1)"
MINUTI_DB="$(echo "$RIGA" | cut -d'|' -f2)"
ORARIO_DB="$(echo "$RIGA" | cut -d'|' -f3)"

if [ -n "$MINUTI_OVERRIDE" ]; then
    TIPO="countdown"
    MINUTI_DB="$MINUTI_OVERRIDE"
fi

testo_tick() {
    echo "🚧 Manutenzione in arrivo

Il gestionale verrà aggiornato a breve. Prossimo tick tra ${1}s."
}

testo_subito() {
    echo "🚧 Manutenzione in corso

Il gestionale viene aggiornato ora."
}

case "$TIPO" in
    subito)
        echo "Tipo: subito." >&2
        ID="$(tg_invia "$(testo_subito)")"
        [ -n "$ID" ] || { echo "Invio fallito." >&2; exit 1; }
        echo "$ID"
        ;;
    programmato)
        if [ -z "$ORARIO_DB" ]; then
            echo "Tipo 'programmato' ma nessun orario configurato." >&2
            exit 1
        fi
        echo "Tipo: programmato, alle $ORARIO_DB (nessun countdown visibile nell'attesa)." >&2
        ORA_ORA="$(date +%H)"
        ORA_MIN="$(date +%M)"
        TARGET_ORA="${ORARIO_DB%%:*}"
        TARGET_MIN="${ORARIO_DB##*:}"
        SECONDI_ADESSO=$((10#$ORA_ORA * 3600 + 10#$ORA_MIN * 60))
        SECONDI_TARGET=$((10#$TARGET_ORA * 3600 + 10#$TARGET_MIN * 60))
        ATTESA=$((SECONDI_TARGET - SECONDI_ADESSO))
        if [ "$ATTESA" -lt 0 ]; then
            ATTESA=$((ATTESA + 86400))
        fi
        echo "Aspetto ${ATTESA}s fino alle $ORARIO_DB..." >&2
        sleep "$ATTESA"
        ID="$(tg_invia "$(testo_subito)")"
        [ -n "$ID" ] || { echo "Invio fallito." >&2; exit 1; }
        echo "$ID"
        ;;
    countdown | *)
        if [ -z "$MINUTI_DB" ] || [ "$MINUTI_DB" -le 0 ] 2>/dev/null; then
            MINUTI_DB=5
        fi
        DURATA=$((MINUTI_DB * 60))
        echo "Tipo: countdown, ${MINUTI_DB} minuti (${DURATA}s)." >&2

        echo "Invio il messaggio di countdown..." >&2
        ID="$(tg_invia "$(testo_tick "$DURATA")")"
        [ -n "$ID" ] || { echo "Invio fallito." >&2; exit 1; }

        SCADENZA=$(($(date +%s) + DURATA))
        RESTANO="$DURATA"
        while [ "$RESTANO" -gt 0 ]; do
            sleep 1
            RESTANO=$((SCADENZA - $(date +%s)))
            [ "$RESTANO" -lt 0 ] && RESTANO=0
            tg_modifica "$ID" "$(testo_tick "$RESTANO")" || exit 1
        done

        tg_modifica "$ID" "$(testo_subito)" || exit 1
        echo "$ID"
        ;;
esac
