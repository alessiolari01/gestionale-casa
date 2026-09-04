#!/usr/bin/env bash
# Sotto-step 5b del punto 6 del ciclo di automazione (deploy): rollback
# senza ricompilazione. Ferma quello che sta girando ed esegue direttamente
# l'ultimo binario salvato da scripts/salva-binario.sh
# (data/run/binario_precedente), bypassando `cargo build`/`cargo run` --
# un rollback dev'essere istantaneo, non aspettare una build (che
# potrebbe anche essere quella appena rivelatasi rotta).
#
# Il binario ripristinato torna in modalita' normale (senza RISERVATO):
# era gia' la versione in produzione prima dello swap fallito, non ha
# bisogno di ripartire riservata.
#
# Stesso schema di processo di avvia-bot.sh (nohup+disown, PID in
# data/run/bot.pid, log in data/run/bot.out): dopo un rollback,
# ferma-bot.sh continua a funzionare come sempre, senza saperne nulla.
#
# Uso:
#   ./scripts/rollback-binario.sh

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

CARTELLA_RUN="$PWD/data/run"
PIDFILE="$CARTELLA_RUN/bot.pid"
LOGFILE="$CARTELLA_RUN/bot.out"
BINARIO="$CARTELLA_RUN/binario_precedente"

if [ ! -x "$BINARIO" ]; then
    echo "Nessun binario precedente salvato in $BINARIO (scripts/salva-binario.sh non è mai stato lanciato con successo)." >&2
    exit 1
fi

# Ferma quello che sta girando adesso, se qualcosa gira: Telegram rifiuta
# un secondo long-polling con lo stesso token (409 Conflict), quindi il
# vecchio processo deve essere morto prima di avviare quello ripristinato.
if [ -f "$PIDFILE" ]; then
    VECCHIO_PID="$(cat "$PIDFILE" 2>/dev/null)"
    if [ -n "$VECCHIO_PID" ] && kill -0 "$VECCHIO_PID" 2>/dev/null; then
        echo "Fermo il processo attuale (pid $VECCHIO_PID) prima del rollback..."
        kill -INT "$VECCHIO_PID"
        ATTESA=0
        while [ "$ATTESA" -lt 30 ]; do
            if ! kill -0 "$VECCHIO_PID" 2>/dev/null; then
                break
            fi
            sleep 1
            ATTESA=$((ATTESA + 1))
        done
        if kill -0 "$VECCHIO_PID" 2>/dev/null; then
            echo "Il processo (pid $VECCHIO_PID) non si è fermato in 30s, lo termino con forza." >&2
            kill -KILL "$VECCHIO_PID" 2>/dev/null
            sleep 1
        fi
    fi
    rm -f "$PIDFILE"
fi

echo "Avvio il binario precedente (rollback, log in $LOGFILE)..."
nohup "$BINARIO" > "$LOGFILE" 2>&1 &
PID=$!
disown

echo "$PID" > "$PIDFILE"
echo "PID: $PID"

# Nessuna build da aspettare: se non e' online in pochi secondi, qualcosa
# non va anche col binario salvato -- timeout molto più corto di
# avvia-bot.sh (180s), che invece deve coprire una `cargo build` vera.
ATTESA=0
MASSIMO=30
while [ "$ATTESA" -lt "$MASSIMO" ]; do
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "Il processo è morto durante l'avvio del rollback. Ultime righe del log:" >&2
        tail -n 30 "$LOGFILE" >&2
        rm -f "$PIDFILE"
        exit 1
    fi
    if grep -q "Gestionale Casa online" "$LOGFILE" 2>/dev/null; then
        echo "OK Rollback completato, binario precedente online (pid $PID)."
        exit 0
    fi
    sleep 1
    ATTESA=$((ATTESA + 1))
done

echo "Il processo è ancora vivo (pid $PID) ma non ho visto 'Gestionale Casa" >&2
echo "online' nel log entro ${MASSIMO}s. Controlla $LOGFILE a mano." >&2
exit 1
