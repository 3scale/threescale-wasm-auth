use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use threescalers::application::Application;

use crate::{
    configuration::Source,
    proxy::{request_headers::RequestHeaders, HttpAuthThreescale},
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("credentials not found")]
    NotFound,
}

// Consider defining an order of look-up and short-circuiting?
#[derive(Debug, Clone)]
pub struct Credentials {
    user_key: Option<Vec<Source>>,
    app_id: Option<Vec<Source>>,
    app_key: Option<Vec<Source>>,
}

impl Credentials {
    #[allow(dead_code)]
    pub fn new(
        user_key: Option<Vec<Source>>,
        app_id: Option<Vec<Source>>,
        app_key: Option<Vec<Source>>,
    ) -> Self {
        Self {
            user_key,
            app_id,
            app_key,
        }
    }

    pub fn user_key(&self) -> Option<&Vec<Source>> {
        self.user_key.as_ref()
    }

    pub fn app_id(&self) -> Option<&Vec<Source>> {
        self.app_id.as_ref()
    }

    pub fn app_key(&self) -> Option<&Vec<Source>> {
        self.app_key.as_ref()
    }

    pub fn resolve(
        &self,
        ctx: &HttpAuthThreescale,
        rh: &RequestHeaders,
        url: &url::Url,
    ) -> Result<Vec<Application>, Error> {
        let mut apps = vec![];

        let user_key = self
            .user_key()
            .and_then(|sources| {
                sources
                    .iter()
                    .find_map(|source| source.resolve(ctx, rh, url))
                    .and_then(|values| values.get(0).map(|val| val.as_ref().to_string()))
            })
            .map(|user_key| Application::UserKey(user_key.into()));

        let app_values = self.app_id().and_then(|sources| {
            sources
                .iter()
                .find_map(|source| source.resolve(ctx, rh, url))
                .map(|values| {
                    let app_id = values.get(0).map(|val| val.as_ref().to_string());
                    let app_key = values.get(1).map(|val| val.as_ref().to_string());
                    (app_id, app_key)
                })
        });

        let (app_id, app_key) = match app_values {
            Some((id, key)) => (id, key),
            _ => (None, None),
        };

        let app_key = app_key.or_else(|| {
            self.app_key().and_then(|sources| {
                sources
                    .iter()
                    .find_map(|source| source.resolve(ctx, rh, url))
                    .and_then(|values| values.get(0).map(|k| k.as_ref().to_string()))
            })
        });

        let app_id = app_id.map(|id| Application::AppId(id.into(), app_key.map(|k| k.into())));

        if let Some(uk) = user_key {
            apps.push(uk);
        }

        if let Some(app) = app_id {
            apps.push(app);
        }

        if apps.is_empty() {
            return Err(Error::NotFound);
        }

        Ok(apps)
    }
}

impl<'de> Deserialize<'de> for Credentials {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            UserKey,
            AppId,
            AppKey,
        }

        struct CredentialsVisitor;

        impl<'de> de::Visitor<'de> for CredentialsVisitor {
            type Value = Credentials;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str(
                    "credentials definition looking up user_key or app_id with optional app_key",
                )
            }

            fn visit_map<V: de::MapAccess<'de>>(self, mut map: V) -> Result<Self::Value, V::Error> {
                let mut user_key = None;
                let mut app_id = None;
                let mut app_key = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::UserKey => {
                            if user_key.is_some() {
                                return Err(de::Error::duplicate_field("user_key"));
                            }
                            user_key = Some(map.next_value()?);
                        }
                        Field::AppId => {
                            if app_id.is_some() {
                                return Err(de::Error::duplicate_field("app_id"));
                            }
                            app_id = Some(map.next_value()?);
                        }
                        Field::AppKey => {
                            if app_key.is_some() {
                                return Err(de::Error::duplicate_field("app_key"));
                            }
                            app_key = Some(map.next_value()?);
                        }
                    }
                }
                if let (None, None) = (&user_key, &app_id) {
                    return Err(de::Error::custom(
                        "you must provide at least one of user_key or app_id",
                    ));
                }

                let credentials = Credentials {
                    user_key,
                    app_id,
                    app_key,
                };

                Ok(credentials)
            }
        }

        deserializer.deserialize_struct(
            "Credentials",
            &["user_key", "app_id", "app_key"],
            CredentialsVisitor,
        )
    }
}

impl Serialize for Credentials {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        enum FieldAction<'n, 's> {
            Serialize(&'n str, &'s Vec<Source>),
            Skip(&'n str),
        }

        let user_key_action = self
            .user_key()
            .map_or(FieldAction::Skip("user_key"), |user_key| {
                FieldAction::Serialize("user_key", user_key)
            });

        let app_id_action = self.app_id().map_or(FieldAction::Skip("app_id"), |app_id| {
            FieldAction::Serialize("app_id", app_id)
        });

        let app_key_action = self
            .app_key()
            .map_or(FieldAction::Skip("app_key"), |app_key| {
                FieldAction::Serialize("app_key", app_key)
            });

        let actions = [user_key_action, app_id_action, app_key_action];

        let mut state = serializer.serialize_struct(
            "Credentials",
            actions
                .iter()
                .filter(|&a| matches!(a, FieldAction::Serialize(_, _)))
                .count(),
        )?;

        for f in &actions {
            match f {
                FieldAction::Serialize(key, value) => state.serialize_field(key, value)?,
                FieldAction::Skip(key) => state.skip_field(key)?,
            }
        }
        state.end()
    }
}
