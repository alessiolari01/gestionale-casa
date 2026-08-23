# Roadmap interna Step 7

## 7.0 — Specifica e organizzazione

**Stato: IN SVILUPPO**

Obiettivi:

- consolidare tutte le decisioni emerse prima di toccare lo schema;
- rendere il `README.md` centrale sintetico e spostare i dettagli nei moduli;
- definire i confini fra Alimentazione, Acquisti, Viaggi e Spese;
- definire utenti, spazi, condivisione/copia e audit;
- fissare la politica delle migration e del DB di sviluppo.

Questo checkpoint è **docs-only**: nessuna nuova migration e nessuna modifica
funzionale al bot.

### Uscita da 7.0

Prima di iniziare 7.1 devono essere chiari:

- entità che appartengono allo spazio;
- strategia per associare i dati Step 6 esistenti allo spazio bootstrap;
- identità dell'autore negli eventi storici;
- modello account interno vs Telegram/Google;
- comportamento di condivisione, copia e provenienza;
- strategia reminder Telegram/email;
- compatibilità con il DB di prova esistente.

## 7.1 — Fondazioni condivise

**Stato: PREVISTO**

Perimetro previsto:

- `utenti` interni;
- account Telegram collegati agli utenti;
- `spazi` personali/familiari/condivisi;
- membership e ruoli;
- inviti con token sicuri, scadenza e revoca;
- associazione delle principali entità esistenti a uno spazio;
- regole di autorizzazione centralizzate;
- condivisione/copia per i modelli che la supportano;
- attribuzione dell'autore nello storico;
- origine dell'azione: utente, sistema, integrazione/automazione;
- infrastruttura reminder riusabile.

La prima migration reale dello Step 7 nasce qui, non nel checkpoint 7.0.

### Verifiche minime 7.1

- migration applicabile su DB vuoto;
- migration applicabile su copia del DB di test Step 6C;
- dati preesistenti conservati e assegnati allo spazio bootstrap;
- foreign key e `PRAGMA integrity_check` puliti;
- nessun evento storico retroattivo inventato;
- azioni condivise attribuite all'autore corretto;
- effetti automatici distinguibili dall'azione principale;
- autorizzazioni testate per ruoli differenti;
- test Rust/Clippy e runtime Telegram sul Galaxy S9.

## 7.2 — Alimentazione completa

**Stato: PREVISTO**

Comprende:

- catalogo alimenti;
- unità di misura strutturate;
- alimenti globali e personalizzati dello spazio;
- ricette strutturate, categorie, tag, foto e procedimento;
- profili alimentari separati dagli account;
- quantità/porzioni personalizzate per persona;
- turni e routine giornaliere;
- orari e luogo/situazione dei pasti;
- preparazione anticipata;
- planner su intervalli reali di date;
- partecipanti ai pasti;
- lista della spesa aggregata;
- reminder Telegram/email;
- export PDF/immagine.

Il modulo [Alimentazione](../moduli/alimentazione/README.md) è la fonte di
dettaglio funzionale.

## 7.3 — Integrazioni e condivisione operativa

**Stato: PREVISTO**

Comprende:

- flusso completo di invito/accettazione utenti;
- condivisione operativa di ricette, routine e altri modelli;
- invio di copie indipendenti quando appropriato;
- Google Calendar;
- inviti a pasti/cene;
- email;
- account Google dedicato al gestionale nella prima integrazione;
- predisposizione per account Google personali futuri;
- impostazioni per calendario, inviti e canale reminder.

Le regole OAuth e le policy Google devono essere riverificate sulle fonti
ufficiali nel momento dell'implementazione, perché possono cambiare nel tempo.

## Dopo Step 7

L'ordine esatto non è ancora bloccato. Sono già specificati:

- **Acquisti e prezzi** — prodotti, confezioni, prezzi base e confronto
  temporaneo con volantini;
- **Viaggi** — bagagli, checklist, oggetti in viaggio e controllo rientro;
- **Spese** — spese personali/condivise, quote, saldi e rimborsi.

Restano inoltre approvate funzioni già presenti nella roadmap storica:

documenti/garanzie, ricerca globale, manutenzioni, prestiti, QR/codici,
archiviazione, registro acquisti, dashboard/statistiche, Veicoli e Vestiti.
