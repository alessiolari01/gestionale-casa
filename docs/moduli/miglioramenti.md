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

## Workflow approvato da implementare

Lo stato del miglioramento e la lettura da parte dell'amministratore devono essere separati.

Stati previsti:

```text
🟡 Da approvare
🟢 Verificato
🔵 Pianificato
✅ Fatto
❌ Scartato
```

Regole:

- autore admin → nuovo miglioramento `verificato`;
- autore non admin → nuovo miglioramento `da_approvare`;
- `🆕` in fondo alla riga indica esclusivamente che l'admin non ha ancora letto l'elemento;
- aprire il dettaglio rimuove `🆕`; una decisione esplicita lo rimuove comunque;
- la stessa semantica `🆕` va usata per le richieste di accesso in Amministrazione;
- `fatto` e `scartato` non entrano nel lavoro da implementare;
- durante una revisione dei miglioramenti richiesta dall'utente, i record `scartato` devono essere eliminati insieme ai file allegati;
- `da_approvare` deve essere verificato dall'admin prima di diventare backlog operativo.

Implementare queste regole tramite una nuova migration append-only, senza modificare la migration originale di Step 7.2E.
