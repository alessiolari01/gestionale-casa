#!/usr/bin/env bash
# Sotto-step 5d del punto 6 del ciclo di automazione: il seguito di
# scripts/deploy.sh, lanciato quando arriva la conferma o il rifiuto del
# collaudo guidato (sotto-step 5c). Punto 9 della specifica.
#
# Legge data/run/esito_collaudo.txt sull'S9 (scritto dal bot quando
# l'amministratore principale preme "✅ Confermo, funziona" o
# "❌ Non funziona"):
#   - "confermato": procede al merge del ramo su `main` (git, sul PC) e
#     avvisa. Il bot è già uscito da solo dalla modalità riservata (lo fa
#     il bottone di conferma stesso).
#   - "rifiutato": rollback al binario precedente (scripts/rollback-binario.sh)
#     e avvisa -- resta in modalità manutenzione per gli utenti normali,
#     come da specifica, finché non c'è una nuova versione pronta.
#   - file assente: il collaudo è ancora in corso, nessuna azione.
#
# Il merge su `main` è un'azione con conseguenze reali (pubblica il ramo):
# per questo lo script lo fa solo con --confermo-merge esplicito. Senza
# quel flag, in caso di "confermato" descrive solo cosa farebbe.
#
# Uso:
#   ./scripts/completa-deploy.sh --ramo NOME_RAMO [--confermo-merge]

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1
source ./scripts/telegram-api.sh

RAMO=""
CONFERMO_MERGE=0

while [ $# -gt 0 ]; do
    case "$1" in
        --ramo)
            RAMO="${2:-}"
            shift 2
            ;;
        --confermo-merge)
            CONFERMO_MERGE=1
            shift
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

ESITO="$(ssh s9 "cat ~/gestionale-casa/data/run/esito_collaudo.txt 2>/dev/null" || true)"

if [ -z "$ESITO" ]; then
    echo "Nessun esito ancora: il collaudo guidato è probabilmente ancora in corso."
    exit 2
fi

echo "Leggo le credenziali dall'S9..."
tg_leggi_credenziali || exit 1

case "$ESITO" in
    confermato)
        echo "Collaudo confermato."
        if [ "$CONFERMO_MERGE" != "1" ]; then
            echo "Non lancio il merge senza --confermo-merge esplicito. Per completare:"
            echo "  ./scripts/completa-deploy.sh --ramo '$RAMO' --confermo-merge"
            exit 0
        fi
        echo "Merge di '$RAMO' su main..."
        git fetch origin || { echo "git fetch fallito." >&2; exit 1; }
        git checkout main || { echo "git checkout main fallito." >&2; exit 1; }
        git pull --ff-only origin main || { echo "git pull main fallito." >&2; exit 1; }
        git merge --no-ff "origin/$RAMO" -m "Merge $RAMO: collaudo confermato su Telegram" \
            || { echo "git merge fallito, risolvi a mano." >&2; exit 1; }
        git push origin main || { echo "git push main fallito." >&2; exit 1; }
        ssh s9 "rm -f ~/gestionale-casa/data/run/esito_collaudo.txt ~/gestionale-casa/data/run/riepilogo_deploy.txt" || true
        tg_invia "✅ Deploy completato

Il ramo '$RAMO' è stato unito a main dopo la tua conferma." >/dev/null
        echo "OK Merge completato e pushato."
        ;;
    rifiutato)
        echo "Collaudo rifiutato: rollback al binario precedente, riservato (nessuna versione corretta ancora pronta)..."
        # Il riepilogo va tolto PRIMA del rollback, non dopo: il binario
        # precedente riparte comunque riservato (--riservato qui sotto), e
        # se trovasse ancora quel file manderebbe da solo un messaggio di
        # collaudo che parla della versione appena rifiutata.
        ssh s9 "rm -f ~/gestionale-casa/data/run/esito_collaudo.txt ~/gestionale-casa/data/run/riepilogo_deploy.txt" || true
        ssh s9 "cd ~/gestionale-casa && ./scripts/rollback-binario.sh --riservato" \
            || {
                tg_invia "🔴 Rollback fallito

Il collaudo è stato rifiutato, ma il rollback al binario precedente è fallito. L'S9 potrebbe essere in uno stato incoerente: intervento manuale urgente." >/dev/null 2>&1 || true
                echo "FERMO: rollback fallito." >&2
                exit 1
            }
        tg_invia "↩️ Deploy rifiutato

Il collaudo non ha funzionato: rollback completato, il gestionale è tornato alla versione precedente. Resta in modalità manutenzione per gli utenti normali finché non arriva una versione corretta." >/dev/null
        echo "OK Rollback completato."
        ;;
    *)
        echo "Esito non riconosciuto: '$ESITO'." >&2
        exit 1
        ;;
esac
