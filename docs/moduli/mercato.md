# Mercato — negozi, prezzi, preferiti, codice a barre

Consegna B del 17 settembre 2026, concordata con Alessio subito dopo la
consegna A. Codice in `src/modules/mercato.rs`, con gli agganci nella lista
della spesa (`src/modules/lista_spesa.rs`). Schema in `docs/database.md`
(Step 7.4sexies).

**Il principio**: tutto facoltativo. Chi non registra un prezzo vede la
lista della spesa esattamente come prima; i pulsanti in più non chiedono
niente e non bloccano nulla.

## Negozi

`🏪` nella lista apre la scelta del negozio di **questa** spesa: è lì che
finiscono i prezzi che si segnano mentre si è dentro al supermercato
(`liste_spesa.negozio_id`).

`🏪 Gestisci negozi` (o `🏪 Negozi` dal confronto) apre l'elenco: le catene
diffuse in Italia arrivano già con la migration — Conad, Coop, Famila,
Esselunga, Lidl, Penny ed Eurospin sono quelle nominate da Alessio, le altre
diciotto sono le più comuni — e si toccano per metterle nel confronto o
toglierle (`negozi_confronto`, per utente).

**La scelta è dell'utente, esplicitamente**: confrontare venticinque catene
non serve a nessuno, e nessuno fa la spesa in tutte.

`➕ Altro negozio` crea un negozio non di catena ("Il fruttivendolo di via
Roma"): appartiene allo spazio attivo e nasce già nel confronto, perché chi
lo scrive lo sta usando adesso.

## Prezzi

Dalla schermata `📦 Ho preso…` di una voce:

- **`💶 Prezzo`** chiede quanto è costato (`1,29`, `1.29`, `1,29 €`: si
  legge come lo si scrive). Segnare un prezzo spunta anche la voce.
- L'avviso riporta il prezzo al chilo o al litro quando si può calcolare
  (`5,00 € al kg`), che è l'unico modo di capire se un prezzo è buono.
- Con un negozio scelto, il prezzo finisce anche in `prezzi_osservati` e da
  lì serve alle stime; senza negozio resta sulla voce e nello scontrino, e
  il bot lo dice.

La lista mostra `💶 Segnato finora: …` solo se almeno un prezzo c'è.

## Dove conviene

`📊 Dove conviene` valuta le voci **ancora da comprare** con l'ultimo
prezzo visto in ciascun negozio scelto:

```
📊 Dove conviene

• Coop: 19,00 € (5/5 voci)
• Conad: 21,40 € (5/5 voci)
• Lidl: 5,00 € (1/5 voci)
```

L'ordine premette chi copre **più voci**, e solo dopo guarda il totale: un
negozio che conosce un prodotto su cinque non può vincere perché la sua
somma è piccola. Il testo dice sempre che è una stima sui prezzi visti da
noi, non un listino.

## Totale dello scontrino

Chiudendo la spesa il bot chiede il totale (`🧾`), con `➖ Salta il totale`
sempre disponibile: è l'opzione "c" scelta da Alessio, prezzo per voce e
totale sono indipendenti e facoltativi. Il totale resta in
`liste_spesa_chiusure.totale_centesimi` e **diventerà una transazione del
modulo Soldi** (approvato da Alessio, `docs/previsto/spese.md`).

## Prodotti preferiti

Nella schermata `📦 Ho preso…`, accanto a ogni confezione c'è `⭐`: segna il
prodotto preferito per quell'alimento (uno per alimento, per utente). Il
preferito va in cima e porta la stella anche nell'etichetta, così la
prossima spesa è un tocco solo.

## Codice a barre (Open Food Facts)

`🏷 Codice a barre` nella schermata `📦 Ho preso…`: si scrivono le cifre
sotto le righe nere.

1. il codice si controlla prima (8, 12, 13 o 14 cifre): un errore di
   battitura non diventa una richiesta di rete;
2. se il catalogo conosce già quel codice, non si chiede niente a nessuno;
3. altrimenti si legge da **Open Food Facts** nome, marca e quantità; il
   prodotto viene creato sull'alimento della voce, con il suo `codice_ean`;
4. la confezione diventa la quantità presa (300 g, non i 250 g che
   servivano).

Se la quantità dichiarata non si capisce (`"una confezione"`), il prodotto
non si crea: meglio chiedere all'utente che inventare una quantità.

## Open Prices — suggerimento, non prezzo

Dopo una lettura del codice a barre il bot chiede a **Open Prices** l'ultimo
prezzo segnato da altri per quel codice e lo riporta così:

> 💶 Su Open Prices altri l'hanno pagato 1,49 €: se è anche il tuo, segnalo
> con 💶 Prezzo.

Non viene mai registrato da solo: `prezzi_osservati.fonte` distingue
`spesa` (visto da noi) da `open_prices`, e un prezzo di altri non deve
finire nelle nostre stime senza conferma.

## Rete assente

Entrambe le fonti hanno 8 secondi di tempo e un errore proprio: "non riesco
a raggiungere Open Food Facts adesso, scrivi la quantità a mano". Il bot
resta usabile senza rete: è un bot di casa, non un servizio online.

## Tabelle

```text
negozi                catene comuni (spazio_id NULL) e negozi dello spazio
negozi_confronto      quali negozi l'utente vuole nel confronto
prezzi_osservati      un prezzo visto, per negozio e giorno, con la fonte
prodotti_preferiti    il prodotto preferito per un alimento, per utente
```

Più le colonne aggiunte a `liste_spesa` (`negozio_id`), `liste_spesa_voci` e
`liste_spesa_voci_archiviate` (`prezzo_centesimi`), `liste_spesa_chiusure`
(`negozio_id`, `totale_centesimi`).

## Fuori scope, per ora

- **Volantini e offerte**: restano nel modulo Acquisti previsto
  (`docs/previsto/acquisti.md`).
- **Prezzi per punto vendita**: si registra la catena, non il singolo
  supermercato. Chi ha bisogno di distinguerli può creare un negozio suo.
- **Scrivere su Open Prices**: si legge soltanto.
- **Prodotti non alimentari**: il catalogo è quello degli alimenti.
