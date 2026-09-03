#!/usr/bin/env bash
# Collaudo remoto sull'S9 via SSH, punto 4 del ciclo in
# docs/previsto/automazione-ciclo-sviluppo.md: l'agente si collega e lancia
# aggiorna-s9.sh, invece di chiedere al proprietario del progetto di farlo a
# mano da Termux.
#
# Usa sempre --solo-controlli: questo passo verifica che il codice compili,
# passi Clippy/test e non rompa le migration -- non avvia mai il bot. Avviare
# davvero il binario e' il passo 6 del ciclo (swap), non questo.
#
# Uso:
#   ./scripts/collauda-remoto.sh [ramo]
#
# Senza argomenti usa il ramo Git corrente sul PC. --ramo non e' opzionale
# nella chiamata ad aggiorna-s9.sh qui sotto, per lo stesso motivo per cui
# non lo e' a mano (STATO.md, sezione 7): senza, l'S9 aggiorna solo il ramo
# su cui si trova gia' e il collaudo passerebbe sul codice sbagliato senza
# che nulla lo segnali.
#
# Uscita:
#   0  collaudo passato (compilazione, Clippy, test, migration di prova)
#   1  collaudo fallito, o non e' stato possibile eseguirlo (SSH, ramo non
#      pushato, albero di lavoro sull'S9 non pulito, eccetera)
#
# Nota operativa: al termine l'S9 resta sul ramo appena collaudato, non
# torna a quello precedente -- e' lo stesso comportamento di aggiorna-s9.sh
# quando lo lancia una persona. Il passo di deploy vero (6 del ciclo) parte
# da questo stesso stato, quindi non e' un difetto da correggere qui.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

RAMO="${1:-}"
if [ -z "$RAMO" ]; then
    RAMO="$(git rev-parse --abbrev-ref HEAD)"
fi
if [ -z "$RAMO" ] || [ "$RAMO" = "HEAD" ]; then
    echo "Impossibile determinare il ramo; passalo come argomento." >&2
    exit 1
fi

# Il collaudo remoto ha senso solo su un ramo che l'S9 puo' davvero vedere:
# gira contro GitHub, non contro l'albero locale del PC (STATO.md, sezione 4
# -- "l'S9 non vede le modifiche finche' non sono su GitHub").
echo "Verifico che '$RAMO' sia stato pushato..."
if ! git ls-remote --exit-code --heads origin "$RAMO" >/dev/null 2>&1; then
    echo "Il ramo '$RAMO' non esiste su origin. Pusha prima di collaudare da remoto." >&2
    exit 1
fi
LOCALE="$(git rev-parse "$RAMO" 2>/dev/null || true)"
REMOTO="$(git ls-remote origin "refs/heads/$RAMO" | cut -f1)"
if [ -n "$LOCALE" ] && [ "$LOCALE" != "$REMOTO" ]; then
    echo "ATTENZIONE: il ramo locale '$RAMO' ($LOCALE) e origin ($REMOTO) non coincidono." >&2
    echo "Il collaudo userà quello che c'e' su GitHub, non quello che vedi qui: pusha prima." >&2
fi

echo "Collaudo remoto su S9, ramo '$RAMO' (--solo-controlli, il bot non parte)..."
echo

ssh s9 "cd ~/gestionale-casa && ./scripts/aggiorna-s9.sh --ramo '$RAMO' --solo-controlli"
ESITO=$?

echo
if [ "$ESITO" -eq 0 ]; then
    echo "OK Collaudo remoto passato su '$RAMO'."
else
    echo "NO Collaudo remoto fallito su '$RAMO' (uscita $ESITO). Vedi l'output sopra e il log su S9." >&2
fi
exit "$ESITO"
