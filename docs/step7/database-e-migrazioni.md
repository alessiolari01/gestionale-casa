# Database e migration Step 7

## Stato

**PREVISTO.** Il checkpoint 7.0 non introduce migration.

Base Git: `219caba`.

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
