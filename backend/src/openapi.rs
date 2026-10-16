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
    paths(crate::routes::health::health,),
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
