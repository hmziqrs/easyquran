use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(crate::modules::quran_v1::controller::health_ready),
    components(schemas(
        // Quran API DTOs (§6.3) — camelCase success bodies.
        crate::modules::quran_v1::dto::HealthReady,
        crate::modules::quran_v1::dto::SourceDigestsDto,
        crate::modules::quran_v1::dto::VersionData,
        crate::modules::quran_v1::dto::TranslationVersion,
        crate::modules::quran_v1::dto::Ayah,
        crate::modules::quran_v1::dto::AyahRange,
        crate::modules::quran_v1::dto::AyahsList,
        crate::modules::quran_v1::dto::RangeMeta,
        crate::modules::quran_v1::dto::RangeSummary,
        crate::modules::quran_v1::dto::RangeKind,
        crate::modules::quran_v1::dto::VerseKey,
        crate::modules::quran_v1::dto::SuraDto,
        crate::modules::quran_v1::dto::SajdaDto,
        crate::modules::quran_v1::dto::Artifact,
        crate::modules::quran_v1::dto::ScriptsData,
        crate::modules::quran_v1::dto::RandomAyah,
        crate::modules::quran_v1::dto::SearchResponse,
        crate::modules::quran_v1::dto::SearchHit,
        crate::modules::quran_v1::dto::Highlight,
        crate::quran::Script,
        crate::quran::Place,
        crate::quran::Bismillah,
        crate::quran::SajdaKind,
    )),
    info(
        title = "EasyQuran API",
        version = "1.0.0",
        description = "EasyQuran — Quran content API (Arabic MVP). Public read-only surface under /quran/v1."
    )
)]
pub struct ApiDoc;
