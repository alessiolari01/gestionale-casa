#!/usr/bin/env bash
# Aggiornamento e avvio del gestionale sul Galaxy S9.
#
# Sostituisce il vecchio giro zip -> scp -> unzip -> installer python:
# il trasporto e' git, quindi basta che il branch sia stato pushato dal PC.
#
# Uso:
#   ./scripts/aggiorna-s9.sh                    aggiorna, verifica e avvia
#   ./scripts/aggiorna-s9.sh --solo-controlli   si ferma prima dell'avvio
#   ./scripts/aggiorna-s9.sh --ramo <nome>      passa a quel ramo e aggiorna
#
# `--ramo` esiste perche' senza di esso lo script aggiorna soltanto il ramo su
# cui si trova gia'. Consegnando il lavoro su un ramo nuovo, sull'S9 non
# arrivava niente e il collaudo girava sul codice di prima senza che nulla
# segnalasse l'errore: e' successo il 1 settembre 2026.
#
# Ogni esecuzione lascia il log completo dei controlli in data/log/. Serve
# perche' sullo schermo di Termux l'errore vero di rustc scorre via e resta
# visibile solo l'ultima riga ("could not compile ... due to 1 previous
# error"), che da sola non dice niente. L'avvio del bot NON viene registrato:
# un bot lasciato acceso riempirebbe il disco.
#
# Backup e log vengono ruotati: si tengono solo i piu' recenti.
#
# Regola del progetto: niente `set -e`, ogni passo si ferma con `|| exit 1`
# in modo che un errore interrompa lo script senza chiudere la sessione SSH.

PROGETTO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB="$PROGETTO/data/db/gestionale.db"
CARTELLA_DB="$PROGETTO/data/db"
CARTELLA_LOG="$PROGETTO/data/log"
BACKUP_DA_TENERE=5
LOG_DA_TENERE=5
# Sotto questa soglia la compilazione fallisce in modo poco leggibile invece
# di dire che manca spazio: meglio avvisare prima.
MB_MINIMI=1500
SOLO_CONTROLLI=0
RAMO_RICHIESTO=""
while [ $# -gt 0 ]; do
    case "$1" in
        --solo-controlli) SOLO_CONTROLLI=1 ;;
        --ramo)
            shift
            RAMO_RICHIESTO="$1"
            if [ -z "$RAMO_RICHIESTO" ]; then
                echo "--ramo richiede il nome del ramo."
                exit 1
            fi
            ;;
        *)
            echo "Argomento non riconosciuto: $1"
            echo "Uso: ./scripts/aggiorna-s9.sh [--ramo <nome>] [--solo-controlli]"
            exit 1
            ;;
    esac
    shift
done

mkdir -p "$CARTELLA_LOG" || exit 1
# `AGGIORNA_LOG` viene passata dal riavvio dopo un cambio di ramo, cosi' una
# sola esecuzione resta in un solo file di log.
LOG="${AGGIORNA_LOG:-$CARTELLA_LOG/aggiorna_$(date +%Y%m%d_%H%M%S).log}"

passo() {
    echo | tee -a "$LOG"
    echo "===== $* =====" | tee -a "$LOG"
}

# Esegue un comando mostrandolo a schermo e registrandolo nel log.
# `PIPESTATUS[0]` e' l'esito del comando, non quello di `tee`.
esegui() {
    "$@" 2>&1 | tee -a "$LOG"
    return "${PIPESTATUS[0]}"
}

# Tiene solo i file piu' recenti fra quelli che corrispondono al motivo.
# `$2` resta volutamente senza virgolette: e' un glob, non un nome.
tieni_ultimi() {
    CARTELLA="$1"
    MOTIVO="$2"
    QUANTI="$3"
    ls -1t "$CARTELLA"/$MOTIVO 2>/dev/null | tail -n "+$((QUANTI + 1))" |
    while IFS= read -r VECCHIO; do
        rm -f "$VECCHIO" && echo "  rimosso: $(basename "$VECCHIO")"
    done
}

# Chiamata quando cargo fallisce. L'ultima riga di rustc ("could not compile
# ... due to 1 previous error") non dice quale sia l'errore: quello vero e'
# piu' su, spesso gia' scorso via dallo schermo. Qui lo ripeschiamo dal log e
# ricontrolliamo lo spazio, perche' un disco riempitosi durante la
# compilazione produce errori che sembrano di codice e spariscono al secondo
# tentativo.
esito_compilazione() {
    echo
    echo "===== la compilazione si e' fermata ====="
    echo "log completo: $LOG"
    echo
    echo "prime righe di errore trovate nel log:"
    grep -n -m 5 -E "^(error|error\[)" "$LOG" 2>/dev/null || echo "  nessuna riga 'error' nel log"
    echo
    MB_ORA="$(df -Pk "$PROGETTO" 2>/dev/null | awk 'NR==2 {print int($4/1024)}')"
    [ -n "$MB_ORA" ] && echo "spazio libero adesso: ${MB_ORA} MB"
    if grep -q -E "No space left|Killed|signal: 9|SIGKILL" "$LOG" 2>/dev/null; then
        echo
        echo "Nel log compare un problema di spazio o un processo ucciso:"
        echo "non e' un errore di codice. Libera spazio e riprova."
    fi
    if grep -q -E "linking with .cc. failed|Segmentation fault" "$LOG" 2>/dev/null; then
        echo
        echo "Il linker si e' fermato: non e' un errore di codice, e' memoria."
        echo "Controlla che 'codegen-units = 16' e 'debug = 0' siano ancora in"
        echo "Cargo.toml e che build.rs emetta ancora --threads=1 su Android."
    fi
    exit 1
}

cd "$PROGETTO" || exit 1

# I log vecchi si ruotano subito: quello di questa esecuzione non esiste
# ancora, quindi ne teniamo uno in meno e il conto finale torna.
tieni_ultimi "$CARTELLA_LOG" "aggiorna_*.log" "$((LOG_DA_TENERE - 1))"

# --------------------------------------------------------------------------
passo "0/7  spazio su disco"
# --------------------------------------------------------------------------
MB_LIBERI="$(df -Pk "$PROGETTO" 2>/dev/null | awk 'NR==2 {print int($4/1024)}')"
if [ -z "$MB_LIBERI" ]; then
    echo "non riesco a leggere lo spazio libero, proseguo" | tee -a "$LOG"
else
    echo "liberi: ${MB_LIBERI} MB" | tee -a "$LOG"
    if [ "$MB_LIBERI" -lt "$MB_MINIMI" ]; then
        echo | tee -a "$LOG"
        echo "ATTENZIONE: sotto i ${MB_MINIMI} MB la compilazione puo'" | tee -a "$LOG"
        echo "fallire con un errore che non parla di spazio." | tee -a "$LOG"
        echo "Libera spazio, per esempio con 'cargo clean', prima di" | tee -a "$LOG"
        echo "cercare la causa altrove." | tee -a "$LOG"
        echo | tee -a "$LOG"
    fi
fi
echo "log di questa esecuzione: $LOG"

# --------------------------------------------------------------------------
passo "1/7  aggiornamento del codice"
# --------------------------------------------------------------------------
RAMO="$(git rev-parse --abbrev-ref HEAD)" || exit 1
echo "ramo corrente: $RAMO" | tee -a "$LOG"
if [ -n "$(git status --porcelain)" ]; then
    echo "ATTENZIONE: ci sono modifiche locali non committate."
    git status --short
    echo "Committale o mettile da parte con 'git stash' prima di continuare."
    exit 1
fi
if [ -n "$RAMO_RICHIESTO" ] && [ "$RAMO_RICHIESTO" != "$RAMO" ]; then
    echo "passo al ramo: $RAMO_RICHIESTO" | tee -a "$LOG"
    esegui git fetch origin || exit 1
    esegui git checkout "$RAMO_RICHIESTO" || exit 1
    # Bash legge lo script man mano che lo esegue, e il checkout puo' aver
    # appena riscritto questo file: proseguire significherebbe eseguire meta'
    # della versione vecchia e meta' di quella nuova. Si riparte dall'inizio
    # con la versione appena arrivata, tenendo lo stesso file di log.
    echo "riavvio con la versione dello script appena scaricata" | tee -a "$LOG"
    export AGGIORNA_LOG="$LOG"
    if [ "$SOLO_CONTROLLI" -eq 1 ]; then
        exec "$0" --solo-controlli
    fi
    exec "$0"
fi
esegui git pull --ff-only || exit 1
echo "ora a: $(git log --oneline -1)" | tee -a "$LOG"

# --------------------------------------------------------------------------
passo "2/7  impostazioni della macchina"
# --------------------------------------------------------------------------
# Qui restano soltanto le impostazioni che dipendono da QUESTA macchina.
#
# Le impostazioni che proteggono il collegamento — `debug = 0` e
# `codegen-units = 16` in `Cargo.toml`, piu' `--threads=1` per il linker su
# Android emesso da `build.rs` — stanno nel progetto. Prima vivevano solo
# qui, e un `cargo run` dato a mano senza passare da questo script otteneva i
# default di cargo: 257 file oggetto pieni di debuginfo, e il linker
# segfaultava. Una protezione che funziona solo se ti ricordi di usare lo
# script non e' una protezione.
export CARGO_BUILD_JOBS=1
export CARGO_INCREMENTAL=0

# `RUSTFLAGS` nell'ambiente sostituisce la configurazione di cargo invece di
# aggiungersi. Il flag del linker viene da `build.rs` e non e' toccato, ma una
# RUSTFLAGS lasciata li' da una prova vecchia resta una causa possibile di
# compilazioni che si comportano in modo diverso senza motivo apparente.
if [ -n "${RUSTFLAGS:-}" ]; then
    echo "ATTENZIONE: RUSTFLAGS e' impostata nell'ambiente:" | tee -a "$LOG"
    echo "  RUSTFLAGS=$RUSTFLAGS" | tee -a "$LOG"
    echo "Sostituisce la configurazione di cargo: se la compilazione si" | tee -a "$LOG"
    echo "comporta in modo strano, e' il primo posto dove guardare." | tee -a "$LOG"
fi
echo "impostate" | tee -a "$LOG"

# --------------------------------------------------------------------------
passo "3/7  formattazione e compilazione"
# --------------------------------------------------------------------------
esegui cargo fmt --all || exit 1
esegui cargo fmt --all -- --check || exit 1
esegui git diff --check || exit 1
esegui cargo check --locked || esito_compilazione

# --------------------------------------------------------------------------
passo "4/7  Clippy e test"
# --------------------------------------------------------------------------
esegui cargo clippy --all-targets --locked -- -D warnings || esito_compilazione
esegui cargo test --locked -- --test-threads=1 || esito_compilazione

# --------------------------------------------------------------------------
passo "5/7  backup del database e controlli di integrita'"
# --------------------------------------------------------------------------
if [ ! -f "$DB" ]; then
    echo "Database non trovato: $DB"
    exit 1
fi
# Copie di prova lasciate indietro da esecuzioni interrotte: si rimuovono
# prima di crearne una nuova, altrimenti si accumulano senza che nessuno le
# guardi mai.
tieni_ultimi "$CARTELLA_DB" "gestionale_prova_*.db" 0

TS="$(date +%Y%m%d_%H%M%S)"
BACKUP="$CARTELLA_DB/gestionale_pre_${TS}.db"
COPIA="$CARTELLA_DB/gestionale_prova_${TS}.db"
# `.backup` usa l'API di SQLite: consistente anche con il bot in esecuzione.
sqlite3 "$DB" ".backup '$BACKUP'" || exit 1
cp "$BACKUP" "$COPIA" || exit 1
echo "backup: $(basename "$BACKUP")" | tee -a "$LOG"

# Si tengono solo gli ultimi backup: sull'S9 lo spazio e' il vincolo vero, e
# un backup vecchio di settimane non verrebbe comunque ripristinato.
echo "backup tenuti: $BACKUP_DA_TENERE" | tee -a "$LOG"
tieni_ultimi "$CARTELLA_DB" "gestionale_pre_*.db" "$BACKUP_DA_TENERE"

esegui sqlite3 "$DB" "PRAGMA integrity_check;" || exit 1
esegui sqlite3 "$DB" "PRAGMA foreign_key_check;" || exit 1

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
    echo "  provo: $NOME" | tee -a "$LOG"
    sqlite3 "$COPIA" < "$FILE" 2>&1 | tee -a "$LOG"
    [ "${PIPESTATUS[0]}" -eq 0 ] || exit 1
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
    echo "richiesto --solo-controlli: mi fermo qui." | tee -a "$LOG"
    echo "log dei controlli: $LOG"
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
# Da qui in poi non si registra piu' nulla: il bot puo' restare acceso per
# ore e il suo output riempirebbe il log fino a esaurire il disco.
echo "log dei controlli: $LOG"
cargo run --locked
