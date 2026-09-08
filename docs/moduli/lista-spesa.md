# Lista della spesa

**Scritta l'8 settembre 2026, collaudo dal vivo su Telegram da fare.**
`src/modules/lista_spesa.rs`, raggiungibile da
`🍽️ Alimentazione → 🛒 Lista della spesa`.

Aggrega automaticamente gli ingredienti dei pasti pianificati in un
intervallo di date scelto dall'utente, indipendente dalle settimane del
planner, e permette di aggiungere voci a mano e spuntarle mentre si fa la
spesa.

## Una sola lista, con un intervallo proprio

Stesso principio del planner (`spazio_id` nullable, una lista attiva per
spazio o personale): non esiste una creazione manuale, la lista nasce alla
prima apertura con un intervallo di default (oggi + 6 giorni), modificabile
in qualunque momento con `🗓️ Cambia intervallo`.

L'intervallo della lista **non è legato alle settimane del planner**: il
planner crea una riga `planner_alimentari` per settimana, ma la lista
aggrega su *tutti* i `planner_alimentari` non archiviati dello stesso
spazio/proprietario la cui `data_pasto` ricade nell'intervallo scelto,
qualunque settimana attraversi.

## Cosa aggrega

Da `planner_pasto_ingredienti_snapshot` (via `planner_pasti` →
`planner_alimentari`), solo le righe che soddisfano tutte queste condizioni:

- il pasto è `stato = 'pianificato'`, non saltato (`saltato_il IS NULL`), non
  completato (`completato_il IS NULL`) — un pasto saltato non contribuisce,
  decisione esplicita: non si compra qualcosa per un pasto che non si farà;
- `quantita_finale_snapshot IS NOT NULL` — le righe con quantità finale nulla
  sono ingredienti esclusi da quel profilo per quel pasto, non un valore da
  sommare come zero;
- `data_pasto` dentro l'intervallo della lista.

## Conversione di unità

Per ogni riga aggregata, `unita_misura.famiglia_conversione` decide se
convertire: `massa` e `volume` si sommano nell'unità-base della famiglia
(`g` per la massa, `ml` per il volume — fattore 1/1 per definizione); un
simbolo senza famiglia (`pz`, `cucchiaio`, `cucchiaino`, `qb`) o non presente
in `unita_misura` non si converte e si aggrega per simbolo esatto, separato
da qualunque altro simbolo.

La chiave di aggregazione è l'alimento (per id se presente, altrimenti nome
normalizzato) più l'unità risultante dopo l'eventuale conversione: due
ingredienti diversi nella stessa unità restano voci separate, lo stesso
ingrediente in unità di famiglie diverse resta separato.

## Voci generate e manuali

Ogni voce (`liste_spesa_voci`) ha `origine` `generato` o `manuale`. Le
manuali servono sia per alimenti del catalogo sia per testo libero (un
prodotto commerciale non modellato, es. "Detersivo piatti"): descrizione
obbligatoria, quantità e unità opzionali.

## Segna comprato

Flag booleano per riga intera, non quantità parziale: non c'è modo di dire
"ho comprato solo metà" di una voce. Una volta `comprato = 1`, un trigger a
database impedisce di modificare quantità/unità/descrizione/origine — solo
il toggle (tornare a `comprato = 0`) resta permesso, stesso principio del
congelamento di `planner_pasti`.

## Aggiornamento esplicito, mai automatico

`🔄 Aggiorna lista` ricalcola **solo** le voci `origine = 'generato' AND
comprato = 0`: le cancella e re-inserisce da zero il risultato fresco
dell'aggregazione. Le voci comprate (generate o manuali) e le voci manuali
non comprate non vengono mai toccate.

Se dopo un refresh serve più di un alimento già segnato comprato (es. un
nuovo pasto pianificato che usa lo stesso ingrediente), compare una voce
**nuova** non comprata per la sola differenza — non c'è merge con la vecchia
riga comprata, per non alterare mai una quantità già segnata come acquistata.

## Schermate

**Principale** — intervallo, conteggio comprate/totale, voci come pulsanti
`✅`/`☐` (toggle al tocco), paginazione da `modules::liste`; `🔄 Aggiorna
lista`, `➕ Aggiungi voce manuale`, `🗓️ Cambia intervallo`.

**Aggiungi voce manuale** — input ibrido in due passi: descrizione libera
(testo), poi quantità+unità scritte a mano (es. "500 g") oppure `➖ Senza
quantità`. Sessione dedicata (`ListaSpesaSessionStore`, dentro
`lista_spesa.rs`), stesso schema di `DistribuzioneSessionStore`.

**Cambia intervallo** — due passaggi sul calendario di `modules::calendario`
(data di inizio, poi data di fine — i giorni prima dell'inizio sono
bloccati).

## Tabelle

```text
liste_spesa        intervallo, proprietario, spazio
liste_spesa_voci   voci generate o manuali, comprato/comprato_il
```

Vedi `docs/database.md` per i campi.

## Fuori scope

Non modella la dispensa, i prezzi, o la scelta del formato/confezione da
acquistare: quello è il futuro modulo Acquisti, che dirà *quale prodotto o
confezione comprare*, mentre questa lista dice solo *cosa serve* — vedi
`docs/previsto/lista-della-spesa.md`, sezione "Relazione con Acquisti".
