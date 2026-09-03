#!/usr/bin/env bash
# Controlla che i documenti restino coerenti fra loro.
#
# Nasce da un errore vero: il riordino dei documenti del 2 settembre 2026 e'
# stato committato da Windows, che non distingue le maiuscole nei nomi dei
# file. `docs/infrastruttura.md` e' finito nell'albero come
# `docs/INFRASTRUTTURA.md`, e quattro documenti hanno continuato a rimandare al
# nome minuscolo: su Linux, cioe' su GitHub e nella CI, erano rimandi rotti.
# Nessun controllo se ne e' accorto perche' la verifica era stata fatta sulla
# copia di lavoro invece che sull'albero committato.
#
# Gira in CI su ogni push. Non serve rete e non serve cargo.

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

# Su alcune installazioni Windows "python3" esiste sul PATH ma e' lo stub
# del Microsoft Store, che esce con un errore invece di eseguire: non basta
# che ci sia, deve rispondere davvero. Su Termux e sulla CI (Linux) il primo
# "python3" e' quello vero e la verifica passa al primo giro.
PYTHON=""
for candidato in python3 python; do
    if command -v "$candidato" >/dev/null 2>&1 && "$candidato" --version >/dev/null 2>&1; then
        PYTHON="$candidato"
        break
    fi
done
if [ -z "$PYTHON" ]; then
    echo "python3 (o python) non trovato o non funzionante." >&2
    exit 1
fi

"$PYTHON" - <<'PYTHON'
import posixpath, re, subprocess, sys

# `git ls-files` restituisce sempre percorsi con "/", su qualunque sistema
# operativo. `os.path` invece e' nativo della macchina: su Windows diventa
# `ntpath`, che normalizza con "\" — cosi' un percorso valido come
# "docs/x.md" smette di combaciare con se stesso dopo `normpath` e ogni
# rimando sembra rotto, anche quelli mai toccati. Qui si usa sempre
# `posixpath`, indipendentemente dal sistema operativo su cui gira lo
# script (Termux, Linux della CI, o Windows in locale).

def tracciati():
    out = subprocess.run(["git", "ls-files"], capture_output=True, text=True)
    return [r for r in out.stdout.splitlines() if r]

file_git = tracciati()
if not file_git:
    print("nessun file tracciato: controllo saltato")
    sys.exit(0)

problemi = []

# 1. Percorsi che differiscono solo per maiuscole/minuscole.
#    Su Windows e macOS sono lo stesso file, su Linux sono due: un albero che
#    li contiene entrambi si comporta in modo diverso a seconda della macchina.
visti = {}
for percorso in file_git:
    chiave = percorso.lower()
    if chiave in visti and visti[chiave] != percorso:
        problemi.append(f"due percorsi differiscono solo per maiuscole: {visti[chiave]} e {percorso}")
    visti[chiave] = percorso

# 2. Rimandi a file inesistenti dentro i documenti del presente.
#    Il CHANGELOG e' escluso: e' un verbale, e le sue voci vecchie citano
#    percorsi dell'epoca.
esistenti = set(file_git)
esclusi = {"CHANGELOG.md"}
riferimento = re.compile(r"`([A-Za-z0-9_./-]+\.(?:md|rs|sql|sh|toml|yml))`|\]\(([^)\s]+\.md)\)")

for percorso in file_git:
    if not percorso.endswith(".md") or percorso in esclusi:
        continue
    cartella = posixpath.dirname(percorso)
    with open(percorso, encoding="utf-8", errors="replace") as f:
        for numero, riga in enumerate(f, 1):
            for trovato in riferimento.finditer(riga):
                ref = trovato.group(1) or trovato.group(2)
                if not ref or ref.startswith(("http", "_project_handoff/")):
                    continue
                if "/" not in ref:
                    continue          # nomi nudi: possono essere generici
                candidati = {
                    posixpath.normpath(ref),
                    posixpath.normpath(posixpath.join(cartella, ref)),
                }
                if not (candidati & esistenti):
                    problemi.append(f"{percorso}:{numero} rimanda a `{ref}`, che non esiste")

if problemi:
    print("Documenti incoerenti:\n")
    for p in problemi:
        print(f"  - {p}")
    print(f"\n{len(problemi)} problemi. Vedi la sezione 0 di STATO.md.")
    sys.exit(1)

print(f"documenti coerenti: {sum(1 for p in file_git if p.endswith('.md'))} file, nessun rimando rotto")
PYTHON
