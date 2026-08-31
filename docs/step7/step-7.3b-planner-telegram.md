# Step 7.3B — Planner alimentare su Telegram

**Stato: IN SVILUPPO — decisioni approvate, migration scritta e provata, modulo da scrivere.**
**Branch:** `step-7-alimentazione`. **Baseline:** `00150d0a` (Step 7.3A).

## 1. Obiettivo

Rendere operative su Telegram le fondazioni introdotte in 7.3A: creare planner,
navigare per settimana, aggiungere/modificare/rimuovere pasti, scegliere i
Profili partecipanti e congelare gli snapshot al salvataggio.

Non introduce lista della spesa, turni/routine, reminder né integrazioni: restano
ai blocchi successivi.

## 2. Stato di partenza già disponibile

Dal 7.3A esistono e non vanno riscritti:

- tabelle `planner_alimentari`, `planner_pasti`, `planner_pasto_profili`,
  `planner_pasto_ingredienti_snapshot` (migration `20260831113000_planner_alimentare_fondazioni.sql`, applicata e immutabile);
- trigger di periodo, membership e congelamento del pasto completato;
- `modules::planner_alimentare` con `MealType`, `PlannedMealState`,
  `build_ingredient_snapshot` e `recipe_update_available`.

Il 7.3B aggiunge una sola migration,
`20260831213000_73b_versione_ricetta_e_pasti_liberi.sql` (vedi sezione 3).

Dal 7.2I esiste il calcolo da riusare **senza duplicarlo**:

- `porzioni::calculate_profile_quantity` — base → fattore profilo → override;
- `porzioni::calculate_multi_profile_ingredient` — somma dei contributi, esclusi tenuti separati;
- la query di `porzioni_ingredienti` che unisce `ricetta_ingredienti`, `alimenti`,
  `unita_misura`, `profilo_ricetta_porzioni` e `profilo_ricetta_ingredienti_override`
  è la forma da riutilizzare per generare gli snapshot.

## 3. Decisioni approvate

Entrambe si traducono in un'unica migration append-only,
`20260831213000_73b_versione_ricetta_e_pasti_liberi.sql`, che non tocca nulla di
gia' applicato e non popola dati retroattivi.

### 3.1 `ricette.aggiornato_il` diventa una versione del contenuto — APPROVATO

`recipe_update_available` confronta `ricetta_aggiornato_il_snapshot` con
`ricette.aggiornato_il`. Verificato su tutte e 40 le migration e su `ricette.rs`:
quel campo veniva scritto solo da rinomina, cambio `porzioni_base` e
archiviazione, e nessun trigger lo toccava. Aggiungere, modificare o rimuovere un
ingrediente non lo muoveva, cioe' proprio il caso che interessa al planner.

La migration aggiunge tre trigger `AFTER INSERT/UPDATE/DELETE` su
`ricetta_ingredienti` che aggiornano `ricette.aggiornato_il`. Agiscono solo su
quella colonna, quindi non fanno scattare `trg_ricetta_nome_unico_spazi_update`,
che e' un `BEFORE UPDATE OF nome_normalizzato`. Il procedimento (`ricetta_step`)
resta escluso di proposito: cambiarlo non modifica le quantita' e farebbe
comparire `🔄 Aggiorna` senza motivo.

### 3.2 Pasti liberi — APPROVATI, entrano nel 7.3B

Un pasto puo' essere una voce libera senza ricetta: "cena fuori", "avanzi",
"panino". Serve un discriminante esplicito perche' `ricetta_id` puo' diventare
`NULL` anche per una ricetta eliminata (`ON DELETE SET NULL`): senza di esso i due
casi sarebbero indistinguibili.

La migration aggiunge `planner_pasti.tipo_voce` (`'ricetta'` | `'libero'`, default
`'ricetta'`, quindi i pasti esistenti non cambiano) piu' i trigger che tengono
coerenti i due casi. Per una voce libera:

- `ricetta_id` resta `NULL` e collegarla a una ricetta viene rifiutato;
- `ricetta_nome_snapshot` contiene il titolo scritto dall'utente;
- `ricetta_porzione_base_snapshot` vale `1`, valore neutro imposto dal CHECK
  originale della tabella, che `ALTER TABLE` non puo' modificare;
- non puo' avere righe in `planner_pasto_ingredienti_snapshot`;
- puo' comunque avere partecipanti: serve sapere chi mangia fuori;
- non contribuira' alla futura lista della spesa, per definizione.

Un pasto `'ricetta'` deve nascere con una ricetta reale, ma puo' restare orfano se
la ricetta viene poi eliminata: in quel caso sopravvive con il solo snapshot.

### 3.3 Partecipanti storici — risolto nella stessa migration

`planner_pasto_profili` ha `PRIMARY KEY (pasto_id, profilo_alimentare_id)` con la
colonna profilo `ON DELETE SET NULL`, e SQLite ammette NULL nelle primary key
composite: due profili eliminati avrebbero prodotto due righe `(pasto, NULL)` non
piu' distinguibili. La migration aggiunge un indice unico parziale su
`(pasto_id, profilo_nome_snapshot)` per le sole righe con profilo `NULL`.

Resta noto e non risolto, perche' fuori perimetro: `trg_planner_spazio_membership_insert`
e' solo `BEFORE INSERT`, quindi spostare un planner in uno spazio dove non si e'
membri non e' bloccato dal DB. In 7.3B lo spostamento non viene esposto in UI.

## 4. Punto di ingresso

`🏠 Menù principale → 🍽️ Alimentazione → 📅 Planner`.

`alimentation_menu_keyboard` diventa:

```text
🥕 Alimenti
🍳 Ricette
👥 Profili alimentari
📅 Planner
⬅️ Indietro | 🏠 Menù principale
```

Comando testuale equivalente: `/planner`.

## 5. Schermate

Tutte seguono le convenzioni già in uso: schermata singola via `ContextBot`,
massimo due pulsanti per riga di norma, paginazione a 5 elementi, nessun ID
tecnico mostrato, riga finale `⬅️ Indietro | 🏠 Menù principale | 💡 Migliora`,
`⚙️ Gestisci` per le operazioni amministrative e `🗑 Elimina` isolato.

### 5.1 Elenco planner — `planner:menu`

Elenca i planner attivi visibili: personali dell'utente e quelli degli spazi di
cui è membro, filtrati come i Profili (`view_all` rispettato). Riga per planner:
nome + periodo + `👥` se legato a uno spazio.

```text
➕ Nuovo planner
[planner 1] … [planner 5]
◀ Pagina | Pagina ▶
📦 Archiviati
⬅️ Indietro | 🏠 Menù principale | 💡 Migliora
```

### 5.2 Nuovo planner — `planner:new`

Wizard in tre passi, bozza **solo in memoria** fino a `✅ Salva`:

1. nome (testo, 1–60 caratteri, normalizzato come i Profili);
2. ambito: `👤 Personale` oppure uno spazio scrivibile fra quelli disponibili;
3. periodo: `Questa settimana`, `Prossima settimana`, `Personalizzato`.

Il periodo personalizzato riusa il calendario già scritto per gli inviti degli
Spazi: mese sopra, intestazioni `Lun`…`Dom` come pulsanti no-op, date non valide
marcate `❌`. Le settimane iniziano di lunedì.

Il vincolo di unicità è `(proprietario, nome, periodo)` fra i non archiviati:
un nome ripetuto sullo stesso periodo va rifiutato con messaggio leggibile, non
con l'errore SQLite.

### 5.3 Vista settimana — `planner:week:{planner_id}:{offset}`

È la schermata principale del modulo.

```text
📅 Spesa famiglia
Settimana 31/08 – 06/09

Lun 31/08 · 3 pasti
Mar 01/09 · 2 pasti
…
Dom 06/09 · nessun pasto

◀ Settimana | Settimana ▶
⚙️ Gestisci
⬅️ Indietro | 🏠 Menù principale | 💡 Migliora
```

`◀ Settimana` / `Settimana ▶` compaiono solo se esiste un'altra settimana dentro
`data_inizio`–`data_fine`. Un giorno con almeno un pasto da aggiornare mostra `🔄`
accanto al conteggio.

### 5.4 Giorno — `planner:day:{planner_id}:{yyyymmdd}`

Pasti del giorno nell'ordine canonico dei tipi (colazione, spuntino mattina,
pranzo, spuntino pomeriggio, cena, altro) e poi per `ordinamento`. Ogni riga
mostra tipo, nome ricetta snapshot, `✅` se completato e `🔄` se aggiornabile.

```text
➕ Aggiungi pasto
[pasto 1] … [pasto 5]
◀ Giorno | Giorno ▶
⬅️ Indietro | 🏠 Menù principale | 💡 Migliora
```

### 5.5 Dettaglio pasto — `planner:meal:{meal_id}`

Mostra tipo, data, ricetta snapshot, stato, partecipanti con la loro percentuale
snapshot e la lista ingredienti calcolata, con gli esclusi indicati come esclusi
e non come quantità zero.

```text
🍽 Pranzo · giovedì 03/09
Ricetta: Pasta al pesto (4 porzioni)
Partecipanti: Alessio 120% · Giorgia 80%

Pasta 200 g
Pesto 60 g
Pinoli — escluso per Giorgia

👥 Partecipanti | 🔄 Aggiorna
✅ Completa
⚙️ Gestisci
🗑 Elimina
⬅️ Indietro | 🏠 Menù principale | 💡 Migliora
```

Una voce libera mostra solo titolo, tipo, data e partecipanti: niente lista
ingredienti e niente `🔄 Aggiorna`, che per definizione non le si applica.

Su un pasto completato restano solo consultazione, `⚙️ Gestisci → nessuna azione
funzionale` e `🗑 Elimina`: `👥 Partecipanti`, `🔄 Aggiorna` e `✅ Completa`
spariscono, coerentemente con il trigger di congelamento.

### 5.6 Aggiungi pasto — `planner:meal:new:{planner_id}:{yyyymmdd}`

1. tipo pasto (sei pulsanti, due per riga);
2. origine: `🍳 Da ricetta` oppure `✏️ Voce libera`;
3a. da ricetta: elenco paginato + `🔎 Cerca`, riusando la ricerca ricette esistente;
3b. voce libera: titolo scritto a mano, 1–60 caratteri;
4. partecipanti: elenco dei Profili visibili con selezione multipla `☑️`/`⬜`,
   almeno uno obbligatorio;
5. riepilogo con quantità calcolate (o il solo titolo, per la voce libera) e
   `✅ Salva` / `❌ Annulla`.

Nulla viene scritto prima di `✅ Salva`.

## 6. Salvataggio e snapshot

Al `✅ Salva` una sola transazione:

1. `INSERT` in `planner_pasti` con `ricetta_nome_snapshot`,
   `ricetta_porzione_base_snapshot` e `ricetta_aggiornato_il_snapshot` letti in
   quel momento;
2. `INSERT` in `planner_pasto_profili` per ogni profilo scelto, con
   `profilo_nome_snapshot` e `fattore_porzione_snapshot` preso da
   `profilo_ricetta_porzioni` (default `1.0`);
3. per ogni coppia profilo × ingrediente, calcolo con
   `porzioni::calculate_profile_quantity` e `INSERT` in
   `planner_pasto_ingredienti_snapshot` costruito con
   `planner_alimentare::build_ingredient_snapshot`.

Regole invariate: l'escluso ha `quantita_finale_snapshot IS NULL` e
`tipo_override_snapshot = 'escluso'`; una modifica successiva a ricetta, porzioni
o override **non** tocca i pasti già salvati.

Per una voce libera il passo 3 non esiste: si scrive solo `planner_pasti` con
`tipo_voce = 'libero'`, `ricetta_id NULL`, il titolo in `ricetta_nome_snapshot` e
`1` come porzione base, piu' i partecipanti.

`🔄 Aggiorna` ricalcola e riscrive gli snapshot del solo pasto selezionato, dopo
conferma esplicita che mostra cosa cambia. Mai automatico, mai su pasti completati,
mai su voci libere.

## 7. Callback

Prefisso dedicato `planner:`, coerente con `foodprof:` e `space-members:`.
Registrazione in `main.rs` accanto agli altri, con `Box::pin` se il future cresce.

```text
planner:menu
planner:list:page:{page}
planner:new                      planner:new:scope:{space_id|self}
planner:new:period:{preset}      planner:new:cal:{yyyymm}
planner:view:{planner_id}
planner:week:{planner_id}:{offset}
planner:day:{planner_id}:{yyyymmdd}
planner:meal:{meal_id}
planner:meal:new:{planner_id}:{yyyymmdd}
planner:meal:type:{tipo}
planner:meal:src:recipe                 planner:meal:src:free
planner:meal:recipe:{recipe_id}
planner:meal:recipe:page:{page}
planner:meal:prof:{profile_id}
planner:meal:save
planner:meal:refresh:{meal_id}   planner:meal:refresh:yes:{meal_id}
planner:meal:complete:{meal_id}
planner:meal:delete:{meal_id}    planner:meal:delete:yes:{meal_id}
planner:archive:{planner_id}     planner:archive:yes:{planner_id}
planner:noop
```

## 8. Casi limite da gestire esplicitamente

- ricetta archiviata o eliminata dopo il salvataggio: il pasto resta leggibile dal
  solo snapshot, `🔄 Aggiorna` non compare;
- profilo archiviato o non più condiviso: resta il nome snapshot, il pasto non si
  rompe;
- pasto fuori periodo: intercettare il trigger e rispondere con testo leggibile;
- planner di uno spazio da cui l'utente è stato rimosso: sparisce dall'elenco,
  nessun errore tecnico;
- data del planner che attraversa il cambio di mese o di anno nella vista settimana;
- ricetta senza ingredienti: impedire l'assegnazione con messaggio chiaro;
- input testuale inatteso: vale la regola 7.2H.4F, la schermata attiva non viene
  sostituita.

## 9. Fuori perimetro

Lista della spesa, turni/routine, reminder, export, planner condiviso modificabile
da più membri contemporaneamente, spostamento di un planner fra spazi.

## 10. Verifiche previste

- unit test del dominio: costruzione snapshot multi-profilo, ordine delle settimane,
  confini del periodo, pasto completato non aggiornabile;
- `cargo fmt` → `cargo check --locked` → `git diff --check` → Clippy `-D warnings`
  → `cargo test --locked -- --test-threads=1`;
- **gia' eseguito**: la migration e' stata applicata su uno SQLite di prova sopra
  lo schema 7.3A reale, con 18 controlli superati — bump di `aggiornato_il` su
  insert/update/delete ingrediente, cascata di eliminazione ricetta senza errori,
  voce libera accettata e voce libera con ricetta rifiutata, pasto su ricetta senza
  ricetta rifiutato, `tipo_voce` fuori dominio rifiutato, snapshot su voce libera
  rifiutato, congelamento del pasto completato esteso a `tipo_voce`, duplicato
  storico dei partecipanti rifiutato, `integrity_check` e `foreign_key_check` puliti;
- resta comunque obbligatorio sull'S9: backup DB, `integrity_check`,
  `foreign_key_check` e prova su copia prima di applicare la migration reale;
- collaudo Telegram sull'S9 dal percorso `🏠 Menù principale → 🍽️ Alimentazione → 📅 Planner`.

## 11. Step successivo

**7.3C — Lista della spesa aggregata** a partire dagli snapshot dei pasti non
completati, con aggiornamento esplicito e separato da quello del planner.
