# Viaggi — previsto

> Specifica di un modulo **non ancora costruito**. Nessuna riga di codice, nessuna tabella.

---


**Stato: RIMANDATO — specificato per compatibilità architetturale futura.**

Funzione principale: preparare un viaggio usando bagagli reali del gestionale,
checklist generiche modificabili e collegamenti opzionali agli oggetti
registrati.

## Funzioni previste

- viaggio con date e partecipanti;
- scelta di una valigia/bagaglio già presente come oggetto;
- creazione immediata del bagaglio come nuovo oggetto se manca;
- checklist base generica e modificabile;
- modelli checklist riutilizzabili/copiabili/condivisibili;
- quantità base + quantità extra opzionale;
- collegamento di più oggetti reali alla stessa voce quantitativa;
- stato temporaneo `in viaggio`;
- bagaglio che contiene l'oggetto durante il viaggio;
- verifica degli oggetti al rientro;
- collegamento opzionale al futuro modulo Spese.

Dettagli in [Bagagli e checklist](bagagli-e-checklist.md).

---


**Stato: RIMANDATO.**

## Checklist base

Il gestionale fornisce/consente una base di categorie e voci generiche, per
esempio abbigliamento, igiene, elettronica e documenti.

Le voci devono essere completamente modificabili:

- aggiunta;
- rinomina;
- rimozione;
- quantità/regola quantità;
- quantità extra opzionale;
- controllo al ritorno sì/no.

## Quantità

Una voce può calcolare una quantità suggerita in base alla durata del viaggio o
usare una quantità fissa/personalizzata.

Esempio:

```text
Calzini
quantità base: 5
extra: 1
totale: 6
```

La UI compatta può mostrare solo `Calzini ×6`; il dettaglio resta modificabile.

## Collegamento con oggetti reali

Una voce generica può essere coperta da zero, uno o **più** oggetti registrati.

Esempio:

```text
Calzini richiesti: 5
- Calzini neri Nike [oggetto]
- Calzini bianchi Nike [oggetto]
- Calzini grigi [oggetto]
- Calzini Adidas [oggetto]
- 1 paio generico
```

Quantità generica e oggetti reali possono convivere.

## Bagaglio

Il viaggio permette di scegliere un bagaglio già esistente nel modulo Oggetti.
Se non esiste, può essere creato al momento come vero oggetto e poi associato al
viaggio.

## Stato temporaneo

Un oggetto portato non perde la posizione abituale.

Esempio:

```text
Posizione abituale: Casa / Camera / Armadio
Stato temporaneo: In viaggio — Corfù
Bagaglio: Trolley nero
```

La posizione abituale serve anche come riferimento per il rientro.

## Partenza e rientro

La stessa voce può mantenere stati distinti:

- preparato/portato alla partenza;
- verificato al rientro.

Per oggetti reali il controllo può indicare esattamente quale elemento manca.

Stati candidati al rientro:

- recuperato;
- non trovato;
- lasciato altrove;
- non più con me.

Il viaggio non deve essere chiuso come completamente rientrato ignorando oggetti
ancora da verificare senza una scelta esplicita dell'utente.

## Condivisione

Checklist/modelli possono essere condivisi o copiati secondo le regole Step 7.
Un viaggio condiviso può avere voci personali e comuni; proprietario dell'oggetto
e persona che lo trasporta possono differire.
