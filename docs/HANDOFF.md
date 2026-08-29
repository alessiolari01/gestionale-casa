# Handoff operativo corrente — 29/08/2026

## 1. Punto di ripartenza

Repository: `alessiolari01/gestionale-casa`
Branch: `step-7-alimentazione`
Baseline Git precedente alla finalizzazione: `34d076c` — `Step 7.2G.1-7.2G.6: completa miglioramenti e UI Telegram`.

Il blocco locale da consolidare è **7.2H.0→7.2H.4F**. Non resettare il working tree e non ricostruire le funzionalità già presenti.

## 2. Cosa è stato completato

### Profili alimentari

- entità persona alimentare separata dall'account Telegram;
- collegamento account opzionale;
- profilo privato di default;
- condivisione tramite uno o più Spazi compatibili;
- creazione, dettaglio, modifica, archiviazione e storico;
- nessuna visibilità globale per i Profili.

### Spazi, membri e inviti

- lo Spazio è un contesto collaborativo, non una casa;
- gestione membri esplicita;
- nessuna membership automatica soltanto perché un account è autorizzato al bot;
- inviti privati via deep-link Telegram;
- ruolo, monouso/riutilizzabile, limite utilizzi e scadenza;
- calendario con date passate non selezionabili;
- orari rapidi e input manuale `HH:MM`;
- modifica inviti attivi, revoca, cambio ruolo e rimozione membro;
- notifiche contestuali e navigazione verso lo Spazio.

### Miglioramenti e UX Telegram

- gli elementi implementati restano `Fatto · da verificare` finché non vengono collaudati;
- quando l'utente conferma il collaudo, vanno direttamente in `📦 Archiviato`;
- verifiche guidate con pulsante verso la sezione pertinente;
- input inatteso fuori dai wizard non distrugge la schermata;
- al terzo input inatteso consecutivo appare anche il suggerimento `/start`;
- navigazione valida/comando/wizard azzera il contatore.

### Export

`💡 Miglioramenti` espone:

```text
📦 Esporta miglioramenti
📦 Esporta progetto
```

`📦 Esporta progetto` produce uno ZIP per una nuova chat/sviluppatore con codice, migration, documentazione, script e manifest, escludendo dati sensibili/runtime.

Ogni export ricrea da zero:

```text
_project_handoff/
├── CURRENT_STATE.md
├── PROJECT_OVERVIEW.md
├── GIT_MANIFEST.json
├── FILE_MANIFEST.json
├── TREE.txt
└── SENSITIVE_EXCLUSIONS.md
```

## 3. Migration 7.2H

```text
20260827190000_profili_alimentari_fondazioni.sql
20260828074000_catalogo_gallette.sql
20260828101500_inviti_spazi_operativi.sql
20260828202500_h4c_inviti_verifica_guidata.sql
20260829002500_h4d_rifiniture_finali.sql
20260829005000_h4e_input_export_progetto.sql
```

Totale file migration nel repository: **36**.

Regola: una migration già applicata al DB reale è immutabile. Correzioni successive richiedono una nuova migration append-only.

## 4. File chiave nuovi/centrali

```text
src/modules/profili_alimentari.rs
src/modules/spazi_membri.rs
scripts/export_progetto.py
src/context_bot.rs
src/modules/miglioramenti.rs
src/modules/alimentazione.rs
src/main.rs
```

## 5. Collaudi

Confermati manualmente dall'utente:

- flusso Profili H.2;
- gestione Spazi/Membri di base;
- inviti privati almeno in un flusso reale con secondo account durante H.4A;
- rifiniture H.4B non dipendenti dal secondo account;
- tre input inattesi consecutivi con suggerimento `/start`;
- `📦 Esporta progetto` e contenuto dell'handoff tecnico.

Verifiche **differite**, non da dichiarare eseguite finché non viene riutilizzato il secondo account:

1. secondo account accetta un nuovo invito e `👥 Apri spazio` apre lo spazio appena condiviso;
2. il creatore riceve notifica di accettazione leggibile e senza ID tecnici;
3. il secondo account riceve la notifica di cambio ruolo;
4. il secondo account riceve la notifica di rimozione e lo spazio non è più accessibile.

Queste verifiche non bloccano il prossimo sviluppo.

## 6. Prossimo step funzionale

**Porzioni e override**.

Ordine previsto:

1. porzioni personali per Profilo;
2. override quantità/esclusione ingrediente per Profilo;
3. turni/routine;
4. planner pasti versionato;
5. lista della spesa aggregata;
6. reminder/export e integrazioni residue.

Planner: deve conservare la versione/snapshot della ricetta applicata. `🔄 Aggiorna planner` compare solo se una ricetta usata da pasti non completati è cambiata. I pasti completati restano congelati. L'aggiornamento della lista della spesa resta separato.

## 7. Regole operative per il prossimo sviluppatore/chat

- preferire ZIP di aggiornamento al posto di molte modifiche manuali;
- PC → S9 via `scp`, poi un blocco Termux unico;
- nei blocchi S9 non usare `set -e`: usare `|| return 1` per fermarsi senza chiudere SSH;
- pipeline standard: fmt → check → diff-check → Clippy → test → backup/DB se migration → `cargo run`;
- prima di una migration reale: backup SQLite, `integrity_check`, `foreign_key_check` e prova su copia;
- non fare commit/push se la pipeline fallisce;
- Telegram: massimo 5 elementi/pagina, nessun ID tecnico, accenti italiani corretti, navigazione standard `⬅️ Indietro | 🏠 Menù principale | 💡 Migliora` quando applicabile;
- ogni checklist Telegram deve indicare il percorso completo dal `🏠 Menù principale`.

## 8. File da leggere

1. `README.md`;
2. `docs/HANDOFF.md`;
3. `docs/HANDOFF_COMPLETO.md`;
4. `docs/step7/roadmap.md`;
5. `ARCHITETTURA.md`;
6. `docs/step7/modello-condivisione.md`;
7. `docs/moduli/alimentazione/README.md`;
8. `docs/moduli/miglioramenti.md`;
9. `migrations/README.md`.
