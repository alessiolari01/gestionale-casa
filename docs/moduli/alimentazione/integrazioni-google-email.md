# Google Calendar ed email

**Stato: PREVISTO — Step 7.3.**

## Strategia iniziale

La prima integrazione può usare un account Google definitivo dedicato al
gestionale, con un calendario come `Gestionale Casa - Pasti`.

Usi previsti:

- creare eventi pasto;
- creare eventi/preparazioni quando configurato;
- invitare partecipanti;
- inviare email/export.

## Evoluzione

In futuro ogni utente potrà collegare il proprio account Google.

Scelta account prevista:

- account del gestionale;
- account personale;
- chiedi ogni volta.

## Calendario

Scelta prevista:

- calendario dedicato `Gestionale Casa - Pasti`;
- calendario personale principale;
- chiedi ogni volta.

Default progettuale consigliato: calendario dedicato.

## Inviti a cena/pasto

Comportamento configurabile:

- mai;
- chiedi ogni volta;
- automatico quando ci sono altri partecipanti.

Default progettuale: **chiedi ogni volta**.

## Turni e preparazioni

L'utente può decidere separatamente se esportare:

- pasti;
- preparazioni;
- turni/routine.

## Sicurezza

Client secret e token non vanno versionati. Eventuali refresh token persistenti
devono essere protetti e la chiave/secret di protezione deve restare fuori dal
DB/repository secondo la strategia che verrà scelta in implementazione.

Le regole OAuth/Google cambiano nel tempo: prima di implementare vanno
riverificate sulle fonti ufficiali correnti.
