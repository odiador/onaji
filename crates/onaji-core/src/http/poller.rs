use super::state::{ContentState, HttpPollOutcome, HttpResourceState, ResourceAvailability};
use crate::domain::{Event, EventId, EventType, ResourceId};
use crate::fingerprint::Fingerprinter;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, IF_MODIFIED_SINCE, IF_NONE_MATCH};
use reqwest::{Client, StatusCode};
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

pub struct HttpPoller {
    client: Client,
    states: HashMap<ResourceId, HttpResourceState>,
}

impl HttpPoller {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();

        Self {
            client,
            states: HashMap::new(),
        }
    }

    pub fn with_client(client: Client) -> Self {
        Self {
            client,
            states: HashMap::new(),
        }
    }

    pub async fn poll_resource(
        &mut self,
        resource_id: &ResourceId,
        url: &str,
        custom_headers: &HashMap<String, String>,
    ) -> HttpPollOutcome {
        let state = self.states.entry(resource_id.clone()).or_default();

        let mut req_builder = self.client.get(url);

        // Headers condicionales
        if let Some(etag) = &state.last_etag {
            req_builder = req_builder.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_mod) = &state.last_modified {
            req_builder = req_builder.header(IF_MODIFIED_SINCE, last_mod);
        }

        // Headers personalizados
        for (k, v) in custom_headers {
            if let (Ok(h_name), Ok(h_val)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
                req_builder = req_builder.header(h_name, h_val);
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let response = match req_builder.send().await {
            Ok(resp) => resp,
            Err(err) => {
                let availability = if err.is_timeout() {
                    ResourceAvailability::Unavailable {
                        reason: "HTTP request timed out".to_string(),
                    }
                } else if err.is_connect() {
                    ResourceAvailability::Unavailable {
                        reason: "Connection refused / Host unreachable".to_string(),
                    }
                } else {
                    ResourceAvailability::Unavailable {
                        reason: err.to_string(),
                    }
                };

                let mut emitted_events = Vec::new();
                if state.last_availability.as_ref() != Some(&availability) {
                    state.last_availability = Some(availability.clone());
                    emitted_events.push(Event::new(
                        EventId::new(format!("ev-http-{}", now)),
                        resource_id.clone(),
                        EventType::Unavailable {
                            reason: match &availability {
                                ResourceAvailability::Unavailable { reason } => reason.clone(),
                                _ => "Unknown".to_string(),
                            },
                        },
                        now,
                    ));
                }

                return HttpPollOutcome {
                    availability,
                    content_state: ContentState::Unknown,
                    emitted_events,
                };
            }
        };

        let status = response.status();
        let headers: HeaderMap = response.headers().clone();

        // 1. Caso 304 Not Modified
        if status == StatusCode::NOT_MODIFIED {
            let availability = ResourceAvailability::Available;
            let mut emitted_events = Vec::new();

            if state.last_availability.as_ref() != Some(&availability) {
                state.last_availability = Some(availability.clone());
                emitted_events.push(Event::new(
                    EventId::new(format!("ev-http-{}", now)),
                    resource_id.clone(),
                    EventType::Available,
                    now,
                ));
            }

            return HttpPollOutcome {
                availability,
                content_state: ContentState::Unchanged,
                emitted_events,
            };
        }

        // 2. Caso 2xx Exitoso
        if status.is_success() {
            let new_etag = headers
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());
            let new_last_mod = headers
                .get("Last-Modified")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let body_bytes = match response.bytes().await {
                Ok(b) => b,
                Err(err) => {
                    let availability = ResourceAvailability::Degraded {
                        status: Some(status.as_u16()),
                        reason: format!("Failed reading response body: {}", err),
                    };
                    return HttpPollOutcome {
                        availability,
                        content_state: ContentState::Unknown,
                        emitted_events: Vec::new(),
                    };
                }
            };

            let current_fingerprint = Fingerprinter::compute_bytes(&body_bytes);
            let content_changed = match &state.last_fingerprint {
                Some(prev_fp) => *prev_fp != current_fingerprint,
                None => true, // Primera vez que se observa el recurso
            };

            // Actualizamos estado interno del recurso
            state.last_etag = new_etag;
            state.last_modified = new_last_mod;
            state.last_fingerprint = Some(current_fingerprint);

            let availability = ResourceAvailability::Available;
            let mut emitted_events = Vec::new();

            // Cambio de disponibilidad
            if state.last_availability.as_ref() != Some(&availability) {
                state.last_availability = Some(availability.clone());
                emitted_events.push(Event::new(
                    EventId::new(format!("ev-http-avail-{}", now)),
                    resource_id.clone(),
                    EventType::Available,
                    now,
                ));
            }

            let content_state = if content_changed {
                emitted_events.push(
                    Event::new(
                        EventId::new(format!("ev-http-mod-{}", now)),
                        resource_id.clone(),
                        EventType::Modified,
                        now,
                    )
                    .with_payload_hash(current_fingerprint.as_u64()),
                );
                ContentState::Modified {
                    fingerprint: current_fingerprint,
                }
            } else {
                ContentState::Unchanged
            };

            return HttpPollOutcome {
                availability,
                content_state,
                emitted_events,
            };
        }

        // 3. Caso 4xx / 5xx Degradado
        let availability = ResourceAvailability::Degraded {
            status: Some(status.as_u16()),
            reason: format!("HTTP error: {}", status),
        };

        let mut emitted_events = Vec::new();
        if state.last_availability.as_ref() != Some(&availability) {
            state.last_availability = Some(availability.clone());
            emitted_events.push(Event::new(
                EventId::new(format!("ev-http-{}", now)),
                resource_id.clone(),
                EventType::Degraded {
                    status: Some(status.as_u16()),
                    reason: status.to_string(),
                },
                now,
            ));
        }

        HttpPollOutcome {
            availability,
            content_state: ContentState::Unknown,
            emitted_events,
        }
    }
}

impl Default for HttpPoller {
    fn default() -> Self {
        Self::new()
    }
}
