# Roadmap funzionale

## Stato corrente — Step 7.2H chiuso, finalizzazione documentale

**Branch:** `step-7-alimentazione`
**Baseline Git precedente:** `34d076c`
**Stato runtime:** Profili, Spazi/Membri/Inviti, input inattesi ed export progetto collaudati sul Galaxy S9 entro il 29 agosto 2026.

Completato in 7.2H:

- fondazioni e UI Profili alimentari;
- gestione membri degli Spazi;
- inviti privati via deep-link con ruoli, utilizzi e scadenze;
- rifiniture UX inviti/Miglioramenti;
- verifiche guidate e differimento esplicito dei test che richiedono un secondo account;
- input inatteso non distruttivo + suggerimento `/start` al terzo tentativo;
- `📦 Esporta progetto` sanitizzato e auto-documentante.

Prossimo blocco funzionale: **Porzioni e override**. La finalizzazione corrente aggiorna documentazione e GitHub senza aggiungere nuove funzionalità.
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

1. **porzioni personali** per Profilo e override per ingrediente;
2. **turni/routine**;
3. **planner pasti** su date reali e partecipanti, con snapshot/versione ricetta;
4. **lista della spesa** aggregata e scelta dei formati;
5. reminder/export Alimentazione e integrazioni residue.

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
