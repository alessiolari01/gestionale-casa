#!/usr/bin/env bash
# Verifica lo stato reale della run CI piu' recente su un ramo, leggendo
# l'API di GitHub Actions -- non un riassunto locale.
#
# E' la stessa regola gia' in vigore per lo sviluppo umano (STATO.md, punto
# aperto 1): un esito locale verde non e' un lasciapassare, e' solo l'assenza
# di una brutta notizia. Il 2 settembre uno strumento ha riassunto "verde"
# dove la pagina della run diceva rosso, e su quella base era gia' stato
# consigliato il merge. Questo script legge l'API, non riassume nulla: lo fa
# leggere a chi lo chiama.
#
# Uso:
#   ./scripts/verifica-ci.sh [ramo] [--attendi] [--timeout SECONDI]
#
# Senza argomenti usa il ramo corrente. Il repository e' pubblico: non serve
# un token (limite 60 richieste/ora senza autenticazione, ampio per un
# controllo occasionale).
#
# --attendi fa ripetere il controllo ogni 20 secondi finche' la run non e'
# completata o scade il timeout (default 1200s = 20 minuti, come il timeout
# del job nel workflow).
#
# Uscita:
#   0  l'ultima run e' completata con successo
#   1  l'ultima run e' fallita/cancellata, o non ne esiste nessuna, o errore
#   2  l'ultima run e' ancora in corso (solo senza --attendi)

set -uo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.." || exit 1

REPO="alessiolari01/gestionale-casa"
RAMO=""
ATTENDI=0
TIMEOUT=1200

while [ $# -gt 0 ]; do
    case "$1" in
        --attendi)
            ATTENDI=1
            shift
            ;;
        --timeout)
            TIMEOUT="${2:-1200}"
            shift 2
            ;;
        *)
            RAMO="$1"
            shift
            ;;
    esac
done

if [ -z "$RAMO" ]; then
    RAMO="$(git rev-parse --abbrev-ref HEAD)"
fi

if [ -z "$RAMO" ] || [ "$RAMO" = "HEAD" ]; then
    echo "Impossibile determinare il ramo corrente; passalo come argomento." >&2
    exit 1
fi

PYTHON=""
for candidato in python3 python; do
    # Non basta che "command -v" lo trovi: su Windows "python3" puo' essere
    # uno stub del Microsoft Store che esiste sul PATH ma non esegue nulla
    # (esce con un errore invece di stampare la versione). Si verifica che
    # risponda davvero.
    if command -v "$candidato" >/dev/null 2>&1 && "$candidato" --version >/dev/null 2>&1; then
        PYTHON="$candidato"
        break
    fi
done
if [ -z "$PYTHON" ]; then
    echo "Serve python3 (o python) per leggere la risposta dell'API, non trovato o non funzionante." >&2
    exit 1
fi

# Lo script Python vive in un file temporaneo, non sullo stdin del comando:
# passare la risposta dell'API sullo stdin dello stesso comando che legge il
# proprio sorgente da stdin (`python - <<EOF`) la fa arrivare vuota, perche'
# lo stdin e' gia' consumato dal sorgente. Risposta e script viaggiano quindi
# come due file separati, passati come argomenti.
PY_SCRIPT="$(mktemp)"
trap 'rm -f "$PY_SCRIPT"' EXIT

cat > "$PY_SCRIPT" <<'PYEOF'
import json, sys

ramo, percorso_json = sys.argv[1], sys.argv[2]
with open(percorso_json, encoding="utf-8") as f:
    testo = f.read()
try:
    dati = json.loads(testo)
except json.JSONDecodeError as errore:
    print(f"Risposta dell'API non e' JSON valido: {errore}", file=sys.stderr)
    sys.exit(1)

run_totali = dati.get("workflow_runs", [])
if not run_totali:
    print(f"Nessuna run CI trovata per il ramo '{ramo}'.")
    sys.exit(1)

run = run_totali[0]
status = run.get("status")
conclusion = run.get("conclusion")
sha = run.get("head_sha", "")
html_url = run.get("html_url", "")
run_number = run.get("run_number")

print(f"Ramo:       {ramo}")
print(f"Run:        #{run_number}")
print(f"Commit:     {sha[:12]}")
print(f"Stato:      {status}")
print(f"Esito:      {conclusion or '(nessuno ancora)'}")
print(f"Pagina:     {html_url}")

if status == "completed":
    if conclusion == "success":
        print("OK CI verde.")
        sys.exit(0)
    else:
        print(f"NO CI non verde (esito: {conclusion}). Vedi la pagina della run, non un riassunto.")
        sys.exit(1)
elif status in ("queued", "in_progress", "waiting", "requested", "pending"):
    print("... CI ancora in corso.")
    sys.exit(2)
else:
    print(f"Stato non riconosciuto: {status}", file=sys.stderr)
    sys.exit(1)
PYEOF

controlla_una_volta() {
    local url="https://api.github.com/repos/${REPO}/actions/runs?branch=${RAMO}&per_page=1"
    local json_tmp
    json_tmp="$(mktemp)"
    if ! curl -sS -H "Accept: application/vnd.github+json" "$url" -o "$json_tmp"; then
        echo "Richiesta all'API GitHub fallita." >&2
        rm -f "$json_tmp"
        return 1
    fi
    if [ ! -s "$json_tmp" ]; then
        echo "Nessuna risposta dall'API GitHub." >&2
        rm -f "$json_tmp"
        return 1
    fi

    "$PYTHON" "$PY_SCRIPT" "$RAMO" "$json_tmp"
    local esito=$?
    rm -f "$json_tmp"
    return $esito
}

if [ "$ATTENDI" != "1" ]; then
    controlla_una_volta
    exit $?
fi

trascorso=0
intervallo=20
while true; do
    controlla_una_volta
    esito=$?
    if [ "$esito" != "2" ]; then
        exit "$esito"
    fi
    if [ "$trascorso" -ge "$TIMEOUT" ]; then
        echo "Timeout (${TIMEOUT}s) raggiunto con la CI ancora in corso." >&2
        exit 2
    fi
    sleep "$intervallo"
    trascorso=$((trascorso + intervallo))
    echo "--- ricontrollo dopo ${trascorso}s ---"
done
