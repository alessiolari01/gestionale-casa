# Modulo Miglioramenti

<!-- STEP_7_2G_CHIUSURA_DOCS -->
## Step 7.2G — workflow operativo

Il modulo Miglioramenti usa ora un backlog amministrativo semplice.

### Stati attivi

| Stato DB | Significato |
|---|---|
| `da_approvare` | proposta di un utente normale in attesa di decisione |
| `da_fare` | miglioramento approvato o creato direttamente da un admin |
| `scartato` | proposta rifiutata, ancora consultabile finché non viene eliminata |

`fatto` non è più uno stato del backlog: quando un elemento viene completato,
viene trasferito nell'archivio.

### Lettura amministrativa e `🆕`

`letto_admin_il` è separato da `stato`.

Questo consente, per esempio:

```text
da_approvare + letto_admin_il NULL
→ 🆕 Da approvare

da_approvare + letto_admin_il valorizzato
→ Da approvare
```

Aprire il dettaglio rimuove quindi il flag `🆕` senza approvare automaticamente
la proposta.

### Creazione

Admin:

```text
nuovo miglioramento
→ da_fare
→ già letto
```

Utente normale:

```text
nuovo miglioramento
→ da_approvare
→ non letto
→ 🆕 lato admin
```

### Decisioni admin

```text
Approva
→ da_fare

Scarta
→ scartato

Completa
→ archivio
→ rimozione dal backlog
```

Uno `scartato` può essere eliminato; il backend elimina le righe relazionali
degli allegati e tenta anche la pulizia dei file fisici.

### Archivio

Tabelle:

```text
miglioramenti_archivio
miglioramento_archivio_allegati
```

L'archivio conserva:

- ID archivio;
- ID del miglioramento originario;
- autore;
- descrizione;
- modulo opzionale;
- data creazione;
- data completamento;
- data archiviazione;
- admin che ha archiviato, se disponibile;
- screenshot/allegati.

La migration sposta nell'archivio anche gli elementi legacy con stato `fatto`.

### Richieste di accesso

`richieste_accesso` dispone ora di `letto_admin_il`.

Le richieste nuove possono quindi essere evidenziate con `🆕` senza confondere
la lettura con `pendente / approvata / rifiutata`.

Le richieste già decise prima dello Step 7.2G vengono backfillate come già lette.

### Migration

```text
20260826024500_miglioramenti_workflow_admin.sql
```

È una migration append-only già applicata al DB reale: **non modificarla**.

### Verifiche

Sul Galaxy S9:

```text
142 test passati
0 falliti
Clippy -D warnings OK
integrity_check = ok
foreign_key_check = nessuna riga
```

Il flusso completamento → archivio è stato verificato anche direttamente nel DB
reale con un record di prova.

Resta da eseguire, quando disponibile un secondo account, lo smoke live dei
flussi utente normale e delle nuove richieste di accesso.

---


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
