pub mod auth_v1;
pub mod category_v1;
pub mod csrf_v1;
pub mod feed_v1;
pub mod mail_v1;
pub mod media_v1;
pub mod post_v1;
pub mod tag_v1;
pub mod user_v1;

pub mod analytics_v1;
pub mod google_auth_v1;
pub mod email_verification_v1;
pub mod forgot_password_v1;
pub mod post_comment_v1;
pub mod newsletter_v1;
pub mod admin_acl_v1;
pub mod admin_route_v1;

#[cfg(feature = "seed-system")]
pub mod seed_v1;

pub mod billing_v1;

pub mod search_v1;

pub mod apple_auth_v1;
pub mod device_v1;
pub mod facebook_auth_v1;
pub mod github_auth_v1;
pub mod notification_v1;
pub mod passkey_v1;

pub mod quran_v1;
