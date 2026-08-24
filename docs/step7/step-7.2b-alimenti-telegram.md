# Step 7.2B — Proprietà personale e condivisione degli alimenti

## Regola centrale

Un alimento creato da un utente appartiene all'utente, non allo spazio.

Gli spazi definiscono solamente dove l'alimento viene condiviso.

Alla creazione si può scegliere:

- `🔒 Solo mio`;
- `🎯 Spazio predefinito`;
- `🌐 Tutti i miei spazi`;
- `🎛 Scegli spazi`.

Se il proprietario perde la membership di uno spazio, l'alimento continua a
esistere nel suo catalogo personale.

## Sincronizzazione

Non vengono create copie indipendenti degli alimenti condivisi.

Quando un altro utente crea un alimento e lo condivide con uno spazio comune,
i membri autorizzati vedono lo stesso record. `🔄 Aggiorna alimenti` forza una
nuova lettura del database e mostra immediatamente le aggiunte degli altri
profili.

In questo modo una futura modifica dell'alimento potrà propagarsi agli spazi
condivisi senza dover sincronizzare copie divergenti.

## Sicurezza

- un utente può condividere soltanto verso spazi dove ha diritto di scrittura;
- un membro `lettura` può creare alimenti personali, ma non condividerli;
- la visibilità degli alimenti altrui richiede una membership corrente;
- il proprietario vede sempre i propri alimenti;
- la rimozione di una membership non cancella la proprietà personale.

## Compatibilità Step 7.2A

La migration `20260824160500_alimenti_proprieta_condivisione.sql` è append-only.

La colonna legacy `alimenti.spazio_id` rimane fisicamente nello schema ma non
viene più usata dal runtime come fonte di proprietà o visibilità.

## Convenzione di navigazione Telegram

La UI Alimentazione segue la convenzione generale del bot:

- `🏠 Menu principale` deve essere sempre disponibile;
- dove esiste un livello precedente, `⬅️ Indietro` compare sulla stessa riga;
- se il passaggio prevede `⏭ Salta` o `❌ Annulla`, questi pulsanti condividono
  la stessa riga di navigazione con `⬅️ Indietro` e `🏠 Menu principale`;
- il menu radice `🍽️ Alimentazione` mostra soltanto `🏠 Menu principale`, perché
  il suo livello precedente coincide con il menu principale stesso.

## Unità obbligatoria e annullamento della bozza

Per un nuovo alimento l'unità di misura è obbligatoria: non esiste più
l'azione `Salta` e anche il backend rifiuta un salvataggio senza unità.

Nome, unità e visibilità restano nella sessione temporanea fino al salvataggio
finale. `❌ Annulla`, `/annulla` e `🏠 Menu principale` scartano la bozza e
mostrano un avviso esplicito. Nessun alimento parziale viene scritto nel
database.
