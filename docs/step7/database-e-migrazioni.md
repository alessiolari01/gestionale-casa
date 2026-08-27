# Database e migration Step 7

## Stato

**IN SVILUPPO.** Il checkpoint `a650bc8` ha verificato la migration
`20260823153000_fondazioni_condivise.sql`; il blocco corrente aggiunge
`20260823174500_spazi_operativi.sql` per attivare isolamento e unicità per
spazio.

Base tecnica del pacchetto: `135dd33`.

Lo schema applicativo corrente è quello prodotto dalle migration Step 2→6C.
Le migration già applicate non devono essere riscritte.

## Database di prova

È disponibile un database Step 6C contenente dati di prova reali del progetto.
Verrà usato come banco di prova delle migration 7.1 e 7.2.

Regola:

1. lavorare sempre su una copia;
2. eseguire backup prima dell'applicazione sul Galaxy S9;
3. verificare `PRAGMA integrity_check`;
4. verificare `PRAGMA foreign_key_check`;
5. verificare conteggi e relazioni significative prima/dopo;
6. avviare il backend e fare test Telegram.

## Politica dati di sviluppo

Il DB attuale non contiene ancora dati definitivi di produzione.

Prima del go-live è ammessa una sola procedura manuale controllata:

1. backup del DB di sviluppo;
2. rimozione manuale del DB di prova, fuori dal bot;
3. ricreazione da zero tramite tutte le migration ufficiali;
4. bootstrap del primo utente/spazio;
5. smoke test;
6. inizio inserimento dati reali.

Non va aggiunto un comando/pulsante di reset globale all'applicazione.

Dopo il go-live ogni migration deve preservare i dati reali.

## Principi migration 7.1

La prima migration Step 7 dovrà essere progettata per:

- introdurre utenti/spazi/membership senza perdere dati;
- assegnare i dati Step 6 esistenti a uno spazio bootstrap;
- permettere nomi di case/tag uguali in spazi diversi dove semanticamente
  corretto;
- preparare l'audit con autore;
- mantenere lo storico esistente interpretabile;
- non creare eventi retroattivi fittizi;
- evitare cross-space incoerenti;
- essere applicabile sia su DB vuoto sia sul DB di test corrente.

## Bootstrap

Durante lo sviluppo può esistere un utente/spazio bootstrap chiaramente
marcato come tecnico/test. Non vanno inventati Telegram ID, email o account
Google reali.

Il bootstrap definitivo del go-live sarà separato dalla migration di test.

## Vincoli e calcoli

Regole preferite:

- importi monetari persistiti come interi nella minima unità monetaria quando
  appropriato;
- quantità/rapporti progettati per evitare errori float non necessari;
- foreign key abilitate;
- operazioni multi-tabella in transazione;
- vincoli di spazio applicati sia dal dominio Rust sia dal DB dove possibile.

## Test richiesti prima del commit di una migration

- migration da zero;
- migration sul DB di prova;
- riavvio idempotente;
- integrity/foreign key check;
- test Rust;
- Clippy `-D warnings`;
- `git diff --check`;
- runtime su Galaxy S9 per gli step che cambiano il comportamento Telegram.

## Migration `20260823153000_fondazioni_condivise.sql`

Introduce:

- `utenti`;
- `spazi`;
- `membri_spazio`;
- `account_telegram`;
- `preferenze_utente`;
- `inviti_spazio`;
- `spazio_id` sulle principali entità radice già esistenti;
- audit autore/origine/spazio su `storico_eventi`;
- trigger di coerenza cross-space per i collegamenti Step 6 più sensibili.

### Scelta SQLite transitoria

`items`, `abitazioni`, `tag`, `storico_entita` e `storico_eventi` ricevono `spazio_id INTEGER NOT NULL DEFAULT 1`. Con `ALTER TABLE`, SQLite non permette in modo portabile di aggiungere nello stesso passaggio una colonna `REFERENCES NOT NULL` con default non nullo. Fino al futuro rebuild space-aware, l'esistenza dello spazio viene quindi protetta da trigger.

La prima migration lascia temporaneamente i vincoli `UNIQUE` globali legacy.
La migration `20260823174500_spazi_operativi.sql` ricostruisce `abitazioni` e
`tag` mantenendo gli ID e sostituisce tali vincoli con
`UNIQUE(spazio_id, nome)`. I trigger cross-space di `item_luogo` e `item_tag`
vengono ricreati nello stesso passaggio.

### Bootstrap runtime

La migration non inventa utenti o Telegram ID. Alla prima interazione di una chat presente in `ALLOWED_CHAT_IDS`, `src/identity.rs` crea/aggiorna l'utente interno e l'account Telegram e lo aggiunge allo spazio bootstrap.

Durante lo sviluppo:

- primo account → `proprietario` dello spazio bootstrap `#1`;
- account autorizzati successivi senza membership → nuovo spazio personale di cui diventano `proprietario`.

La condivisione fra utenti avverrà tramite membership/inviti espliciti, non inserendo automaticamente nuovi account nello spazio bootstrap.


## Migration `20260823174500_spazi_operativi.sql`

**Stato: IMPLEMENTATO, da verificare sull'S9 prima del commit.**

La migration:

- non crea né sposta dati fra spazi;
- mantiene i dati legacy nello spazio `#1`;
- ricostruisce `abitazioni` e `tag` preservando gli ID;
- rende i nomi unici nel singolo spazio;
- mantiene i vincoli che impediscono collegamenti item↔casa/tag cross-space;
- mantiene le foreign key attive durante il rebuild e ne rinvia il controllo al commit SQLx;
- mantiene coerente `preferenze_utente.spazio_attivo_id` quando viene rimossa una membership attiva;
- impedisce di aggirare tale coerenza modificando direttamente gli ID della chiave di `membri_spazio`.

Con SQLx `0.8.6` le migration SQLite vengono sempre eseguite dentro una
transazione del driver: in questa versione il flag `-- no-transaction` non è
ancora supportato dal backend SQLite. La migration resta quindi pienamente
transazionale e mantiene `foreign_keys` attivo. Per eliminare i vecchi vincoli
`UNIQUE(nome)` senza disabilitare le FK, salva temporaneamente e ricostruisce
anche le tabelle figlie di `abitazioni` e `tag`, reinserendo le righe con gli
stessi ID. `PRAGMA defer_foreign_keys = ON` rinvia la verifica dei vincoli al
commit della transazione SQLx.

Verifiche obbligatorie:

- migrazione da database Step 7.1 esistente;
- migrazione da database vuoto attraverso tutta la catena;
- `integrity_check = ok`;
- `foreign_key_check` vuoto;
- stessa casa/tag consentiti in spazi diversi e rifiutati nello stesso spazio;
- trigger cross-space ancora operativi;
- rimozione membership attiva → fallback a un altro spazio disponibile;
- rimozione ultima membership → preferenza attiva rimossa e ricreabile dal bootstrap identità.


## Migration `20260823200000_vista_multispazio_condivisione.sql`

**Stato: IN SVILUPPO — non applicare al DB reale prima di fmt/check/clippy/test.**

La migration è append-only rispetto alle 8 già applicate e non modifica retroattivamente `20260823174500_spazi_operativi.sql`. Introduce:

- `preferenze_utente.vista_spazi`, con valori `predefinito` / `tutti`;
- `item_condivisioni(item_id, spazio_id, permesso, ...)`;
- rimozione dei due trigger Step 7.1A che vietavano qualsiasi `item_luogo` cross-space.

La rimozione dei trigger non rende libere le relazioni: la sicurezza viene spostata sulla logica applicativa, che verifica membership e permessi dello spazio proprietario e della destinazione prima delle mutazioni. Le foreign key strutturali casa→stanza→contenitore restano attive.

Test richiesti prima del commit:

- catena completa delle 9 migration da zero;
- migration 9 sul DB di sviluppo con backup;
- `integrity_check = ok` e `foreign_key_check` vuoto;
- vista globale che include solo membership reali;
- vista singola che torna al solo spazio predefinito;
- oggetto personale spostabile in casa condivisa senza cambiare `items.spazio_id`;
- destinazione senza membership o in sola lettura rifiutata;
- ID appartenenti a spazi non accessibili ancora negati;
- storico attribuito allo spazio proprietario dell'entità.

## Migration `20260823232000_storico_spazi_luogo.sql`

**Stato: IN SVILUPPO — applicare solo dopo fmt/check/clippy/test.**

Aggiunge snapshot separati dello spazio della posizione allo storico:

- `storico_eventi.luogo_spazio_id` e `luogo_spazio_nome_snapshot`;
- `storico_cambi_luogo.spazio_prima_*` e `spazio_dopo_*`;
- backfill degli eventi esistenti usando `storico_entita` della casa, senza inventare cronologia.

Questo permette di distinguere correttamente, per esempio, un oggetto di `Spazio principale` spostato da `Casa principale · Spazio principale` a `Casa principale · Test isolamento`.

## Step 7.2G — Miglioramenti e runtime Telegram

Le seguenti migration sono state applicate sul database reale durante la rifinitura 7.2G e sono **immutabili**:

```text
20260826024500_miglioramenti_workflow_admin.sql
20260826123000_miglioramenti_verifica_guidata.sql
20260826223000_miglioramenti_contesto_rifiniture.sql
20260827003000_miglioramenti_ultimo_passaggio.sql
20260827014500_miglioramenti_finalissimi.sql
20260827104500_runtime_ui_persistente.sql
20260827123000_esporta_miglioramenti_bot.sql
```

Evoluzione principale:

- workflow amministrativo e archivio;
- stato `fatto` nuovamente attivo come “implementato da verificare”;
- dati/piani/allegati di verifica;
- persistenza UI Telegram in `telegram_ui_state`;
- stato del miglioramento Export #8 aggiornato dalla migration 7.2G.6 quando compatibile con i dati presenti.

L'export ZIP è una funzione applicativa in sola lettura e non richiede una tabella runtime dedicata: i file temporanei restano sotto `data/tmp/miglioramenti_export/` e non sono parte del database.

La suite finale dopo 7.2G.6 è **153/153 test**.
