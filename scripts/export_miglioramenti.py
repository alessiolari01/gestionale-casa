#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sqlite3
import subprocess
import sys
import tempfile
import zipfile
from datetime import datetime, timezone
from pathlib import Path

EXCLUDED_DIRS = {
    ".git",
    "target",
    "data",
    ".idea",
    ".vscode",
    "__pycache__",
    "_handoff",
}


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


def resolve_database(root: Path) -> Path:
    db_path = root / "data/db/gestionale.db"
    env_path = root / ".env"
    if not env_path.exists():
        return db_path

    for raw in env_path.read_text(encoding="utf-8", errors="ignore").splitlines():
        raw = raw.strip()
        if not raw.startswith("DATABASE_URL="):
            continue
        value = raw.split("=", 1)[1].strip().strip('"').strip("'")
        if value.startswith("sqlite://"):
            value = value[len("sqlite://") :]
        value = value.split("?", 1)[0]
        candidate = Path(value)
        return candidate if candidate.is_absolute() else root / candidate
    return db_path


def should_skip_repo_file(relative: Path) -> bool:
    if any(part in EXCLUDED_DIRS for part in relative.parts):
        return True
    name = relative.name.lower()
    if name == ".env" or name.startswith(".env."):
        return True
    if name.endswith((".db", ".db-shm", ".db-wal", ".sqlite", ".sqlite3", ".log")):
        return True
    if "backup" in {part.lower() for part in relative.parts}:
        return True
    return False


def safe_user_rows(connection: sqlite3.Connection) -> list[dict]:
    tables = {
        row[0]
        for row in connection.execute(
            "SELECT name FROM sqlite_master WHERE type='table'"
        ).fetchall()
    }
    if "utenti" not in tables:
        return []
    columns = [
        row[1]
        for row in connection.execute('PRAGMA table_info("utenti")').fetchall()
    ]
    candidates = [
        "id",
        "nome_visualizzato",
        "nome",
        "ruolo_sistema",
        "amministratore_principale",
        "creato_il",
    ]
    safe = [column for column in candidates if column in columns]
    if not safe:
        return []
    quoted = ", ".join(f'"{column}"' for column in safe)
    cursor = connection.execute(f'SELECT {quoted} FROM "utenti" ORDER BY id')
    return [dict(row) for row in cursor.fetchall()]


def table_rows(connection: sqlite3.Connection, table: str) -> list[dict]:
    cursor = connection.execute(f'SELECT * FROM "{table}"')
    return [dict(row) for row in cursor.fetchall()]


def find_attachment(root: Path, raw: object) -> Path | None:
    if not isinstance(raw, str) or not raw.strip():
        return None
    path = Path(raw.strip())
    candidates = [path] if path.is_absolute() else [root / path, Path.home() / path]
    for candidate in candidates:
        try:
            if candidate.exists() and candidate.is_file():
                return candidate
        except OSError:
            continue
    return None


def build_summary(active: list[dict], archive: list[dict], attachment_count: int, branch: str, head: str, dirty: bool) -> str:
    def value(item: dict, key: str) -> str:
        raw = item.get(key)
        if raw is None:
            return "-"
        text = str(raw).replace("\n", " ").strip()
        return text or "-"

    lines = [
        "# Handoff Miglioramenti correnti",
        "",
        f"- Branch: `{branch}`",
        f"- HEAD: `{head}`",
        f"- Working tree: {'con modifiche locali' if dirty else 'pulito'}",
        f"- Miglioramenti attivi: **{len(active)}**",
        f"- Archivio: **{len(archive)}**",
        f"- Allegati locali copiati: **{attachment_count}**",
        "",
        "## Miglioramenti attivi",
        "",
    ]
    for item in active:
        lines.extend(
            [
                f"### #{value(item, 'id')} — {value(item, 'stato')}",
                f"- Descrizione: {value(item, 'descrizione')}",
                f"- Modulo: {value(item, 'modulo')}",
                f"- Autore utente ID: {value(item, 'autore_utente_id')}",
                f"- Letto admin: {value(item, 'letto_admin_il')}",
                f"- Verificato: {value(item, 'verificato_il')}",
                "",
            ]
        )
    lines.extend(["## Archivio", ""])
    for item in archive:
        lines.extend(
            [
                f"### Archivio #{value(item, 'id')}",
                f"- Origine: {value(item, 'miglioramento_origine_id')}",
                f"- Descrizione: {value(item, 'descrizione')}",
                f"- Completato: {value(item, 'completato_il')}",
                f"- Archiviato: {value(item, 'archiviato_il')}",
                "",
            ]
        )
    return "\n".join(lines)


def export_bundle(root: Path, output_dir: Path) -> tuple[Path, int, int, int]:
    if not (root / ".git").exists():
        raise RuntimeError(f"Repository Git non trovato: {root}")

    db_path = resolve_database(root)
    if not db_path.exists():
        raise RuntimeError(f"Database non trovato: {db_path}")

    output_dir.mkdir(parents=True, exist_ok=True)
    stamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output = output_dir / f"gestionale-casa_handoff_miglioramenti_{stamp}.zip"
    partial = output.with_suffix(".zip.part")
    partial.unlink(missing_ok=True)

    branch = run_git(root, "branch", "--show-current")
    head = run_git(root, "rev-parse", "--short", "HEAD")
    status = run_git(root, "status", "--short")
    log5 = run_git(root, "log", "-5", "--oneline", "--decorate")

    connection = sqlite3.connect(db_path, timeout=20)
    connection.row_factory = sqlite3.Row
    try:
        table_names = [
            row[0]
            for row in connection.execute(
                """
                SELECT name
                FROM sqlite_master
                WHERE type='table'
                  AND lower(name) LIKE '%miglior%'
                ORDER BY name
                """
            ).fetchall()
        ]
        exported = {table: table_rows(connection, table) for table in table_names}
        active = exported.get("miglioramenti", [])
        archive = exported.get("miglioramenti_archivio", [])
        users = safe_user_rows(connection)
        schema_rows = connection.execute(
            """
            SELECT type, name, sql
            FROM sqlite_master
            WHERE type IN ('table','index','trigger')
              AND lower(name) LIKE '%miglior%'
            ORDER BY type, name
            """
        ).fetchall()
    finally:
        connection.close()

    attachment_entries: list[tuple[Path, str, dict]] = []
    attachment_manifest: list[dict] = []
    used_names: set[str] = set()
    for table, rows in exported.items():
        for row_index, item in enumerate(rows, start=1):
            for key, raw_value in item.items():
                lowered = key.lower()
                if not any(token in lowered for token in ("percorso", "path", "file")):
                    continue
                source = find_attachment(root, raw_value)
                if source is None:
                    continue
                record_id = item.get("id", row_index)
                extension = source.suffix.lower()
                if len(extension) > 10 or any(ch not in ".abcdefghijklmnopqrstuvwxyz0123456789" for ch in extension):
                    extension = ""
                sequence = 1
                candidate = f"{record_id}_{key}_{sequence}{extension}"
                while f"{table}/{candidate}" in used_names:
                    sequence += 1
                    candidate = f"{record_id}_{key}_{sequence}{extension}"
                relative_archive = f"allegati/{table}/{candidate}"
                used_names.add(f"{table}/{candidate}")
                # Non esportare il percorso runtime originale: può contenere identificativi
                # Telegram o dettagli del filesystem non necessari a ChatGPT.
                item[key] = relative_archive
                metadata = {
                    "tabella": table,
                    "record_id": record_id,
                    "campo": key,
                    "copiato_in": relative_archive,
                }
                attachment_entries.append((source, relative_archive, metadata))
                attachment_manifest.append(metadata)

    manifest = {
        "creato_il": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "branch": branch,
        "head": head,
        "git_status_short": status,
        "git_log_5": log5,
    }
    schema_text = "\n\n".join(
        f"-- {row['type']}: {row['name']}\n{row['sql']};"
        for row in schema_rows
        if row["sql"]
    )
    summary = build_summary(active, archive, len(attachment_entries), branch, head, bool(status))

    try:
        with zipfile.ZipFile(partial, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=6) as archive_zip:
            # Snapshot repository corrente, inclusi file modificati/non tracciati.
            for path in root.rglob("*"):
                if not path.is_file():
                    continue
                relative = path.relative_to(root)
                if should_skip_repo_file(relative):
                    continue
                archive_zip.write(path, Path("gestionale-casa") / relative)

            handoff_prefix = Path("gestionale-casa/_handoff")
            archive_zip.writestr(
                str(handoff_prefix / "GIT_MANIFEST.json"),
                json.dumps(manifest, indent=2, ensure_ascii=False),
            )
            archive_zip.writestr(
                str(handoff_prefix / "utenti_riferimento.json"),
                json.dumps(users, indent=2, ensure_ascii=False, default=str),
            )
            for table, rows in exported.items():
                archive_zip.writestr(
                    str(handoff_prefix / f"{table}.json"),
                    json.dumps(rows, indent=2, ensure_ascii=False, default=str),
                )
            archive_zip.writestr(
                str(handoff_prefix / "MIGLIORAMENTI_SCHEMA.sql"), schema_text
            )
            archive_zip.writestr(
                str(handoff_prefix / "ALLEGATI_MANIFEST.json"),
                json.dumps(attachment_manifest, indent=2, ensure_ascii=False, default=str),
            )
            archive_zip.writestr(
                str(handoff_prefix / "MIGLIORAMENTI_RIEPILOGO.md"), summary
            )
            for source, relative_archive, _ in attachment_entries:
                archive_zip.write(source, handoff_prefix / relative_archive)
        partial.replace(output)
    except Exception:
        partial.unlink(missing_ok=True)
        output.unlink(missing_ok=True)
        raise

    return output.resolve(), len(active), len(archive), len(attachment_entries)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()

    try:
        path, active, archived, attachments = export_bundle(
            args.root.expanduser().resolve(), args.output_dir.expanduser()
        )
    except Exception as error:
        print(f"ERROR={error}", file=sys.stderr)
        return 1

    print(f"EXPORT_PATH={path}")
    print(f"ACTIVE={active}")
    print(f"ARCHIVED={archived}")
    print(f"ATTACHMENTS={attachments}")
    print(f"SIZE_BYTES={path.stat().st_size}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
