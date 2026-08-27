# Step 7 — Fondazioni condivise e Alimentazione

**Stato: IN SVILUPPO — fondazioni, Alimenti, Ricette e rifinitura UI operative; prossimo blocco Profili e porzioni.**

**Branch di lavoro:** `step-7-alimentazione`.

La baseline committata precedente al blocco finale Miglioramenti è `54dc4dd`. Il working tree 7.2G.1→7.2G.6 è stato verificato sull'S9 con **153/153 test** e deve essere committato insieme alla chiusura documentale.

## Macro-fasi

| Macro-fase | Stato | Contenuto |
|---|---|---|
| 7.0 — Specifica e organizzazione | VERIFICATO | decisioni, confini e piano migration |
| 7.1 — Fondazioni condivise | OPERATIVE | utenti, spazi, membership, ruoli, vista multi-spazio, audit, accesso DB-driven |
| 7.2 — Alimentazione completa | IN SVILUPPO | Alimenti e Ricette operative; profili/turni/planner/spesa da fare |
| 7.3 — Integrazioni | PREVISTO | Google Calendar, email, inviti/rifiniture multiutente |

## Fondazioni operative

- identità interna separata da Telegram;
- spazi personali/condivisi e membership;
- spazio predefinito + vista tutti i propri spazi;
- ruoli globali `utente/admin` separati dai ruoli nello spazio;
- proprietà delle risorse separata da visibilità e permessi;
- audit con autore/origine;
- accesso tramite richiesta approvata dall'amministratore principale;
- `ALLOWED_CHAT_IDS` soltanto bootstrap/emergenza.

## 7.2 — stato Alimentazione

### Completato

- Alimenti e unità;
- catalogo base, categorie e compatibilità alimentare;
- ownership/condivisione/permessi;
- prodotti commerciali, formati e nutrizione;
- Ricette con ingredienti strutturati;
- procedimento guidato con foto/video;
- ricerca nome/categoria/ingredienti;
- rifiniture UX raccolte e chiuse tramite Miglioramenti.

### Prossimo ordine

1. profili alimentari separati dagli account Telegram;
2. porzioni personali e override ingrediente;
3. turni/routine;
4. planner pasti;
5. lista della spesa;
6. reminder/export e integrazioni residue.

## Step 7.2G — Miglioramenti e UI Telegram

Chiuso funzionalmente con:

- workflow `da_approvare → da_fare → fatto → verificato → archivio`;
- paginazione e ritorno contestuale;
- descrizioni lunghe/multimessaggio;
- `💡 Migliora` globale con sezione e azioni recenti;
- UI a schermata singola;
- stato UI persistente tra riavvii;
- spegnimento amministrativo;
- export ZIP del backlog direttamente dal bot.

Restano fuori dalla chiusura:

- #7 gestione distruttiva/reset account;
- #9 Zona test / aggiornamenti quasi zero-downtime, funzione infrastrutturale futura.

## Documenti Step 7

- [Roadmap](roadmap.md)
- [Decisioni architetturali](decisioni-architetturali.md)
- [Modello di condivisione](modello-condivisione.md)
- [Storico e audit](storico-e-audit.md)
- [Database e migrazioni](database-e-migrazioni.md)
- [Alimentazione](../moduli/alimentazione/README.md)

## Regola di stato

Usare sempre una delle etichette: **PREVISTO**, **IN SVILUPPO**, **IMPLEMENTATO**, **VERIFICATO**, **RIMANDATO**. Le verifiche runtime devono riferirsi a prove realmente eseguite sull'S9.
