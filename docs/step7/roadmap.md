# Roadmap Step 7

## Stato corrente

Il blocco 7.2G.1→7.2G.6 è verificato sul Galaxy S9 con **153/153 test**. Prima di aprire il prossimo dominio funzionale va chiuso con documentazione, commit e push.

## Sequenza

### 7.0 — Specifica e organizzazione — VERIFICATO

Decisioni, confini dei moduli, modello utenti/spazi e politica migration.

### 7.1 — Fondazioni condivise — OPERATIVE

Utenti, account Telegram, spazi, membership, ruoli, vista multi-spazio, ownership, condivisione, permessi e audit.

Da non confondere con il backlog #7: eliminazione/reset account richiede ancora una progettazione amministrativa dedicata.

### 7.2 — Alimentazione — IN SVILUPPO

Completato:

- Alimenti/unità/categorie;
- catalogo base e compatibilità;
- prodotti commerciali/formati/nutrizione;
- Ricette operative e procedimento guidato;
- accesso approvato, amministrazione e Miglioramenti;
- rifiniture UI Telegram e export Miglioramenti.

Prossimi blocchi funzionali, nell'ordine:

1. **Profili alimentari** — persona alimentare non obbligatoriamente legata a un account;
2. **Porzioni e override** — quantità personali e override ingrediente;
3. **Turni/routine**;
4. **Planner pasti**;
5. **Lista della spesa** aggregata;
6. reminder/export Alimentazione.

### 7.3 — Integrazioni — PREVISTO

Google Calendar, email, inviti e completamento delle funzioni multiutente esterne.

## Dopo i domini funzionali

Rimangono già specificati Acquisti, Viaggi, Spese, documenti/garanzie, manutenzioni, prestiti, ricerca globale, QR/codici, Veicoli e Vestiti.

## Ultima evoluzione infrastrutturale futura

`🧪 Zona test` + aggiornamenti quasi zero-downtime (#9).

Non è prioritaria adesso. Quando verrà implementata dovrà consentire all'admin di testare una candidata separata mentre tutti gli altri restano sulla stabile, con database/snapshot di test, pipeline automatica, `✅ Conferma versione`, `🚀 Installa e riavvia`, backup e rollback. Un solo processo deve ricevere gli update Telegram per lo stesso token.
