#!/usr/bin/env bash
# Ferma il bot avviato da avvia-bot.sh, con lo stesso spegnimento pulito
# del Ctrl+C interattivo o di "🛠️ Amministrazione -> ⏻ Spegni gestionale"
# (docs/architettura.md, "Shutdown controllato": "equivalente allo
# shutdown controllato via Ctrl+C").
#
# Manda SIGINT, non SIGTERM: in src/main.rs il dispatcher e' collegato solo
# a `.enable_ctrlc_handler()`, che ascolta SIGINT. Un SIGTERM ucciderebbe
# il processo senza passare dal percorso che manda "🔴 Gestionale Casa è
# offline." agli amministratori e chiude il dispatcher in modo ordinato --
# verificato leggendo il codice, non per tentativi sul processo vero.
#
# Sotto-step 2/5 del punto 6 del ciclo (deploy).
#
# Uso:
#   ./scripts/ferma-bot.sh [--timeout SECONDI]     default 60

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

CARTELLA_RUN="$PWD/data/run"
PIDFILE="$CARTELLA_RUN/bot.pid"
LOGFILE="$CARTELLA_RUN/bot.out"
TIMEOUT=60

if [ "${1:-}" = "--timeout" ]; then
    TIMEOUT="${2:-60}"
fi

if [ ! -f "$PIDFILE" ]; then
    echo "Nessun file PID ($PIDFILE): il bot non risulta avviato da avvia-bot.sh." >&2
    exit 1
fi

PID="$(cat "$PIDFILE")"
if [ -z "$PID" ] || ! kill -0 "$PID" 2>/dev/null; then
    echo "Il processo (pid $PID) non è vivo. Rimuovo il file PID residuo." >&2
    rm -f "$PIDFILE"
    exit 1
fi

echo "Mando SIGINT al pid $PID (spegnimento pulito, come Ctrl+C)..."
kill -INT "$PID"

ATTESA=0
while [ "$ATTESA" -lt "$TIMEOUT" ]; do
    if ! kill -0 "$PID" 2>/dev/null; then
        rm -f "$PIDFILE"
        if grep -q "Gestionale Casa offline" "$LOGFILE" 2>/dev/null; then
            echo "OK Bot fermato con spegnimento pulito (pid $PID)."
        else
            echo "Bot fermato (pid $PID), ma 'Gestionale Casa offline' non è nel log -- controlla $LOGFILE."
        fi
        exit 0
    fi
    sleep 2
    ATTESA=$((ATTESA + 2))
done

echo "Il processo (pid $PID) non si è fermato entro ${TIMEOUT}s dopo SIGINT." >&2
echo "Non forzo lo spegnimento: controlla $LOGFILE e decidi a mano se serve altro." >&2
exit 1
