# Alimenti e unità

**Operativo.** `src/modules/alimentazione.rs`, `🍽️ Alimentazione → 🥕 Alimenti`.

## Alimento

Entità culinaria riutilizzabile nelle ricette. È distinta da prodotto commerciale, formato acquistabile e scorta fisica.

Il catalogo base è globale; gli utenti possono possedere alimenti personali e renderli visibili nei propri spazi senza trasferirne la proprietà.

## Proprietà, visibilità e permessi

- il creatore resta proprietario dell'alimento;
- la visibilità in uno spazio non concede automaticamente modifica;
- i collaboratori usano `inviti_risorsa` / `permessi_risorsa`;
- perdere una membership non elimina l'alimento posseduto;
- il catalogo base è gestibile soltanto dagli admin secondo le regole applicative.

## Categorie

`categorie_alimento` + `alimento_categorie` permettono più categorie per alimento. Il filtro multi-categoria usa semantica **OR**.

La categoria viene scelta durante la creazione; `Altro` rimane un fallback di schema, non un passaggio obbligatorio del wizard.

## Elenco, ricerca e filtri sul bot

Le tre liste del modulo — elenco, risultati di ricerca, risultati del filtro —
seguono C1, C6 e C7 di `docs/convenzioni-telegram.md`. Il messaggio **non
elenca gli alimenti**: stanno sui pulsanti. Nel testo restano solo le cose che
un pulsante non può dire:

- l'**ordinamento**, che non è alfabetico — prima i propri alimenti, poi i
  condivisi, poi il catalogo base — e che quindi C6 impone di dichiarare;
- la **legenda** `👤 tuo · 👥 condiviso`, mostrata solo quando in pagina c'è
  almeno un marcatore da spiegare. Il catalogo base non ne porta;
- nella ricerca, **perché** un alimento è fra i risultati quando il suo nome
  non contiene la parola cercata: è finito lì per un prodotto commerciale
  collegato, e senza quella riga sembra un errore del bot.

Con 422 alimenti l'elenco è lungo 85 pagine, quindi nel menu `🥕 Alimenti` la
prima azione è `🔎 Cerca` e la seconda `📋 Elenco alimenti · 422`, con il
conteggio sull'etichetta (C7). Sotto le 20 voci l'ordine si inverte: con poche
voci scorrere è più veloce che scrivere.

## Alias e ricerca

Gli alias normalizzati evitano duplicati e supportano ricerca umana. La ricerca Alimenti considera anche marca/nome dei prodotti commerciali collegati: cercare `Philadelphia` può restituire l'alimento generico collegato, non una riga prodotto separata nella lista alimenti.

## Unità

Quantità = valore + unità strutturata.

Famiglie convertibili:

- massa: `g`, `kg`;
- volume: `ml`, `l`.

Unità discrete:

- pezzi;
- cucchiaino/cucchiaio;
- altre unità non convertibili universalmente.

La UI usa etichette descrittive, ad esempio:

```text
⚖️ Unità predefinita: grammi (g)
```

Non vengono inventate conversioni massa↔volume senza dati specifici dell'alimento.

## Prodotti e formati

Un alimento può avere più prodotti commerciali; ogni prodotto può avere più formati acquistabili. Il prodotto conserva identità commerciale/nutrizione, il formato conserva quantità/unità/barcode.

I formati sono modificabili ed eliminabili senza cambiare l'identità dell'alimento o del prodotto.

## Storico

Creazione/modifica dei prodotti commerciali, marca/nome, nutrizione e altre operazioni rilevanti vengono registrate nello storico trasversale con nomi umani, non ID tecnici.

## Futuro collegamento alle scorte

```text
ALIMENTO: Pollo
SCORTA: 800 g disponibili
RICETTA: 300 g richiesti
PIANO: martedì Pollo
SPESA: quantità mancante
```

La dispensa resta RIMANDATA e non blocca lo Step 7 corrente.
