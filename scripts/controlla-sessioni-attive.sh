#!/usr/bin/env bash
# Sotto-step 4/5 del punto 6 del ciclo di automazione (deploy): prima di
# fermare il bot per uno swap, aspetta che nessuna chat sia "in attesa di
# input testuale" -- verificato interrogando le dieci mappe di sessione
# indipendenti in main.rs, senza unificarle prima (deciso il 3 settembre
# 2026, docs/previsto/automazione-ciclo-sviluppo.md).
#
# Le mappe vivono solo nella memoria del processo Rust sull'S9: questo
# script non puo' leggerle direttamente, quindi manda SIGUSR1 al bot (via
# SSH), che alla ricezione scrive data/run/sessioni.txt con il numero di
# chat con una sessione attiva. Deciso insieme ad Alessio il 4 settembre
# 2026: un segnale su richiesta, non una scrittura periodica del bot --
# meno lavoro quando nessuno lo controlla, e un dato sempre fresco quando
# serve davvero.
#
# Va lanciato da questa macchina (il PC), non sull'S9: usa `ssh s9`, come
# telegram-api.sh e collauda-remoto.sh.
#
# Uso:
#   ./scripts/controlla-sessioni-attive.sh [--timeout SECONDI] [--intervallo SECONDI]
#       default: timeout 120s, intervallo 5s tra un controllo e il successivo.
#
# Uscita:
#   0  nessuna sessione attiva (libero per lo stop), oppure timeout
#      raggiunto -- si procede comunque, come deciso nella specifica.
#      Il messaggio stampato dice quale dei due casi si e' verificato.
#   1  errore: il bot non risulta avviato da avvia-bot.sh, o il processo
#      non e' vivo, o SIGUSR1 non puo' essere mandato.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

CARTELLA_RUN_S9="~/gestionale-casa/data/run"
TIMEOUT=120
INTERVALLO=5

while [ $# -gt 0 ]; do
    case "$1" in
        --timeout)
            TIMEOUT="${2:-120}"
            shift 2
            ;;
        --intervallo)
            INTERVALLO="${2:-5}"
            shift 2
            ;;
        *)
            echo "Argomento non riconosciuto: $1" >&2
            exit 1
            ;;
    esac
done

# Un solo comando SSH per giro invece di tre (leggi PID, manda segnale,
# leggi file): meno round-trip di rete, e il segnale e la lettura restano
# vicini nel tempo sulla stessa connessione.
controlla_una_volta() {
    ssh s9 "
        set -u
        cd ~/gestionale-casa || exit 1
        pidfile='$CARTELLA_RUN_S9/bot.pid'
        statofile='$CARTELLA_RUN_S9/sessioni.txt'
        if [ ! -f \"\$pidfile\" ]; then
            echo 'ERRORE: nessun file PID, il bot non risulta avviato da avvia-bot.sh.' >&2
            exit 2
        fi
        pid=\"\$(cat \"\$pidfile\")\"
        if [ -z \"\$pid\" ] || ! kill -0 \"\$pid\" 2>/dev/null; then
            echo \"ERRORE: il processo (pid \$pid) non e' vivo.\" >&2
            exit 2
        fi
        kill -USR1 \"\$pid\" || { echo 'ERRORE: impossibile mandare SIGUSR1.' >&2; exit 2; }
        sleep 1
        if [ ! -f \"\$statofile\" ]; then
            echo 'ATTESA: il bot non ha ancora scritto il file di stato.'
            exit 1
        fi
        head -n 1 \"\$statofile\"
    "
}

TRASCORSO=0
while true; do
    OUTPUT="$(controlla_una_volta)"
    ESITO=$?

    if [ "$ESITO" = "2" ]; then
        echo "$OUTPUT" >&2
        exit 1
    fi

    if [ "$ESITO" = "0" ] && [ "$OUTPUT" = "0" ]; then
        echo "OK Nessuna sessione attiva, si puo' fermare il bot."
        exit 0
    fi

    if [ "$ESITO" = "0" ]; then
        echo "Sessioni attive: ${OUTPUT:-?}. Aspetto ancora..."
    else
        echo "$OUTPUT"
    fi

    if [ "$TRASCORSO" -ge "$TIMEOUT" ]; then
        echo "Timeout (${TIMEOUT}s) raggiunto con sessioni ancora attive: procedo comunque con lo stop."
        exit 0
    fi
    sleep "$INTERVALLO"
    TRASCORSO=$((TRASCORSO + INTERVALLO))
done
