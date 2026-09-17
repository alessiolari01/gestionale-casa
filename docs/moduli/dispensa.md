# Scorte: dispensa, frigo e freezer

**Prima fetta scritta il 16 settembre 2026 e distribuita lo stesso giorno;
secondo giro scritto il 17 settembre 2026 dopo il collaudo dal vivo di
Alessio — collaudo dal vivo del secondo giro da fare.** Decisioni di
struttura e loro motivazione in `docs/previsto/dispensa.md`;
`src/modules/dispensa.rs`, raggiungibile da `🍽️ Alimentazione → 🥫 Scorte`.

Risponde a una domanda sola: **cosa ho già in casa.** Il principio, parole di
Alessio: tutto il più automatico possibile, ma l'utente può aggiustare a mano
qualunque cosa.

## I tre luoghi, senza configurazione

`dispensa`, `frigo`, `freezer` esistono sempre: chi non imposta niente ce li
ha comunque. Sono un'entità propria (colonna `scorte.conservazione`), non un
riuso di case/stanze/contenitori — la differenza e il perché stanno in
`docs/previsto/dispensa.md`: un contenitore dice *dove sta un oggetto*, una
scorta dice *in che condizione è conservata una quantità*, ed è quella
condizione a decidere quanto dura.

Ogni luogo ha la sua icona, una sola (C4): `🧺 Dispensa`, `🧊 Frigo`,
`❄️ Freezer`.

## Dove va a finire ogni cosa (17 settembre 2026)

Chiudendo la spesa, ogni voce entra nel **suo** posto, non più sempre in
dispensa. `destinazione_per` decide in quest'ordine:

1. **la scelta fatta a mano per lo spazio** — `📌 Mettilo sempre qui` nel
   dettaglio di una scorta (tabella `scorte_destinazioni`), prima sul
   prodotto e poi sull'alimento. Vale per lo spazio e non per tutti: il
   catalogo globale è condiviso, e cambiare lì il posto della pasta lo
   cambierebbe a ogni utente del bot;
2. **il nome** (`conservazione_da_nome`, dominio puro):
   - surgelati, congelati, gelati → freezer, qualunque sia la categoria (gli
     spinaci congelati sono "verdura", ma non vanno in frigo);
   - conservazione lunga — UHT, "lunga conservazione", secchi, essiccati,
     uvetta — → dispensa;
   - la verdura che il frigo rovina — patate, cipolle, aglio, scalogno,
     pomodori, zucca, avocado, olive — → dispensa. Viene prima della frutta:
     il primo test ha trovato che i "pomodori ciliegini" finivano in frigo
     come se fossero ciliegie;
   - la frutta che sta meglio al fresco — frutti di bosco, fragole,
     mirtilli, lamponi, ribes, more, ciliegie, uva, mele, prugne, susine —
     → frigo (scelta fatta cercando come si conserva la frutta, come chiesto
     da Alessio);
3. **la categoria dell'alimento** (`categorie_alimento.conservazione_predefinita`):
   verdure, carne, pesce, latticini, uova → frigo; tutto il resto (cereali e
   quindi anche il pane, legumi, condimenti, bevande, dolci, frutta, altro)
   → dispensa;
4. **la dispensa**, se niente di tutto questo decide.

Le regole per nome stanno nel codice e non nel database perché così si
testano, e valgono anche per gli alimenti che gli utenti creeranno in futuro.

**Scelta di Alessio, contro le indicazioni ufficiali** (17 settembre 2026):
zucchine, cetrioli, fagiolini, melanzane, peperoni e mele vanno in
**frigo**. Le indicazioni del Ministero della Salute per alcune di queste
verdure consigliano un posto fresco fuori dal frigo, per il danno da freddo.
Alessio ha scelto il frigo sapendolo, quindi queste verdure **non** sono
nell'elenco delle eccezioni per la dispensa: seguono la categoria "verdura".
Le mele sono nella frutta da frigo. Chi vuole un altro posto può usare
`📌 Mettilo sempre qui`.

## Una riga per alimento, con le sue confezioni (17 settembre 2026)

Trovato da Alessio collaudando: due `Pasta sfoglia · 500 g` comparivano come
righe separate. Ora:

- **aggiungendo** una scorta uguale a una già presente — stesso alimento (o
  prodotto, o nome per quelle scritte a mano), stesso posto, stessa scadenza
  (anche "nessuna"), unità convertibile — le quantità si **sommano**
  (`unisci_o_inserisci`). Vale per le aggiunte a mano e per la spesa chiusa;
- confezioni con **scadenze diverse** restano righe distinte a database
  (fonderle farebbe perdere la data più vicina), ma **l'elenco le mostra
  come una riga sola** con il totale (`raggruppa_scorte`): `Pasta sfoglia ·
  1 kg`, e a capo `📅 prima scadenza Gio 1 Ott · 2 confezioni`. Toccandola
  si sceglie la confezione, ognuna con la sua scadenza (idea di Alessio).

Le quantità grandi si leggono nell'unità comoda: `1500 g` diventa `1.5 kg`,
`2000 ml` diventa `2 l`.

## La scadenza è opzionale

Una scorta nasce senza, e chi vuole la aggiunge dopo con `📅 Scadenza`. Si
scrive come `31/12/2026` oppure `2026-12-31`, con validazione semantica:
`30/02/2026` viene rifiutata subito invece di arrivare a SQLite e diventare
`NULL` (`interpreta_scadenza`). `🚫 Nessuna scadenza` la toglie.

## Ingresso automatico dalla spesa

Chiudendo la spesa (`lista_spesa::chiudi_spesa`), le voci archiviate
**collegate al catalogo** entrano da sole in casa, ognuna nel suo posto, con
`origine = 'spesa'` e il riferimento alla chiusura da cui vengono. Il
messaggio della chiusura dice dove: `Entrate in casa: 2 in 🧺 Dispensa, 1 in
🧊 Frigo.`

- **Acceso di default**, deciso con Alessio; si spegne con
  `⚙️ Ingresso automatico`, preferenza **per utente**
  (`preferenze_utente.dispensa_ingresso_automatico`).
- **Le voci libere restano fuori**: "Detersivo piatti" non è un alimento.
  Chi la vuole la aggiunge a mano.
- Avviene **fuori** dalla transazione dell'archivio: un problema qui non fa
  perdere la chiusura, che è già valida da sola.

## Le scorte si consumano con i pasti (17 settembre 2026)

Senza questo, la lista della spesa — che sottrae le scorte dal fabbisogno —
dopo una settimana direbbe "hai la pasta" di una pasta già mangiata. Le
scorte si scalano **una volta sola per pasto**
(`planner_pasti.scorte_scalate_il`), qualunque sia la strada:

- **`🍲 Segna come preparato`** nel planner: è il momento in cui gli
  ingredienti escono davvero dal frigo (se prepari lunedì sera il pranzo di
  martedì, la pasta sparisce lunedì). Non è un nuovo stato del pasto: resta
  pianificato finché non lo si consuma;
- **`✅ Segna come consumato`** senza averlo preparato: si scala in quel
  momento;
- **da solo a orario passato** (o a giorno finito, se il pasto non ha
  orario), se il pasto non è stato né preparato né saltato
  (`scala_pasti_scaduti`). Lo **stato** del pasto invece non cambia:
  segnarlo consumato lo congelerebbe, e non si potrebbe più dire che è stato
  saltato. Non c'è uno scheduler: il controllo gira quando si aprono la
  lista della spesa, le scorte o le ricette con quello che ho, che sono gli
  unici momenti in cui il risultato serve.

Per ogni alimento si usano **prima le confezioni che scadono prima**, e
vanno bene anche quelle di un prodotto specifico dello stesso alimento. Una
confezione finita sparisce. Se in casa non c'è abbastanza, si toglie quello
che c'è.

Ogni prelievo resta in `scorte_movimenti`, e serve a **restituirlo**:

- **pasto saltato dopo uno scarico automatico**: tutto torna esattamente
  com'era — stessa confezione, stesso posto, stessa scadenza (richiesta
  esplicita di Alessio). Dopo uno scarico fatto a mano con `🍲 Preparato` il
  planner chiede se gli ingredienti preparati ci sono ancora (consegna A):
  sì li restituisce, no li lascia tolti;
- **pasto consumato riportato a pianificato**: se non era preparato, torna
  quello scalato al consumo; se era preparato, niente;
- **pasto modificato** (`✏️ Modifica`, che cambia ricetta o partecipanti):
  torna tutto, e se il pasto era preparato si riscala subito con gli
  ingredienti nuovi;
- **pasto consumato sostituito** (`🔁 Sostituisci`, vedi
  `docs/moduli/planner.md`): torna tutto quello del pasto vecchio, e si
  scala quello del nuovo.

**Quando in casa non c'è abbastanza** (consegna A, 17 settembre 2026):

- prima di `🍲 Preparato` e `✅ Consumato`, `mancanti_per_pasto` dice cosa
  manca senza toccare niente, e il planner chiede conferma;
- lo scarico (a mano o automatico) prende quello che c'è e annota il resto
  in `planner_pasti.scorte_mancanti` (testo pronto, "Pasta brisée 200 g");
  restituire le scorte cancella l'annotazione;
- `scala_pasti_scaduti` restituisce i pasti scaricati con le loro
  mancanze, e `avviso_scarichi` ne fa l'avviso mostrato in testa alla
  lista della spesa e alle scorte, solo se mancava qualcosa.

Nel primo avvio dopo la migration, i pasti dei **giorni passati** si
considerano già scaricati: sono storia precedente alla funzione, e non
devono svuotare le scorte di oggi.

## Ricette con quello che ho (17 settembre 2026)

Chiesto da Alessio: le ricette ordinate da quella per cui hai più
ingredienti in casa a quella per cui ne hai meno
(`ricette_con_quello_che_ho`). A parità, prima quelle a cui ne mancano meno.
Un ingrediente conta come "in casa" se ce n'è almeno una confezione,
qualunque quantità: la domanda è "cosa posso cucinare", non "ne ho per
tutti". Compaiono solo le ricette con almeno un ingrediente in casa, con il
conteggio sul pulsante (`Carbonara · 3/5`), e il tocco apre il dettaglio
della ricetta di sempre.

Raggiungibile da `🥫 Scorte → 🍳 Ricette con quello che ho` e da
`🍳 Ricette → 🥫 Con quello che ho in casa`; `⬅️ Indietro` torna da dove si
è arrivati (C3).

## Schermate

**🥫 Scorte** (menù) — i tre luoghi con quante righe hanno (C7),
`🍳 Ricette con quello che ho`, `⚙️ Ingresso automatico: attivo/spento`. Il
testo dice l'unica cosa che i pulsanti non dicono: cosa succede chiudendo la
spesa.

**Un luogo** — una riga per alimento, cinque per pagina (C6; l'eccezione
senza paginazione resta solo della lista della spesa), **prima quelle che
scadono**. Vuoto, dice cosa fare invece di scrivere "0" (C8). In fondo
`➕ Nuova scorta`.

**Una riga con più confezioni** — le confezioni come pulsanti, ognuna con
quantità e scadenza.

**Nuova scorta** — `🔎 Cerca nel catalogo` (ricerca condivisa con la lista
della spesa) oppure `📝 Scrivila a mano`, poi la quantità (con l'unità
predefinita dell'alimento basta il numero).

**Dettaglio di una confezione** — dov'è, quanto ce n'è, la scadenza;
`✏️ Quantità`, `📅 Scadenza`, `🔀 Sposta`, `📌 Mettilo sempre qui` (solo se
collegata al catalogo), `🗑 Elimina` con la conferma esplicita di C16.

## Tabelle

```text
scorte                                    una confezione: quantità, luogo, scadenza opzionale
scorte_destinazioni                       il posto scelto a mano per un alimento/prodotto, per spazio
scorte_movimenti                          ogni prelievo di un pasto, per poterlo restituire
categorie_alimento.conservazione_predefinita   il posto di default per categoria
preferenze_utente.dispensa_ingresso_automatico se la spesa chiusa entra da sola
planner_pasti.preparato_il / scorte_scalate_il / scorte_scalate_automaticamente
```

Vedi `docs/database.md` per i campi.

## Fuori scope, per ora

- **Quantità realmente comprata e formato della confezione**: in casa entra
  la quantità che era in lista, non quella della confezione presa. È il
  punto 1 di "Cosa manca" in `docs/previsto/dispensa.md`; nel frattempo si
  corregge a mano con `✏️ Quantità`.
- **Avvisi di scadenza**: richiedono i promemoria, che non esistono ancora
  (`docs/previsto/reminder.md`).
