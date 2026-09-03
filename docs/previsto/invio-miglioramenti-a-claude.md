# Invio diretto dei miglioramenti a Claude

**Stato: PREVISTO.**

Riprende la voce di backlog #9 già segnata in `docs/moduli/miglioramenti.md`
("🧪 Zona test / aggiornamenti quasi zero-downtime"), nel contesto più ampio
del ciclo di sviluppo automatizzato descritto altrove in `docs/previsto/`.

## Problema

Oggi un miglioramento approvato (stato `da_fare`) diventa una richiesta di
lavoro solo se l'amministratore principale lo riporta a mano in una sessione
Claude Code. Manca un canale diretto dal bot verso l'agente, sia per i
miglioramenti creati dall'admin sia per quelli proposti da utenti normali e
poi approvati.

## Comportamento previsto

Sui miglioramenti in stato `da_fare` compare l'azione:

```text
📤 Invia a Claude
```

Visibile e utilizzabile **solo dall'amministratore principale**, chiunque
ne sia l'autore originale (admin o utente normale approvato) — non un
permesso condiviso con altri ruoli.

L'azione mette il miglioramento in una coda (nuova colonna/tabella dedicata,
da progettare in fase di implementazione, coerente con lo schema esistente
di `miglioramenti`). Non implica esecuzione immediata: è l'agente, quando
attivo su una sessione Claude Code sul PC fisso, a leggere la coda e a
trattare ogni voce come una richiesta di funzionalità, seguendo lo stesso
ciclo descritto per l'automazione del deploy (scrittura, collaudo, conferma
funzionale, merge).

## Cosa passa nella richiesta

Lo stesso contenuto già presente nel record del miglioramento:

- testo/descrizione (multimessaggio, come oggi);
- allegati collegati;
- autore originale (admin o utente normale approvato), per contesto;
- eventuale piano di verifica già presente in `miglioramento_piani_verifica`,
  utile come base per la checklist di collaudo guidato.

## Relazione con il resto

Dipende dal ciclo di automazione già descritto in `docs/previsto/` (deploy a
downtime minimo, modalità riservata, conferma via Telegram): questo
documento aggiunge solo un canale di **ingresso** alternativo alla richiesta
scritta a mano in chat, non cambia il ciclo stesso una volta che la
richiesta è stata presa in carico.

## Transizioni di stato guidate da Claude

Per i miglioramenti presi in carico tramite questo canale, è l'agente a far
avanzare lo stato secondo il workflow già esistente nel modulo, non
l'amministratore a mano:

- `da_fare` → `fatto` quando l'implementazione è completa e la CI è verde
  (corrisponde al passaggio 6 del ciclo di automazione);
- `fatto` → verificato/archiviato quando arriva la tua conferma funzionale
  su Telegram (passaggio 9) — la conferma stessa sostituisce la pressione
  manuale di `📦 Archivia miglioramento`, non introduce un passaggio in più.

Se rifiuti il collaudo, il miglioramento resta `da_fare` con le note di
cosa non ha funzionato, coerente con il rollback già previsto nel ciclo di
automazione.

## Deciso il 3 settembre 2026

- **invii duplicati**: una colonna `in_coda_claude` (booleano) + timestamp
  sulla riga `miglioramenti`, non una tabella coda separata. Premere
  `📤 Invia a Claude` quando è già in coda aggiorna solo il timestamp, non
  crea una seconda voce; il pulsante mostra lo stato "già in coda" finché
  l'agente non lo prende in carico e lo azzera.
