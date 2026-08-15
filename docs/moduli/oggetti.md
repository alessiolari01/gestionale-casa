# Modulo Oggetti generici — Step 5A

## Scopo

Il modulo cataloga tutto cio' che non appartiene a un modulo dedicato: attrezzi,
elettronica, elettrodomestici, valigie, mobili, accessori e altri beni di casa.

Lo Step 5A copre il ciclo minimo completo:

`Telegram -> Rust -> SQLx -> items + oggetti -> Telegram`

La revisione dei **campi della bozza** prima di `✅ Salva` fa parte dello Step
5A. La modifica di un **oggetto già salvato**, invece, resta volutamente fuori
da questo sottostep e sarà aggiunta successivamente. Foto, documenti/tag,
garanzie/promemoria e prestiti restano anch'essi fuori dal 5A.

## Modello dati

Ogni oggetto ha sempre una riga in `items`:

- `items.tipo = 'oggetto'`;
- `items.nome` e' l'unico dato obbligatorio per l'utente;
- `items.id` e' anche la chiave primaria di `oggetti.item_id`.

La migration `20260814121600_oggetti.sql` aggiunge:

| Campo | Tipo logico | Obbligatorio | Note |
|---|---|---:|---|
| `item_id` | ID | si | PK + FK verso `items(id)`, cascade |
| `descrizione` | testo | no | dettaglio libero |
| `marca` | testo | no | es. Bosch |
| `modello` | testo | no | es. UniversalImpact 800 |
| `numero_serie` | testo | no | tenuto per gli oggetti dove ha senso |
| `posizione` | testo | no | es. Garage - scaffale 2 |
| `data_acquisto` | data ISO | no | salvata come `AAAA-MM-GG` |
| `prezzo_acquisto_centesimi` | intero | no | denaro in centesimi, mai `REAL` |
| `venditore` | testo | no | es. Amazon, MediaWorld, privato |
| `valore_stimato_centesimi` | intero | no | valore attuale stimato |
| `condizione` | enum testo | no | ottimo/buono/usurato/da_riparare |
| `note` | testo | no | note libere |

> `posizione` è testuale nello Step 5A. In futuro verrà migrata verso il sistema
> condiviso di luoghi/case/stanze, dopo approvazione della relativa architettura.

Foto, scontrini, garanzie, tag e promemoria non vengono duplicati in questa
tabella: useranno le tabelle core gia' predisposte negli step successivi.

## Interfaccia Telegram

L'interfaccia principale usa **inline keyboard**. I comandi testuali restano in
parallelo come scorciatoie e per debug; entrambe le strade richiamano la stessa
logica applicativa.

| Azione | Pulsante | Comando equivalente |
|---|---|---|
| menu oggetti | `📦 Oggetti` | `/oggetti` |
| nuova bozza | `➕ Nuovo oggetto` | `/oggetto_nuovo [nome]` |
| elenco | `📋 Elenco oggetti` | `/oggetti_lista` |
| ricerca | `🔎 Cerca` | `/oggetto_cerca [testo]` |
| scheda per ID | pulsante risultato | `/oggetto <id>` |
| annulla operazione | `❌ Annulla` | `/annulla` |
| salta campo opzionale | — | `/salta` |

`/start` apre il menu principale con Oggetti e Stato sistema; Vestiti, Veicoli
e Ricette sono gia' rappresentati esteticamente ma marcati come prossimamente.

## Creazione con pannello dettagli

1. il bot chiede solo il nome;
2. dopo il nome mostra il pannello dettagli;
3. l'utente puo' salvare subito oppure aggiungere dati opzionali;
4. `Marca e modello` e `Acquisto` sono piccoli flussi guidati;
5. `Condizione` usa quattro pulsanti;
6. `Altri dettagli` contiene descrizione, valore stimato e numero seriale;
7. le sezioni gia' compilate vengono marcate con `✅` nel pannello;
8. riaprendo un campo gia' valorizzato, il bot mostra il valore attuale prima di chiedere quello nuovo; `/salta` mantiene il valore esistente invece di sovrascriverlo;
9. `✅ Salva` inserisce `items` e `oggetti` nella stessa transazione SQL.

Una bozza incompleta vive solo in memoria: se il backend viene riavviato prima
del salvataggio, la bozza viene persa ma il database resta invariato.

## Elenco e ricerca

- elenco alfabetico, 8 oggetti per pagina;
- ogni riga ha un pulsante che apre la scheda;
- la ricerca controlla nome, marca, modello, numero seriale, posizione, venditore,
  descrizione e note;
- massimo 12 risultati per ricerca nello Step 5A.

## Validazione

- nome: 1-120 caratteri;
- date accettate: `GG/MM/AAAA` e `AAAA-MM-GG`, salvate in ISO;
- prezzi/valori: accettano `89`, `89,90`, `89.90` e formati italiani come
  `1.234,56`; vengono salvati come centesimi interi;
- importi negativi sono rifiutati sia dal parser sia dai `CHECK` SQLite;
- `condizione` e' vincolata a quattro valori ammessi.

## Test automatici Step 5A

Sono inclusi test per:

- parsing prezzi;
- normalizzazione/validazione date;
- parsing dei comandi con suffisso `@nome_bot`;
- salvataggio + lettura + elenco + ricerca;
- `ON DELETE CASCADE` tra `items` e `oggetti`;
- rifiuto di valori monetari negativi a livello SQLite.

La verifica definitiva resta da eseguire sul Galaxy S9 e in GitHub Actions prima
di chiudere lo Step 5A.

## Fuori perimetro

- Step 5B: foto degli oggetti;
- Step 5C: modifica ed eliminazione sicura degli oggetti già salvati;
- Step 5D: documenti e tag;
- Step 5E: garanzie e promemoria;
- Step 5F: prestiti e storico;
- Step 6: luoghi e multi-abitazione (più case, stanze, filtri e ricerca globale),
  con architettura da confermare prima dell'implementazione.
