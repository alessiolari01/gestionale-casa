# Gestionale Casa — come lavorare a questo progetto

La fonte della verità è il repository, non questo documento. I documenti del progetto sono stati riordinati il 2 settembre 2026: 55 file ridotti a 29, un documento per domanda. Qui resta solo ciò che serve a una sessione Claude e che nel repository non ha senso scrivere.

## Prima cosa da fare

Leggi `STATO.md` nel repository. È l'unico documento del presente: test, migration, cosa è fatto, cosa è aperto. Non fidarti di questo file per i numeri.

La mappa completa dei documenti è nel `README.md`.

## Il prossimo lavoro

Applicare le convenzioni dell'interfaccia a Spazi e Profilo (punto 4 della parte 3 di `docs/convenzioni-telegram.md`), che è la regola C5 e la parte concettualmente più difficile: la coppia spazio predefinito / vista, due impostazioni indipendenti con nomi che si somigliano — una decide dove finiscono le cose nuove, l'altra cosa vedi — e nessuna schermata che le spieghi insieme. È la cosa più fraintendibile del gestionale.

Poi restano il menù principale (C12) e le date (C13), e dopo di quelli la lista della spesa, che è la prossima funzione vera della roadmap.

Fatto il 2 settembre: il blocco liste (C1, C6, C7) su alimenti, ricette, storico e miglioramenti, sul ramo `ux-liste`, in tre commit. È nato `src/modules/liste.rs`, che tiene paginazione e intestazioni per tutte le liste.

Da portarsi dietro nei prossimi blocchi: restano sei righe di paginazione scritte a mano in `spazi_membri`, `porzioni_profili`, `porzioni_ingredienti`, `profili_alimentari` (due) e `planner_alimentare`. Vanno convertite a `modules::liste` quando si tocca quel modulo.

## La lezione del 2 settembre: guardare prima di scrivere, e guardare di nuovo dopo

Il giro sul bot reale ha corretto il lavoro quattro volte, e ogni volta era una cosa che dal codice non si vedeva.

**Prima di scrivere.** La convenzione stessa era sbagliata:

- C1 diceva che nello Storico «Giorgia» fosse l'autore dell'evento. È invece il profilo su cui la porzione è stata modificata, cioè una cosa che sul pulsante c'era già. Seguendo la convenzione alla lettera si sarebbe aggiunto al pulsante ciò che c'era, continuando a non aggiungere ciò che mancava;
- due eventi che sembravano doppioni erano due modifiche opposte della stessa porzione (120% → 100% e 100% → 120%) fatte nello stesso minuto: data e ora da sole non li separano.

**Dopo aver consegnato**, il collaudo di Alessio e il giro successivo hanno trovato altre due cose:

- avevo messo nei documenti un esempio inventato («cercando barilla compare Amido di mais»), presentato come osservato. Era falso alla radice: Amido di mais non ha nessun prodotto associato. Cercando l'esempio vero è saltato fuori che la funzione era incompleta — la ricerca fa comparire un alimento per nome, alias o prodotto, e la spiegazione copriva solo i prodotti;
- lo 📜 Storico mostrava ancora la vecchia riga di paginazione (`1 / 21`, frecce nude) mentre le etichette erano già quelle nuove: nello storico ci sono due tastiere (`global_history_keyboard` per il menù principale, `history_list_keyboard` per un oggetto) e ne avevo convertita una sola. Il conteggio «sei posti» che avevo scritto era sbagliato: erano nove.

Le tre regole che ne escono, e valgono per i blocchi successivi:

1. **Non scrivere mai un esempio che non si è visto.** Se serve un esempio, prenderlo da un test o aprirlo sul bot. Un esempio inventato in un documento passa per una verifica già fatta, e costa più di un esempio mancante.
2. **Contare i punti da cambiare con grep, non a memoria**, e ricontrollare dopo la conversione che non ne resti nessuno: `grep -rn 'Pagina precedente\|"⬅️"\|{} / {}' --include=*.rs src/`
3. **Una primitiva condivisa va progettata perché ci entrino tutti.** La prima versione di `riga_paginazione` prendeva il totale delle voci e dava per scontate cinque voci per pagina, così i chiamanti che contano diversamente (sette per pagina, o pagine di testo) non potevano usarla e si sono tenuti la loro riga. Una primitiva che non entra dove serve non unifica niente.

Vale la pena rifare il giro sul bot per ogni blocco, sia prima sia dopo la consegna. Serve il login QR da telefono su Telegram Web nel pannello browser, e il pannello va anche autorizzato (`request_access` su web.telegram.org).

## La regola che governa tutto

Nessuna modifica è finita finché i documenti non la raccontano. Il metro: una persona che apre la cartella senza aver visto nessuna conversazione deve poter capire dove siamo e perché è fatto così. La tabella di cosa aggiornare è nella sezione 0 di `STATO.md`.

Due controlli automatici: `aggiorna-s9.sh` confronta i test dichiarati con quelli reali, e `scripts/controlla-documenti.sh` gira in CI e verifica rimandi rotti e percorsi che differiscono solo per maiuscole.

## Le due macchine

| | PC | S9 |
|---|---|---|
| shell | PowerShell | bash di Termux |
| percorso | `C:\Users\aless\Desktop\Gestionale_Casa_X_AI` | `~/gestionale-casa` |
| cosa ci si fa | si scrive, si committa, si pusha | si aggiorna, si collauda, gira il bot |

In PowerShell il `\` a fine riga non è una continuazione: i comandi lunghi vanno su una riga sola. Dare sempre blocchi interi da copiare.

Sull'S9: `./scripts/aggiorna-s9.sh --ramo <nome>`. Senza `--ramo` aggiorna solo il ramo su cui si trova già, e il collaudo gira verde sul codice di prima.

## Nota del 3 settembre 2026 — questo documento descrive Claude Desktop, non Claude Code

Il resto di questo file (a partire dalla sezione qui sotto) è stato scritto
per una sessione **Claude Desktop**, con container e VM separati dal PC. La
cartella del progetto si è spostata da lì a
`C:\Users\aless\Desktop\CLaude_Code_Workspce\Gestionale_Casa_X_AI`, e la
sessione che lavora qui ora è **Claude Code**, che non ha quella separazione:

- **legge e scrive direttamente** i file del repository reale sul PC — non
  esiste un "container" separato da cui generare patch;
- **esegue `git` direttamente sul repository**, `push` compreso: non serve
  `device_commit_files` né una patch da verificare con `md5sum`;
- **non ha un pannello browser** per Telegram Web: non può aprire il bot e
  guardarlo di persona. Il collaudo prima/dopo resta necessario (vedi sopra),
  ma lo fa Alessio — per iterare comunque su schermate vere, si può chiedere
  uno screenshot e leggerlo con lo strumento di lettura file;
- **compila in locale sul PC**, non in un container con una toolchain ferma:
  il 3 settembre sono stati installati `rustup` (toolchain
  `stable-x86_64-pc-windows-gnu`, allineata alla **1.98** della CI, non più
  alla 1.95) e un GCC minimale (WinLibs, via `winget`) per il linker. Il PATH
  utente li contiene entrambi in modo permanente, ma **le shell già aperte in
  questa sessione non lo rileggono**: va anteposto a mano ad ogni comando che
  usa `cargo`/`gcc`,
  `export PATH="$HOME/.cargo/bin:/c/Users/aless/AppData/Local/Microsoft/WinGet/Packages/BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe/mingw64/bin:$PATH"`,
  finché la sessione non viene riavviata. `--ignore-rust-version` non serve
  più con questa toolchain.

Quello che segue sotto resta valido per il **contenuto** (regola dei
documenti, tabella delle due macchine, trappole di Windows su CRLF e
maiuscole, la questione di sqlx 0.9.0): cambia solo *come* l'assistente ci
arriva.

## Cosa può e non può fare l'assistente

**Può:** leggere e scrivere file nella cartella collegata sul PC; eseguire git sul PC (status, diff, branch, commit) tramite la VM Linux dell'app desktop; compilare, clippy e test nel proprio container; usare Telegram Web nel pannello browser per collaudare il bot di persona (login con QR dal telefono, da rifare se il pannello viene chiuso).

**Non può:** `git push` (nessuna credenziale); compilare sul PC (Rust non installato lì, e la VM non può installarlo); avviare il bot in nessun ambiente che raggiunge, perché `api.telegram.org` è bloccato in uscita sia dal container sia dalla VM; scrivere dentro `.cargo/` e `.github/workflows/`, che sono protetti.

La cartella va collegata a ogni sessione nuova. Non è collegata da sola: serve `device_request_folder_access` su `C:\Users\aless\Desktop\Gestionale_Casa_X_AI`.

Compilare nel container richiede `--ignore-rust-version`. Il lockfile pinna `takecell 0.1.2`, che dichiara Rust 1.96, e il container ha la 1.95 e non può aggiornarsi (`static.rust-lang.org` è chiuso). Il flag non cambia niente nel repository ed è solo per la verifica locale.

Consegnare al PC si fa con una patch, non incollando base64. Il giro che funziona: si lavora nel container, `git diff --binary > patch`, poi `SendUserFile` + `device_commit_files` per portarla sul PC byte per byte, e si verifica con `md5sum` prima di applicarla. Passare il base64 nel testo della conversazione corrompe il file — provato, e la patch è arrivata rotta.

**Trappole di Windows, già costate due errori.** Il filesystem non distingue maiuscole e non ha il bit di esecuzione: `docs/infrastruttura.md` è finito nell'albero come `docs/INFRASTRUTTURA.md`, e uno script è stato committato non eseguibile. Quando si consegna da Windows, nome e permesso vanno verificati nell'indice di Git (`git ls-files -s`), non sul disco.

L'albero di lavoro su Windows è CRLF, i blob nel repository sono LF (`core.autocrlf` è attivo nel git di Windows, non in quello della VM). Una patch generata altrove non si applica finché i file toccati non vengono normalizzati a LF: allora la VM li vede identici ai blob e il diff resta solo quello vero.

La VM non può cancellare file senza un permesso esplicito, e git lascia `.git/index.lock` dopo ogni commit: il permesso va richiesto una volta per sessione — e va richiesto di nuovo se il collegamento cade a metà sessione, altrimenti quei lock bloccano i comandi git dell'utente.

## Toolchain

La CI usa `rustup default stable` ed è più recente sia dell'S9 sia dell'ambiente dell'assistente (rustc 1.95, che non può aggiornarsi perché `static.rust-lang.org` è chiuso). Quando gli esiti divergono ha ragione la CI. In pratica: `div_ceil` sugli interi con segno è ancora instabile sulla 1.95, e la divisione scritta a mano fa scattare `manual_div_ceil` sulla Clippy del runner — il conto si fa senza segno, dove nessuna delle due protesta.

## Note tecniche aperte

- `.gitattributes` contiene `* text=auto eol=lf` ma ha un BOM UTF-8 in testa, quindi Git non lo applica e il working tree su Windows resta in CRLF. Si vede anche a occhio: `scripts/controlla-documenti.sh` sul PC stampa righe `$'\r': command not found` e funziona lo stesso. Sistemarlo produrrebbe un diff di rinormalizzazione su tutto il repo: va fatto da solo, in un commit dedicato.
- `sqlx 0.9.0` (PR #6) è stata valutata e rinviata il 2 settembre, con il motivo scritto nei punti aperti di `STATO.md`. Non riaprirla senza leggerlo.
