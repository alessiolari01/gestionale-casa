# Scorte: dispensa, frigo e freezer

**Scritto il 16 settembre 2026, collaudo dal vivo su Telegram da fare.**
Via libera di Alessio nello stesso giro di lavoro della chiusura della spesa
(`docs/moduli/lista-spesa.md`), che è il momento in cui la merce entra in
casa. Decisioni di struttura e loro motivazione in
`docs/previsto/dispensa.md`; `src/modules/dispensa.rs`, raggiungibile da
`🍽️ Alimentazione → 🥫 Scorte`.

Risponde a una domanda sola: **cosa ho già in casa.**

## I tre luoghi, senza configurazione

`dispensa`, `frigo`, `freezer` esistono sempre: chi non imposta niente ce li
ha comunque. Sono un'entità propria (colonna `scorte.conservazione`), non un
riuso di case/stanze/contenitori — la differenza e il perché stanno in
`docs/previsto/dispensa.md`: un contenitore dice *dove sta un oggetto*, una
scorta dice *in che condizione è conservata una quantità*, ed è quella
condizione a decidere quanto dura.

Ogni luogo ha la sua icona, una sola (C4): `🧺 Dispensa`, `🧊 Frigo`,
`❄️ Freezer`.

## Cosa contiene una scorta

Descrizione, quantità e unità sono obbligatorie; il collegamento al catalogo
(alimento generico o prodotto commerciale) c'è quando la scorta è stata
scelta da lì, e servirà alla futura sottrazione dal fabbisogno della lista
della spesa — senza un id non si potrebbe riconoscere l'alimento.

La **scadenza è opzionale** (decisione di Alessio): una scorta nasce senza,
e chi vuole la aggiunge dopo con `📅 Scadenza`. Si scrive come `31/12/2026`
oppure `2026-12-31`, e la validazione è semantica: `30/02/2026` viene
rifiutata subito invece di arrivare a SQLite e diventare `NULL`
(`interpreta_scadenza`, dominio puro). `🚫 Nessuna scadenza` la toglie.

## Ingresso automatico dalla spesa

Chiudendo la spesa (`lista_spesa::chiudi_spesa`), le voci archiviate
**collegate al catalogo** entrano da sole in `🧺 Dispensa`, con
`origine = 'spesa'` e il riferimento alla chiusura da cui vengono.

- **Acceso di default**, deciso con Alessio: la quantità giusta è quasi
  sempre quella comprata, e un passaggio obbligatorio in più a fine spesa —
  con le borse ancora da svuotare — è il modo più sicuro per far smettere di
  usare la funzione. Si spegne con `⚙️ Ingresso automatico`, preferenza
  **per utente** (`preferenze_utente.dispensa_ingresso_automatico`, stesso
  schema di `vista_spazi`: più persone usano lo stesso bot).
- **Le voci libere restano fuori**: "Detersivo piatti" non è un alimento e
  non ha niente da scalare da nessun fabbisogno. Chi la vuole in dispensa la
  aggiunge a mano.
- **Il luogo di partenza è sempre la dispensa**: è quello giusto per la
  maggior parte della spesa, e spostare in frigo o freezer è un tocco.
- L'ingresso avviene **fuori** dalla transazione dell'archivio: un problema
  qui non deve far perdere la chiusura, che è già valida da sola.

## Schermate

**🥫 Scorte** (menù) — i tre luoghi come pulsanti con il conteggio (C7), più
`⚙️ Ingresso automatico: attivo/spento`. Il testo dice l'unica cosa che i
pulsanti non dicono: cosa succede chiudendo la spesa.

**Un luogo** — elenco paginato, cinque per pagina come ogni altra lista del
bot (C6; l'eccezione senza paginazione resta solo della lista della spesa).
**Prima le scorte che scadono**, poi quelle senza scadenza: una scadenza
vicina è la cosa che si vuole vedere per prima. Ogni pulsante porta nome e
quantità, e la scadenza **a capo** (C15: una parte opzionale non si accoda
con " · ", altrimenti Telegram taglia l'etichetta senza avvisare). Vuoto,
dice cosa fare invece di scrivere "0" (C8). In fondo `➕ Nuova scorta`.

**Nuova scorta** — `🔎 Cerca nel catalogo` (così resta collegata
all'alimento, riusando la ricerca della lista della spesa) oppure
`📝 Scrivila a mano`. Poi la quantità: se l'alimento o il prodotto ha
un'unità predefinita basta il numero, scriverne un'altra la sovrascrive —
stessa regola della lista della spesa (`valida_quantita_con_default`,
condivisa, non duplicata).

**Dettaglio** — dov'è, quanto ce n'è, la scadenza; `✏️ Quantità` (per
aggiornarla quando se ne usa un po'), `📅 Scadenza`, `🔀 Sposta` (dal
freezer al frigo per scongelare, per esempio), `🗑 Elimina` con la conferma
esplicita di C16.

## Tabelle

```text
scorte                                   quantità, luogo di conservazione, scadenza opzionale
preferenze_utente.dispensa_ingresso_automatico   se la spesa chiusa entra da sola
```

Vedi `docs/database.md` per i campi.

## Fuori scope, per ora

- **Sottrarre le scorte dal fabbisogno della lista della spesa.** È il passo
  che chiude il cerchio ed è già descritto in `docs/previsto/dispensa.md`,
  ma cambia il cuore dell'aggregazione (`calcola_fresche`), che è collaudato
  e in produzione: va fatto come blocco a sé, con il suo collaudo dal vivo.
- **Scarico automatico al consumo di un pasto.** Naturale, ma prima deve
  esistere la scorta — ed esiste da oggi.
- **Avvisi di scadenza**: richiedono l'infrastruttura dei reminder, che non
  esiste ancora (`docs/previsto/reminder.md`).
- **Quantità realmente comprata e formato della confezione**: oggi entra in
  dispensa la quantità che era in lista, non quella della confezione presa.
  È il punto 1 di "Cosa manca" in `docs/previsto/dispensa.md`.
