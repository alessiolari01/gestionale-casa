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
- stato legacy attuale: `aperto`, `pianificato`, `fatto`, `scartato`;
- il prossimo step introdurrà il workflow semplificato documentato sotto;
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

Lo stato operativo del miglioramento e la lettura da parte dell'amministratore
sono concetti separati. Il workflow deve restare volutamente semplice: se un
miglioramento è approvato e va fatto, non esiste una fase intermedia di
“verifica” o “pianificazione”.

Stati operativi previsti:

```text
🟡 Da approvare
🟢 Da fare
❌ Scartato
```

I miglioramenti completati non restano nell'elenco attivo: dopo
implementazione, test e aggiornamento della documentazione vengono
**archiviati**. L'archivio è consultabile solo come storico e non rappresenta
un backlog operativo. Gli allegati usati soltanto per descrivere il problema
possono essere eliminati al momento dell'archiviazione, conservando il record
testuale minimo.

Regole:

- autore con `ruolo_sistema = admin` → nuovo miglioramento `da_fare` e già letto;
- autore non admin → nuovo miglioramento `da_approvare` e non letto;
- `🆕` in fondo alla riga indica esclusivamente che l'admin non ha ancora letto l'elemento;
- aprire il dettaglio rimuove `🆕`; una decisione esplicita lo rimuove comunque;
- approvare un miglioramento `da_approvare` lo porta direttamente a `da_fare`;
- durante una revisione richiesta dall'utente, i miglioramenti `da_fare` vanno presi in carico e realizzati, non semplicemente ripianificati;
- dopo implementazione, test e documentazione, il miglioramento viene archiviato e sparisce dall'elenco attivo;
- i record `scartato` vengono eliminati durante la revisione insieme ai relativi allegati fisici;
- un `da_approvare` non va implementato finché l'admin non lo approva;
- la stessa semantica `🆕` va usata per le richieste di accesso in Amministrazione; aprire, approvare o rifiutare una richiesta la marca come letta.

### Migrazione dei dati legacy

La futura migration append-only deve ricondurre gli stati esistenti al nuovo
modello senza modificare `20260825153000_accesso_miglioramenti.sql`:

```text
aperto/pianificato creato da admin  → da_fare
aperto/pianificato non admin        → da_approvare, salvo decisioni già prese
fatto                                → archiviato
scartato                             → resta scartato fino alla prima revisione
```

Le richieste di accesso già approvate/rifiutate e i miglioramenti sui quali
l'admin ha già preso una decisione devono risultare letti nel backfill.
