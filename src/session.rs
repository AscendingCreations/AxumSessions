use crate::{AxumSessionData, AxumSessionID, AxumSessionStore};
use async_trait::async_trait;
use axum_core::extract::{FromRequest, RequestParts};
use cookie::CookieJar;
use http::{self, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

/// A Session Store.
///
/// Provides a Storage Handler to AxumSessionStore and contains the AxumSessionID(UUID) of the current session.
///
/// This is Auto generated by the Session Layer Upon Service Execution.
#[derive(Debug, Clone)]
pub struct AxumSession {
    pub(crate) store: AxumSessionStore,
    pub(crate) id: AxumSessionID,
}

/// Adds FromRequest<B> for AxumSession
///
/// Returns the AxumSession from Axums request extensions.
#[async_trait]
impl<B> FromRequest<B> for AxumSession
where
    B: Send,
{
    type Rejection = (http::StatusCode, &'static str);

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        req.extensions().get::<AxumSession>().cloned().ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "Can't extract AxumSession. Is `AxumSessionLayer` enabled?",
        ))
    }
}

impl AxumSession {
    pub(crate) async fn new(store: &AxumSessionStore, cookies: &CookieJar) -> AxumSession {
        let value = cookies
            .get(&store.config.cookie_name)
            .and_then(|c| Uuid::parse_str(c.value()).ok());

        let uuid = match value {
            Some(v) => v,
            None => {
                let store_ug = store.inner.read().await;
                loop {
                    let token = Uuid::new_v4();

                    if !store_ug.contains_key(&token.to_string()) {
                        break token;
                    }
                }
            }
        };

        AxumSession {
            id: AxumSessionID(uuid),
            store: store.clone(),
        }
    }
    /// Runs a Closure upon the Current Sessions stored data to get or set session data.
    ///
    /// Provides an Option<T> that returns the requested data from the Sessions store.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.tap(|sess| {
    ///   let string = sess.data.get(key)?;
    ///   serde_json::from_str(string).ok()
    /// }).await;
    /// ```
    ///
    pub async fn tap<T: DeserializeOwned>(
        &self,
        func: impl FnOnce(&mut AxumSessionData) -> Option<T>,
    ) -> Option<T> {
        let store_rg = self.store.inner.read().await;

        if let Some(v) = store_rg.get(&self.id.0.to_string()) {
            let mut instance = v.lock().await;
            func(&mut instance)
        } else {
            tracing::warn!("Session data unexpectedly missing");
            None
        }
    }

    /// Sets the Current Session to be Destroyed on the next run.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.destroy().await;
    /// ```
    ///
    pub async fn destroy(&self) {
        self.tap(|sess| {
            sess.destroy = true;
            Some(1)
        })
        .await;
    }

    /// Sets the Current Session to a long term expiration. Useful for Remember Me setups.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.set_longterm(true).await;
    /// ```
    ///
    pub async fn set_longterm(&self, longterm: bool) {
        self.tap(|sess| {
            sess.longterm = longterm;
            Some(1)
        })
        .await;
    }

    /// Sets the Current Session to be GDPR Accepted.
    ///
    /// This will allow the Session to save its data and push a Cookie to the Browser if set to true.
    /// If this is set to false it will unload the stored session. It will not unload an already set cookie.
    /// Use this to tell if the end user accepted your cookie Policy or not based on GDPR Rules.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.set_accepted(true).await;
    /// ```
    ///
    pub async fn set_accepted(&self, accepted: bool) {
        self.tap(|sess| {
            sess.accepted = accepted;
            Some(1)
        })
        .await;
    }

    /// Gets data from the Session's HashMap
    ///
    /// Provides an Option<T> that returns the requested data from the Sessions store.
    /// Returns None if Key does not exist or if serdes_json failed to deserialize.
    ///
    /// # Examples
    /// ```rust no_run
    /// let id = session.get("user-id").await.unwrap_or(0);
    /// ```
    ///
    ///Used to get data stored within SessionDatas hashmap from a key value.
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.tap(|sess| {
            let string = sess.data.get(key)?;
            serde_json::from_str(string).ok()
        })
        .await
    }

    /// Sets data to the Current Session's HashMap.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.set("user-id", 1).await;
    /// ```
    ///
    pub async fn set(&self, key: &str, value: impl Serialize) {
        let value = serde_json::to_string(&value).unwrap_or_else(|_| "".to_string());

        self.tap(|sess| {
            if sess.data.get(key) != Some(&value) {
                sess.data.insert(key.to_string(), value);
            }
            Some(1)
        })
        .await;
    }

    /// Removes a Key from the Current Session's HashMap.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.remove("user-id").await;
    /// ```
    ///
    pub async fn remove(&self, key: &str) {
        self.tap(|sess| sess.data.remove(key)).await;
    }

    /// Clears all data from the Current Session's HashMap.
    ///
    /// # Examples
    /// ```rust no_run
    /// session.clear_all().await;
    /// ```
    ///
    pub async fn clear_all(&self) {
        let store_rg = self.store.inner.read().await;

        if let Some(v) = store_rg.get(&self.id.0.to_string()) {
            let mut instance = v.lock().await;

            instance.data.clear();
        }

        if self.store.is_persistent() {
            self.store.clear_store().await.unwrap();
        }
    }

    /// Returns a i64 count of how many Sessions exist.
    ///
    /// If the Session is persistant it will return all sessions within the database.
    /// If the Session is not persistant it will return a count within AxumSessionStore.
    ///
    /// # Examples
    /// ```rust no_run
    /// let count = session.count().await;
    /// ```
    ///
    pub async fn count(&self) -> i64 {
        if self.store.is_persistent() {
            self.store.count().await.unwrap_or(0i64)
        } else {
            self.store.inner.read().await.len() as i64
        }
    }
}
