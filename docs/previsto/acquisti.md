# Acquisti — previsto

> Specifica di un modulo **non ancora costruito**. Nessuna riga di codice, nessuna tabella.
> La gerarchia alimento → prodotto → formato descritta qui e' invece gia' realizzata:
> vedi `docs/moduli/alimenti.md`. Qui resta futuro il monitoraggio dei prezzi e i
> prodotti non alimentari.

---


**Stato: RIMANDATO — specificato ora, implementazione successiva allo Step 7.**

Modulo trasversale per prodotti acquistabili, confezioni, negozi e prezzi base.
Serve sia all'Alimentazione sia a beni/consumabili acquistati frequentemente.

## Principi

- alimento ≠ prodotto acquistabile;
- prodotto acquistabile ≠ singolo oggetto posseduto;
- monitoraggio prezzi solo dove ha senso;
- prezzo normale/base modificabile;
- volantino usato come confronto temporaneo;
- mostrare prezzo confezione + prezzo normalizzato quando possibile;
- modifica prezzi attribuita all'autore nello storico.

## Documentazione

- [Prodotti e confezioni](prodotti-e-confezioni.md)
- [Negozi](negozi.md)
- [Prezzi e volantini](prezzi-e-volantini.md)

---


**Stato: RIMANDATO.**

Il prezzo base non deve essere una proprietà assoluta del prodotto: appartiene
a un prodotto/confezione **in un determinato contesto di vendita**.

## Struttura proposta

Quando utile distinguere:

- catena/insegna, per esempio `Lidl`;
- punto vendita specifico, se i prezzi cambiano localmente;
- eventuale area/ambito del listino quando il prezzo è comune a più punti vendita.

Non va obbligato un indirizzo preciso se il dato disponibile è solo un prezzo
di catena/area.

## Regola di confronto

Ogni prezzo deve indicare chiaramente a quale negozio/ambito si riferisce. Il
confronto può quindi ordinare più negozi sul prezzo normalizzato senza perdere
il prezzo reale della confezione.

## Volantino

Il volantino può indicare una catena, un'area o un punto vendita. Questo
contesto viene usato solo durante il confronto temporaneo e non sostituisce il
prezzo base persistente.

---


**Stato: RIMANDATO.**

## Prezzo persistente

Il gestionale conserva principalmente il **prezzo normale/base** di un prodotto
presso un negozio.

Il prezzo è modificabile e conserva almeno:

- prezzo confezione;
- quantità/unità confezione;
- prezzo normalizzato calcolabile;
- data ultimo aggiornamento;
- autore dell'aggiornamento.

## Visualizzazione

Mostrare entrambi:

```text
Confezione: 500 g
Prezzo confezione: 1,29 €
Prezzo normalizzato: 2,58 €/kg
```

Per altri prodotti l'unità può essere €/l, €/pezzo, €/rotolo o altra metrica
sensata e dichiarata.

## Volantini

Le offerte temporanee non sostituiscono il prezzo base e non sono parte dello
storico principale dei prezzi.

Il volantino viene usato come input di confronto:

```text
Prezzo base negozio: 1,29 €
Prezzo visto sul volantino: 0,89 €
Risparmio: 0,40 € / 31%
```

Il confronto può anche mostrare che un'offerta di un negozio resta più cara del
prezzo base di un altro.

L'eventuale lettura assistita da PDF/foto è futura e deve richiedere conferma dei
dati estratti prima di usarli.

## Storico

Ha senso conservare lo storico delle modifiche ai **prezzi base**. Non va
riempito con ogni sconto settimanale visto su un volantino.

---


**Stato: RIMANDATO.**

## Prodotto acquistabile

Rappresenta qualcosa che si può comprare, eventualmente collegato a un alimento
o ad una tipologia di bene.

Esempio:

```text
Alimento: Pasta
Prodotto: Barilla Spaghetti n.5
Confezione: 500 g
```

## Oggetto posseduto

Il trapano fisico presente in casa resta un oggetto reale. Il prodotto/catalogo
commerciale da cui deriva è un concetto differente.

Questo permette in futuro di avere:

```text
Prodotto: detergente X
Acquisti ripetuti: più date/prezzi
```

senza creare più copie dell'oggetto posseduto.

## Monitoraggio selettivo

Per beni non alimentari il prezzo viene gestito solo su richiesta quando il
prodotto è acquistato frequentemente o vale la pena confrontarlo.

Esempi candidati:

- detersivo;
- carta igienica;
- batterie;
- olio motore;
- cartucce;
- altri consumabili.

## Confezione

Una confezione deve conservare quantità e unità sufficienti a calcolare il
prezzo normalizzato quando possibile.
