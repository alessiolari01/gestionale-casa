#!/usr/bin/env python3
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path

EXCLUDED_DIRS = {
    ".git", "target", "data", ".idea", ".vscode", "__pycache__", "_handoff", "_project_handoff",
    ".pytest_cache", ".mypy_cache", ".ruff_cache", ".cache",
}
EXCLUDED_EXACT = {
    ".env", ".env.local", ".env.production", ".env.development",
}
EXCLUDED_SUFFIXES = {
    ".db", ".db-shm", ".db-wal", ".sqlite", ".sqlite3", ".log",
    ".zip", ".part", ".tmp", ".bak", ".backup", ".pyc",
}
TELEGRAM_TOKEN = re.compile(r"\b\d{6,12}:[A-Za-z0-9_-]{30,}\b")
PRIVATE_KEY_MARKERS = (
    "-----BEGIN " + "PRIVATE KEY-----",
    "-----BEGIN " + "RSA PRIVATE KEY-----",
    "-----BEGIN " + "OPENSSH PRIVATE KEY-----",
    "-----BEGIN " + "EC PRIVATE KEY-----",
)
OPENAI_KEY = re.compile(r"\bsk-[A-Za-z0-9_-]{20,}\b")


def run_git(root: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *args],
        cwd=root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        check=False,
    )
    return proc.stdout.strip()


def git_status_path(line: str) -> str:
    value = line[3:].strip() if len(line) >= 4 else line.strip()
    if " -> " in value:
        value = value.split(" -> ", 1)[1]
    return value.strip('"')


def is_status_noise(line: str) -> bool:
    path = git_status_path(line)
    lower = path.lower()
    if not path:
        return True
    return (
        ".pre_" in lower
        or lower.endswith("~")
        or "/__pycache__/" in f"/{lower}"
        or lower.endswith(".pyc")
        or lower.endswith(".part")
        or lower.endswith(".tmp")
        or lower.endswith(".bak")
        or lower.endswith(".backup")
    )


def filtered_git_status(status: str) -> str:
    return "\n".join(
        line for line in status.splitlines() if line.strip() and not is_status_noise(line)
    )


def should_skip(relative: Path) -> bool:
    if any(part in EXCLUDED_DIRS for part in relative.parts):
        return True
    lowered_parts = {part.lower() for part in relative.parts}
    if "backup" in lowered_parts or "backups" in lowered_parts:
        return True
    name = relative.name
    lower = name.lower()
    if lower in EXCLUDED_EXACT or lower.startswith(".env."):
        return True
    if lower in {"id_rsa", "id_ed25519", "credentials.json", "secrets.json", "secrets.toml"}:
        return True
    if lower.endswith((".pem", ".key", ".p12", ".pfx")):
        return True
    if any(lower.endswith(suffix) for suffix in EXCLUDED_SUFFIXES):
        return True
    if ".pre_" in lower or lower.endswith("~"):
        return True
    if lower.startswith("gestionale-casa_step") and lower.endswith(".zip"):
        return True
    return False


def looks_textual(path: Path) -> bool:
    return path.suffix.lower() in {
        ".rs", ".toml", ".lock", ".md", ".txt", ".py", ".sh", ".sql",
        ".yml", ".yaml", ".json", ".gitignore", ".gitattributes",
    } or path.name in {"README", "LICENSE", "Dockerfile", "Makefile"}


def assert_no_obvious_secret(path: Path) -> None:
    if not looks_textual(path):
        return
    try:
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return
    if TELEGRAM_TOKEN.search(text):
        raise RuntimeError(f"Possibile token Telegram rilevato in {path}")
    if OPENAI_KEY.search(text):
        raise RuntimeError(f"Possibile API key rilevata in {path}")
    if any(marker in text for marker in PRIVATE_KEY_MARKERS):
        raise RuntimeError(f"Possibile chiave privata rilevata in {path}")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def project_overview(branch: str, head: str, dirty: bool) -> str:
    return f"""# Esportazione tecnica gestionale-casa

Questo archivio è pensato per consegnare il progetto a una persona o a una nuova chat che non lo ha mai visto.

## Stato Git
- Branch: `{branch or '-'}`
- HEAD: `{head or '-'}`
- Working tree: {'con modifiche locali' if dirty else 'pulito'}

## Da leggere per primo
1. `_project_handoff/CURRENT_STATE.md` — fotografia generata automaticamente dall'export corrente;
2. `README.md`;
3. `ARCHITETTURA.md`;
4. `docs/HANDOFF_COMPLETO.md` — checkpoint documentale, che può essere meno recente del working tree fino alla chiusura formale dello step;
5. `docs/ROADMAP.md`;
6. `docs/step7/roadmap.md`;
7. `migrations/README.md`;
8. `src/main.rs`.

## Cosa contiene
- sorgenti Rust (`src/`);
- migration SQLite/SQLx (`migrations/`);
- documentazione (`docs/`, README, architettura e changelog);
- script operativi non sensibili (`scripts/`);
- configurazione build/CI (`Cargo.toml`, `Cargo.lock`, `.github/`, `build.rs`);
- manifest, stato corrente e albero dei file generati al momento dell'export.

## Cosa NON contiene
- `.env` e varianti;
- token, password o chiavi private;
- database SQLite e WAL/SHM;
- `data/`, allegati utente e file runtime;
- backup;
- `target/`;
- repository `.git/`;
- ZIP temporanei e cache.
"""


def current_state(root: Path, branch: str, head: str, status: str, log10: str) -> str:
    migrations = sorted((root / "migrations").glob("*.sql"))
    modules = sorted((root / "src" / "modules").glob("*.rs"))
    latest_migration = migrations[-1].name if migrations else "-"
    status_block = status if status else "(working tree pulito)"
    modules_text = "\n".join(f"- `{path.name}`" for path in modules) or "- (nessun modulo trovato)"
    log_block = log10 or "(nessun log disponibile)"
    return f"""# Stato corrente generato automaticamente

Generato: {datetime.now(timezone.utc).isoformat(timespec='seconds')}

Questo file descrive la fotografia **reale del repository al momento dell'export**.
Ha priorità sul checkpoint `docs/HANDOFF_COMPLETO.md` quando il working tree contiene modifiche non ancora documentate/committate.

## Git
- Branch: `{branch or '-'}`
- HEAD: `{head or '-'}`
- Working tree: {'con modifiche locali rilevanti' if status else 'pulito'}

### Modifiche locali rilevanti
```text
{status_block}
```

I file tecnici temporanei (`.pre_*`, backup locali, cache e simili) vengono esclusi da questo riepilogo.

## Migration
- File migration SQL presenti: **{len(migrations)}**
- Migration più recente nel repository: `{latest_migration}`

Nota: il numero di migration *applicate nel DB runtime* non viene dedotto da questo export perché il database è intenzionalmente escluso.

## Moduli Rust in `src/modules/`
{modules_text}

## Ultimi commit visibili
```text
{log_block}
```

## Regola di lettura
Per capire lo stato corrente usa questo file insieme a `GIT_MANIFEST.json`, `FILE_MANIFEST.json` e al codice esportato.
Dopo il collaudo e la chiusura formale dello step, `docs/HANDOFF_COMPLETO.md` deve essere riallineato e il repository committato/pushato.
"""


def export_bundle(root: Path, output_dir: Path) -> tuple[Path, int]:
    if not (root / ".git").exists():
        raise RuntimeError(f"Repository Git non trovato: {root}")

    output_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output = output_dir / f"gestionale-casa_handoff_progetto_{stamp}.zip"
    partial = output.with_suffix(".zip.part")
    partial.unlink(missing_ok=True)

    branch = run_git(root, "branch", "--show-current")
    head = run_git(root, "rev-parse", "--short", "HEAD")
    raw_status = run_git(root, "status", "--short")
    status = filtered_git_status(raw_status)
    log10 = run_git(root, "log", "-10", "--oneline", "--decorate")

    files: list[tuple[Path, Path]] = []
    manifest_files: list[dict] = []
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(root)
        if should_skip(relative):
            continue
        assert_no_obvious_secret(path)
        files.append((path, relative))
        manifest_files.append(
            {
                "path": relative.as_posix(),
                "size_bytes": path.stat().st_size,
                "sha256": sha256(path),
            }
        )

    tree = "\n".join(item["path"] for item in manifest_files)
    metadata = {
        "creato_il": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "branch": branch,
        "head": head,
        "working_tree_dirty": bool(status),
        "git_status_short": status,
        "git_log_10": log10,
        "file_count": len(files),
    }
    exclusions = """# Esclusioni di sicurezza

L'export tecnico esclude intenzionalmente:
- `.env` e file equivalenti;
- database e file SQLite;
- `data/` e contenuti utente/runtime;
- backup e file temporanei;
- `.git/` e `target/`;
- ZIP e cache.

Prima di creare lo ZIP viene inoltre eseguito un controllo conservativo sui file testuali inclusi.
Se viene rilevato un pattern compatibile con token Telegram, API key o chiave privata, l'esportazione fallisce invece di inviare il file.
"""

    try:
        with zipfile.ZipFile(
            partial, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6
        ) as archive:
            for source, relative in files:
                archive.write(source, Path("gestionale-casa") / relative)

            handoff = Path("gestionale-casa/_project_handoff")
            archive.writestr(
                str(handoff / "PROJECT_OVERVIEW.md"),
                project_overview(branch, head, bool(status)),
            )
            archive.writestr(
                str(handoff / "CURRENT_STATE.md"),
                current_state(root, branch, head, status, log10),
            )
            archive.writestr(
                str(handoff / "GIT_MANIFEST.json"),
                json.dumps(metadata, indent=2, ensure_ascii=False),
            )
            archive.writestr(
                str(handoff / "FILE_MANIFEST.json"),
                json.dumps(manifest_files, indent=2, ensure_ascii=False),
            )
            archive.writestr(str(handoff / "TREE.txt"), tree)
            archive.writestr(str(handoff / "SENSITIVE_EXCLUSIONS.md"), exclusions)

        partial.replace(output)
    except Exception:
        partial.unlink(missing_ok=True)
        output.unlink(missing_ok=True)
        raise

    return output.resolve(), len(files)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    try:
        path, files = export_bundle(
            args.root.expanduser().resolve(),
            args.output_dir.expanduser().resolve(),
        )
    except Exception as error:
        print(f"ERROR={error}", file=sys.stderr)
        return 1

    print(f"EXPORT_PATH={path}")
    print(f"FILES={files}")
    print(f"SIZE_BYTES={path.stat().st_size}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
