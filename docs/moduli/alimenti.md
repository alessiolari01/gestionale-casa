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
