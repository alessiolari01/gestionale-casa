#!/data/data/com.termux/files/usr/bin/bash
# Script avviato da Termux:Boot al riavvio del telefono.
# Installazione: copia questo file in ~/.termux/boot/termux-boot.sh
# (crea la cartella ~/.termux/boot/ se non esiste) e rendilo eseguibile:
#   chmod +x ~/.termux/boot/termux-boot.sh

# Impedisce ad Android di sospendere la CPU mentre il bot e' in esecuzione.
termux-wake-lock

# Percorso del progetto: aggiorna se lo hai clonato altrove.
PROJECT_DIR="$HOME/gestionale-casa"

cd "$PROJECT_DIR" || exit 1

# Riavvia automaticamente il processo se termina in modo inatteso.
while true; do
    cargo run --release --locked
    echo "Il gestionale si e' fermato, riavvio tra 5 secondi..."
    sleep 5
done
