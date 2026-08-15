#!/usr/bin/env bash
# Backup consistente del database SQLite e dei file multimediali.
#
# Uso: ./scripts/backup.sh /percorso/di/destinazione
# Richiede il comando `sqlite3`, gia' previsto nel setup del progetto.

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:?Specifica la cartella di destinazione del backup}"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
BACKUP_DIR="$DEST/gestionale-casa_$TIMESTAMP"
# Lo script segue il percorso database standard dello Step 4. Se in futuro
# DATABASE_URL viene personalizzata, aggiornare anche questo percorso o rendere
# il backup consapevole della configurazione runtime.
DB_FILE="$PROJECT_DIR/data/db/gestionale.db"
MEDIA_DIR="$PROJECT_DIR/data/media"

mkdir -p "$BACKUP_DIR/db"

if [[ -f "$DB_FILE" ]]; then
    # `.backup` usa l'API di backup di SQLite ed e' preferibile a copiare il
    # file .db mentre il backend potrebbe avere una connessione aperta.
    sqlite3 "$DB_FILE" ".backup '$BACKUP_DIR/db/gestionale.db'"
else
    echo "Database non trovato: $DB_FILE" >&2
    exit 1
fi

if [[ -d "$MEDIA_DIR" ]]; then
    cp -r "$MEDIA_DIR" "$BACKUP_DIR/"
fi

echo "Backup completato in: $BACKUP_DIR"

# TODO: quando il numero di backup cresce, aggiungere una pulizia dei
# backup piu' vecchi di N giorni.
