# ⚙️ Impostazioni: accendere e spegnere le funzioni

Chiesto da Alessio il 24 settembre 2026: «dai la possibilità all'utente di
disattivare le funzioni, tipo scorta/dispensa perché magari non vuole
tracciare costantemente il cibo».

Un gestionale personale che obbliga a usare tutto diventa un lavoro. Le
Scorte sono l'esempio esatto: valgono qualcosa solo se qualcuno le tiene
aggiornate, e chi non ha voglia di farlo si ritrova una dispensa ferma che
però continua a togliere roba dalla lista della spesa. Meglio spegnerle.

## Il principio

**Spegnere una funzione non nasconde soltanto un pulsante: spegne anche
quello che quella funzione fa da sola.** Con le Scorte spente:

- il pulsante 🥫 Scorte non c'è più nel menù Alimentazione;
- la spesa chiusa non entra in casa;
- un pasto preparato o consumato non scala niente;
- **la lista della spesa smette di sottrarre quello che c'è in casa**, e
  chiede le quantità intere.

Quest'ultimo è il punto che rende la cosa onesta: una lista dimezzata da
scorte che nessuno aggiorna più sarebbe peggio che nessuna lista.

**I dati non si toccano mai.** Spegnere è reversibile e non cancella niente:
riaccendendo si ritrova tutto dov'era. Per questo qui non c'è nessuna
conferma (C16 riguarda le eliminazioni definitive, e qui non si elimina).

## Cosa si può spegnere

Sezioni: 🍽️ Alimentazione, 🍳 Ricette, 👥 Profili alimentari, 📅 Planner
alimentare, 🛒 Lista della spesa, 🥫 Scorte, 📋 Turni e routine, 💶 Prezzi e
negozi, 🏷️ Oggetti, 🏠 Case, stanze e contenitori, 📜 Storico.

Cosa fa da solo: 📥 la spesa chiusa entra in casa, 🍲 i pasti scalano le
scorte, 🔄 la lista si aggiorna da sola, 💡 la legenda dei simboli.

**Non si spengono**: ⚙️ Impostazioni, 👤 Profilo, 👥 Spazi, 📋 Miglioramenti e
🛠️ Amministrazione — senza di loro non ci sarebbe più modo di riaccendere
quello che si è spento, o di segnalare che qualcosa non va. E **🥕 Alimenti**,
che è il catalogo da cui dipendono tutte le altre voci di Alimentazione.

## Chi contiene chi

Una funzione ha un padre: spento il padre, il figlio non si raggiunge
comunque.

| funzione | padre |
|---|---|
| Ricette, Profili alimentari, Planner, Lista della spesa, Scorte, Turni | Alimentazione |
| Prezzi e negozi, La lista si aggiorna da sola | Lista della spesa |
| La spesa entra in casa, I pasti scalano le scorte | Scorte |

La scelta fatta su un figlio resta la sua: spegnendo Alimentazione le Scorte
risultano spente, ma riaccendendo Alimentazione tornano come le si era
lasciate. Per questo esistono due domande diverse: `attiva` (la si può usare
adesso?) e `spenta_di_suo` (che interruttore ha questa funzione?), e la
schermata delle impostazioni mostra la seconda.

## I pulsanti delle schermate vecchie

Su Telegram un messaggio di tre giorni fa resta lì con i suoi pulsanti, e
nascondere una voce dal menù non basta. Ogni callback passa da un controllo
(`Funzioni::blocca`) che confronta il prefisso — `dispensa:`, `recipe:`,
`lista_spesa:`… — con le funzioni spente, e risponde con una schermata che
dice cosa è spento e come riaccenderlo. Il controllo legge le preferenze
**solo** se il callback appartiene davvero a una sezione spegnibile
(`puo_essere_spento`): la maggior parte dei click non paga niente.

Il messaggio nomina l'interruttore che si è davvero spostato: se le Scorte
sono spente perché è spenta l'Alimentazione, dire "Scorte" manderebbe a
cercare un interruttore già a posto.

## Leggere le impostazioni non deve far cadere il bot

`funzioni()` passa da `identity::current_actor_opt()`, non da
`current_actor()`. La differenza non è cosmetica: `current_actor()` in
produzione **va in panic** quando il contesto dell'attore manca, e alcune
schermate girano proprio fuori da quel contesto — la notifica di avvio agli
amministratori e la risposta "questa schermata non è più attiva", che arriva
prima che l'attore venga risolto.

Il 24 settembre 2026 la prima versione di questo modulo usava
`current_actor()`, e il bot è caduto per davvero:
panic su un worker di Tokio, dispatcher di teloxide morto, processo finito,
bot giù per un'ora e mezza. Senza utente le funzioni risultano **tutte
accese**: è il default giusto, perché quelle schermate devono solo mostrare
qualcosa, non decidere niente sui dati di nessuno.

I test non prendono questa classe di errore da soli: in `#[cfg(test)]` il
contesto mancante ricade sull'attore di sistema invece di far cadere niente.

## Dove sta il sì/no

Le funzioni nuove stanno in **`funzioni_spente`**, una riga per ogni funzione
spenta di quell'utente: l'assenza vuol dire accesa. Così non serve popolare
niente per gli utenti che ci sono già, e una funzione nuova nasce accesa per
tutti senza altre migration.

Le tre preferenze che esistevano prima — `dispensa_ingresso_automatico`,
`lista_spesa_aggiornamento_automatico`, `mostra_legenda` — **restano nelle
loro colonne** di `preferenze_utente`: la schermata legge e scrive quelle.
Un fatto in un posto solo, anche a costo di due modi di salvare
(`Dove::Tabella` e `Dove::Colonna`).

## Per utente, non per spazio

L'interruttore è di chi guarda. Due persone nello stesso spazio possono
usare il gestionale in modo diverso: chi tiene le scorte le vede e le
aggiorna, chi non le tiene non se le trova davanti. I dati restano condivisi
come prima — spegnere non è un permesso, è una preferenza.

## Schermate

| schermata | come ci si arriva |
|---|---|
| ⚙️ Impostazioni | `settings:menu`, dal menù principale |
| 🧩 Sezioni | `settings:sezioni` |
| 🤖 Cosa fa da solo | `settings:automatismi` |
| (interruttore) | `settings:toggle:<chiave>`, poi si rimostra l'elenco |
| "… è spento" | premendo il pulsante di una funzione spenta |

Negli elenchi `✅` è acceso e `⬜` spento, e la spiegazione di cosa si perde
compare accanto alle funzioni **ancora accese**: è lì che serve saperlo.

## Tabelle

`funzioni_spente (utente_id, funzione, spenta_il)`, chiave primaria sulle
prime due. La chiave di una funzione non cambia mai: cambiarla riaccenderebbe
di colpo qualcosa che qualcuno aveva spento.

## Fuori scope, per ora

- Spegnere una funzione per tutto lo spazio invece che per sé.
- Nascondere i comandi testuali (`/alimenti`, `/oggetti`…) delle funzioni
  spente: restano, e chi li conosce li usa.
