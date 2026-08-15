# Foto degli oggetti — Step 5B

## Scopo

Lo Step 5B collega immagini reali agli oggetti generici senza dipendere soltanto
dalla conservazione dei file su Telegram.

Flusso previsto:

```text
Telegram -> download backend -> data/media/oggetti/<item_id>/
         -> tabella core foto -> visualizzazione Telegram
```

## Schema dati

Non viene aggiunta una migration. Viene riusata la tabella core `foto` già
presente dallo Step 2:

- `item_id`: oggetto a cui appartiene l'immagine;
- `percorso_file`: percorso locale relativo al repository;
- `ruolo`: `principale` per la prima foto, `galleria` per le successive;
- `descrizione`: didascalia Telegram, quando presente;
- `caricato_il`: timestamp gestito da SQLite.

Il file Telegram non è la copia primaria del gestionale: il backend scarica una
copia locale. Questo permette di includere le immagini nei backup e facilita una
futura migrazione dall'S9 a un altro host.

## Percorso dei file

Per gli oggetti:

```text
data/media/oggetti/<item_id>/telegram_<message_id>.<estensione>
```

`data/` è esclusa da Git. `scripts/backup.sh` include già `data/media`, quindi le
foto entrano nel backup operativo insieme al database SQLite.

## Interfaccia Telegram

Dalla scheda di un oggetto:

```text
[ 📷 Foto ]
```

apre un menu con:

```text
[ ➕ Aggiungi foto ]
[ 🖼 Vedi foto (N) ]   # mostrato quando N > 0
[ ⬅️ Torna all'oggetto ]
[ 🏠 Menu principale ]
```

Comandi equivalenti:

- `/foto <id>`: apre il menu foto dell'oggetto;
- `/foto_aggiungi <id>`: mette la chat in attesa della prossima foto;
- `/annulla`: annulla l'attesa foto quando quel flusso è attivo.

Quando il bot aspetta una foto, una didascalia facoltativa viene salvata come
`descrizione`.

## Regole di salvataggio

1. viene selezionata la dimensione disponibile con area maggiore;
2. Telegram fornisce il percorso temporaneo tramite `get_file`;
3. il backend scarica l'immagine con Teloxide nel filesystem locale;
4. se è la prima foto dell'oggetto, `ruolo = principale`;
5. altrimenti `ruolo = galleria`;
6. solo dopo il download viene inserita la riga SQLite;
7. se l'INSERT fallisce, il file appena scaricato viene rimosso per evitare
   file orfani.

## Navigazione di sistema aggiunta nello stesso step

Per ridurre l'uso manuale di `/start`:

- quando il backend si avvia e `get_me` ha avuto successo, invia alle chat
  autorizzate `🟢 Gestionale Casa è online` con il menu principale;
- `/status` e il pulsante Stato sistema mostrano sempre `🏠 Menu principale`.

## Test automatici predisposti

- sanitizzazione/normalizzazione dell'estensione del file;
- prima foto `principale` e seconda `galleria` su SQLite in memoria.

## Test runtime richiesti prima della chiusura

- ricezione della notifica online all'avvio;
- ritorno al menu da `/status`;
- caricamento di almeno due foto sullo stesso oggetto;
- verifica dei file reali in `data/media/oggetti/<id>/`;
- visualizzazione delle foto dal bot;
- persistenza dopo riavvio;
- backup che contenga anche la directory media;
- `fmt`, `check`, `test`, `clippy` e CI GitHub Actions verdi.

## Fuori perimetro Step 5B

- cambio manuale della foto principale;
- cancellazione o riordinamento foto;
- documenti PDF/scontrini trattati come file generici;
- modifica ed eliminazione degli oggetti già salvati (Step 5C);
- luoghi multi-casa/stanze (Step 6, architettura ancora da confermare).
