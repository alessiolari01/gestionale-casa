# Roadmap funzionale

## Stato corrente — chiusura Step 7.2G.1→7.2G.6

**Branch:** `step-7-alimentazione`
**Baseline committata precedente:** `54dc4dd`
**Stato runtime:** verificato sul Galaxy S9 il 27 agosto 2026.

Completato nel blocco finale:

- workflow Miglioramenti verificabile e archivio manuale;
- UI Telegram a schermata singola;
- `💡 Migliora` contestuale con sezione/azioni reali;
- persistenza UI online/offline tra riavvii;
- spegnimento controllato da Amministrazione;
- rifiniture Alimentazione/Ricette emerse dagli smoke test;
- export ZIP dei Miglioramenti direttamente dal bot;
- **153/153 test** finali, check e Clippy verdi.

Restano due elementi intenzionalmente non chiusi:

1. **#7 — gestione account:** eliminazione/reset/revoca permessi da progettare come step separato perché coinvolge ownership, membership, storico e risorse;
2. **#9 — Zona test/aggiornamenti quasi zero-downtime:** funzione infrastrutturale futura, da affrontare soltanto quando il gestionale sarà funzionalmente maturo.

## Step 7 — Fondazioni condivise e Alimentazione

### 7.0 — Specifica e organizzazione

**VERIFICATO.** Requisiti, confini di dominio, politica migration e struttura documentale.

### 7.1 — Fondazioni condivise

**OPERATIVE, con evoluzioni amministrative ancora possibili.**

Già presenti:

- utenti interni separati dagli account Telegram;
- spazi, membership, spazio predefinito e vista multi-spazio;
- ruoli di sistema e ruoli nello spazio;
- proprietà separata da posizione/visibilità;
- permessi riutilizzabili sulle risorse;
- audit con autore/origine;
- accesso Telegram tramite richiesta approvata dall'amministratore principale.

La gestione distruttiva/reset degli account (#7) non è parte della chiusura 7.2G e richiederà regole dedicate.

### 7.2 — Alimentazione completa

**IN SVILUPPO.**

#### Completato

- Alimenti e unità;
- catalogo base e categorie;
- proprietà, visibilità e permessi;
- prodotti commerciali, formati acquistabili e nutrizione;
- Ricette operative con ingredienti strutturati;
- prodotto commerciale opzionale nell'ingrediente, senza legare la ricetta al formato;
- procedimento strutturato con foto/video e modalità guidata;
- ricerca per nome/categoria/ingredienti e filtro categoria negli ingredienti;
- rifiniture UI raccolte tramite `💡 Migliora`.

#### Prossima sequenza funzionale

1. **Profili alimentari** indipendenti dall'account Telegram;
2. **porzioni personali** e override per ingrediente;
3. **turni/routine**;
4. **planner pasti** su date reali e partecipanti;
5. **lista della spesa** aggregata e scelta dei formati;
6. reminder/export Alimentazione e integrazioni residue.

### 7.3 — Integrazioni e condivisione operativa

**PREVISTO.**

- gestione membri/inviti più completa dove ancora necessaria;
- Google Calendar;
- email;
- inviti ai pasti;
- impostazioni calendario/reminder;
- account Google dedicato e possibile supporto futuro ad account personali.

## Moduli futuri già specificati

### Acquisti e prezzi — RIMANDATO

Prodotti/confezioni, prezzi base, negozi e confronto volantini. Deve riusare i formati commerciali già presenti in Alimentazione.

### Viaggi — RIMANDATO

Bagagli, checklist, oggetti temporaneamente in viaggio, verifica partenza/rientro e collegamento alle spese.

### Spese — RIMANDATO

Spese personali/condivise, quote, saldi, rimborsi e collegamenti con acquisti/viaggi/spazi.

## Evoluzione infrastrutturale finale — `🧪 Zona test`

**FUTURO / ULTIMA FASE, non prioritaria nell'uso personale corrente.**

Obiettivo:

```text
versione stabile attiva per tutti
→ preparazione/compilazione candidata
→ pipeline automatica verde
→ admin entra in 🧪 Zona test
→ test su candidata e DB/snapshot separato
→ ✅ Conferma versione
→ 🚀 Installa e riavvia
→ backup
→ breve shutdown
→ migration
→ nuova stabile per tutti
→ rollback automatico se i controlli falliscono
```

Vincoli già decisi:

- un solo ingresso Telegram; niente due long-poller concorrenti con lo stesso token;
- candidata visibile soltanto all'admin finché non viene promossa;
- dati di test isolati dalla produzione;
- migration reali applicate soltanto nel passaggio finale, salvo migration esplicitamente compatibili;
- strategia `expand → migrate → contract` per evoluzioni strutturali importanti;
- versione stabile precedente conservata per rollback.

## Principi trasversali

- migration reali append-only e immutabili;
- sicurezza backend fail-closed;
- nessun ID tecnico nella UI utente;
- proprietà, visibilità, membership e permessi separati;
- Telegram è frontend, non dominio;
- liste operative paginate a 5 elementi dove applicabile;
- modifiche runtime verificate sull'S9 prima della chiusura di uno step;
- ogni checkpoint importante aggiorna README, roadmap, architettura, handoff e changelog.
