# Step 7.2I.3 — Calcolo multi-profilo

## Obiettivo

Rendere il dominio Porzioni pronto per planner e lista della spesa senza introdurre
ancora tali moduli.

## Regola di calcolo

Per ogni profilo:

1. quantità originale della ricetta / porzioni base;
2. fattore percentuale del profilo;
3. override del singolo ingrediente;
4. esclusione = nessun contributo.

I contributi finali dei profili inclusi vengono sommati. Gli esclusi sono mantenuti
separatamente, così planner e spesa potranno distinguere una vera esclusione da una
quantità zero.

## UX rifinita nello stesso step

- una percentuale valida torna subito a `⚙️ Personalizzazione ricetta`;
- la conferma è integrata in testa alla schermata di riepilogo e sparisce alla
  successiva interazione;
- la categoria operativa `Verificati` dei Miglioramenti è rimossa;
- un collaudo positivo porta direttamente in Archivio.

## Database

Nessuna nuova struttura dati richiesta per I.3.

## Conferme Ingredienti personalizzati

Dopo quantità personalizzata, esclusione o ripristino della quantità calcolata,
il bot torna alla stessa pagina della lista ingredienti e mostra la conferma in
testa alla schermata. Alla successiva interazione la conferma non viene riproposta.
