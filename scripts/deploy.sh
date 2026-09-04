#!/usr/bin/env bash
# Sotto-step 5d del punto 6 del ciclo di automazione (deploy a downtime
# minimo): lega insieme i pezzi già costruiti e collaudati singolarmente
# nei sotto-step 1-5c in un'unica sequenza. Punto 6 di
# docs/previsto/automazione-ciclo-sviluppo.md.
#
# Va lanciato dal PC (non sull'S9): come gli altri script del ciclo, usa
# `ssh s9` per ogni passo remoto.
#
# Sequenza:
#   1. countdown-manutenzione.sh   -- avvisa (sotto-step 1)
#   2. controlla-sessioni-attive.sh -- aspetta che nessuno stia scrivendo (4)
#   3. salva-binario.sh            -- PRIMA di aggiornare il codice (5b)
#   4. ferma-bot.sh                -- ferma il vecchio processo (2)
#   5. aggiorna-s9.sh --ramo X --solo-controlli -- codice nuovo, compilato
#      e verificato di nuovo sull'S9 (stessa pipeline del punto 4 del
#      ciclo), non un `git pull` nudo: il binario appena compilato resta
#      pronto per il passo 6 senza ricompilare.
#   6. copia il riepilogo/checklist sull'S9, poi avvia-bot.sh --riservato
#      (5a + 5c: il bot nuovo manda da solo il messaggio di collaudo)
#   7. controllo di salute: online + resta vivo per una finestra di
#      stabilità. Se fallisce: rollback-binario.sh (5b) e notifica
#      l'errore preciso -- l'S9 non deve mai restare giù in silenzio.
#
# Da qui l'agente si ferma: il collaudo guidato (checklist, conferma) lo
# gestisce il bot stesso (5c). Il seguito (merge su main o rollback in
# base all'esito) è scripts/completa-deploy.sh, lanciato separatamente
# quando arriva la conferma.
#
# Uso:
#   ./scripts/deploy.sh --ramo NOME_RAMO --riepilogo FILE_LOCALE [--minuti N] [--finestra-stabilita SECONDI]
#
#   --ramo                 il ramo da mettere in produzione sull'S9
#   --riepilogo             file locale (sul PC) con riepilogo + checklist,
#                           stesso formato di src/modules/collaudo.rs:
#                           testo libero, riga "---CHECKLIST---", una voce
#                           per riga
#   --minuti                override dei minuti di countdown (facoltativo)
#   --finestra-stabilita    secondi di attesa dopo "online" prima di
#                           considerare il nuovo processo sano (default 20)

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
source ./scripts/telegram-api.sh

RAMO=""
RIEPILOGO_LOCALE=""
MINUTI=""
FINESTRA_STABILITA=20

while [ $# -gt 0 ]; do
    case "$1" in
        --ramo)
            RAMO="${2:-}"
            shift 2
            ;;
        --riepilogo)
            RIEPILOGO_LOCALE="${2:-}"
            shift 2
            ;;
        --minuti)
            MINUTI="${2:-}"
            shift 2
            ;;
        --finestra-stabilita)
            FINESTRA_STABILITA="${2:-20}"
            shift 2
            ;;
        *)
            echo "Argomento non riconosciuto: $1" >&2
            exit 1
            ;;
    esac
done

if [ -z "$RAMO" ]; then
    echo "Serve --ramo NOME_RAMO." >&2
    exit 1
fi
if [ -z "$RIEPILOGO_LOCALE" ] || [ ! -f "$RIEPILOGO_LOCALE" ]; then
    echo "Serve --riepilogo FILE_LOCALE (file esistente)." >&2
    exit 1
fi
if ! grep -q -- "---CHECKLIST---" "$RIEPILOGO_LOCALE"; then
    echo "Il file di riepilogo non contiene la riga '---CHECKLIST---': non verrebbe interpretato dal bot." >&2
    exit 1
fi

echo "Leggo le credenziali dall'S9..."
tg_leggi_credenziali || exit 1

# Ogni passo che fallisce avvisa su Telegram con l'errore preciso, poi
# esce: mai un fallimento silenzioso (regola della specifica).
fallisci() {
    local motivo="$1"
    echo "FERMO: $motivo" >&2
    tg_invia "🔴 Deploy fermato

$motivo" >/dev/null 2>&1 || true
    exit 1
}

echo "--- 1/7 Countdown di manutenzione ---"
./scripts/countdown-manutenzione.sh ${MINUTI:+--minuti "$MINUTI"} >/dev/null \
    || fallisci "Il countdown di manutenzione è fallito."

echo "--- 2/7 Controllo sessioni attive ---"
./scripts/controlla-sessioni-attive.sh \
    || fallisci "Il controllo delle sessioni attive è fallito."

echo "--- 3/7 Salvataggio del binario corrente (per un rollback senza ricompilazione) ---"
ssh s9 "cd ~/gestionale-casa && ./scripts/salva-binario.sh" \
    || fallisci "Impossibile salvare il binario corrente prima dello swap."

echo "--- 4/7 Arresto del processo attuale ---"
ssh s9 "cd ~/gestionale-casa && ./scripts/ferma-bot.sh" \
    || fallisci "Impossibile fermare il processo attuale in modo pulito."

echo "--- 5/7 Aggiornamento e verifica del codice nuovo sull'S9 ---"
ssh s9 "cd ~/gestionale-casa && ./scripts/aggiorna-s9.sh --ramo '$RAMO' --solo-controlli" \
    || fallisci "La verifica del codice nuovo sull'S9 (build/clippy/test/migration) è fallita. Il vecchio binario resta salvato per il rollback, ma NON è stato riavviato: intervento manuale necessario."

echo "--- 6/7 Avvio del binario nuovo, in modalità riservata ---"
scp "$RIEPILOGO_LOCALE" s9:~/gestionale-casa/data/run/riepilogo_deploy.txt \
    || fallisci "Impossibile copiare il riepilogo/checklist sull'S9."
if ! ssh s9 "cd ~/gestionale-casa && ./scripts/avvia-bot.sh --riservato"; then
    echo "Avvio fallito: rollback al binario precedente..." >&2
    ssh s9 "cd ~/gestionale-casa && ./scripts/rollback-binario.sh" \
        || fallisci "Avvio del binario nuovo fallito, E il rollback al binario precedente È FALLITO. L'S9 potrebbe essere giù: intervento manuale urgente."
    fallisci "Avvio del binario nuovo fallito. Rollback al binario precedente riuscito, l'S9 è di nuovo online con la versione di prima."
fi

echo "--- 7/7 Controllo di stabilità (${FINESTRA_STABILITA}s) ---"
sleep "$FINESTRA_STABILITA"
if ! ssh s9 "cd ~/gestionale-casa && PID=\$(cat data/run/bot.pid 2>/dev/null) && [ -n \"\$PID\" ] && kill -0 \"\$PID\" 2>/dev/null"; then
    echo "Il processo nuovo è morto durante la finestra di stabilità: rollback..." >&2
    ssh s9 "cd ~/gestionale-casa && ./scripts/rollback-binario.sh" \
        || fallisci "Il binario nuovo è andato in errore subito dopo l'avvio, E il rollback al binario precedente È FALLITO. L'S9 potrebbe essere giù: intervento manuale urgente."
    fallisci "Il binario nuovo è andato in errore subito dopo l'avvio (finestra di stabilità di ${FINESTRA_STABILITA}s). Rollback al binario precedente riuscito, l'S9 è di nuovo online con la versione di prima."
fi

echo "OK Deploy completato: il bot nuovo è online in modalità riservata, stabile da ${FINESTRA_STABILITA}s."
echo "Il bot ha mandato da solo il riepilogo e la checklist all'amministratore principale."
echo "Quando arriva la conferma (o il rifiuto), lancia: ./scripts/completa-deploy.sh --ramo '$RAMO'"
