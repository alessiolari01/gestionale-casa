# ⏰ Promemoria

**Stato: COSTRUITO il 7 ottobre 2026** (`src/modules/promemoria.rs`,
migration `20261007090000_promemoria`). Sostituisce `reminder.md`, che
ne era l'abbozzo.

Alessio, 6 ottobre 2026: partire dai Promemoria, con **tutte le
possibilità**, "e l'utente poi decide cosa usare e se usarli tutti".

È il primo modulo in cui **il bot scrive da solo**, a un orario, senza che
nessuno abbia toccato niente. Per questo viene prima di Documenti, Soldi e
Palestra: le scadenze dei documenti, le spese ricorrenti e gli allenamenti
useranno lo stesso motore.

## Cosa fa

1. **Promemoria liberi.** Cosa e quando, scritti come li scrive una persona
   (C20): "domani alle 9", "fra 2 ore", "15/10 18:30", "alle 21", "venerdì".
   Una volta sola, oppure ogni giorno, dal lunedì al venerdì, ogni
   settimana, ogni mese o ogni anno.
2. **I pasti del planner.** Una regola generale ("30 minuti prima di ogni
   pasto") e, pasto per pasto, un'eccezione: un altro anticipo, oppure "non
   ricordarmelo". Solo per i pasti con un orario, ancora da consumare e non
   saltati.
3. **Le scorte che scadono.** Un riepilogo una volta al giorno, all'ora
   scelta, con quello che scade entro i giorni scelti (e quello già
   scaduto). Se non scade niente, il bot tace.
4. **Rimandare.** Ogni avviso ha `✅ Fatto`, `⏰ 10 min`, `⏰ 1 ora`,
   `⏰ Domani`.

Le regole 2 e 3 **nascono spente**: un messaggio che arriva senza averlo
chiesto è un'automazione inattesa. Si accendono da `⏰ Promemoria →
🔁 Automatici`.

## Interruttori

- `⚙️ Impostazioni → 🧩 Sezioni → ⏰ Promemoria`: spenta, il bot non ti
  scrive più da solo, nemmeno i promemoria liberi. I dati restano.
- I pasti arrivano solo con il Planner acceso, le scadenze solo con le Scorte
  accese: la schermata degli automatici lo dice quando non è così.

## Schermate

```
⏰ Promemoria                         (dal menù principale)
  In arrivo: le prossime cinque righe, nel testo
  [➕ Nuovo promemoria]
  [📋 Tutti i promemoria (N)]
  [🔁 Automatici]
  [🏠 Menù principale]

➕ Nuovo: "Cosa ti devo ricordare?" → testo
       → "Quando?" [Fra 1 ora] [Stasera alle 20] [Domani alle 9] o scritto
       → "Si ripete?" [Una volta sola] [Ogni giorno] [Dal lunedì al venerdì]
                      [Ogni martedì] [Ogni mese, il giorno 7] [Ogni anno, il 7 ottobre]
       → scheda, con "✅ Promemoria salvato: …"

Scheda: testo, quando, ripetizione
  [✏️ Testo] [🕐 Quando] [🔁 Ripetizione] [⏸ Sospendi | ▶️ Riattiva] [🗑️ Elimina]

🔁 Automatici
  🍽️ Pasti del planner: 30 minuti prima | spento
  🥫 Scorte che scadono: ogni giorno alle 09:00, entro 3 giorni | spento

Nel dettaglio di un pasto del planner: [⏰ Promemoria] → anticipo di questo pasto.
```

L'avviso che arriva:

```
⏰ Chiama il medico
[✅ Fatto] [⏰ 10 min] [⏰ 1 ora] [⏰ Domani]
```

## Scelte strutturali

- **Ore locali come testo**, `AAAA-MM-GG HH:MM`, confrontate con
  `strftime('%Y-%m-%d %H:%M','now','localtime')`: è come il resto del bot
  conosce il fuso del telefono (Miglioramento 17).
- **Un controllo ogni 30 secondi**, dentro il processo del bot. Se il bot
  era spento, al riavvio manda quello che è scaduto nel frattempo, una
  volta sola, dicendo che è in ritardo. Un promemoria che si ripete salta le
  volte perse e riparte dalla prossima: tre giorni spento non vogliono dire
  tre avvisi di fila. Un pasto il cui avviso è in ritardo di più di mezz'ora
  non si avvisa più.
- **Ogni invio ha una chiave** (`promemoria:12:2026-10-07 09:00`,
  `pasto:55`, `scadenze:2026-10-07`) unica per utente in `promemoria_invii`:
  un controllo ripetuto, o due processi per errore, non mandano mai due
  volte lo stesso avviso.
- **L'avviso non è una schermata.** È un messaggio a parte: non sostituisce
  la schermata su cui stai lavorando, e i suoi pulsanti funzionano anche
  dopo, quando quella schermata è cambiata. `✅ Fatto` lo toglie dalla chat.
- **Rimandare crea un promemoria nuovo, una volta sola**, con lo stesso
  testo. Il promemoria che si ripete continua per conto suo.
- **"Ogni mese, il giorno 31"** cade l'ultimo giorno dei mesi più corti, e a
  marzo torna il 31; "ogni anno il 29 febbraio" cade il 28 negli anni non
  bisestili.
- **Le tabelle**: `promemoria_liberi` (i promemoria scritti a mano e
  quelli rimandati), `promemoria_invii` (ogni avviso mandato, con la sua
  chiave, e l'ora in cui è stato premuto `✅ Fatto` o `⏰`, `esito_il`),
  `promemoria_regole` (gli automatici, una riga per utente),
  `promemoria_pasti` (l'eccezione di un pasto). `promemoria`, senza
  suffisso, è una tabella del primo schema legata agli oggetti, mai usata e
  vuota: resta perché `db::status` la conta.
- **L'attesa di testo** sta nel modulo, per chat, come l'orario del
  planner: si chiude da sola con qualunque pulsante che non sia `remind:`,
  con un comando e con il menù principale.
- **Personali.** Un promemoria è di chi lo crea e arriva solo a lui. I pasti
  e le scorte sono quelli degli spazi di cui si è membri. I promemoria
  condivisi con altri membri di uno spazio sono un passo successivo.

## Dopo

- promemoria per un altro membro dello spazio;
- la preparazione in anticipo dei pasti (meal-prep dei turni);
- la lista della spesa ("ricordamelo quando…" non ha un orario: serve altro);
- email come secondo canale (Step 7).
