#!/usr/bin/env bash
# Backup periodico del database e dei file multimediali.
#
# Copia database e foto in una cartella di destinazione (disco esterno,
# storage cloud montato, o una cartella sincronizzata a piacere).
# Da schedulare con cron o con `termux-job-scheduler` su Termux.
#
# Uso: ./backup.sh /percorso/di/destinazione

set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:?Specifica la cartella di destinazione del backup}"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
BACKUP_DIR="$DEST/gestionale-casa_$TIMESTAMP"

mkdir -p "$BACKUP_DIR"
cp -r "$PROJECT_DIR/data/db" "$BACKUP_DIR/"
cp -r "$PROJECT_DIR/data/media" "$BACKUP_DIR/"

echo "Backup completato in: $BACKUP_DIR"

# TODO: quando il numero di backup cresce, aggiungere una pulizia dei
# backup piu' vecchi di N giorni.
