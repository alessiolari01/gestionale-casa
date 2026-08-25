# Handoff completo — Gestionale Casa

> Documento di continuità tecnica e funzionale destinato a chi deve proseguire il progetto senza conoscere le conversazioni precedenti.
>
> **Regola di manutenzione:** questo file va aggiornato dopo ogni checkpoint che modifica in modo significativo architettura, schema dati, sicurezza, flussi applicativi o interfaccia utente.

## Come riprendere il progetto

Se una nuova persona o una nuova chat riceve semplicemente l'istruzione:

```text
Prendi il progetto GitHub e inizia a leggere l'handoff.
```

deve aprire il repository `alessiolari01/gestionale-casa`, usare il branch `step-7-alimentazione`, leggere **interamente questo file** e considerarlo il punto di partenza ufficiale. Prima di modificare il progetto deve verificare `git status` e `git log -5 --oneline --decorate`, controllare la baseline indicata qui sotto e non chiedere all'utente di ricostruire da zero il contesto già documentato.

## 1. Stato del progetto al momento di questo handoff

Repository GitHub: `alessiolari01/gestionale-casa`
Branch di sviluppo corrente: `step-7-alimentazione`
Ultimo checkpoint consolidato: `2c55de7` — `Step 7.2D.0.4-0.4.1: rifinisce alimenti e integra storico prodotti`.

Il macro-step **Step 7.2E — Accesso controllato + Miglioramenti** è stato applicato e verificato anche con un secondo account Telegram approvato. Dopo il rinforzo dello stack Tokio, il secondo account riesce a navigare normalmente anche in Alimentazione. Il lavoro resta da consolidare nel prossimo checkpoint Git insieme alle modifiche strutturali successive.

Il macro-step strutturale corrente è **Step 7.2F.0 — Formati dei prodotti commerciali**.

Baseline già verificata prima di iniziare 7.2E:

- `cargo fmt --all -- --check` → ok;
- `cargo check --locked` → ok;
- `cargo clippy --all-targets --locked -- -D warnings` → ok;
- `cargo test --locked -- --test-threads=1` → **124 test superati**;
- `PRAGMA integrity_check` → `ok`;
- `PRAGMA foreign_key_check` → nessuna violazione;
- catalogo base → **418 alimenti** attivi;
- working tree pulito al commit `2c55de7`.

La nuova migration di 7.2E è `20260825153000_accesso_miglioramenti.sql`. Una volta applicata al database reale diventa immutabile come tutte le migration precedenti.

Rimane il warning esterno di future incompatibility relativo a `proc-macro-error2 v2.0.1`; non è un warning del codice del progetto.

Il database reale è il riferimento operativo; le migration già applicate **non devono essere riscritte**.

---

## 2. Obiettivo del gestionale

`gestionale-casa` è un gestionale personale e condiviso pensato per rappresentare progressivamente vari aspetti della vita quotidiana senza ridursi a un semplice inventario.

Struttura principale già adottata:

```text
Utenti
  ↓
Spazi
  ↓
Abitazioni
  ↓
Stanze
  ↓
Contenitori
  ↓
Oggetti
```

In parallelo sono presenti o previsti altri domini:

```text
Alimentazione
├── Alimenti
├── Prodotti commerciali
├── Compatibilità alimentare
├── Valori nutrizionali
├── Ricette
├── Ingredienti
├── prezzi / punti vendita          [futuro]
├── dispensa / quantità             [futuro]
└── pianificazione                  [futuro]

Vestiti                              [fondazione/modulo non ancora operativo]
Veicoli                              [fondazione/modulo non ancora operativo]

Miglioramenti
├── backlog interno del gestionale
├── autore
├── stato
└── screenshot/allegati
```

Principio generale: il backend deve modellare entità e permessi in modo riutilizzabile, mentre Telegram è soltanto un frontend. Una futura applicazione deve poter riusare la stessa logica senza dipendere dai pulsanti Telegram.

---

## 3. Principi architetturali fondamentali

Questi concetti **non sono equivalenti**:

```text
Ruolo di sistema
≠
Ruolo nello spazio
≠
Proprietario della risorsa
≠
Posizione fisica
≠
Spazio predefinito
≠
Visibilità
≠
Permesso di modifica
≠
Permesso di gestione/condivisione
```

Esempio alimento:

```text
🥩 Petto di pollo
Proprietario: utente A
Visibile: spazio personale + spazio condiviso
Utente B: può vedere
Utente C: può modificare
```

La visibilità non concede automaticamente il diritto di modifica.

Un amministratore del sistema non diventa automaticamente proprietario delle risorse degli altri utenti.

---

## 4. Sicurezza: backend fail-closed

Nascondere un pulsante **non è una barriera di sicurezza**.

Ogni operazione sensibile deve verificare realmente:

```text
callback/comando/richiesta
        ↓
risoluzione identità
        ↓
utente autorizzato?
        ↓
risorsa visibile?
        ↓
ownership / permesso richiesto?
        ↓
operazione consentita
```

Se una condizione manca, l'operazione deve fallire.

Esempio importante:

```text
utente possiede permesso modifica
+
perde l'accesso allo spazio necessario
=
permesso non operativo
```

Il record del permesso può rimanere nel database, ma il backend non deve consentire l'operazione.

---

## 5. Ruoli globali e amministrazione

Gli utenti hanno un `ruolo_sistema` separato dai permessi su spazi e risorse.

Ruoli attuali:

- `utente`;
- `admin`.

Il primo utente/bootstrap è amministratore.

### Utente normale

Non deve vedere funzioni tecniche come stato del backend o amministrazione globale.

### Admin

Nel menu principale vede:

```text
🛠️ Amministrazione
```

Area amministrativa attuale:

```text
🛠️ Amministrazione
├── 🧭 Panoramica
├── 📊 Stato sistema
├── 👥 Utenti
└── 📨 Richieste di accesso (N)      [solo amministratore principale]
```

`/admin`, `/status` e le callback amministrative sono protette anche lato backend.

Le notifiche backend `online/offline` sono riservate agli amministratori.

### Accesso al bot — Step 7.2E

Dallo Step 7.2E la whitelist Telegram statica non è più il modello ordinario di autorizzazione. `ALLOWED_CHAT_IDS` resta come **bootstrap/emergenza**: serve a inizializzare il primo amministratore su un database nuovo e non deve essere usata per aggiungere manualmente ogni nuovo utente.

Flusso applicativo:

```text
account Telegram sconosciuto
        ↓
può contattare il bot
        ↓
📨 Richiedi accesso
        ↓
richiesta in attesa nel database
        ↓
👑 amministratore principale
        ↓
✅ Approva / ❌ Rifiuta
        ↓
se approvato:
utente normale + account Telegram + spazio personale
```

L'approvazione **non concede automaticamente** accesso allo spazio bootstrap, alle case, agli alimenti personali o ad altre risorse di altri utenti. Gli spazi condivisi continuano a richiedere membership/inviti espliciti.

È introdotto il concetto distinto di `amministratore_principale`: oggi è unico e viene assegnato al proprietario/bootstrap già amministratore. Solo lui vede e decide le richieste di accesso. Questo concetto è separato da `ruolo_sistema = admin`, così in futuro potranno esistere altri amministratori senza ricevere automaticamente il potere di approvare nuovi account.

Tabelle principali:

- `richieste_accesso`;
- `utenti.amministratore_principale`;
- `account_telegram` resta il collegamento definitivo dopo l'approvazione.

Il backend prima prova a risolvere un account Telegram già approvato dal database; usa `ALLOWED_CHAT_IDS` solo come fallback bootstrap per un account non ancora esistente.

### Sezione `💡 Miglioramenti`

Tutti gli utenti Telegram approvati possono usare la sezione `💡 Miglioramenti`. Non è collegata agli spazi domestici: rappresenta feedback sul gestionale stesso.

Flusso iniziale:

```text
💡 Miglioramenti
├── ➕ Nuovo miglioramento
│   ├── descrizione
│   ├── screenshot facoltativo
│   └── salvataggio
├── 📋 I miei miglioramenti
└── 🗂️ Tutti i miglioramenti        [admin]
```

Gli screenshot sono salvati localmente sotto `data/media/miglioramenti/<id>/` e registrati in `miglioramento_allegati`, quindi rientrano nel modello di backup dei media. La struttura DB supporta più allegati per miglioramento anche se l'UX iniziale rimane volutamente semplice.

Stati iniziali:

```text
🟡 Aperto
🔵 Pianificato
✅ Fatto
❌ Scartato
```

L'autore vede i propri miglioramenti. Gli admin possono vedere tutti i miglioramenti e cambiarne lo stato.

**Nuova regola di sviluppo:** il progetto procede prima per macro-struttura e funzionalità principali; le osservazioni UX non bloccanti vanno registrate nella sezione `💡 Miglioramenti` e vengono raffinate in una fase successiva.

---

## 6. Identità, utenti e spazi

Le fondazioni condivise introdotte nello Step 7.1 comprendono:

- utenti;
- account Telegram;
- spazi;
- membership negli spazi;
- spazio attivo;
- vista multi-spazio;
- audit autore/origine.

L'utente può avere più spazi e può visualizzare il solo spazio attivo oppure una vista multi-spazio, in base alle preferenze e ai permessi.

Il progetto supporta più case/abitazioni separate.

---

## 7. Luoghi

Gerarchia:

```text
Spazio
└── Abitazione
    └── Stanza
        └── Contenitore
            └── eventuali contenitori figli
```

Gli oggetti possono essere assegnati e spostati tra luoghi compatibili.

### Regola UI: niente ID tecnici

Non mostrare:

```text
Casa #2
Stanza #4
Contenitore #8
/luogo_r4
```

Mostrare nomi umani.

La struttura completa dei luoghi dispone inoltre di comandi diretti human-friendly, ad esempio:

```text
/casa_casa_principale
/stanza_camera
/contenitore_scaffale_prova
```

In caso di duplicato viene aggiunto progressivamente contesto umano, non l'ID:

```text
/stanza_camera_casa_principale
/stanza_camera_casa_livorno
```

Il backend risolve poi il comando nell'ID interno reale e verifica visibilità/permessi.

---

## 8. Oggetti

Il modulo Oggetti permette almeno:

- creazione;
- elenco;
- ricerca;
- dettaglio;
- modifica degli oggetti già salvati;
- assegnazione e spostamento nei luoghi;
- integrazione con storico e foto;
- proprietà separata dalla posizione fisica;
- condivisione multi-spazio secondo le fondazioni introdotte nello Step 7.

Gli ID tecnici devono rimanere solo in database, callback, query, foreign key e log tecnici.

---

## 9. Contenitori

I contenitori sono gerarchici e possono appartenere a stanze o ad altri contenitori secondo i vincoli del database.

La UI deve mostrare percorsi umani e non ID.

Le operazioni di spostamento devono rispettare i confini degli spazi e la coerenza casa/stanza/contenitore.

---

## 10. Storico

Lo storico è trasversale e usa almeno:

- `storico_entita`;
- `storico_eventi`;
- `storico_cambiamenti`;
- `storico_cambi_luogo`.

Sono presenti snapshot utili per mantenere leggibili gli eventi anche quando la struttura corrente cambia.

Gli ID degli eventi non vengono mostrati nella UI.

Per eventi collegati si usa testo umano come:

```text
🔗 Collegato a un evento precedente
```

### Stato UI dopo D0.4/D0.4.1

Lo Storico usa **massimo 5 eventi per pagina**, mantenendo conteggio totale e indicatore pagina corrente/totale.

La navigazione tra pagine è separata dalla navigazione tra sezioni. La parte finale della schermata segue il principio:

```text
⬅️ Pagina precedente | X / Y | Pagina successiva ➡️

🔎 Filtri | 🧹 Azzera filtri

⬅️ Indietro | 🏠 Menu principale
```

Il pulsante centrale della paginazione è informativo/no-op.

`⬅️ Indietro` torna alla sezione precedente e non va confuso con `⬅️ Pagina precedente`.

È presente anche il filtro modulo `Alimentazione`.

### Audit prodotti commerciali

D0.4.1 integra nello Storico i prodotti commerciali associati agli alimenti. Vengono registrati almeno:

- creazione/associazione prodotto;
- modifica marca;
- modifica nome commerciale;
- modifica quantità confezione;
- modifica unità confezione;
- aggiunta/modifica/rimozione EAN;
- aggiunta/modifica/rimozione valori nutrizionali.

Gli eventi usano nomi umani, ad esempio `Philadelphia · Original` collegato a `🥛 Formaggio spalmabile`, senza esporre ID tecnici.

Le modifiche al prodotto e la relativa registrazione Storico avvengono nella stessa transazione: se l'audit fallisce, non deve restare una modifica parziale.

L'attore deve essere realmente valido nello spazio dell'evento e la lettura dello Storico ricontrolla anche la visibilità della risorsa alimentare associata. Non indebolire questi controlli per rendere più permissiva la UI.

Gli eventi avvenuti prima dell'introduzione dell'audit prodotti **non vengono inventati retroattivamente**.

---

## 11. Foto

Il modulo Foto è integrato con le entità del gestionale e con lo Storico.

La UI non deve mostrare `foto_id`, path filesystem o altri dettagli tecnici quando non necessari all'utente.

---

# ALIMENTAZIONE

## 12. Modello generale Alimentazione

Il modulo principale è:

```text
src/modules/alimentazione.rs
```

Le fondazioni Ricette sono in:

```text
src/modules/ricette.rs
```

I permessi generici sono in:

```text
src/resource_permissions.rs
```

Concetti principali:

```text
Alimento generico
├── proprietà / catalogo globale
├── unità predefinita
├── categorie
├── visibilità
├── permessi
├── compatibilità alimentare
└── prodotti commerciali

Prodotto commerciale
├── alimento generico
├── marca
├── nome commerciale
├── formato confezione
├── unità confezione
├── EAN/barcode predisposto
└── valori nutrizionali opzionali

Ricetta
└── ingredienti
    ├── alimento_id              SEMPRE
    ├── prodotto_alimentare_id  opzionale
    ├── quantità
    └── unità
```

---

## 13. Alimenti generici

Ogni alimento normale ha un proprietario reale, salvo il catalogo globale di base.

La stessa risorsa può essere visibile in più spazi senza duplicazione.

### Creazione alimento personale

Flusso definito:

```text
➕ Nuovo alimento
Nome
 ↓
Unità
 ↓
Categoria
 ↓
Visibilità
 ↓
✅ Salva
```

L'unità è obbligatoria.

### Modifica alimento

Dal dettaglio, quando autorizzato:

```text
✏️ Modifica alimento
├── 📝 Nome
├── 📏 Unità
├── 🏷 Categoria
├── 👥 Visibilità
└── 🔐 Collaboratori
```

Per il catalogo base globale solo l'admin può modificare nome/unità/categoria; visibilità e collaboratori non hanno senso per un elemento globale e non vengono trattati come per gli alimenti personali.

---

## 14. Unità di misura

Unità leggibili nella UI:

- `grammi (g)`;
- `chilogrammi (kg)`;
- `millilitri (ml)`;
- `litri (l)`;
- `pezzi (pz)`;
- `cucchiaio`;
- `cucchiaino`;
- `quanto basta (q.b.)` dove previsto.

Conversioni automatiche previste soltanto per famiglie sicure:

```text
g ↔ kg
ml ↔ l
```

Non convertire automaticamente `pezzi ↔ grammi`, perché manca il peso unitario.

### Regola importante

L'unità predefinita dell'alimento è solo un **default per i nuovi inserimenti**.

Prodotti commerciali e ingredienti ricetta salvano la propria unità esplicitamente.

Se cambia l'unità predefinita dell'alimento, dati già esistenti **non devono essere modificati o convertiti silenziosamente**.

---

## 15. Categorie alimenti

Categorie merceologiche attuali:

- Verdure;
- Frutta;
- Carne;
- Pesce;
- Latticini;
- Uova;
- Cereali e derivati;
- Legumi;
- Condimenti e salse;
- Bevande;
- Dolci;
- Altro.

La relazione DB `alimento_categorie` è molti-a-molti, anche se la UI attuale usa principalmente una categoria principale.

Il filtro categorie supporta selezione multipla con semantica **OR**.

Esempio:

```text
Carne + Verdure
=
alimenti di Carne OPPURE Verdure
```

I risultati devono essere deduplicati.

---

## 16. Catalogo base

Migration immutabile già applicata:

```text
20260825014500_catalogo_alimenti_base.sql
```

Ha sostituito gli alimenti presenti al momento dell'introduzione con un catalogo globale condiviso di **418 alimenti base**.

Gli alimenti del catalogo:

- sono visibili globalmente;
- sono utilizzabili da tutti come ingredienti;
- non vengono duplicati per utente/spazio;
- sono modificabili, nell'assetto attuale, solo dall'admin.

I nomi includono un'emoji descrittiva, per esempio:

```text
🥩 Petto di pollo
🍚 Riso basmati
🥬 Zucchine
🥛 Formaggio spalmabile
```

---

## 17. Compatibilità alimentare

Migration immutabile già applicata:

```text
20260825023000_compatibilita_alimentare.sql
```

Compatibilità separata dalle categorie merceologiche.

Etichette attuali:

- 🌱 Vegano;
- 🥬 Vegetariano;
- 🐟 Pescetariano;
- 🌾 Senza glutine;
- 🥛 Senza lattosio;
- 🚫🥛 Senza latte;
- 🥚 Senza uova;
- 🥜 Senza arachidi;
- 🌰 Senza frutta a guscio;
- 🫘 Senza soia;
- 🐟 Senza pesce;
- 🦐 Senza crostacei;
- 🦪 Senza molluschi;
- 🌿 Senza sedano;
- 🌼 Senza senape;
- ⚪ Senza sesamo;
- 🌱 Senza lupini;
- 🧪 Senza solfiti;
- 🍷 Senza alcol.

Ogni alimento/etichetta usa:

```text
si
no
verificare
```

`verificare` è intenzionale quando marca, formulazione o processo produttivo possono cambiare il risultato.

Per allergie/intolleranze questa funzione è un supporto gestionale e non sostituisce la lettura dell'etichetta reale.

### Compatibilità delle Ricette

Vista DB:

```text
v_ricetta_compatibilita_alimentare
```

Regola:

```text
tutti gli ingredienti = si
→ ricetta compatibile

almeno uno = no
→ ricetta non compatibile

nessun no ma almeno uno = verificare
→ ricetta da verificare

compatibilità mancante
→ verificare (fail-closed)
```

---

## 18. Elenco, ricerca e filtri alimenti — stato attuale

La paginazione reale è operativa e dopo D0.4 usa:

```text
FOOD_PAGE_SIZE = 5
```

Le schermate mostrano il totale e la pagina corrente, ad esempio:

```text
📋 Alimenti · 418 risultati
Pagina 2/84
```

La riga di navigazione include un pulsante centrale informativo/no-op:

```text
⬅️ Pagina precedente | 2/84 | Pagina successiva ➡️
```

Lo stesso schema vale per:

- elenco completo;
- risultati di ricerca;
- filtri per categoria.

Gli alimenti base non mostrano più `🌐` né nel testo né nei pulsanti. Gli indicatori vengono usati solo per le eccezioni:

```text
🥩 Alimento personale 👤
🍰 Alimento condiviso 👥
```

Legenda:

```text
👤 tuo · 👥 condiviso
```

Nel dettaglio alimento è ancora possibile mostrare `🌐 Catalogo base`.

### Ricerca tramite prodotto commerciale

La ricerca alimenti cerca anche in:

- marca del prodotto commerciale;
- nome commerciale del prodotto.

Esempio: cercando `Philadelphia` oppure `Original` può essere restituito `🥛 Formaggio spalmabile` con i prodotti corrispondenti come contesto.

Se più prodotti dello stesso alimento corrispondono, l'alimento compare **una sola volta**.

I filtri mantengono contesto e pagina con `🔄 Aggiorna`.

---

## 19. Prodotti commerciali e formati di vendita

Migration prodotto già applicata e immutabile:

```text
20260825101500_prodotti_alimentari.sql
```

Nuova migration Step 7.2F.0:

```text
20260825220000_formati_prodotti_alimentari.sql
```

Dal 7.2F.0 **prodotto commerciale** e **formato acquistabile** sono separati.

Esempio corretto:

```text
🥛 Formaggio spalmabile                    ← alimento generico
└── 🛒 Philadelphia · Original             ← prodotto commerciale
    ├── 📦 175 g                           ← formato
    ├── 📦 200 g
    └── 📦 350 g
```

Il prodotto conserva identità stabile: alimento associato, marca, nome
commerciale, valori nutrizionali e futuro eventuale metadata di prodotto.

Il formato conserva invece:

- quantità confezione;
- unità confezione;
- barcode/EAN;
- stato attivo;
- in futuro disponibilità/prezzo per punto vendita.

La tabella autorevole è `formati_prodotto_alimentare`. Le vecchie colonne
`quantita_confezione`, `unita_confezione_id` e `codice_ean` rimaste in
`prodotti_alimentari` esistono soltanto per compatibilità con le migration già
applicate: il codice nuovo non deve usarle come fonte autorevole. La migration
7.2F.0 copia automaticamente ogni vecchia confezione nel primo formato e sposta
logicamente l'EAN sul formato.

È disponibile la vista:

```text
v_prodotti_formati_attivi
```

che restituisce una riga per formato attivo ed è la base prevista per Lista
spesa, disponibilità e prezzi.

### Regola Ricette

Una Ricetta può usare:

```text
alimento_id                 obbligatorio
prodotto_alimentare_id      facoltativo
quantità/unità              proprie della ricetta
```

La Ricetta **non salva il formato**. Se richiede 150 g di `Philadelphia ·
Original`, non deve sapere se verrà acquistata una confezione da 175 g, 200 g o
350 g. Questa decisione appartiene alla futura Lista spesa.

### Regola Lista spesa futura

La Lista spesa dovrà aggregare la quantità richiesta e scegliere tra i formati
disponibili la combinazione più adatta. Prima logica prevista:

```text
quantità sufficiente
→ minor avanzo
→ minor numero di confezioni
```

quando saranno presenti prezzi reali potrà essere privilegiato il costo totale
più conveniente, mantenendo visibili avanzo previsto e punto vendita.

### UI Telegram strutturale

Dal dettaglio prodotto:

```text
🛒 Philadelphia · Original
├── 📦 Formati (N)
├── 🧮 Valori nutrizionali
└── ✏️ Modifica prodotto
```

`✏️ Modifica prodotto` modifica marca e nome commerciale. Quantità, unità ed
EAN si gestiscono nel singolo formato.

La sezione formati permette almeno:

```text
📦 Formati disponibili
├── ⚖️ 175 g
├── ⚖️ 200 g
├── ⚖️ 350 g
└── ➕ Aggiungi formato
```

Ogni formato ha dettaglio e modifica di quantità/unità/EAN. L'aggiunta di un
nuovo formato con la stessa marca e lo stesso nome commerciale riusa lo stesso
prodotto invece di crearne un duplicato. Un formato identico sullo stesso
prodotto viene rifiutato.

### Storico

Creazione e modifica dei formati vengono registrate nello storico dello stesso
prodotto commerciale con componente `formato_prodotto`; non viene introdotta
una seconda identità di prodotto soltanto perché cambia la confezione.

### Icone unità

- `⚖️` → g / kg;
- `🥤` → ml / l;
- `🔢` → pz;
- `🥄` → cucchiaio / cucchiaino.

## 20. Valori nutrizionali prodotto

Migration immutabile già applicata:

```text
20260825113000_prodotti_nutrizione_ricette.sql
```

Tabella:

```text
valori_nutrizionali_prodotto
```

I dati sono facoltativi e appartengono al prodotto reale, non all'alimento generico.

Riferimento:

```text
per 100 g
oppure
per 100 ml
```

Campi principali:

- kcal;
- kJ;
- grassi;
- saturi;
- carboidrati;
- zuccheri;
- fibre;
- proteine;
- sale.

I singoli valori possono essere assenti.

### Stato UI dopo D0.4

L'inserimento usa un messaggio con valori separati da `;`.

Se l'utente fornisce meno di 9 valori ma almeno un valore valido, i campi mancanti in coda vengono completati automaticamente con `-`.

Esempio:

```text
225; 934; -
```

viene interpretato come:

```text
225; 934; -; -; -; -; -; -; -
```

Prima del salvataggio viene mostrato **sempre** un riepilogo completo e viene richiesta conferma esplicita:

```text
✅ Conferma
✏️ Modifica
❌ Annulla
```

Regole:

- meno di 9 → completare con `-`;
- 9 → accettare;
- più di 9 → errore;
- token non numerico diverso da `-` → errore;
- nessun salvataggio prima di `✅ Conferma`.

Le operazioni nutrizionali vengono inoltre registrate nello Storico del prodotto commerciale.

### Rifinitura UI ancora approvata

Quando un prodotto non possiede alcun valore nutrizionale, non limitarsi al testo “nessun valore inserito”: mostrare anche una breve spiegazione che invita ad aggiungerli, ricordando che la sezione è facoltativa. Deve restare disponibile `➕ Inserisci valori`; se l'utente entra nel wizard deve poter usare `❌ Annulla` senza salvare nulla.

---

# RICETTE

## 21. Stato Ricette

Le fondazioni database sono già presenti, ma la UI Telegram completa delle Ricette **non è ancora operativa**.

Migration base:

```text
20260824222000_ricette_fondazioni.sql
```

Tabelle principali:

```text
ricette
ricetta_spazi
ricetta_ingredienti
```

Le Ricette devono seguire lo stesso modello:

```text
Proprietario
↓
Spazi visibili
↓
Collaboratori autorizzati
```

Visibile ≠ modificabile.

I permessi devono riutilizzare:

```text
inviti_risorsa
permessi_risorsa
```

con `tipo_risorsa = "ricetta"`.

---

## 22. Ingredienti Ricetta

Regola fondamentale:

**Mai salvare l'ingrediente come semplice testo libero se esiste nel catalogo.**

Struttura:

```text
ricetta_ingredienti
├── ricetta_id
├── alimento_id                 obbligatorio
├── prodotto_alimentare_id      opzionale
├── quantità
└── unità
```

Lo Step D0.3 ha già aggiunto `prodotto_alimentare_id` opzionale.

Il DB verifica che l'eventuale prodotto scelto appartenga allo stesso alimento.

### Flusso approvato per il prossimo wizard

```text
🥕 Scegli alimento
        ↓
esistono prodotti commerciali?
   ├── no → continua con generico
   └── sì
        ↓
   🌐 Usa alimento generico
   oppure
   🛒 Scegli prodotto reale
        ↓
   quantità
        ↓
   unità proposta dall'alimento
   + possibilità di cambiarla
```

La scelta del prodotto **non sostituisce** `alimento_id`.

Esempio:

```text
alimento_id = Formaggio spalmabile
prodotto_alimentare_id = Philadelphia Original (opzionale)
quantità ricetta = 250 g
confezione prodotto = 200 g
```

Quantità ricetta e quantità confezione sono indipendenti.

### Vantaggio futuro

Ricetta generica:

```text
Formaggio spalmabile
→ confronta tutti i prodotti associati
→ suggerisce il più conveniente
```

Ricetta specifica:

```text
Philadelphia Original
→ cerca prezzi e disponibilità proprio di quel prodotto
```

Se il prodotto specifico viene disattivato, la ricetta deve restare valida grazie ad `alimento_id`.

---

## 23. Ricerca Ricette per ingredienti

La fondazione Rust è già presente in `src/modules/ricette.rs`.

Semantica scelta: **OR**, ordinata per numero di ingredienti richiesti presenti.

Esempio:

```text
Richiesti:
pollo + riso + zucchine

Ricetta A → 3/3
Ricetta B → 2/3
Ricetta C → 1/3
```

Ordine:

```text
3/3
2/3
1/3
```

La query conta `COUNT(DISTINCT alimento_id)` limitato agli ingredienti richiesti.

Criteri secondari attuali nella fondazione Rust:

1. numero di corrispondenze decrescente;
2. nome ricetta;
3. ID interno stabile, mai mostrato in UI.

---

## 24. UI Ricette prevista

Direzione approvata:

```text
🍽 Alimentazione
├── 🥕 Alimenti
└── 🍳 Ricette
    ├── 📋 Elenco ricette
    ├── ➕ Nuova ricetta
    ├── 🔎 Cerca
    └── 🥕 Cerca per ingredienti
```

Creazione prevista:

```text
➕ Nuova ricetta
Nome
 ↓
Ingredienti
 ↓
per ogni ingrediente:
  generico/prodotto specifico se disponibile
  quantità
  unità
 ↓
Procedimento
 ↓
Visibilità
 ↓
✅ Salva
```

Dettaglio previsto:

```text
🍳 Pollo e riso

👤 Proprietà: tua
👥 Visibile in: ...

🥕 Ingredienti
• Petto di pollo — 150 g
• Riso — 100 g

📖 Procedimento
...

🎯 Compatibilità
...

✏️ Modifica ricetta
```

Modifica prevista:

```text
✏️ Modifica ricetta
├── 📝 Nome
├── 🥕 Ingredienti
├── 📖 Procedimento
├── 👥 Visibilità
└── 🔐 Collaboratori
```

---

## 25. Nutrizione Ricette — direzione futura

Se gli ingredienti usano prodotti commerciali dotati di valori nutrizionali, il gestionale potrà calcolare automaticamente valori totali e per porzione.

Esempio futuro:

```text
Totale ricetta
🔥 kcal
💪 proteine
🍞 carboidrati
🥑 grassi
...

8 porzioni
→ valori per porzione
```

Se mancano dati affidabili, non inventare valori.

Mostrare invece che il calcolo è incompleto e quali ingredienti non hanno dati sufficienti.

---

# TELEGRAM / UX

## 26. Regole obbligatorie UI

### Menu principale sempre raggiungibile

Ogni schermata deve offrire:

```text
🏠 Menu principale
```

### Indietro

Se esiste un livello precedente:

```text
⬅️ Indietro
```

### Pulsanti di navigazione compatti

Quando possibile:

```text
⬅️ Indietro | 🏠 Menu principale
```

Nei wizard:

```text
⬅️ Indietro | ❌ Annulla | 🏠 Menu principale
```

### Annulla reale

`❌ Annulla` deve:

- eliminare la bozza/sessione;
- non salvare dati parziali;
- non lasciare stato conversazionale attivo.

Anche tornare al Menu principale durante una creazione deve annullare la bozza quando previsto.

### Niente ID tecnici

Non mostrare ID DB all'utente.

### Liste sintetiche

Mostrare solo ciò che serve per scegliere.

Informazioni dettagliate appartengono al dettaglio.

### Italiano corretto

Usare accenti reali:

```text
unità
visibilità
identità
è
può
già
```

Non usare forme tipo `unita'`.

### Pulsanti e comandi in parallelo

I comandi testuali devono continuare a funzionare anche se non vengono elencati nel menu principale.

Scopo: mantenere la logica riutilizzabile da un futuro frontend.

---

## 27. Menu principale

La riga “Comandi rapidi” è stata rimossa.

Menu orientato ai pulsanti, con moduli come:

```text
📜 Storico
👤 Profilo
👥 Spazi
🏷 Oggetti
🏠 Case, stanze e contenitori
👕 Vestiti · prossimamente
🚗 Veicoli · prossimamente
🍽 Alimentazione
```

Per admin:

```text
🛠️ Amministrazione
```

---

# PERMESSI RISORSE

## 28. Sistema generico

Migration:

```text
20260824201500_permessi_risorse_condivise.sql
```

Tabelle:

```text
inviti_risorsa
permessi_risorsa
```

Identificazione:

```text
tipo_risorsa
risorsa_id
```

Esempi:

```text
alimento / 17
ricetta / 42
```

Livelli concettuali:

- può vedere;
- può modificare;
- può modificare + gestire permessi/condivisione.

Gli inviti devono essere accettati/rifiutati esplicitamente prima che il permesso diventi operativo.

---

# DATABASE

## 29. Migration principali in ordine

Non modificare migration già applicate.

```text
20260812120000_schema_core.sql
20260814121600_oggetti.sql
20260815183000_luoghi.sql
20260815215400_storico.sql
20260817171600_contenitori.sql
20260820230000_storico_contenitori.sql
20260823153000_fondazioni_condivise.sql
20260823174500_spazi_operativi.sql
20260823200000_vista_multispazio_condivisione.sql
20260823232000_storico_spazi_luogo.sql
20260824143000_alimenti_unita.sql
20260824160500_alimenti_proprieta_condivisione.sql
20260824173500_categorie_alimenti.sql
20260824201500_permessi_risorse_condivise.sql
20260824222000_ricette_fondazioni.sql
20260825003000_ruoli_sistema_amministrazione.sql
20260825014500_catalogo_alimenti_base.sql
20260825023000_compatibilita_alimentare.sql
20260825101500_prodotti_alimentari.sql
20260825113000_prodotti_nutrizione_ricette.sql
```

Migration SQLx incorporate tramite:

```rust
sqlx::migrate!("./migrations")
```

Vengono applicate automaticamente all'avvio.

Foreign key abilitate per le connessioni SQLite.

---

## 30. Regola migration

Una migration applicata al database reale è immutabile.

Se serve cambiare schema:

```text
NON modificare vecchia migration
→ creare migration nuova e incrementale
```

Questa regola è già stata importante durante l'introduzione di compatibilità e prodotti commerciali.

---

# STRUTTURA CODICE

## 31. File principali

```text
src/main.rs
```

- bootstrap applicazione;
- routing comandi/callback Telegram;
- menu principale;
- amministrazione;
- gestione startup/shutdown.

```text
src/db.rs
```

- apertura pool SQLite;
- migration automatiche;
- impostazioni DB.

```text
src/config.rs
```

- configurazione runtime;
- whitelist/bootstrap Telegram attuale;
- variabili ambiente.

```text
src/identity.rs
```

- identità utente corrente;
- spazio corrente / vista multi-spazio;
- ruolo sistema;
- actor usato dalle query/moduli.

```text
src/resource_permissions.rs
```

- fondazione generica per inviti e permessi su risorse.

Moduli:

```text
src/modules/alimentazione.rs
src/modules/ricette.rs
src/modules/oggetti.rs
src/modules/luoghi.rs
src/modules/contenitori.rs
src/modules/storico.rs
src/modules/foto.rs
src/modules/vestiti.rs
src/modules/veicoli.rs
```

---

# AMBIENTE DI SVILUPPO

## 32. Runtime principale

Dispositivo di test/runtime principale:

```text
Samsung Galaxy S9
Termux
repo: ~/gestionale-casa
```

Il PC Windows è usato soprattutto per:

- scaricare pacchetti ZIP;
- trasferirli via `scp`;
- GitHub/integrrazione;
- eventuale sviluppo locale.

---

## 33. Avvio standard bot sull'S9

```bash
cd ~/gestionale-casa && \
CARGO_BUILD_JOBS=1 \
CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_PROFILE_DEV_CODEGEN_UNITS=16 \
CARGO_INCREMENTAL=0 \
RUSTFLAGS="-C debuginfo=0 -C link-arg=-Wl,--threads=1" \
cargo run --locked
```

---

## 34. Test standard sull'S9

```bash
cd ~/gestionale-casa && \
CARGO_BUILD_JOBS=1 \
CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_PROFILE_TEST_CODEGEN_UNITS=16 \
CARGO_INCREMENTAL=0 \
RUSTFLAGS="-C debuginfo=0 -C link-arg=-Wl,--threads=1" \
cargo test --locked -- --test-threads=1
```

---

## 35. Pipeline preferita

Quando possibile fornire un unico blocco copiabile che arrivi fino al bot:

```text
backup DB se necessario
→ patch
→ cargo fmt
→ fmt --check
→ cargo check
→ git diff --check
→ clippy -D warnings
→ test
→ integrity_check
→ foreign_key_check
→ cargo run
```

Non usare `set -e` nelle sessioni interattive SSH/Termux.

Usare concatenazione:

```bash
comando1 && \
comando2 && \
comando3
```

Così un errore ferma la pipeline ma non chiude la sessione SSH.

---

## 36. Warning esterno noto

Può comparire il warning di future incompatibility relativo a:

```text
proc-macro-error2 v2.0.1
```

È una dipendenza esterna e non blocca il checkpoint.

Il codice del progetto deve invece passare:

```bash
cargo clippy --all-targets --locked -- -D warnings
```

---

# GIT / CONSEGNA PATCH

## 37. Workflow

Branch attuale:

```text
step-7-alimentazione
```

Regola pratica:

- creare checkpoint frequenti;
- S9 esegue runtime/test;
- GitHub mantiene la cronologia;
- dopo commit/push il working tree deve tornare pulito.

Controlli tipici:

```bash
git status
git diff --check
git diff --cached --check
git diff --cached --stat
git log -3 --oneline --decorate
```

---

## 38. Regola per script/patch

Evitare patch fragili basate su enormi `replace()` di blocchi Rust formattati esattamente.

Preferire:

- patch semantiche robuste;
- sostituzione completa controllata del file;
- hash/fingerprint solo quando basati sul reale stato corrente;
- backup automatico.

Se uno script stampa:

```text
STOP: ...
Nessun file scritto.
```

significa che **quello script non ha modificato il repository**; non fare restore automaticamente.

---

# MODIFICHE APPROVATE DA FARE NEI PROSSIMI STEP

**Politica di sviluppo attuale:** completare prima la struttura e le funzionalità principali dei moduli. Le rifiniture UX non bloccanti, come categorie più naturali, scorciatoie di navigazione o testi più guidati, vanno annotate nel backlog `💡 Miglioramenti` e affrontate in una fase di rifinitura dedicata.

## 39. Rifiniture e classificazione ancora da applicare

Le rifiniture D0.4/D0.4.1 elencate nelle versioni precedenti di questo handoff sono ora operative. Restano approvate queste modifiche successive.

### Categorie Alimentazione

Aggiungere una categoria dedicata:

```text
🍝 Pasta
```

separata da `🌾 Cereali e derivati`.

Prima della migration rivalutare anche altre categorie troppo ampie mantenendo però un numero contenuto e utile per ricerca, filtri e scelta ingredienti. Candidati già discussi:

```text
🍚 Riso
🍞 Pane e prodotti da forno
🥜 Frutta secca e semi
🌿 Spezie e aromi
```

Non creare invece categorie eccessivamente granulari quando il nome dell'alimento o le etichette di compatibilità coprono già la distinzione.

Questa modifica deve usare **una nuova migration** e riclassificare in modo esplicito i 418 alimenti base esistenti. Non riscrivere migration già applicate.

### Accesso rapido ai prodotti associati

Nelle schermate di ricerca/elenco dove un alimento ha prodotti commerciali associati, valutare un pulsante diretto:

```text
🛒 Prodotti associati (N)
```

così l'utente può raggiungere i prodotti senza dover prima aprire il dettaglio dell'alimento.

Il pulsante è condizionale e va mostrato solo quando esistono prodotti associati. Se la ricerca è stata soddisfatta tramite marca/nome commerciale, questa scorciatoia è particolarmente utile.

Se i prodotti sono molti, non elencarli tutti nel testo: mostrare un contesto compatto e usare `🛒 Prodotti associati (N)` per aprire la lista completa.

### Valori nutrizionali vuoti

Quando non sono presenti valori nutrizionali, mostrare un messaggio guidato che spieghi che:

- i valori possono essere aggiunti ora;
- la sezione è facoltativa;
- sarà sempre possibile tornare indietro;
- entrando nell'inserimento si può usare `❌ Annulla` senza salvare.

---

# PROSSIMA DIREZIONE

## 40. Step immediatamente successivo consigliato

Lo Step 7.2E ha consolidato accesso approvato al bot e backlog `💡 Miglioramenti`. Lo Step 7.2F.0 separa ora prodotto commerciale e formato acquistabile, fondazione necessaria prima di Ricette, prezzi e Lista spesa.

Dopo verifica e checkpoint di 7.2F.0, il prossimo macro-step consigliato torna a:

```text
Step 7.2D.1 — Ricette operative Telegram
```

Prima tranche strutturale:

1. menu Ricette;
2. elenco e dettaglio;
3. creazione ricetta;
4. ingredienti collegati ad `alimenti.id`;
5. scelta alimento generico oppure prodotto commerciale specifico quando disponibile;
6. quantità/unità indipendenti dalla confezione del prodotto;
7. procedimento;
8. ownership e visibilità negli spazi;
9. modifica e permessi collaboratori;
10. ricerca per nome e ricerca OR per ingredienti con ranking.

Le rifiniture già annotate — `🍝 Pasta`, eventuale `🍚 Riso`, `🍞 Pane e prodotti da forno`, pulsante diretto `🛒 Prodotti associati (N)`, messaggi guidati dei valori nutrizionali e altre osservazioni UX — **non devono più bloccare il macro-step strutturale**. Possono essere registrate e gestite tramite `💡 Miglioramenti`.

Successivamente:

- compatibilità alimentare derivata nelle Ricette;
- calcolo nutrizionale;
- prezzi/punti vendita;
- dispensa e quantità;
- pianificazione;
- fase dedicata di rifinitura UX.


## 41. Regola fissa di documentazione

Dopo ogni step che modifica in modo rilevante:

- architettura;
- schema DB;
- modello permessi;
- flussi applicativi;
- struttura dei moduli;
- interfaccia Telegram;
- regole UX;
- funzionalità future già approvate;

aggiornare **questo file** prima di considerare chiuso il checkpoint.

Il documento deve permettere a una terza persona di capire:

1. cosa esiste;
2. cosa funziona;
3. cosa è solo predisposto;
4. cosa è stato deciso per il futuro;
5. quali file modificare;
6. quali migration non toccare;
7. come testare;
8. da quale branch/checkpoint proseguire.

---

## 42. Sintesi finale per il prossimo sviluppatore

Il progetto è un gestionale multiutente/multispazio con Telegram come frontend corrente. Oggetti, luoghi, contenitori, foto e storico sono operativi; Alimentazione dispone di catalogo globale, categorie, ownership/condivisione, permessi generici, compatibilità alimentare, prodotti commerciali, nutrizione e audit. Le Ricette hanno fondazioni DB ma non ancora la UI Telegram completa.

Lo Step 7.2E aggiunge due fondazioni trasversali: **accesso Telegram approvato dal database** e **backlog `💡 Miglioramenti` con screenshot**. `ALLOWED_CHAT_IDS` resta solo bootstrap/emergenza. Un nuovo account richiede accesso; solo l'amministratore principale approva/rifiuta; l'approvazione crea un utente normale e uno spazio personale senza concedere accesso alle risorse altrui.

La strategia di sviluppo attuale è completare prima la macro-struttura e usare `💡 Miglioramenti` per raccogliere dettagli UX emersi durante gli smoke test. Le priorità tecniche restano: backend fail-closed, separazione ownership/visibilità/permessi, niente ID tecnici in UI, migration già applicate immutabili, Telegram come frontend e non come dominio applicativo, aggiornamento di questo handoff dopo ogni checkpoint strutturale.
