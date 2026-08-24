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

## Ricerca dall'elenco e struttura categorie

L'elenco alimenti espone direttamente `➕ Nuovo alimento` e `🔎 Cerca`, senza
obbligare a tornare al menu Alimentazione.

La migration `20260824173500_categorie_alimenti.sql` predispone categorie
alimentari globali ed estendibili tramite `categorie_alimento` e la relazione
molti-a-molti `alimento_categorie`. Le categorie iniziali sono Verdure, Frutta,
Carne, Pesce, Latticini, Uova, Cereali e derivati, Legumi, Condimenti e salse,
Bevande, Dolci e Altro.

Gli alimenti esistenti e quelli nuovi ricevono inizialmente `Altro`. La UI di
assegnazione e filtro per categoria verrà collegata sopra questa struttura
senza ulteriori modifiche strutturali al database.

## Filtri categoria operativi

L'elenco alimenti espone direttamente `🏷 Filtra` oltre a `➕ Nuovo alimento`,
`🔎 Cerca` e `🔄 Aggiorna`. Il filtro usa le categorie già introdotte dalla
migration `20260824173500_categorie_alimenti.sql` e mantiene i controlli di
visibilità multi-spazio del catalogo.

Dal dettaglio di un alimento il proprietario può impostare o cambiare la
categoria principale. Gli altri utenti possono vedere la categoria ma non
modificarla. La relazione dati resta molti-a-molti, quindi in futuro sarà
possibile consentire più categorie per alimento senza cambiare lo schema.

## Filtro multi-categoria

Il filtro categorie supporta la selezione multipla prima dell'applicazione.

Esempio: selezionando `🥩 Carne` e `🥬 Verdure` vengono mostrati gli alimenti
che appartengono ad almeno una delle categorie selezionate (semantica OR).
I risultati sono deduplicati anche quando, in futuro, un alimento appartiene a
più categorie contemporaneamente.

La schermata filtro usa checkbox, `✅ Applica`, `🧹 Azzera`, `📋 Tutti` e la
navigazione standard `⬅️ Indietro | 🏠 Menu principale`.

## Predisposizione ricerca ricette per ingredienti

La futura ricerca Ricette deve riutilizzare gli `alimenti` come ingredienti,
senza creare copie indipendenti.

Dato un insieme di alimenti richiesti, una ricetta entra nei risultati se
contiene almeno uno degli alimenti selezionati (semantica OR).

L'ordinamento deve privilegiare le ricette con il maggior numero di ingredienti
richiesti effettivamente presenti:

1. maggior numero di ingredienti selezionati trovati nella ricetta;
2. a parità, criterio secondario stabile da definire nella Step 7.2C.

Il futuro schema ingredienti ricetta dovrà quindi contenere almeno il
riferimento `ricetta_id -> alimento_id` ed essere indicizzato in modo da
supportare efficientemente `COUNT(DISTINCT alimento_id)` sui soli ingredienti
richiesti.

## Elenco alimenti: unità nascosta

Nelle schermate elenco, ricerca e filtro non viene mostrata l'unità di misura
predefinita dell'alimento: in quel contesto serve identificare l'alimento, non
la quantità.

L'unità resta memorizzata e continua a essere mostrata/usata nel dettaglio
dell'alimento e nei flussi in cui è necessaria (creazione, quantità e future
ricette).

`Uova` resta una categoria alimentare: il filtro per categoria raggruppa tipi
di alimento, mentre la futura ricerca Ricette per ingredienti lavorerà sui
singoli alimenti selezionati. Sono quindi due livelli distinti.

## Fondazione permessi espliciti

La visibilita di un alimento in uno spazio non concede automaticamente il
diritto di modificarlo. Il proprietario resta sempre autorizzato; un altro
utente puo modificare solo se possiede un permesso esplicito e continua ad
avere visibilita dell'alimento.

La migration introduce anche gli inviti e il livello separato di gestione dei
permessi. La UI Telegram per creare/accettare/revocare questi inviti viene
collegata nel checkpoint operativo successivo, insieme alla modifica completa
dell'alimento e alla categoria obbligatoria prima del salvataggio.

## Checkpoint operativo: creazione, modifica e collaboratori

Il flusso di creazione e ora `Nome -> Unita -> Categoria -> Visibilita -> Salva`.
La categoria viene quindi scelta esplicitamente prima del salvataggio; `Altro`
resta soltanto il fallback tecnico dello schema e una scelta esplicita possibile.

Dal dettaglio di un alimento compare `Modifica alimento` per il proprietario e
per chi possiede un permesso esplicito. Nome, unita e categoria richiedono
`puo_modificare`; visibilita e collaboratori richiedono `puo_gestire_permessi`.
La sola condivisione nello stesso spazio non concede alcun diritto di modifica.

Il gestore puo invitare utenti che condividono almeno uno spazio nel quale
l'alimento e visibile. L'invito arriva su Telegram e puo essere accettato o
rifiutato. Il permesso puo essere revocato. Se l'utente perde la visibilita,
il permesso resta registrato ma il backend lo considera non operativo.

Gli ID tecnici degli alimenti non vengono mostrati in elenco, ricerca, filtri o
dettaglio; restano usati internamente nei callback e nel database.
