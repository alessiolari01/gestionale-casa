#!/usr/bin/env bash
# Guardiano del bot: controlla che il processo sia vivo e, se non lo e', lo
# riaccende. Gira sull'S9, da solo, anche a PC spento.
#
# Nasce da un guasto vero: il 24 settembre 2026 il bot e' andato in panic al
# primo click dopo un aggiornamento, il dispatcher di Telegram e' morto e il
# processo e' finito. Nessuno si e' accorto di niente per otto ore, perche'
# `avvia-bot.sh` accende il bot e poi non lo guarda piu'. Verificare che il
# bot "sia partito" non basta: va guardato mentre lavora.
#
# Cosa fa, ogni minuto:
#   1. legge data/run/bot.pid e controlla se quel processo esiste ancora;
#   2. se non esiste, salva le ultime righe di bot.out (cosi' la caduta si
#      puo' leggere dopo, invece di sparire con il riavvio) e richiama
#      avvia-bot.sh;
#   3. scrive ogni cosa che fa in data/log/guardiano.log.
#
# Cosa NON fa, di proposito:
#   - non riaccende un bot fermato a mano (`ferma-bot.sh` lascia un file di
#     pausa: fermare il bot per fare uno swap e' una scelta, non un guasto);
#   - non insiste all'infinito. Oltre 5 riaccensioni in mezz'ora si arrende e
#     lo scrive: se il bot cade sempre appena parte, riaccenderlo per sempre
#     nasconde il problema invece di risolverlo.
#
# Uso (sull'S9):
#   ./scripts/guardiano-bot.sh --avvia     # parte in background e resta
#   ./scripts/guardiano-bot.sh --ferma     # lo spegne
#   ./scripts/guardiano-bot.sh --stato     # dice se c'e' e cosa ha fatto
#   ./scripts/guardiano-bot.sh --una-volta # un solo controllo, senza loop
#
# Da remoto: ssh s9 'cd ~/gestionale-casa && ./scripts/guardiano-bot.sh --avvia'
#
# Non va usato insieme al ciclo di `scripts/termux-boot.sh`, che ha un suo
# riavvio automatico e lancia la build release: due supervisori vorrebbero
# dire due bot accesi, e Telegram ne accetta uno solo per token.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

CARTELLA_RUN="$PWD/data/run"
CARTELLA_LOG="$PWD/data/log"
PIDFILE_BOT="$CARTELLA_RUN/bot.pid"
LOG_BOT="$CARTELLA_RUN/bot.out"
PIDFILE_GUARDIANO="$CARTELLA_RUN/guardiano.pid"
PAUSA="$CARTELLA_RUN/guardiano.pausa"
LOG="$CARTELLA_LOG/guardiano.log"

INTERVALLO="${GUARDIANO_INTERVALLO:-60}"
MAX_RIACCENSIONI=5
FINESTRA_SECONDI=1800

mkdir -p "$CARTELLA_RUN" "$CARTELLA_LOG" || exit 1

adesso() { date '+%Y-%m-%d %H:%M:%S'; }

scrivi() { echo "$(adesso) $*" >>"$LOG"; }

bot_vivo() {
    local pid
    [ -f "$PIDFILE_BOT" ] || return 1
    pid="$(cat "$PIDFILE_BOT" 2>/dev/null)"
    [ -n "$pid" ] || return 1
    kill -0 "$pid" 2>/dev/null
}

# Le ultime righe del log del bot, senza i codici colore: e' l'unica traccia
# di come e' morto, e `avvia-bot.sh` riscrive bot.out da zero.
salva_ultime_righe() {
    local destinazione="$CARTELLA_LOG/caduta_$(date '+%Y%m%d_%H%M%S').log"
    if [ -f "$LOG_BOT" ]; then
        tail -80 "$LOG_BOT" | sed 's/\x1b\[[0-9;]*m//g' >"$destinazione" 2>/dev/null
        scrivi "ultime righe salvate in $destinazione"
        local panico
        panico="$(grep -m 1 -A 1 'panicked' "$destinazione" 2>/dev/null | tr '\n' ' ')"
        [ -n "$panico" ] && scrivi "causa probabile: $panico"
    fi
    # Al massimo venti referti: piu' vecchi di cosi' non servono a nessuno.
    ls -1t "$CARTELLA_LOG"/caduta_*.log 2>/dev/null | tail -n +21 | while read -r vecchio; do
        rm -f "$vecchio"
    done
}

controlla_una_volta() {
    if [ -f "$PAUSA" ]; then
        return 0
    fi
    if bot_vivo; then
        return 0
    fi
    scrivi "bot non vivo: provo a riaccenderlo"
    salva_ultime_righe

    # Quante riaccensioni nella finestra: se sono troppe il bot cade appena
    # parte, e continuare a riaccenderlo nasconde il guasto.
    local limite recenti
    limite=$(( $(date +%s) - FINESTRA_SECONDI ))
    recenti=0
    while read -r riga; do
        local quando
        quando="$(date -d "${riga:0:19}" +%s 2>/dev/null)" || continue
        [ "$quando" -ge "$limite" ] && recenti=$((recenti + 1))
    done < <(grep 'riaccensione riuscita\|riaccensione fallita' "$LOG" 2>/dev/null | tail -20)
    if [ "$recenti" -ge "$MAX_RIACCENSIONI" ]; then
        scrivi "MI ARRENDO: $recenti riaccensioni in meno di mezz'ora. Serve un occhio umano, non un altro riavvio."
        touch "$PAUSA"
        return 1
    fi

    if ./scripts/avvia-bot.sh >>"$LOG" 2>&1; then
        scrivi "riaccensione riuscita"
    else
        scrivi "riaccensione fallita"
    fi
}

case "${1:---avvia}" in
--una-volta)
    controlla_una_volta
    ;;
--avvia)
    if [ -f "$PIDFILE_GUARDIANO" ]; then
        vecchio="$(cat "$PIDFILE_GUARDIANO" 2>/dev/null)"
        if [ -n "$vecchio" ] && kill -0 "$vecchio" 2>/dev/null; then
            echo "Il guardiano c'e' gia' (pid $vecchio)." >&2
            exit 1
        fi
        echo "File PID residuo di un guardiano non piu' vivo, lo rimuovo."
        rm -f "$PIDFILE_GUARDIANO"
    fi
    rm -f "$PAUSA"
    scrivi "guardiano avviato (controllo ogni ${INTERVALLO}s)"
    nohup bash -c '
        while true; do
            "'"$PWD"'/scripts/guardiano-bot.sh" --una-volta
            sleep '"$INTERVALLO"'
        done
    ' >/dev/null 2>&1 &
    echo $! >"$PIDFILE_GUARDIANO"
    disown 2>/dev/null
    echo "OK guardiano avviato (pid $(cat "$PIDFILE_GUARDIANO")), log in $LOG."
    ;;
--ferma)
    if [ ! -f "$PIDFILE_GUARDIANO" ]; then
        echo "Nessun guardiano da fermare."
        exit 0
    fi
    pid="$(cat "$PIDFILE_GUARDIANO" 2>/dev/null)"
    if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
        # Il guardiano e' una shell con dentro un ciclo: si spegne il gruppo,
        # altrimenti il `sleep` resta orfano e il ciclo riparte.
        kill -- "-$pid" 2>/dev/null || kill "$pid" 2>/dev/null
        scrivi "guardiano fermato a mano"
        echo "OK guardiano fermato (pid $pid)."
    else
        echo "Il guardiano non era vivo."
    fi
    rm -f "$PIDFILE_GUARDIANO"
    ;;
--stato)
    if [ -f "$PIDFILE_GUARDIANO" ] && kill -0 "$(cat "$PIDFILE_GUARDIANO" 2>/dev/null)" 2>/dev/null; then
        echo "Guardiano attivo (pid $(cat "$PIDFILE_GUARDIANO"))."
    else
        echo "Guardiano NON attivo."
    fi
    if [ -f "$PAUSA" ]; then
        echo "In pausa: non riaccendera' il bot finche' il file $PAUSA esiste."
    fi
    if bot_vivo; then
        echo "Bot vivo (pid $(cat "$PIDFILE_BOT"))."
    else
        echo "Bot NON vivo."
    fi
    echo "--- ultime righe del log del guardiano ---"
    tail -10 "$LOG" 2>/dev/null || echo "(nessun log)"
    ;;
*)
    echo "Uso: $0 [--avvia|--ferma|--stato|--una-volta]" >&2
    exit 2
    ;;
esac
