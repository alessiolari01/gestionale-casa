# Roadmap funzionale

## Stato corrente

Gli Step 1→6C sono chiusi e confluiti in `main` con il merge `219caba`.

Branch corrente: `step-7-alimentazione`.

Lo Step 7 è stato ridefinito: la precedente sequenza documenti/garanzie,
promemoria, tag/ricerca globale non è più il prossimo sviluppo. Quelle funzioni
restano approvate ma vengono riposizionate dopo la nuova fondazione condivisa.

## Step 7 — Fondazioni condivise e Alimentazione

### 7.0 — Specifica e organizzazione

**Stato: VERIFICATO.**

Checkpoint documentale chiuso con `135dd33`, senza migration/codice funzionale.

- consolidamento requisiti;
- utenti/spazi/condivisione;
- storico con autore;
- confini Alimentazione/Acquisti/Viaggi/Spese;
- politica migration e DB di sviluppo;
- README centrale con rimandi ai documenti di modulo.

### 7.1 — Fondazioni condivise

**Stato: IN SVILUPPO.**

Prima implementazione tecnica già predisposta: migration fondazioni condivise, identità Telegram→utente interno, spazio bootstrap, `/profilo` e audit con autore/origine. Restano scoping completo delle query, permessi operativi, condivisione/copia e reminder trasversali.

- utenti interni;
- account Telegram;
- spazi personali/familiari/condivisi;
- membership e ruoli;
- inviti sicuri;
- dati esistenti associati a uno spazio;
- autorizzazioni;
- condivisione vs copia;
- audit con autore e origine dell'azione;
- reminder trasversali Telegram/email.

### 7.2 — Alimentazione completa

**Stato: PREVISTO.**

- alimenti globali e personalizzati;
- unità strutturate;
- ricette con ingredienti e procedimento;
- categorie/tag/foto;
- profili alimentari;
- porzioni e override per persona;
- turni/routine con nome personalizzato;
- pasti a casa/lavoro/fuori/saltati;
- preparazione anticipata;
- reminder configurabili;
- planner su date/intervalli reali;
- partecipanti;
- lista della spesa aggregata;
- export PDF/immagine.

Specifica: `docs/moduli/alimentazione/README.md`.

### 7.3 — Condivisione operativa e integrazioni

**Stato: PREVISTO.**

- inviti e gestione membri completi;
- condivisione/copia dei modelli supportati;
- Google Calendar;
- inviti ai pasti;
- email;
- account Google dedicato al gestionale;
- futuri account Google personali;
- impostazioni calendario/inviti/reminder.

## Moduli già specificati per il futuro

### Acquisti e prezzi

**Stato: RIMANDATO.**

- prodotto e confezione separati da alimento/oggetto;
- prezzi normali/base modificabili;
- prezzo confezione + prezzo normalizzato;
- negozi/punti vendita quando necessario;
- volantino usato come confronto temporaneo, senza sostituire il prezzo base;
- monitoraggio solo per prodotti/oggetti per cui ha senso;
- integrazione futura con lista della spesa e spese reali.

Specifica: `docs/moduli/acquisti/README.md`.

### Viaggi

**Stato: RIMANDATO.**

- viaggio e partecipanti;
- valigia/bagaglio scelto fra gli oggetti o creato al momento;
- checklist generica modificabile;
- modelli checklist;
- quantità dinamiche + extra opzionale;
- più oggetti reali collegabili alla stessa voce;
- stato temporaneo `in viaggio` senza perdere la posizione abituale;
- verifica partenza/rientro;
- collegamento alle spese.

Specifica: `docs/moduli/viaggi/README.md`.

### Spese

**Stato: RIMANDATO.**

- personali e condivise;
- pagatore e partecipanti;
- ospiti senza account;
- divisione uguale/importi/percentuali/quote;
- saldi netti e rimborsi;
- collegamento a viaggio/acquisto/spazio;
- storico con autore.

Specifica: `docs/moduli/spese/README.md`.

## Funzioni storiche ancora approvate

L'ordine dopo Step 7 verrà deciso quando la nuova architettura sarà stabile.
Restano in progettazione:

- documenti e garanzie;
- ricerca globale;
- tag evoluti;
- manutenzioni e interventi;
- costi/valore;
- prestiti;
- QR code e codici a barre;
- archivio per elementi venduti/regalati/buttati/persi;
- registro acquisti;
- dashboard/statistiche;
- modulo Veicoli;
- modulo Vestiti.

## Principi trasversali

1. le funzioni comuni vanno riusate dai moduli;
2. nessuna cancellazione di un luogo deve cancellare automaticamente un bene;
3. lo storico deve restare interpretabile nel tempo;
4. in multiutente ogni modifica umana significativa deve avere autore;
5. condividere e copiare sono operazioni diverse;
6. il DB resta centrale;
7. niente reset generale nel bot;
8. le migration già applicate non si riscrivono;
9. prima del go-live il DB di sviluppo può essere azzerato manualmente dopo backup;
10. dopo il go-live le migration devono preservare i dati reali.

## Regola per le future decisioni

Una feature approvata viene prima documentata nel modulo o nella roadmap. Se
influenza lo schema o più domini, va descritta anche in `ARCHITETTURA.md` o in
`docs/step7/decisioni-architetturali.md` prima dell'implementazione.
