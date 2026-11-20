use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{Error, Result};
use crate::ids::ProfileId;
use crate::store::Store;
use crate::time::Timestamp;
use crate::url;
use crate::writer::Coalesce;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Permission {
    Notifications,
    Geolocation,
    Camera,
    Microphone,
    Popups,
    MediaKeySystem,
    StorageAccess,
}

impl Permission {
    const ALL: [Self; 7] = [
        Self::Notifications,
        Self::Geolocation,
        Self::Camera,
        Self::Microphone,
        Self::Popups,
        Self::MediaKeySystem,
        Self::StorageAccess,
    ];

    fn code(self) -> i64 {
        match self {
            Self::Notifications => 0,
            Self::Geolocation => 1,
            Self::Camera => 2,
            Self::Microphone => 3,
            Self::Popups => 6,
            Self::MediaKeySystem => 9,
            Self::StorageAccess => 10,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|permission| permission.code() == code)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Decision {
    Allow,
    Deny,
}

impl Decision {
    fn allowed(self) -> bool {
        self == Self::Allow
    }

    fn from_allowed(allowed: bool) -> Self {
        if allowed { Self::Allow } else { Self::Deny }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OriginDecision {
    pub origin: String,
    pub top_site: Option<String>,
    pub permission: Permission,
    pub decision: Decision,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SiteSettings {
    pub host: String,
    pub permissions: Vec<OriginDecision>,
    pub zoom: Option<f64>,
    pub https_only_exception: bool,
}

fn origin_key(origin: &str) -> String {
    origin.trim_end_matches('/').to_ascii_lowercase()
}

fn host_key(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

fn host_of_origin(origin: &str) -> String {
    url::host(origin).unwrap_or_else(|| origin.to_owned())
}

impl Store {
    pub fn permission(
        &self,
        profile: ProfileId,
        origin: &str,
        permission: Permission,
    ) -> Result<Option<Decision>> {
        self.decision(profile, origin, permission, None)
    }

    pub fn set_permission(
        &self,
        profile: ProfileId,
        origin: &str,
        permission: Permission,
        decision: Decision,
    ) {
        self.decide(profile, origin, permission, None, Some(decision));
    }

    pub fn clear_permission(&self, profile: ProfileId, origin: &str, permission: Permission) {
        self.decide(profile, origin, permission, None, None);
    }

    pub fn storage_access(
        &self,
        profile: ProfileId,
        origin: &str,
        top_site: &str,
    ) -> Result<Option<Decision>> {
        self.decision(profile, origin, Permission::StorageAccess, Some(top_site))
    }

    pub fn set_storage_access(
        &self,
        profile: ProfileId,
        origin: &str,
        top_site: &str,
        decision: Decision,
    ) {
        let top_site = Some(top_site);
        self.decide(
            profile,
            origin,
            Permission::StorageAccess,
            top_site,
            Some(decision),
        );
    }

    pub fn clear_storage_access(&self, profile: ProfileId, origin: &str, top_site: &str) {
        self.decide(
            profile,
            origin,
            Permission::StorageAccess,
            Some(top_site),
            None,
        );
    }

    fn decision(
        &self,
        profile: ProfileId,
        origin: &str,
        permission: Permission,
        top_site: Option<&str>,
    ) -> Result<Option<Decision>> {
        let origin = origin_key(origin);
        let top_site = top_site.map(origin_key).unwrap_or_default();
        self.read(|connection| {
            let allowed: Option<bool> = connection
                .prepare_cached(
                    "SELECT decision FROM site_permissions \
                     WHERE profile_id = ?1 AND origin = ?2 AND kind = ?3 \
                     AND top_site IN (?4, '') ORDER BY top_site DESC LIMIT 1",
                )?
                .query_row(
                    params![profile, origin, permission.code(), top_site],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(allowed.map(Decision::from_allowed))
        })
    }

    fn decide(
        &self,
        profile: ProfileId,
        origin: &str,
        permission: Permission,
        top_site: Option<&str>,
        decision: Option<Decision>,
    ) {
        let origin = origin_key(origin);
        let top_site = top_site.map(origin_key).unwrap_or_default();
        let host = host_of_origin(&origin);
        let changed_at = Timestamp::now();
        self.write(move |connection, _| {
            match decision {
                Some(decision) => connection
                    .prepare_cached(
                        "INSERT INTO site_permissions \
                         (profile_id, origin, top_site, kind, host, decision, changed_at) \
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                         ON CONFLICT (profile_id, origin, top_site, kind) DO UPDATE SET \
                         decision = excluded.decision, changed_at = excluded.changed_at",
                    )?
                    .execute(params![
                        profile,
                        origin,
                        top_site,
                        permission.code(),
                        host,
                        decision.allowed(),
                        changed_at
                    ])?,
                None => connection
                    .prepare_cached(
                        "DELETE FROM site_permissions WHERE profile_id = ?1 AND origin = ?2 \
                         AND top_site = ?3 AND kind = ?4",
                    )?
                    .execute(params![profile, origin, top_site, permission.code()])?,
            };
            Ok(())
        });
    }

    pub fn zoom(&self, profile: ProfileId, host: &str) -> Result<Option<f64>> {
        let host = host_key(host);
        self.read(|connection| {
            let zoom: Option<Option<f64>> = connection
                .prepare_cached("SELECT zoom FROM site_hosts WHERE profile_id = ?1 AND host = ?2")?
                .query_row(params![profile, host], |row| row.get(0))
                .optional()?;
            Ok(zoom.flatten())
        })
    }

    pub fn set_zoom(&self, profile: ProfileId, host: &str, zoom: f64) -> Result<()> {
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err(Error::InvalidZoom(zoom));
        }
        self.store_zoom(profile, host, Some(zoom));
        Ok(())
    }

    pub fn clear_zoom(&self, profile: ProfileId, host: &str) {
        self.store_zoom(profile, host, None);
    }

    fn store_zoom(&self, profile: ProfileId, host: &str, zoom: Option<f64>) {
        let host = host_key(host);
        let changed_at = Timestamp::now();
        self.write_behind(
            Some(Coalesce::Zoom(profile, host.clone())),
            move |connection, _| {
                connection
                    .prepare_cached(
                        "INSERT INTO site_hosts (profile_id, host, zoom, changed_at) \
                         VALUES (?1, ?2, ?3, ?4) \
                         ON CONFLICT (profile_id, host) DO UPDATE SET zoom = excluded.zoom, \
                         changed_at = excluded.changed_at",
                    )?
                    .execute(params![profile, host, zoom, changed_at])?;
                drop_empty(connection, profile, &host)
            },
        );
    }

    pub fn https_only_exception(&self, profile: ProfileId, host: &str) -> Result<bool> {
        let host = host_key(host);
        self.read(|connection| {
            let excepted: Option<bool> = connection
                .prepare_cached(
                    "SELECT https_only_exception FROM site_hosts \
                     WHERE profile_id = ?1 AND host = ?2",
                )?
                .query_row(params![profile, host], |row| row.get(0))
                .optional()?;
            Ok(excepted.unwrap_or(false))
        })
    }

    pub fn set_https_only_exception(&self, profile: ProfileId, host: &str, excepted: bool) {
        let host = host_key(host);
        let changed_at = Timestamp::now();
        self.write(move |connection, _| {
            connection
                .prepare_cached(
                    "INSERT INTO site_hosts (profile_id, host, https_only_exception, changed_at) \
                     VALUES (?1, ?2, ?3, ?4) \
                     ON CONFLICT (profile_id, host) DO UPDATE SET \
                     https_only_exception = excluded.https_only_exception, \
                     changed_at = excluded.changed_at",
                )?
                .execute(params![profile, host, excepted, changed_at])?;
            drop_empty(connection, profile, &host)
        });
    }

    pub fn https_only_exceptions(&self, profile: ProfileId) -> Result<Vec<String>> {
        self.read(|connection| {
            let mut statement = connection.prepare_cached(
                "SELECT host FROM site_hosts \
                 WHERE profile_id = ?1 AND https_only_exception = 1 ORDER BY host",
            )?;
            let rows = statement.query_map([profile], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    pub fn site_settings(&self, profile: ProfileId, host: &str) -> Result<SiteSettings> {
        let host = host_key(host);
        self.read(|connection| {
            let mut sites = collect(connection, profile, Some(&host))?;
            Ok(sites.remove(&host).unwrap_or(SiteSettings {
                host,
                ..SiteSettings::default()
            }))
        })
    }

    pub fn sites_with_settings(&self, profile: ProfileId) -> Result<Vec<SiteSettings>> {
        self.read(|connection| Ok(collect(connection, profile, None)?.into_values().collect()))
    }

    pub fn clear_site(&self, profile: ProfileId, host: &str) {
        let host = host_key(host);
        self.write(move |connection, _| {
            connection.execute(
                "DELETE FROM site_permissions WHERE profile_id = ?1 AND host = ?2",
                params![profile, host],
            )?;
            connection.execute(
                "DELETE FROM site_hosts WHERE profile_id = ?1 AND host = ?2",
                params![profile, host],
            )?;
            Ok(())
        });
    }
}

fn drop_empty(connection: &Connection, profile: ProfileId, host: &str) -> Result<()> {
    connection
        .prepare_cached(
            "DELETE FROM site_hosts WHERE profile_id = ?1 AND host = ?2 AND zoom IS NULL \
             AND https_only_exception = 0",
        )?
        .execute(params![profile, host])?;
    Ok(())
}

fn collect(
    connection: &Connection,
    profile: ProfileId,
    only: Option<&str>,
) -> Result<BTreeMap<String, SiteSettings>> {
    let mut sites: BTreeMap<String, SiteSettings> = BTreeMap::new();
    let mut statement = connection.prepare_cached(
        "SELECT host, zoom, https_only_exception FROM site_hosts \
         WHERE profile_id = ?1 AND (?2 IS NULL OR host = ?2)",
    )?;
    let rows = statement.query_map(params![profile, only], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<f64>>(1)?,
            row.get::<_, bool>(2)?,
        ))
    })?;
    for row in rows {
        let (host, zoom, https_only_exception) = row?;
        let settings = site(&mut sites, host);
        settings.zoom = zoom;
        settings.https_only_exception = https_only_exception;
    }
    let mut statement = connection.prepare_cached(
        "SELECT host, origin, kind, decision, top_site FROM site_permissions \
         WHERE profile_id = ?1 AND (?2 IS NULL OR host = ?2) \
         ORDER BY host, origin, kind, top_site",
    )?;
    let rows = statement.query_map(params![profile, only], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, bool>(3)?,
            row.get::<_, String>(4)?,
        ))
    })?;
    for row in rows {
        let (host, origin, kind, allowed, top_site) = row?;
        let Some(permission) = Permission::from_code(kind) else {
            continue;
        };
        site(&mut sites, host).permissions.push(OriginDecision {
            origin,
            top_site: (!top_site.is_empty()).then_some(top_site),
            permission,
            decision: Decision::from_allowed(allowed),
        });
    }
    Ok(sites)
}

fn site(sites: &mut BTreeMap<String, SiteSettings>, host: String) -> &mut SiteSettings {
    sites.entry(host.clone()).or_insert_with(|| SiteSettings {
        host,
        ..SiteSettings::default()
    })
}
