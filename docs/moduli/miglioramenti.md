# Modulo Miglioramenti

## Scopo

`💡 Miglioramenti` è il backlog interno del gestionale. Serve a registrare
problemi, idee e rifiniture UX direttamente durante l'uso del bot, senza
interrompere il macro-step strutturale in corso.

Il dominio non appartiene a uno spazio/casa: è globale rispetto al gestionale.

## Permessi

- ogni utente Telegram approvato può creare miglioramenti;
- ogni autore può leggere i propri miglioramenti e aggiungere screenshot;
- gli admin possono leggere tutti i miglioramenti e cambiarne lo stato;
- le richieste di accesso al bot restano una funzione distinta e riservata al
  solo amministratore principale.

## Dati

Tabella `miglioramenti`:

- autore interno;
- descrizione;
- modulo/sezione opzionale predisposto per uso futuro;
- stato: `aperto`, `pianificato`, `fatto`, `scartato`;
- timestamp di creazione/aggiornamento.

Tabella `miglioramento_allegati`:

- miglioramento;
- tipo `foto`;
- percorso locale;
- descrizione/caption opzionale.

Gli allegati sono salvati sotto:

```text
data/media/miglioramenti/<id>/
```

La struttura supporta più screenshot per miglioramento.

## Telegram

Flusso minimo:

```text
💡 Miglioramenti
├── ➕ Nuovo miglioramento
│   ├── descrizione
│   ├── screenshot facoltativo
│   └── salva
├── 📋 I miei miglioramenti
└── 🗂️ Tutti i miglioramenti [admin]
```

Dal dettaglio è possibile aggiungere ulteriori screenshot. Gli admin possono
impostare lo stato del miglioramento.

## Strategia UX

Il modulo nasce per sostenere la regola progettuale: prima macro-struttura e
funzionalità principali, poi fase dedicata di rifinitura UX.
