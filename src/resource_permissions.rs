//! Fondazione trasversale dei permessi espliciti sulle risorse condivise.

use anyhow::{bail, Context, Result};
use sqlx::{FromRow, SqlitePool};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourcePermission {
    Edit,
    Manage,
}

impl ResourcePermission {
    fn flags(self) -> (i64, i64) {
        match self {
            Self::Edit => (1, 0),
            Self::Manage => (1, 1),
        }
    }
}

#[derive(Debug, Clone, FromRow)]
pub struct ResourceInvite {
    pub resource_type: String,
    pub resource_id: i64,
    pub invited_user_id: i64,
    pub created_by_user_id: i64,
    pub can_edit: i64,
    pub can_manage: i64,
}

pub async fn has_edit_permission(
    pool: &SqlitePool,
    resource_type: &str,
    resource_id: i64,
    user_id: i64,
) -> Result<bool> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM permessi_risorsa \
            WHERE tipo_risorsa = ? AND risorsa_id = ? AND utente_id = ? \
              AND puo_modificare = 1\
         )",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il permesso esplicito di modifica")?;
    Ok(allowed)
}

pub async fn has_manage_permission(
    pool: &SqlitePool,
    resource_type: &str,
    resource_id: i64,
    user_id: i64,
) -> Result<bool> {
    let allowed: bool = sqlx::query_scalar(
        "SELECT EXISTS(\
            SELECT 1 FROM permessi_risorsa \
            WHERE tipo_risorsa = ? AND risorsa_id = ? AND utente_id = ? \
              AND puo_gestire_permessi = 1\
         )",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(user_id)
    .fetch_one(pool)
    .await
    .context("Impossibile verificare il permesso esplicito di gestione")?;
    Ok(allowed)
}

pub async fn create_invite(
    pool: &SqlitePool,
    resource_type: &str,
    resource_id: i64,
    invited_user_id: i64,
    created_by_user_id: i64,
    permission: ResourcePermission,
) -> Result<i64> {
    if invited_user_id == created_by_user_id {
        bail!("Non puoi invitare te stesso");
    }
    let (can_edit, can_manage) = permission.flags();
    let result = sqlx::query(
        "INSERT INTO inviti_risorsa (\
            tipo_risorsa, risorsa_id, invitato_utente_id, creato_da_utente_id, \
            puo_modificare, puo_gestire_permessi\
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(invited_user_id)
    .bind(created_by_user_id)
    .bind(can_edit)
    .bind(can_manage)
    .execute(pool)
    .await
    .context("Impossibile creare l'invito")?;
    Ok(result.last_insert_rowid())
}

pub async fn accept_invite(
    pool: &SqlitePool,
    invite_id: i64,
    accepting_user_id: i64,
) -> Result<ResourceInvite> {
    let mut tx = pool
        .begin()
        .await
        .context("Impossibile aprire la risposta invito")?;
    let invite = sqlx::query_as::<_, ResourceInvite>(
        "SELECT tipo_risorsa AS resource_type, risorsa_id AS resource_id, \
                invitato_utente_id AS invited_user_id, \
                creato_da_utente_id AS created_by_user_id, \
                puo_modificare AS can_edit, puo_gestire_permessi AS can_manage \
         FROM inviti_risorsa \
         WHERE id = ? AND invitato_utente_id = ? AND stato = 'pendente'",
    )
    .bind(invite_id)
    .bind(accepting_user_id)
    .fetch_optional(&mut *tx)
    .await
    .context("Impossibile leggere l'invito")?
    .context("Invito non disponibile o già gestito")?;

    sqlx::query(
        "INSERT INTO permessi_risorsa (\
            tipo_risorsa, risorsa_id, utente_id, puo_modificare, \
            puo_gestire_permessi, concesso_da_utente_id\
         ) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&invite.resource_type)
    .bind(invite.resource_id)
    .bind(invite.invited_user_id)
    .bind(invite.can_edit)
    .bind(invite.can_manage)
    .bind(invite.created_by_user_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile attivare il permesso")?;

    sqlx::query(
        "UPDATE inviti_risorsa \
         SET stato = 'accettato', risposto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND stato = 'pendente'",
    )
    .bind(invite_id)
    .execute(&mut *tx)
    .await
    .context("Impossibile chiudere l'invito")?;

    tx.commit()
        .await
        .context("Impossibile completare l'accettazione")?;
    Ok(invite)
}

pub async fn decline_invite(pool: &SqlitePool, invite_id: i64, user_id: i64) -> Result<()> {
    let result = sqlx::query(
        "UPDATE inviti_risorsa \
         SET stato = 'rifiutato', risposto_il = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ? AND invitato_utente_id = ? AND stato = 'pendente'",
    )
    .bind(invite_id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile rifiutare l'invito")?;
    if result.rows_affected() != 1 {
        bail!("Invito non disponibile o già gestito");
    }
    Ok(())
}

pub async fn revoke_permission(
    pool: &SqlitePool,
    resource_type: &str,
    resource_id: i64,
    user_id: i64,
) -> Result<()> {
    sqlx::query(
        "DELETE FROM permessi_risorsa \
         WHERE tipo_risorsa = ? AND risorsa_id = ? AND utente_id = ?",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile revocare il permesso")?;
    sqlx::query(
        "UPDATE inviti_risorsa \
         SET stato = 'revocato', risposto_il = COALESCE(risposto_il, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) \
         WHERE tipo_risorsa = ? AND risorsa_id = ? AND invitato_utente_id = ? \
           AND stato = 'pendente'",
    )
    .bind(resource_type)
    .bind(resource_id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Impossibile revocare gli inviti pendenti")?;
    Ok(())
}
