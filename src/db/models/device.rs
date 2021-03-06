use chrono::{NaiveDateTime, Utc};

use super::User;

#[derive(Debug, Identifiable, Queryable, Insertable, Associations)]
#[table_name = "devices"]
#[belongs_to(User, foreign_key = "user_uuid")]
#[primary_key(uuid)]
pub struct Device {
    pub uuid: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub user_uuid: String,

    pub name: String,
    /// https://github.com/bitwarden/core/tree/master/src/Core/Enums
    pub type_: i32,
    pub push_token: Option<String>,

    pub refresh_token: String,

    pub twofactor_remember: Option<String>,
}

/// Local methods
impl Device {
    pub fn new(uuid: String, user_uuid: String, name: String, type_: i32) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            uuid,
            created_at: now,
            updated_at: now,

            user_uuid,
            name,
            type_,

            push_token: None,
            refresh_token: String::new(),
            twofactor_remember: None,
        }
    }

    pub fn refresh_twofactor_remember(&mut self) -> String {
        use data_encoding::BASE64;
        use crypto;

        let twofactor_remember = BASE64.encode(&crypto::get_random(vec![0u8; 180]));
        self.twofactor_remember = Some(twofactor_remember.clone());

        twofactor_remember
    }

    pub fn delete_twofactor_remember(&mut self) {
        self.twofactor_remember = None;
    }


    pub fn refresh_tokens(&mut self, user: &super::User, orgs: Vec<super::UserOrganization>) -> (String, i64) {
        // If there is no refresh token, we create one
        if self.refresh_token.is_empty() {
            use data_encoding::BASE64URL;
            use crypto;

            self.refresh_token = BASE64URL.encode(&crypto::get_random_64());
        }

        // Update the expiration of the device and the last update date
        let time_now = Utc::now().naive_utc();
        self.updated_at = time_now;


        let orgowner: Vec<_> = orgs.iter().filter(|o| o.type_ == 0).map(|o| o.org_uuid.clone()).collect();
        let orgadmin: Vec<_> = orgs.iter().filter(|o| o.type_ == 1).map(|o| o.org_uuid.clone()).collect();
        let orguser: Vec<_> = orgs.iter().filter(|o| o.type_ == 2).map(|o| o.org_uuid.clone()).collect();


        // Create the JWT claims struct, to send to the client
        use auth::{encode_jwt, JWTClaims, DEFAULT_VALIDITY, JWT_ISSUER};
        let claims = JWTClaims {
            nbf: time_now.timestamp(),
            exp: (time_now + *DEFAULT_VALIDITY).timestamp(),
            iss: JWT_ISSUER.to_string(),
            sub: user.uuid.to_string(),

            premium: true,
            name: user.name.to_string(),
            email: user.email.to_string(),
            email_verified: true,

            orgowner,
            orgadmin,
            orguser,

            sstamp: user.security_stamp.to_string(),
            device: self.uuid.to_string(),
            scope: vec!["api".into(), "offline_access".into()],
            amr: vec!["Application".into()],
        };


        (encode_jwt(&claims), DEFAULT_VALIDITY.num_seconds())
    }
}

use diesel;
use diesel::prelude::*;
use db::DbConn;
use db::schema::devices;

/// Database methods
impl Device {
    pub fn save(&mut self, conn: &DbConn) -> bool {
        self.updated_at = Utc::now().naive_utc();

        match diesel::replace_into(devices::table)
            .values(&*self)
            .execute(&**conn) {
            Ok(1) => true, // One row inserted
            _ => false,
        }
    }

    pub fn delete(self, conn: &DbConn) -> bool {
        match diesel::delete(devices::table.filter(
            devices::uuid.eq(self.uuid)))
            .execute(&**conn) {
            Ok(1) => true, // One row deleted
            _ => false,
        }
    }

    pub fn find_by_uuid(uuid: &str, conn: &DbConn) -> Option<Self> {
        devices::table
            .filter(devices::uuid.eq(uuid))
            .first::<Self>(&**conn).ok()
    }

    pub fn find_by_refresh_token(refresh_token: &str, conn: &DbConn) -> Option<Self> {
        devices::table
            .filter(devices::refresh_token.eq(refresh_token))
            .first::<Self>(&**conn).ok()
    }

    pub fn find_by_user(user_uuid: &str, conn: &DbConn) -> Vec<Self> {
        devices::table
            .filter(devices::user_uuid.eq(user_uuid))
            .load::<Self>(&**conn).expect("Error loading devices")
    }
}
