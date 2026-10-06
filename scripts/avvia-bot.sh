#!/usr/bin/env bash
# Avvia il bot sull'S9 in background, sopravvivendo alla chiusura della
# sessione SSH che lo ha lanciato -- nohup + disown, PID salvato in un
# file. Deciso il 4 settembre 2026: niente tmux/screen/supervisore nuovo,
# coerente con la scelta gia' fatta nel progetto di evitare complessita'
# extra (niente Docker, niente container -- docs/architettura.md, 2.4).
#
# Sotto-step 2/5 del punto 6 del ciclo (deploy). Lo usano il guardiano (a
# ogni riaccensione, compreso il cambio di database), aggiorna-s9.sh alla
# fine e l'agente via SSH. Avvia il binario gia' compilato: non compila mai.
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

# Il bot riparte di proposito: il guardiano puo' ricominciare a guardarlo.
rm -f "$CARTELLA_RUN/guardiano.pausa"

if [ -f "$PIDFILE" ]; then
    VECCHIO_PID="$(cat "$PIDFILE" 2>/dev/null)"
    if [ -n "$VECCHIO_PID" ] && kill -0 "$VECCHIO_PID" 2>/dev/null; then
        echo "Il bot risulta gia' in esecuzione (pid $VECCHIO_PID, $PIDFILE)." >&2
        exit 1
    fi
    echo "File PID residuo di un processo non piu' vivo, lo rimuovo." >&2
    rm -f "$PIDFILE"
fi

# Si avvia il binario gia' compilato, mai `cargo run`: dal 6 ottobre 2026
# il guardiano riaccende il bot anche a ogni cambio di database, e un
# `cargo run` che trova codice nuovo compila. Quel giorno un riavvio e'
# partito mentre aggiorna-s9.sh compilava: due compilazioni insieme hanno
# finito la memoria del telefono, sono state uccise tutte e due e il bot e'
# rimasto spento. Compilare e' compito di aggiorna-s9.sh, e solo suo.
BINARIO="$PWD/target/release/gestionale-casa"
if [ ! -x "$BINARIO" ]; then
    echo "Manca il binario $BINARIO: compilalo con ./scripts/aggiorna-s9.sh --solo-controlli." >&2
    exit 1
fi
if [ "$RISERVATO" = "1" ]; then
    echo "Avvio in background, modalita' riservata (nohup, log in $LOGFILE)..."
    RISERVATO=1 nohup "$BINARIO" > "$LOGFILE" 2>&1 &
else
    echo "Avvio in background (nohup, log in $LOGFILE)..."
    nohup "$BINARIO" > "$LOGFILE" 2>&1 &
fi
PID=$!
disown

echo "$PID" > "$PIDFILE"
echo "PID: $PID"

# "Gestionale Casa online" e' la riga vera che il codice scrive dopo
# essersi collegato a Telegram con successo (src/main.rs, dopo get_me()) --
# non e' un segnale inventato. Il bot puo' metterci qualche secondo (le
# migration, la rete del telefono), quindi non basta che il processo esista
# subito dopo averlo lanciato: si aspetta fino a un massimo, controllando
# che resti vivo nel frattempo.
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
