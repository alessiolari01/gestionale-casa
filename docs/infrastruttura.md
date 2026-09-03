# Infrastruttura operativa e comunicazione

Questo documento descrive **come comunicano tra loro PC, Galaxy S9, GitHub e
Telegram** e quale ruolo ha ogni componente. Non contiene chiavi private,
token o password.

## 0. PC fisso vs portatile — dove gira l'agente oggi

`docs/previsto/automazione-ciclo-sviluppo.md` distingue un **PC fisso** (dove
girerebbe la sessione che scrive/committa/collauda) da un **portatile** (solo
controllo remoto, non parla con l'S9). Quel PC fisso **non esiste ancora**.

**Verificato il 3 settembre 2026**: la sessione Claude Code che scrive questo
codice gira sul portatile stesso, `galaxybookalessio` — lo stesso dispositivo
di cui parla la sezione 2 qui sotto, con Tailscale e la chiave SSH verso l'S9
già configurate e funzionanti (`ssh s9` risponde). Per ora il portatile fa da
host dell'automazione. Il giorno in cui arriva un PC fisso vero, la migrazione
è a basso costo: clonare il repository, installare Rust/GCC e Tailscale,
generare **una chiave SSH dedicata e nuova** per quel PC (non copiare quella
del portatile) e autorizzarla sull'S9 — nessuna modifica strutturale al
progetto.

## 1. Topologia attuale

```text
PC Windows (galaxybookalessio)
        |
        | Tailscale + OpenSSH / SCP
        | alias: ssh s9
        v
Galaxy S9 / Termux (galaxy-s9-di-alessio)
        |
        +---- Git via SSH ----> GitHub
        |                      repo ufficiale
        |                      alessiolari01/gestionale-casa
        |
        +---- HTTPS long polling ----> Telegram Bot API
```

GitHub resta la **fonte ufficiale del codice e della cronologia**. Il collegamento
SSH PC ↔ S9 serve per sviluppo, trasferimento patch e test runtime; non sostituisce
Git/GitHub come fonte di verità.

## 2. PC Windows ↔ Galaxy S9: Tailscale + OpenSSH

Tailscale è installato e autenticato sugli stessi dispositivi:

- PC: `galaxybookalessio`;
- S9: `galaxy-s9-di-alessio`.

Il vantaggio è che non serve più conoscere o aggiornare l'IP LAN del telefono.
Quando PC e S9 sono sulla stessa rete, Tailscale può usare automaticamente il
percorso diretto LAN; quando sono su reti diverse continua a fornire il nome/IP
della rete privata Tailscale.

Sul PC il comando Tailscale non è attualmente nel `PATH`, quindi per diagnosi si
usa:

```powershell
& "C:\Program Files\Tailscale\tailscale.exe" status
& "C:\Program Files\Tailscale\tailscale.exe" ping galaxy-s9-di-alessio
```

### Configurazione SSH del PC

Nel file SSH del PC è presente un alias dedicato all'S9:

```text
Host s9
    HostName galaxy-s9-di-alessio
    User u0_a266
    Port 8022
    IdentityFile ~/.ssh/id_ed25519_s9
    IdentitiesOnly yes
```

La chiave privata resta **solo sul PC** e non deve essere aggiunta al repository.
La relativa chiave pubblica è autorizzata in Termux tramite
`~/.ssh/authorized_keys`.

Uso normale:

```powershell
ssh s9
```

Non viene richiesta la password SSH.

Trasferimento PC → S9:

```powershell
scp "C:\percorso\file.zip" s9:~/
```

Trasferimento S9 → PC:

```powershell
scp s9:~/file.zip "$HOME\Downloads\"
```

Questa è la via preferita per trasferire ZIP di patch, snapshot e file di test
senza usare GitHub come semplice mezzo di trasporto.

## 3. Server SSH sul Galaxy S9

Termux esegue OpenSSH sulla porta `8022` tramite:

```bash
sshd
```

È stato verificato anche l'uso con schermo spento. Prima di sessioni lunghe di
build/test/runtime è utile mantenere il wake lock:

```bash
termux-wake-lock
```

Per rilasciarlo:

```bash
termux-wake-unlock
```

### Avvio automatico

Termux:Boot è configurato per avviare il wake lock e `sshd` dopo un riavvio del
telefono. Il comportamento osservato sul Galaxy S9 è:

1. Android/Tailscale tornano raggiungibili;
2. Samsung/Android può attendere alcuni minuti prima di avviare Termux:Boot;
3. una volta eseguito Termux:Boot, wake lock e `sshd` risultano pronti in circa
   un secondo.

Il ritardo dopo il reboot è quindi un comportamento dell'avvio Android, non un
problema del server SSH. Per l'uso corrente è accettato aspettare che il servizio
diventi disponibile.

## 4. Galaxy S9 ↔ GitHub: Git via SSH, senza PAT

Il repository sull'S9 usa il remote SSH:

```text
git@github.com:alessiolari01/gestionale-casa.git
```

Sul telefono è presente una chiave GitHub dedicata:

```text
~/.ssh/id_ed25519_github
~/.ssh/id_ed25519_github.pub
```

La chiave pubblica è registrata nell'account GitHub; la privata resta sul
telefono e non va mai copiata nel repository.

Controlli utili:

```bash
ssh -T git@github.com
git remote -v
git fetch origin
```

Il `git push` dall'S9 non richiede più un Personal Access Token.

## 5. GitHub come fonte di verità

Repository ufficiale:

```text
https://github.com/alessiolari01/gestionale-casa
```

Branch ufficiale stabile:

```text
main
```

Gli step in sviluppo usano branch dedicati, con un nome che dice di cosa si
occupano.

Regola operativa:

- GitHub conserva la cronologia ufficiale;
- S9 e PC devono partire da un branch noto e aggiornato;
- usare `git pull --ff-only` per riallinearsi;
- evitare modifiche contemporanee agli stessi file su PC e S9;
- gli ZIP sono patch/snapshot, non sostituiscono la cronologia Git.

## 6. Workflow di sviluppo consigliato

Per uno step o una correzione normale:

1. verificare branch, HEAD e working tree;
2. preparare/modificare i file;
3. trasferire l'eventuale ZIP sul Galaxy S9 con `scp ... s9:~/`;
4. applicare la patch sull'S9;
5. eseguire `fmt`, `check`, test e Clippy;
6. eseguire il bot realmente sull'S9 e verificare Telegram;
7. solo dopo i test fare stage/commit/push;
8. controllare GitHub/CI e successivamente riallineare l'altro dispositivo con
   `git pull --ff-only`.

Comandi di controllo standard:

```bash
git status
git log -1 --oneline
cargo fmt --all
cargo fmt --all -- --check
cargo check --locked

CARGO_BUILD_JOBS=1 \
CARGO_PROFILE_TEST_DEBUG=0 \
cargo test --locked -- --test-threads=1

cargo clippy --all-targets --locked -- -D warnings
git diff --check
```

Il Galaxy S9 può mostrare sporadici `Segmentation fault` del linker LLVM/`cc`
per pressione di memoria. Non va confuso automaticamente con un errore Rust del
progetto. Per il runtime è già stato utile:

```bash
CARGO_BUILD_JOBS=1 \
CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_INCREMENTAL=0 \
cargo run --locked
```

## 7. Backend ↔ Telegram

Il bot usa Teloxide e comunica con Telegram tramite **HTTPS long polling in
uscita**. Non serve pubblicare una porta del Galaxy S9 su Internet e non serve
port forwarding del router.

`ALLOWED_CHAT_IDS` resta un bootstrap/meccanismo di emergenza. L'accesso ordinario è gestito dal database: un account Telegram sconosciuto può inviare una richiesta e soltanto l'amministratore principale può approvarla o rifiutarla.

## 8. Segreti e dati che non devono entrare in Git

Non committare mai:

- `.env` reale;
- `TELOXIDE_TOKEN`;
- chiavi SSH private;
- password;
- PAT GitHub;
- database reale sotto `data/`;
- foto/documenti personali reali;
- copie di `authorized_keys` contenenti chiavi operative se non intenzionalmente
  sanitizzate.

Le configurazioni documentate devono mostrare solo nomi file, host, porte e
procedure, mai il contenuto delle credenziali.

## 9. Diagnostica rapida

Dal PC, verificare prima Tailscale:

```powershell
& "C:\Program Files\Tailscale\tailscale.exe" ping galaxy-s9-di-alessio
```

Poi SSH:

```powershell
ssh s9
```

Se Tailscale risponde ma SSH rifiuta/non accetta ancora la connessione dopo un
riavvio, attendere l'avvio di Termux:Boot e verificare sull'S9:

```bash
pgrep -a sshd
```

Per GitHub dall'S9:

```bash
ssh -T git@github.com
git remote -v
git status
```

Questa separazione permette di capire rapidamente se un problema riguarda
**rete Tailscale**, **server SSH**, **Git/GitHub** oppure **backend Telegram**.

## 10. Runtime UI, shutdown ed export amministrativo

Dal blocco 7.2G.5 il bot persiste in SQLite il `message_id` della schermata UI principale per ogni chat. Questo permette di mantenere la UI a schermata singola anche attraverso riavvii: allo shutdown resta una schermata offline amministrativa e al successivo startup quella schermata viene sostituita/ripulita.

L'amministratore principale può arrestare il dispatcher da:

```text
🛠️ Amministrazione
→ ⏻ Spegni gestionale
→ ⏻ Conferma spegnimento
```

Il percorso è equivalente allo shutdown controllato via `Ctrl+C`; non vanno avviati due runtime long-polling con lo stesso token Telegram.

Dal 7.2G.6 il progetto contiene inoltre `scripts/export_miglioramenti.py`. L'export normale non richiede più SCP:

```text
💡 Miglioramenti
→ 📦 Esporta miglioramenti
→ download documento Telegram
→ ✅ Ho scaricato il file
```

La copia temporanea vive esclusivamente sotto `data/tmp/miglioramenti_export/` e viene cancellata dopo conferma; gli orfani più vecchi di 24 ore sono ripuliti automaticamente. Lo ZIP esclude segreti, database completo, `.git`, `target`, backup e runtime non necessario.
