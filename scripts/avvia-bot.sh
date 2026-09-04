#!/usr/bin/env bash
# Avvia il bot sull'S9 in background, sopravvivendo alla chiusura della
# sessione SSH che lo ha lanciato -- nohup + disown, PID salvato in un
# file. Deciso il 4 settembre 2026: niente tmux/screen/supervisore nuovo,
# coerente con la scelta gia' fatta nel progetto di evitare complessita'
# extra (niente Docker, niente container -- docs/architettura.md, 2.4).
#
# Sotto-step 2/5 del punto 6 del ciclo (deploy). Non e' pensato per l'uso
# quotidiano sull'S9: li' resta aggiorna-s9.sh, che fa "cargo run" in
# foreground in una sessione Termux tenuta aperta. Questo script serve
# all'agente, via SSH, per lo swap del binario.
#
# Uso (sull'S9, o da remoto con: ssh s9 'cd ~/gestionale-casa && ./scripts/avvia-bot.sh'):
#   ./scripts/avvia-bot.sh
#   ./scripts/avvia-bot.sh --riservato   # sotto-step 5a: solo l'amministratore
#                                        # principale puo' usare il bot, gli
#                                        # altri vedono un avviso di manutenzione,
#                                        # finche' non si sblocca da un bottone
#                                        # in chat (nessun riavvio necessario)

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

CARTELLA_RUN="$PWD/data/run"
PIDFILE="$CARTELLA_RUN/bot.pid"
LOGFILE="$CARTELLA_RUN/bot.out"

RISERVATO=0
if [ "${1:-}" = "--riservato" ]; then
    RISERVATO=1
fi

mkdir -p "$CARTELLA_RUN" || exit 1

if [ -f "$PIDFILE" ]; then
    VECCHIO_PID="$(cat "$PIDFILE" 2>/dev/null)"
    if [ -n "$VECCHIO_PID" ] && kill -0 "$VECCHIO_PID" 2>/dev/null; then
        echo "Il bot risulta gia' in esecuzione (pid $VECCHIO_PID, $PIDFILE)." >&2
        exit 1
    fi
    echo "File PID residuo di un processo non piu' vivo, lo rimuovo." >&2
    rm -f "$PIDFILE"
fi

# Stesse variabili di aggiorna-s9.sh: proteggono il collegamento sull'S9
# (memoria in fase di link) anche per un avvio dato da questo script.
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0

if [ "$RISERVATO" = "1" ]; then
    echo "Avvio in background, modalita' riservata (nohup, log in $LOGFILE)..."
    RISERVATO=1 nohup cargo run --locked > "$LOGFILE" 2>&1 &
else
    echo "Avvio in background (nohup, log in $LOGFILE)..."
    nohup cargo run --locked > "$LOGFILE" 2>&1 &
fi
PID=$!
disown

echo "$PID" > "$PIDFILE"
echo "PID: $PID"

# "Gestionale Casa online" e' la riga vera che il codice scrive dopo
# essersi collegato a Telegram con successo (src/main.rs, dopo get_me()) --
# non e' un segnale inventato. cargo run puo' dover compilare, quindi non
# basta che il processo esista subito dopo averlo lanciato: si aspetta fino
# a un massimo, controllando che resti vivo nel frattempo.
ATTESA=0
MASSIMO=180
while [ "$ATTESA" -lt "$MASSIMO" ]; do
    if ! kill -0 "$PID" 2>/dev/null; then
        echo "Il processo e' morto durante l'avvio. Ultime righe del log:" >&2
        tail -n 30 "$LOGFILE" >&2
        rm -f "$PIDFILE"
        exit 1
    fi
    if grep -q "Gestionale Casa online" "$LOGFILE" 2>/dev/null; then
        echo "OK Bot online (pid $PID)."
        exit 0
    fi
    sleep 3
    ATTESA=$((ATTESA + 3))
done

echo "Il processo e' ancora vivo (pid $PID) ma non ho visto 'Gestionale Casa" >&2
echo "online' nel log entro ${MASSIMO}s. Controlla $LOGFILE a mano." >&2
exit 1
