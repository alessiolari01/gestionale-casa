# Handoff completo — Gestionale Casa

> Documento di continuità tecnica e funzionale destinato a chi deve proseguire il progetto senza conoscere le conversazioni precedenti.
>
> **Regola di manutenzione:** questo file va aggiornato dopo ogni checkpoint che modifica in modo significativo architettura, schema dati, sicurezza, flussi applicativi o interfaccia utente.

## 1. Stato del progetto al momento di questo handoff

Repository GitHub: `alessiolari01/gestionale-casa`
Branch di sviluppo corrente: `step-7-alimentazione`
Ultimo commit già consolidato prima delle modifiche D0.2/D0.3: `bedcfa6` — `Step 7.2D.0.1: aggiunge compatibilità alimentare`.

Sono attualmente verificati sul database reale anche gli Step **7.2D.0.2** e **7.2D.0.3**, che al momento della generazione di questo documento devono essere consolidati nel checkpoint successivo insieme a questo handoff.

Verifiche confermate sul Samsung Galaxy S9/Termux:

- `PRAGMA integrity_check` → `ok`;
- `PRAGMA foreign_key_check` → nessuna violazione;
- migration `20260825101500_prodotti_alimentari.sql` → applicata con successo;
- migration `20260825113000_prodotti_nutrizione_ricette.sql` → applicata con successo;
- catalogo base → **418 alimenti** attivi;
- `ricetta_ingredienti.prodotto_alimentare_id` presente;
- pipeline Rust e smoke test Telegram dello Step D0.3 dichiarati funzionanti dall'utente;
- totale test del checkpoint D0.3: **120 test** nel flusso di verifica previsto.

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
└── 👥 Utenti
```

`/admin`, `/status` e le callback amministrative sono protette anche lato backend.

Le notifiche backend `online/offline` sono riservate agli amministratori.

### Accesso al bot — requisito futuro già approvato

La whitelist Telegram statica non deve essere il modello definitivo.

Flusso previsto:

```text
account Telegram sconosciuto
        ↓
può contattare il bot
        ↓
📨 Richiedi accesso
        ↓
richiesta in attesa
        ↓
admin principale
        ↓
✅ Accetta / ❌ Rifiuta
        ↓
se accettato:
utente normale autorizzato
```

L'approvazione al bot **non concede automaticamente** accesso a spazi o risorse.

La whitelist configurata potrà rimanere come bootstrap/emergenza.

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

### Modifica UI già approvata ma non ancora applicata

Ridurre lo Storico a **massimo 5 eventi per pagina**, mantenendo:

- conteggio totale;
- `Pagina X/Y`;
- pagina precedente/successiva.

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

Lo Step D0.2 ha introdotto paginazione reale.

**Stato attuale implementato:** `FOOD_PAGE_SIZE = 10`.

Le schermate mostrano:

```text
📋 Alimenti · 418 risultati
Pagina 1/42
```

Ricerca:

```text
🔎 Risultati per: "ci" · N risultati
Pagina X/Y
```

I filtri mantengono contesto e pagina con `🔄 Aggiorna`.

### Rifiniture già approvate, non ancora implementate

1. Ridurre gli alimenti a **massimo 5 per pagina**.
2. Non mostrare `🌐` per gli alimenti base né nel testo né nei pulsanti.
3. Usare indicatori solo per le eccezioni, a fine riga:

```text
🥩 Alimento personale 👤
🍰 Alimento condiviso 👥
```

Legenda:

```text
👤 tuo · 👥 condiviso
```

4. Nel dettaglio si può continuare a mostrare `🌐 Catalogo base`.
5. Estendere la ricerca anche a **marca e nome commerciale dei prodotti reali**.
6. Se più prodotti dello stesso alimento corrispondono, non duplicare l'alimento; mostrare eventualmente i prodotti trovati come contesto.

---

## 19. Prodotti commerciali

Migration immutabile già applicata:

```text
20260825101500_prodotti_alimentari.sql
```

Il prodotto commerciale rappresenta ciò che si compra realmente, mentre l'alimento generico resta il concetto usato dalle Ricette.

Esempio:

```text
🥛 Formaggio spalmabile
├── Philadelphia · Original · 200 g
├── Philadelphia · Light · 175 g
└── Exquisa · Classico · 175 g
```

Una Ricetta generica continuerà a riferirsi a `Formaggio spalmabile`.

Questo livello è predisposto per il futuro:

```text
prodotto commerciale
→ punto vendita
→ prezzo
→ data rilevazione
→ €/kg o €/l
→ storico prezzi
→ prezzo attuale
→ dove conviene comprarlo
```

### Inserimento prodotto attuale

Flusso:

```text
Marca
 ↓
Nome commerciale
 ↓
Quantità confezione
 ↓
Unità confezione
 ↓
Salva
```

Nello Step D0.3 la schermata quantità mostra l'unità attuale e permette `📏 Cambia unità` nello stesso passaggio.

L'unità viene salvata sul prodotto e non dipende dinamicamente dal default dell'alimento.

### Modifiche prodotto già approvate, non ancora implementate

Dal dettaglio deve essere possibile:

```text
✏️ Modifica prodotto
├── 🏷 Marca
├── 🛒 Nome commerciale
├── quantità confezione
├── 📏 Unità confezione
└── Barcode / EAN
```

I valori nutrizionali restano una sezione separata.

Non cambiare automaticamente `alimento_id`; un eventuale cambio di alimento associato richiederà in futuro una funzione esplicita.

### Icone unità approvate per la UI futura

- `⚖️` → g / kg;
- `🥤` → ml / l;
- `🔢` → pz;
- `🥄` → cucchiaio / cucchiaino.

Esempi:

```text
⚖️ Confezione: 200 g
🥤 Confezione: 500 ml
🔢 Confezione: 6 pz
🥄 Quantità: 2 cucchiai
```

---

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

### Stato UI attuale

L'inserimento usa un messaggio con valori separati da `;`.

### Miglioria già approvata, non ancora implementata

Se l'utente fornisce meno di 9 valori ma almeno un valore valido:

```text
225; 934; -
```

completare automaticamente in coda:

```text
225; 934; -; -; -; -; -; -; -
```

Prima del salvataggio mostrare **sempre** un riepilogo completo e chiedere conferma:

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

# MODIFICHE APPROVATE DA FARE PRIMA/INSIEME AL PROSSIMO STEP

## 39. Rifiniture UI già decise

Queste modifiche sono approvate ma non ancora applicate nello stato rappresentato da questo documento:

### Alimentazione

- massimo **5 alimenti per pagina**;
- niente `🌐` per gli alimenti base nell'elenco e nei pulsanti;
- `👤` e `👥` solo per eccezioni, a fine riga;
- ricerca alimento anche tramite marca/nome di un prodotto commerciale;
- evitare duplicati quando più prodotti corrispondono.

### Storico

- massimo **5 eventi per pagina**;
- conteggio totale;
- `Pagina X/Y`.

### Prodotti commerciali

- icona dinamica per unità (`⚖️`, `🥤`, `🔢`, `🥄`);
- `✏️ Modifica prodotto`;
- modifica marca/nome/formato/unità/EAN;
- mantenere separata la modifica nutrizione.

### Valori nutrizionali

- input parziale completato automaticamente con `-`;
- riepilogo dei 9 valori;
- conferma esplicita prima del salvataggio.

---

# PROSSIMA DIREZIONE

## 40. Step immediatamente successivo consigliato

Prima di sviluppare tutta la UI Ricette, applicare una piccola rifinitura **D0.4** con le modifiche approvate nella sezione precedente.

Dopo D0.4 passare a:

```text
Step 7.2D.1 — Ricette operative Telegram
```

Prima tranche consigliata:

1. menu Ricette;
2. elenco paginato;
3. dettaglio ricetta;
4. wizard nuova ricetta;
5. selezione alimento dal catalogo;
6. se esistono prodotti: generico vs prodotto reale;
7. quantità/unità;
8. procedimento;
9. visibilità;
10. salvataggio fail-closed.

Successivamente:

- modifica ricetta;
- collaboratori;
- ricerca per nome;
- ricerca per ingredienti;
- compatibilità alimentare derivata;
- calcolo nutrizionale;
- prezzi/punti vendita.

---

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

Il progetto è ormai un gestionale multiutente/multispazio con Telegram come frontend corrente. Oggetti, luoghi, contenitori, foto e storico sono già operativi; Alimentazione dispone di un catalogo globale di 418 alimenti, categorie, ownership/condivisione, permessi generici, compatibilità alimentare, prodotti commerciali e nutrizione opzionale. Le Ricette hanno già fondazioni DB e funzioni di ricerca/compatibilità, ma manca ancora la UI Telegram completa.

Le priorità sono mantenere separati ownership, visibilità e permessi; non esporre ID tecnici; mantenere il backend fail-closed; non modificare migration già applicate; trattare alimento generico e prodotto commerciale come livelli distinti; e aggiornare questo handoff dopo ogni cambiamento strutturale importante.
