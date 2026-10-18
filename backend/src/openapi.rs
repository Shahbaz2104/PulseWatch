//! OpenAPI document served via Swagger UI.

use utoipa::OpenApi as OpenApiTrait;
use utoipa::{Modify, OpenApi};

use crate::error::{ErrorBody, ErrorResponse, FieldError};
use crate::models::check_result::{CheckResult, CheckSummary, ListChecksQuery};
use crate::models::incident::{Incident, IncidentStatus, IncidentWithMonitor, ListIncidentsQuery};
use crate::models::monitor::{
    CreateMonitorRequest, ListMonitorsQuery, Monitor, MonitorStatus, MonitorSummary,
    UpdateMonitorRequest,
};
use crate::models::ReportWindow;
use crate::routes::health::HealthResponse;

/// Documents the shared JSON error envelope in the API description.
struct ErrorEnvelopeNote;

impl Modify for ErrorEnvelopeNote {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let description = openapi.info.description.get_or_insert_with(String::new);
        if !description.is_empty() {
            description.push_str("\n\n");
        }
        description.push_str(
            "All error responses use the envelope \
             `{\"error\": {\"code\", \"message\", \"details\"?}}` \
             with codes: `bad_request`, `validation_failed`, `not_found`, \
             `conflict`, `rate_limited`, `internal_error`.",
        );
    }
}

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::health::health,
        crate::routes::monitors::list,
        crate::routes::monitors::get,
        crate::routes::monitors::create,
        crate::routes::monitors::update,
        crate::routes::monitors::delete,
        crate::routes::monitors::check_now,
        crate::routes::monitors::pause,
        crate::routes::monitors::resume,
        crate::routes::checks::list,
        crate::routes::checks::latest,
        crate::routes::checks::summary,
        crate::routes::reports::overview,
        crate::routes::reports::uptime,
        crate::routes::reports::response_times,
        crate::routes::reports::incidents,
        crate::routes::incidents::list,
        crate::routes::incidents::get,
        crate::routes::settings::get,
        crate::routes::settings::update,
    ),
    components(schemas(
        HealthResponse,
        ErrorResponse,
        ErrorBody,
        FieldError,
        Monitor,
        MonitorStatus,
        MonitorSummary,
        CreateMonitorRequest,
        UpdateMonitorRequest,
        ListMonitorsQuery,
        CheckResult,
        CheckSummary,
        ListChecksQuery,
        Incident,
        IncidentStatus,
        IncidentWithMonitor,
        ListIncidentsQuery,
        ReportWindow,
        crate::routes::monitors::Pagination,
        crate::routes::monitors::PaginatedMonitors,
        crate::routes::monitors::CheckResponse,
        crate::routes::checks::PaginatedChecks,
        crate::routes::incidents::PaginatedIncidents,
        crate::models::reports::TimeBucket,
        crate::models::reports::MonitorUptime,
        crate::models::reports::UptimeReport,
        crate::models::reports::MonitorResponseTimes,
        crate::models::reports::ResponseTimeReport,
        crate::models::reports::MonitorCounts,
        crate::models::reports::RecentCheck,
        crate::models::reports::IncidentStats,
        crate::models::reports::OverviewReport,
        crate::models::reports::IncidentReport,
        crate::models::reports::ReportQuery,
        crate::services::settings_service::AppSettings,
        crate::services::settings_service::SettingsPatch,
        crate::services::settings_service::SettingsUpdateResponse,
    )),
    tags(
        (name = "health", description = "Service health probes"),
        (name = "monitors", description = "Monitor CRUD and control"),
        (name = "checks", description = "Historical check results"),
        (name = "incidents", description = "Outage incidents"),
        (name = "reports", description = "Aggregated uptime and latency reports"),
        (name = "settings", description = "Application settings"),
    ),
    modifiers(&ErrorEnvelopeNote),
    info(
        title = "PulseWatch API",
        version = "0.1.0",
        description = "Website uptime monitoring platform"
    )
)]
pub struct ApiDoc;

pub fn openapi_json() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}
