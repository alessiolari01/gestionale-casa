# Step 7.2H — Profili alimentari, Spazi/Membri/Inviti e chiusura UX

**Stato: IMPLEMENTATO e collaudato per i flussi disponibili; verifiche multi-account residue differite.**

## Obiettivo

Preparare la base collaborativa necessaria a Porzioni, Planner e Lista della spesa senza confondere persone, account Telegram, Spazi di collaborazione e luoghi fisici.

## H.0 — fondazioni Profili

Migration `20260827190000_profili_alimentari_fondazioni.sql`.

- `profili_alimentari` separati da `utenti`;
- collegamento account opzionale;
- profilo privato di default;
- condivisione tramite `profilo_alimentare_spazi`;
- nessun profilo globale.

## H.1–H.2 — UI e gestione Profili

- accesso alla sezione Profili dall'area Alimentazione/Spazi prevista dalla UI corrente;
- creazione e dettaglio;
- modifica dati;
- condivisione negli Spazi consentiti;
- archiviazione;
- storico con descrizioni umane e senza ID tecnici.

## H.3 — membri Spazi e catalogo

- membership esplicita: autorizzazione al bot ≠ membro di uno Spazio;
- schermata Membri dello spazio;
- rifiniture Storico Profili e export Miglioramenti;
- migration `20260828074000_catalogo_gallette.sql` per gallette generiche/riso/mais.

## H.4A — inviti privati

Migration `20260828101500_inviti_spazi_operativi.sql`.

- deep-link Telegram;
- invito monouso o riutilizzabile;
- limite utilizzi;
- scadenza;
- ruolo assegnato;
- inviti attivi, revoca e modifica;
- accettazione esplicita;
- notifiche al creatore e al membro su eventi rilevanti;
- apertura da creatore/membro esistente senza consumo.

## H.4B–H.4D — rifiniture UX

- ritorni contestuali e schermata Spazi aggiornata;
- calendario con date passate marcate `❌` e non selezionabili;
- giorno/mese/orario coerenti;
- input manuale `HH:MM` e preset rapidi;
- limite utilizzi digitabile 1–9999;
- gestione errori recuperabile;
- Miglioramenti `Fatto · da verificare` con piani guidati;
- archiviazione ritorna alla lista dei fatti da verificare.

Le migration di supporto sono:

```text
20260828202500_h4c_inviti_verifica_guidata.sql
20260829002500_h4d_rifiniture_finali.sql
```

## H.4E–H.4F — input inatteso ed export progetto

Migration `20260829005000_h4e_input_export_progetto.sql` registra la verifica guidata delle funzioni H.4E.

- testo inatteso fuori dai wizard: schermata corrente preservata;
- al terzo tentativo consecutivo appare il suggerimento `/start`;
- una navigazione/comando valido azzera il contatore;
- nuovo `📦 Esporta progetto`;
- export con sorgenti, migration, docs, script e manifest;
- esclusione di segreti e runtime;
- `_project_handoff/` ricreato da zero a ogni export;
- `CURRENT_STATE.md` fotografa lo stato Git reale;
- manifest Git filtra `.pre_*` e file tecnici temporanei.

## Collaudi differiti con secondo account

Da eseguire quando disponibile:

1. `🏠 Menù principale → 👥 Spazi → [spazio] → 👥 Membri dello spazio → ➕ Invita membro` → accettazione sul secondo account → `👥 Apri spazio`;
2. verificare notifica di accettazione al creatore;
3. modificare ruolo e verificare la notifica sul secondo account;
4. rimuovere il membro, verificare notifica e perdita accesso allo spazio.

## Prossimo step

Porzioni per Profilo e override ingrediente. Il modello Profili/Spazi qui definito è la base da riusare, non da sostituire.
