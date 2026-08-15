//! Infrastruttura trasversale dello storico - Step 6B.
//!
//! In questo sotto-step sono presenti le primitive di SCRITTURA realmente
//! usate da oggetti, foto e luoghi. Le API di lettura/paginazione verranno
//! aggiunte insieme alla UI Telegram dello storico, evitando codice morto.

use sqlx::SqliteConnection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewHistoryEvent<'a> {
    pub(crate) entita_storico_id: i64,
    pub(crate) modulo: &'a str,
    pub(crate) componente: &'a str,
    pub(crate) operazione: &'a str,
    pub(crate) nome_entita_snapshot: &'a str,
    pub(crate) abitazione_storico_id: Option<i64>,
    pub(crate) abitazione_nome_snapshot: Option<&'a str>,
    pub(crate) stanza_storico_id: Option<i64>,
    pub(crate) stanza_nome_snapshot: Option<&'a str>,
    pub(crate) evento_padre_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NewFieldChange {
    pub(crate) campo: &'static str,
    pub(crate) tipo_valore: &'static str,
    pub(crate) valore_prima: Option<String>,
    pub(crate) valore_dopo: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LocationSnapshot {
    pub(crate) abitazione_storico_id: Option<i64>,
    pub(crate) abitazione_nome: Option<String>,
    pub(crate) stanza_storico_id: Option<i64>,
    pub(crate) stanza_nome: Option<String>,
}

pub(crate) async fn ensure_entity(
    conn: &mut SqliteConnection,
    tipo_entita: &str,
    id_origine: i64,
    nome: &str,
) -> Result<i64, sqlx::Error> {
    if let Some(id) = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM storico_entita \
         WHERE tipo_entita = ? AND id_origine = ? AND eliminato_il IS NULL \
         ORDER BY id DESC LIMIT 1",
    )
    .bind(tipo_entita)
    .bind(id_origine)
    .fetch_optional(&mut *conn)
    .await?
    {
        sqlx::query(
            "UPDATE storico_entita SET nome_ultimo = ? \
             WHERE id = ? AND nome_ultimo <> ?",
        )
        .bind(nome)
        .bind(id)
        .bind(nome)
        .execute(&mut *conn)
        .await?;
        return Ok(id);
    }

    let result = sqlx::query(
        "INSERT INTO storico_entita (tipo_entita, id_origine, nome_ultimo) \
         VALUES (?, ?, ?)",
    )
    .bind(tipo_entita)
    .bind(id_origine)
    .bind(nome)
    .execute(&mut *conn)
    .await?;

    Ok(result.last_insert_rowid())
}

pub(crate) async fn rename_entity(
    conn: &mut SqliteConnection,
    storico_entita_id: i64,
    nuovo_nome: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE storico_entita \
         SET nome_ultimo = ? \
         WHERE id = ? AND eliminato_il IS NULL",
    )
    .bind(nuovo_nome)
    .bind(storico_entita_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) async fn mark_entity_deleted(
    conn: &mut SqliteConnection,
    storico_entita_id: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE storico_entita \
         SET eliminato_il = COALESCE(eliminato_il, CURRENT_TIMESTAMP) \
         WHERE id = ?",
    )
    .bind(storico_entita_id)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

pub(crate) async fn record_event(
    conn: &mut SqliteConnection,
    event: &NewHistoryEvent<'_>,
) -> Result<i64, sqlx::Error> {
    let result = sqlx::query(
        "INSERT INTO storico_eventi (\
            entita_storico_id, modulo, componente, operazione, \
            nome_entita_snapshot, \
            abitazione_storico_id, abitazione_nome_snapshot, \
            stanza_storico_id, stanza_nome_snapshot, evento_padre_id\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(event.entita_storico_id)
    .bind(event.modulo)
    .bind(event.componente)
    .bind(event.operazione)
    .bind(event.nome_entita_snapshot)
    .bind(event.abitazione_storico_id)
    .bind(event.abitazione_nome_snapshot)
    .bind(event.stanza_storico_id)
    .bind(event.stanza_nome_snapshot)
    .bind(event.evento_padre_id)
    .execute(&mut *conn)
    .await?;

    Ok(result.last_insert_rowid())
}

pub(crate) async fn record_field_changes(
    conn: &mut SqliteConnection,
    evento_id: i64,
    changes: &[NewFieldChange],
) -> Result<(), sqlx::Error> {
    for (index, change) in changes.iter().enumerate() {
        if change.valore_prima == change.valore_dopo {
            continue;
        }

        sqlx::query(
            "INSERT INTO storico_cambiamenti (\
                evento_id, campo, tipo_valore, valore_prima, valore_dopo, ordine\
             ) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(evento_id)
        .bind(change.campo)
        .bind(change.tipo_valore)
        .bind(change.valore_prima.as_deref())
        .bind(change.valore_dopo.as_deref())
        .bind(index as i64)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub(crate) async fn record_location_change(
    conn: &mut SqliteConnection,
    evento_id: i64,
    before: &LocationSnapshot,
    after: &LocationSnapshot,
) -> Result<(), sqlx::Error> {
    if before == after {
        return Ok(());
    }

    sqlx::query(
        "INSERT INTO storico_cambi_luogo (\
            evento_id, \
            abitazione_prima_id, abitazione_prima_nome, \
            stanza_prima_id, stanza_prima_nome, \
            abitazione_dopo_id, abitazione_dopo_nome, \
            stanza_dopo_id, stanza_dopo_nome\
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(evento_id)
    .bind(before.abitazione_storico_id)
    .bind(before.abitazione_nome.as_deref())
    .bind(before.stanza_storico_id)
    .bind(before.stanza_nome.as_deref())
    .bind(after.abitazione_storico_id)
    .bind(after.abitazione_nome.as_deref())
    .bind(after.stanza_storico_id)
    .bind(after.stanza_nome.as_deref())
    .execute(&mut *conn)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};

    async fn test_pool() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("database in memoria");

        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await
            .expect("foreign key");

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migration");

        pool
    }

    #[tokio::test]
    async fn migration_storico_non_inventa_eventi() {
        let pool = test_pool().await;
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM storico_eventi")
            .fetch_one(&pool)
            .await
            .expect("conteggio storico");
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn prima_dopo_viene_salvato_in_forma_strutturata() {
        let pool = test_pool().await;
        let mut conn = pool.acquire().await.expect("connessione");

        let entity_id = ensure_entity(&mut conn, "oggetto", 77, "Trapano")
            .await
            .expect("entita");

        let event_id = record_event(
            &mut conn,
            &NewHistoryEvent {
                entita_storico_id: entity_id,
                modulo: "oggetti",
                componente: "anagrafica",
                operazione: "modifica",
                nome_entita_snapshot: "Trapano",
                abitazione_storico_id: None,
                abitazione_nome_snapshot: None,
                stanza_storico_id: None,
                stanza_nome_snapshot: None,
                evento_padre_id: None,
            },
        )
        .await
        .expect("evento");

        record_field_changes(
            &mut conn,
            event_id,
            &[NewFieldChange {
                campo: "marca",
                tipo_valore: "testo",
                valore_prima: Some("Bosch".to_string()),
                valore_dopo: Some("Makita".to_string()),
            }],
        )
        .await
        .expect("cambiamento");

        let values: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT valore_prima, valore_dopo FROM storico_cambiamenti WHERE evento_id = ?",
        )
        .bind(event_id)
        .fetch_one(&mut *conn)
        .await
        .expect("lettura");

        assert_eq!(values.0.as_deref(), Some("Bosch"));
        assert_eq!(values.1.as_deref(), Some("Makita"));
    }
}
