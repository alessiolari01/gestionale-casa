# Web app, accesso da fuori casa, dove far girare il gestionale

Scritto il 26 settembre 2026, dopo tre giorni di collaudi e correzioni, in
risposta alle domande di Alessio: conviene trasformare il bot in una web app?
Serve Figma? Si può usare da fuori casa senza toccare il modem? L'S9 basta? Ha
senso Rust? Come si testa con un secondo account?

Questo è un documento per **decidere**, non un piano già approvato. Niente di
quello che c'è qui è stato fatto.

## 1. Da dove nascono davvero i problemi di questi giorni

Prima di cambiare tecnologia conviene guardare in faccia i difetti trovati fra
il 23 e il 26 settembre, e chiedersi quali sarebbero spariti con una web app.

| difetto | causa vera | una web app lo evitava? |
|---|---|---|
| Aggiunte dal catalogo sbagliate **tre volte** (chiudeva troppo, poi troppo poco, poi cancellava la voce sbagliata) | complessità del dominio e una mia riconciliazione con troppi rami | **No.** Identico in qualunque linguaggio e interfaccia |
| Date in UTC (prezzi, chiusura spesa) | mia disattenzione | **No** |
| Il bot è caduto per un'ora e mezza | `panic` in un punto senza contesto utente, che uccide il dispatcher di Telegram | **In parte sì**: in un server web un errore così rompe *una richiesta*, non tutto il servizio |
| Etichette dei pulsanti tagliate, tentativo di andare a capo, formato prima del nome | **limiti di Telegram**: i pulsanti sono una riga, non vanno a capo, e sono larghi mezza schermata | **Sì, del tutto** |
| I test non hanno preso il crash | in `cfg(test)` il contesto mancante non fa cadere niente | **No**, è un difetto del progetto |

Il conto onesto: **circa due terzi dei problemi non c'entrano niente con
Telegram né con Rust**. Sono logica di dominio e mia disciplina. Un terzo — le
tre tornate passate a combattere con etichette che non si possono mandare a
capo — è colpa dell'interfaccia, e quello sì, con una pagina web non
esisterebbe.

**Conseguenza pratica**: una web app va fatta per avere un'interfaccia
decente, non per avere meno bug. Chi la fa sperando che i bug diminuiscano
resta deluso: la maggior parte se li porta dietro.

## 2. Come si arriva a una web app senza buttare via niente

Il valore di questo progetto **non è il bot**: sono le ~475 prove automatiche e
la logica che c'è sotto (fabbisogno, scorte, unità di misura, permessi,
spazi condivisi). Quella parte non ha niente di Telegram dentro.

La strada che consiglio, in ordine:

1. **Il nucleo resta com'è** (Rust + SQLite). Nessuna riscrittura.
2. **Si aggiunge un'interfaccia HTTP** (in Rust, con `axum`) che chiama le
   stesse funzioni che oggi chiama il bot.
3. **Si aggiunge una pagina web** che parla con quell'interfaccia.
4. **Il bot Telegram resta**, come secondo modo di usare la stessa cosa. Non si
   spegne: per segnare la spesa al supermercato con una mano sola è più comodo
   di qualunque sito.

Quello che questa strada **costa davvero**, e va detto prima:

- **l'autenticazione**. Oggi Telegram fa da portiere gratis: chi scrive al bot
  è già identificato, e il progetto non ha nessun login. Una pagina pubblica
  richiede registrazione, password (salvate con `argon2`, mai in chiaro),
  sessioni, recupero password, e il fatto che da quel momento **esiste una
  porta aperta su internet con dentro i dati di casa tua**;
- **il doppio lavoro sulle schermate**: ogni funzione nuova va fatta due volte,
  una per il bot e una per il sito, oppure si accetta che il bot resti indietro;
- **imparare HTML e CSS** almeno un po', o accettare quello che scrivo io.

## 3. Figma: non adesso

Figma è uno strumento per **disegnare** le schermate, non per programmarle: ne
esce un'immagine, non codice funzionante. Serve quando si vuole decidere
l'aspetto prima di scriverlo, o quando il disegno lo fa una persona e il codice
un'altra.

Qui non è il caso: sarebbe un programma nuovo da imparare per produrre disegni
che poi vanno comunque tradotti a mano. Meglio partire da pagine semplici e
sistemarle guardandole. Se un giorno l'aspetto diventa la cosa importante, si
riprende in mano.

## 4. Arrivare da fuori casa

Questa è la domanda con la risposta più netta, e la notizia è buona: **non
serve toccare il modem in nessuno dei casi**. Aprire porte sul router è la
strada vecchia, ed è anche la più rischiosa.

### 4a. Il bot Telegram non ha questo problema per niente

Il bot **non riceve connessioni**: è lui che chiama i server di Telegram e
chiede se c'è qualcosa di nuovo. Per questo funziona già oggi da qualunque
rete, anche dal telefono in vacanza, senza Tailscale e senza aver configurato
niente. È un vantaggio reale che una web app **perde**: un sito, per essere
visitato, deve essere raggiungibile.

### 4b. Tailscale: già risolto, ma solo per te

Tailscale crea una rete privata virtuale fra i tuoi dispositivi. Rispondendo
alla domanda precisa: **sì, ti mette in una rete locale virtuale anche quando
sei fuori casa.** Se sul portatile o sul telefono hai Tailscale attivo, l'S9 è
raggiungibile come se fossi sul divano, da qualunque rete, senza aprire nulla
sul router.

Il limite: **funziona solo sui dispositivi dove Tailscale è installato e
collegato al tuo account**. Va benissimo per te. Non va bene per far provare
l'app a tua sorella, e non va bene per un sito "pubblico".

### 4c. Far vedere l'app ad altri, gratis e temporaneamente

Due strade, entrambe senza toccare il modem:

- **Tailscale Funnel** — è dentro Tailscale, che hai già. Con un comando
  prende una porta del tuo server e la rende visibile su internet con un
  indirizzo `https://...ts.net` valido, certificato compreso. Gratis,
  nessun account nuovo. Limiti: poche porte ammesse (443, 8443, 10000), il
  traffico passa dai relay di Tailscale e c'è un limite di banda, quindi è
  perfetto per provare e non per un servizio serio.
- **Cloudflare Tunnel** — un programmino sul server apre una connessione
  *verso* Cloudflare, che poi mostra il sito su un indirizzo pubblico. Gratis
  nel piano base, più robusto di Funnel, e in più permette di mettere davanti
  una schermata di accesso (Cloudflare Access) — utile finché il login nostro
  non esiste. Richiede un account Cloudflare; con un dominio proprio si ottiene
  un indirizzo decente, senza dominio l'indirizzo è un `*.trycloudflare.com`
  che cambia ogni volta.

**ngrok** fa la stessa cosa ed è il più conosciuto, ma nel piano gratuito
l'indirizzo cambia a ogni riavvio: per una prova di mezz'ora va bene, per
qualcosa che deve restare acceso no.

Consiglio: **Tailscale Funnel per le prime prove** (è già installato, zero
account nuovi), **Cloudflare Tunnel** quando la web app inizia a servire
davvero a qualcuno.

Una cosa da dire chiaramente: nel momento in cui l'app è raggiungibile da
internet, chiunque trovi l'indirizzo arriva alla porta di casa. Prima di
quel passo serve il login fatto per bene, e un limite ai tentativi di
accesso.

## 5. L'S9 basta ancora?

Oggi: sì, e regge meglio di quanto sembri. Compila Rust in un paio di minuti,
gira i 475 test in sette, tiene il bot con ~25 MB di memoria. Aggiungere un
server HTTP e delle pagine statiche non lo impensierisce: sono richieste
minuscole rispetto alla compilazione che già fa.

I limiti veri, in ordine di quanto sono probabili:

1. **Android uccide i processi.** Serve il wake-lock (c'è) e Termux:Boot per
   ripartire da solo dopo un riavvio del telefono. Il guardiano del bot,
   scritto il 25 settembre, copre il caso "il processo è morto" — e il 24
   settembre quel caso è costato un'ora e mezza di bot spento.
2. **È un telefono con una batteria**, tenuto in carica per mesi. Le batterie
   al litio in quelle condizioni si gonfiano. È un rischio fisico, non
   informatico, e cresce col tempo.
3. **Un aggiornamento di Android o un riavvio imprevisto** fermano tutto finché
   qualcuno non guarda.
4. **Storage**: 2 MB di database e qualche centinaio di MB di build. Non è un
   problema oggi.

Conclusione onesta: l'S9 va bene per **te, adesso**. Non è la macchina su cui
far arrivare estranei, e non è la macchina su cui lasciare i dati di casa come
unica copia (i backup girano già, ed è la cosa che conta di più).

## 6. Spostarlo su un PC-server: facile, e già possibile oggi

È la domanda dove le scelte fatte finora pagano. Il gestionale è **un solo
file eseguibile più un file SQLite**: niente Docker, niente database server,
niente servizi esterni. Trasferirlo vuol dire:

1. clonare il repository sul PC nuovo;
2. `cargo build --release`;
3. copiare il database **con `sqlite3 .backup`**, non con una copia del file —
   ora che gira in WAL, una copia grezza può perdere le ultime transazioni;
4. rimettere il file dei segreti (il token del bot) a mano, perché in Git non
   c'è;
5. accendere.

Un PC-server risolve da solo i punti 1, 2 e 3 dei limiti dell'S9. Se il PC sta
in casa, Tailscale continua a funzionare uguale. Se un giorno si volesse un
server affittato, cambia solo dove gira: il resto no.

## 7. Ha senso Rust?

Sì, e non perché sia di moda.

**Cosa dà, qui:** un eseguibile unico senza niente da installare intorno
(decisivo su un telefono), consumi bassissimi, e un compilatore che ha
impedito una quantità di errori che non si vedono perché non sono mai
arrivati. Le 475 prove automatiche girano in sette minuti **sul telefono**.

**Cosa costa:** la compilazione sull'S9 è lenta (un paio di minuti per una
build, sette per i test); e per la parte web ci sono meno pezzi pronti che in
Python o JavaScript — anche se `axum` è maturo e fa esattamente quello che
serve qui.

**Dove Rust non ha colpe:** nessuno dei difetti di questi giorni viene dal
linguaggio. Uno solo ha una forma legata a Rust — il `panic` che ha ucciso il
bot — e anche quello nasce da una funzione chiamata nel posto sbagliato, non
dal linguaggio.

Riscrivere in un altro linguaggio vorrebbe dire buttare 475 prove e rifare da
zero la logica che è costata più fatica. **Non conviene.** La pagina web si
scrive in HTML, CSS e un po' di JavaScript **sopra** questo nucleo, che resta
dov'è.

## 8. Provare con un secondo account: il vero argomento a favore della web app

Questo punto merita attenzione, perché è il più concreto di tutti.

Il gestionale ha già dentro **spazi condivisi, inviti, permessi e ruoli**, ed è
la parte **meno collaudata di tutte**, perché per provarla servono due persone
vere. Con Telegram un secondo account richiede **un secondo numero di
telefono**: Telegram non apre account senza numero. Ecco perché quelle
funzioni sono ferme a "collaudo pendente" da settembre.

Con un accesso via email il problema sparisce: una seconda email è gratis e
immediata, e da lì si prova tutto — invita, accetta, revoca, cambia permessi,
vedi cosa vede l'altro. Se un giorno si volesse anche il numero di telefono
(per recuperare la password o per la verifica in due passaggi), si aggiunge
dopo: è una colonna e un servizio di invio SMS, non un'architettura diversa.

**Quindi**: la web app non serve solo a fare schermate più belle. Serve a poter
collaudare metà del gestionale che oggi non si riesce a collaudare.

## 9. Cosa consiglio, in ordine

1. **Prima il nucleo, poi l'interfaccia.** Il punto 15 dei Miglioramenti
   (semplificare la riconciliazione delle aggiunte) va fatto **prima** di
   aprire il cantiere web. Mettere un'interfaccia nuova su una parte che si è
   rotta tre volte in tre giorni vuol dire cercare i bug in due posti invece
   che in uno.
2. **Chiudere i difetti aperti** del collaudo (id 16–19): sono contenuti, e due
   riguardano i dati.
3. **Poi l'interfaccia HTTP** sul nucleo esistente, partendo da **una sola
   schermata** — la lista della spesa, che è quella che usi di più e quella
   che soffre più i limiti di Telegram. Una schermata sola dice subito se la
   strada è giusta, senza aver speso settimane.
4. **Accesso**: per provare, Tailscale (già c'è) e poi Tailscale Funnel. Il
   login con email quando la web app smette di essere un esperimento.
5. **L'S9 resta** finché non c'è la web app. Il passaggio a un PC-server ha
   senso nello stesso momento in cui l'app diventa raggiungibile da fuori.
6. **Figma**: non ora.

## 10. E la mia parte

Va scritto, perché è la causa più frequente dei giri a vuoto di questi giorni:
il modo in cui ho lavorato è stato **correggere, poi verificare**. Tre volte su
tre, sul calcolo delle aggiunte, ho cambiato il comportamento e ho scritto la
prova *dopo*, e la prova passava perché era scritta sul codice nuovo invece che
sul caso reale di Alessio.

L'ordine giusto è l'inverso: **prima una prova automatica che riproduce
esattamente il caso del collaudo e che fallisce**, poi la correzione. Se la
prova non fallisce, non ho capito il difetto. Questa regola vale
indipendentemente da Telegram, da Rust e da qualunque interfaccia, e da sola
avrebbe evitato due dei tre giri.
