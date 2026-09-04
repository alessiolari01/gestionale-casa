#!/usr/bin/env bash
# Sotto-step 5b del punto 6 del ciclo di automazione (deploy): conserva il
# binario attualmente compilato come fallback per un rollback istantaneo,
# prima di aggiornare il codice e ricompilare per lo swap.
#
# Deciso insieme ad Alessio il 4 settembre 2026: una copia del binario, non
# due cartelle di lavoro separate (piu' robusto ma piu' spazio/complessita'
# su un telefono). Un rollback senza ricompilazione richiede che il
# binario vecchio sia gia' pronto da qualche parte: se si aspettasse una
# `cargo build` per tornare indietro, si aspetterebbe la stessa build che
# potrebbe essere quella rotta.
#
# Fondamentale l'ORDINE d'uso: va lanciato mentre target/debug/ corrisponde
# ancora al codice IN ESECUZIONE -- cioe' prima di `git pull`/`cargo build`
# per la versione nuova, non dopo. Lanciarlo dopo la build nuova
# salverebbe il binario sbagliato.
#
# Uso:
#   ./scripts/salva-binario.sh

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

CARTELLA_RUN="$PWD/data/run"
SORGENTE="$PWD/target/debug/gestionale-casa"
DESTINAZIONE="$CARTELLA_RUN/binario_precedente"

mkdir -p "$CARTELLA_RUN" || exit 1

if [ ! -x "$SORGENTE" ]; then
    echo "Nessun binario compilato in $SORGENTE (mai buildato con 'cargo build'/'cargo run'?)." >&2
    exit 1
fi

cp "$SORGENTE" "$DESTINAZIONE" || {
    echo "Copia fallita." >&2
    exit 1
}
chmod +x "$DESTINAZIONE"

DIMENSIONE="$(du -h "$DESTINAZIONE" 2>/dev/null | cut -f1)"
echo "OK Binario precedente salvato: $DESTINAZIONE (${DIMENSIONE:-?})"
