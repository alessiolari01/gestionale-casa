# Database e migration Step 7

## Stato

**IN SVILUPPO.** Il checkpoint 7.0 è stato chiuso con `135dd33`; il primo pacchetto tecnico 7.1 introduce la migration `20260823153000_fondazioni_condivise.sql`.

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

I vincoli `UNIQUE` globali legacy di `abitazioni.nome` e `tag.nome` non vengono ricostruiti in questa prima migration. Non viene ancora esposta la creazione di spazi multipli in UI, quindi il limite non è visibile all'utente. La loro conversione a unicità per-spazio avverrà insieme allo scoping completo delle query, evitando un rebuild anticipato di tabelle già referenziate da luoghi/contenitori/storico.

### Bootstrap runtime

La migration non inventa utenti o Telegram ID. Alla prima interazione di una chat presente in `ALLOWED_CHAT_IDS`, `src/identity.rs` crea/aggiorna l'utente interno e l'account Telegram e lo aggiunge allo spazio bootstrap.

Durante lo sviluppo:

- primo account → `proprietario`;
- account autorizzati successivi → `amministratore`.

Questa è una regola di bootstrap transitoria, non il flusso definitivo di invito famiglia.
