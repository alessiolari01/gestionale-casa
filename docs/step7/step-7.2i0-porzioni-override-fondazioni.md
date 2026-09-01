# Step 7.2I.0 — Fondazioni Porzioni e override

## Obiettivo

Introdurre il modello dati e la logica di dominio minima per le porzioni
personalizzate dei Profili alimentari senza collegare ancora il flusso alla UI
Telegram, al Planner o alla Lista della spesa.

## Decisioni

- La ricetta resta la fonte delle quantità base e di `porzioni_base`.
- La personalizzazione appartiene al Profilo alimentare, non allo Spazio.
- Il fattore `1.0` indica una porzione standard della ricetta.
- L'override di quantità viene applicato dopo il fattore personale.
- L'esclusione di un ingrediente è distinta dalla quantità zero.
- Un override è legato alla riga `ricetta_ingredienti`, non al solo alimento
  generico.
- I Profili alimentari restano privati/condivisi secondo le regole già
  introdotte in 7.2H; non diventano globali.
- Nessun dato esistente viene modificato o popolato retroattivamente.

## Schema aggiunto

`profilo_ricetta_porzioni`
- `profilo_alimentare_id`
- `ricetta_id`
- `fattore_porzione`
- timestamp
- chiave primaria composta profilo/ricetta

`profilo_ricetta_ingredienti_override`
- `profilo_alimentare_id`
- `ricetta_ingrediente_id`
- `tipo_override`: `quantita` oppure `escluso`
- `quantita_override`, valorizzata solo per `quantita`
- timestamp
- chiave primaria composta profilo/riga ingrediente

## Regola di calcolo

1. `quantita_base = quantita_ricetta / porzioni_base`
2. `quantita_scalata = quantita_base * fattore_profilo`
3. se esiste un override:
   - `quantita`: sostituisce la quantità scalata;
   - `escluso`: l'ingrediente non partecipa al risultato finale.

## Scope intenzionalmente escluso

- handler e pulsanti Telegram;
- gestione CRUD delle impostazioni;
- conversioni tra unità;
- sostituzione di un ingrediente con un altro;
- aggregazione multi-profilo;
- Planner;
- snapshot/versionamento ricette;
- Lista della spesa.

Queste funzioni appartengono ai sottostep successivi di 7.2I.

## Verifica prevista

Prima di applicare la migration al DB reale:
1. pipeline Rust completa;
2. controllo che la migration non sia già registrata;
3. backup del DB reale;
4. `PRAGMA integrity_check`;
5. `PRAGMA foreign_key_check`;
6. prova migration su copia del DB;
7. nuovi controlli integrity/foreign key sulla copia;
8. solo dopo avvio del gestionale sul DB reale.
