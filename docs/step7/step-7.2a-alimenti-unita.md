# Step 7.2A — Alimenti e unità

**Base:** `62b27f8` — Step 7.1B verificata.

## Obiettivo

Creare la prima fondazione dati del modulo Alimentazione senza introdurre
ancora CRUD Telegram o logica ricette.

## Schema introdotto

### `unita_misura`

Unità canoniche iniziali:

- `g`, `kg` — famiglia massa;
- `ml`, `l` — famiglia volume;
- `pz`, `cucchiaio`, `cucchiaino`, `q.b.` — non convertibili universalmente.

Le conversioni automatiche sono consentite soltanto all'interno della stessa
famiglia. Massa e volume non vengono convertiti automaticamente fra loro.

### `alimenti`

Un alimento può essere:

- globale: `spazio_id IS NULL`;
- personalizzato: `spazio_id` valorizzato.

Il nome normalizzato è unico nel catalogo globale oppure nel singolo spazio.

### `alimento_alias`

Gli alias sono collegati a un alimento strutturato e servono a supportare
sinonimi e ricerca senza trasformare gli alimenti in semplici stringhe.

## Decisioni intenzionali

- nessun alimento viene precaricato dalla migration;
- nessuna delle 10 migration precedenti viene modificata;
- alimento, prodotto acquistabile e scorta restano concetti distinti;
- il CRUD Telegram viene implementato nella Step 7.2B.
