#!/usr/bin/env bash
# Aggiornamento e avvio del gestionale sul Galaxy S9.
#
# Sostituisce il vecchio giro zip -> scp -> unzip -> installer python:
# il trasporto e' git, quindi basta che il branch sia stato pushato dal PC.
#
# Uso:
#   ./scripts/aggiorna-s9.sh                 aggiorna, verifica e avvia il bot
#   ./scripts/aggiorna-s9.sh --solo-controlli   si ferma prima dell'avvio
#
# Regola del progetto: niente `set -e`, ogni passo si ferma con `|| exit 1`
# in modo che un errore interrompa lo script senza chiudere la sessione SSH.

PROGETTO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="$PROGETTO/data/db/gestionale.db"
SOLO_CONTROLLI=0
[ "$1" = "--solo-controlli" ] && SOLO_CONTROLLI=1

passo() { echo; echo "===== $* ====="; }

cd "$PROGETTO" || exit 1

# --------------------------------------------------------------------------
passo "1/7  aggiornamento del codice"
# --------------------------------------------------------------------------
RAMO="$(git rev-parse --abbrev-ref HEAD)" || exit 1
echo "ramo corrente: $RAMO"
if [ -n "$(git status --porcelain)" ]; then
    echo "ATTENZIONE: ci sono modifiche locali non committate."
    git status --short
    echo "Committale o mettile da parte con 'git stash' prima di continuare."
    exit 1
fi
git pull --ff-only || exit 1
echo "ora a: $(git log --oneline -1)"

# --------------------------------------------------------------------------
passo "2/7  impostazioni per non esaurire la memoria in compilazione"
# --------------------------------------------------------------------------
# Sull'S9 il linker viene ucciso senza queste: il binario di test contiene
# tutto il progetto in un solo eseguibile.
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0
export CARGO_PROFILE_DEV_DEBUG=0
export CARGO_PROFILE_DEV_CODEGEN_UNITS=16
export CARGO_PROFILE_TEST_DEBUG=0
export CARGO_PROFILE_TEST_CODEGEN_UNITS=16
export RUSTFLAGS="-C debuginfo=0 -C link-arg=-Wl,--threads=1"
echo "impostate"

# --------------------------------------------------------------------------
passo "3/7  formattazione e compilazione"
# --------------------------------------------------------------------------
cargo fmt --all || exit 1
cargo fmt --all -- --check || exit 1
git diff --check || exit 1
cargo check --locked || exit 1

# --------------------------------------------------------------------------
passo "4/7  Clippy e test"
# --------------------------------------------------------------------------
cargo clippy --all-targets --locked -- -D warnings || exit 1
cargo test --locked -- --test-threads=1 || exit 1

# --------------------------------------------------------------------------
passo "5/7  backup del database e controlli di integrita'"
# --------------------------------------------------------------------------
if [ ! -f "$DB" ]; then
    echo "Database non trovato: $DB"
    exit 1
fi
TS="$(date +%Y%m%d_%H%M%S)"
BACKUP="$PROGETTO/data/db/gestionale_pre_${TS}.db"
COPIA="$PROGETTO/data/db/gestionale_prova_${TS}.db"
# `.backup` usa l'API di SQLite: consistente anche con il bot in esecuzione.
sqlite3 "$DB" ".backup '$BACKUP'" || exit 1
cp "$BACKUP" "$COPIA" || exit 1
echo "backup: $(basename "$BACKUP")"
sqlite3 "$DB" "PRAGMA integrity_check;" || exit 1
sqlite3 "$DB" "PRAGMA foreign_key_check;" || exit 1

# --------------------------------------------------------------------------
passo "6/7  prova delle migration non ancora applicate, su una copia"
# --------------------------------------------------------------------------
# Le versioni gia' applicate vengono lette dal database reale, cosi' questo
# script non va piu' modificato a ogni nuovo step.
APPLICATE=" $(sqlite3 "$DB" "SELECT version FROM _sqlx_migrations;" | tr '\n' ' ') "
DA_APPLICARE=0
for FILE in "$PROGETTO"/migrations/*.sql; do
    NOME="$(basename "$FILE")"
    VERSIONE="${NOME%%_*}"
    case "$VERSIONE" in
        ''|*[!0-9]*) continue ;;   # ignora README e file non versionati
    esac
    case "$APPLICATE" in
        *" $VERSIONE "*) continue ;;
    esac
    DA_APPLICARE=$((DA_APPLICARE + 1))
    echo "  provo: $NOME"
    sqlite3 "$COPIA" < "$FILE" || exit 1
done
if [ "$DA_APPLICARE" -eq 0 ]; then
    echo "  nessuna migration nuova"
else
    echo "  $DA_APPLICARE migration applicate alla copia, controllo l'esito"
    sqlite3 "$COPIA" "PRAGMA integrity_check;" || exit 1
    sqlite3 "$COPIA" "PRAGMA foreign_key_check;" || exit 1
fi
rm -f "$COPIA"

# --------------------------------------------------------------------------
passo "7/7  avvio"
# --------------------------------------------------------------------------
if [ "$SOLO_CONTROLLI" -eq 1 ]; then
    echo "richiesto --solo-controlli: mi fermo qui."
    if [ "$DA_APPLICARE" -gt 0 ]; then
        echo "Ricorda: $DA_APPLICARE migration verranno applicate al database"
        echo "reale al primo 'cargo run'."
    fi
    exit 0
fi
if [ "$DA_APPLICARE" -gt 0 ]; then
    echo "L'avvio applichera' $DA_APPLICARE migration al database reale."
    echo "Il backup e' in $(basename "$BACKUP")."
fi
cargo run --locked
