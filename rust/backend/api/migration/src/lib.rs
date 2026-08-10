pub use sea_orm_migration::prelude::*;

mod m000001_init;
mod m000002_rate_limit_state;
mod m000003_translation_popularity;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m000001_init::Migration),
            Box::new(m000002_rate_limit_state::Migration),
            Box::new(m000003_translation_popularity::Migration),
        ]
    }
}
