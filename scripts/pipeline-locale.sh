#!/usr/bin/env bash
# La stessa sequenza di controlli di .github/workflows/ci.yml, nello stesso
# ordine, eseguita in locale prima di committare. E' il punto 2 del ciclo in
# docs/previsto/automazione-ciclo-sviluppo.md: "l'agente scrive il codice,
# fa girare fmt, check, clippy, test in locale."
#
# Non sostituisce la CI (punto aperto 1 di STATO.md: le toolchain sono
# diverse, un locale verde e' solo l'assenza di una brutta notizia) --
# la riduce a un controllo di velocita' prima del push, cosi' un errore
# banale non aspetta 3-4 minuti di Actions per farsi vedere.
#
# Uso:
#   ./scripts/pipeline-locale.sh
#       Solo i controlli. Uscita 0 se tutti verdi, altrimenti si ferma al
#       primo che fallisce e stampa quale.
#
#   ./scripts/pipeline-locale.sh --commit FILE_MESSAGGIO FILE... [--push]
#       Solo se tutti i controlli passano: `git add` dei FILE elencati,
#       `git commit -F FILE_MESSAGGIO`, poi stampa `git log --stat -1`
#       cosi' si vede subito se il commit contiene i file attesi (regola
#       gia' in vigore in STATO.md). Con --push, pusha anche.
#       Il messaggio lo scrive chi chiama questo script (l'agente, con
#       il "perche'"): lo script non ne genera uno da solo.
#
# Se un controllo fallisce, lo script si ferma li' -- "niente commit o push
# se la pipeline fallisce" e' gia' una regola operativa in STATO.md, qui
# diventa un fatto meccanico invece che una cosa da ricordarsi.

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

MODALITA_COMMIT=0
FILE_MESSAGGIO=""
PUSH=0
FILES=()

while [ $# -gt 0 ]; do
    case "$1" in
        --commit)
            MODALITA_COMMIT=1
            FILE_MESSAGGIO="${2:-}"
            shift 2
            ;;
        --push)
            PUSH=1
            shift
            ;;
        *)
            FILES+=("$1")
            shift
            ;;
    esac
done

if [ "$MODALITA_COMMIT" = "1" ]; then
    if [ -z "$FILE_MESSAGGIO" ] || [ ! -f "$FILE_MESSAGGIO" ]; then
        echo "--commit richiede un file di messaggio esistente come argomento." >&2
        exit 1
    fi
    if [ "${#FILES[@]}" -eq 0 ]; then
        echo "--commit richiede almeno un file da aggiungere." >&2
        exit 1
    fi
fi

passo() {
    local nome="$1"
    shift
    echo "--- $nome ---"
    if ! "$@"; then
        echo "FERMO: '$nome' ha fallito. Nessun commit, nessun push." >&2
        exit 1
    fi
}

# La coerenza dei documenti si controlla sull'albero tracciato da git
# (`git ls-files` dentro controlla-documenti.sh), non sulla copia di lavoro.
# In modalita' --commit un documento puo' rimandare a un file nuovo che
# ancora non e' tracciato: va quindi controllata *dopo* il `git add`, non
# prima, altrimenti un commit legittimo (file nuovo + doc che lo cita)
# fallirebbe sempre. Fuori da --commit non c'e' nessun add da aspettare,
# quindi si controlla subito come gli altri passi.
if [ "$MODALITA_COMMIT" != "1" ]; then
    passo "Coerenza dei documenti" ./scripts/controlla-documenti.sh
fi
passo "Formato" cargo fmt --all -- --check
passo "Check" cargo check --locked
passo "Test" cargo test --locked
passo "Clippy" cargo clippy --all-targets --locked -- -D warnings

echo "OK Pipeline locale verde."

if [ "$MODALITA_COMMIT" != "1" ]; then
    exit 0
fi

git add -- "${FILES[@]}" || {
    echo "git add fallito." >&2
    exit 1
}

if ! ./scripts/controlla-documenti.sh; then
    echo "FERMO: 'Coerenza dei documenti' ha fallito dopo git add. Nessun commit, nessun push." >&2
    git reset -- "${FILES[@]}" >/dev/null 2>&1
    exit 1
fi

git commit -F "$FILE_MESSAGGIO" || {
    echo "git commit fallito (o niente da committare)." >&2
    exit 1
}

echo "--- git log --stat -1 (verifica che siano i file attesi) ---"
git log --stat -1

if [ "$PUSH" = "1" ]; then
    RAMO="$(git rev-parse --abbrev-ref HEAD)"
    echo "--- push su $RAMO ---"
    git push -u origin "$RAMO" || {
        echo "git push fallito." >&2
        exit 1
    }
fi
