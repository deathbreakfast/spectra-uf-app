use crate::dashboards::policies::DASHBOARD_CHILD_RESOURCE;
use valence::prelude::*;
use valence::privacy_policies::common::{BLOCK_ALL, SYSTEM_ONLY};

valence_schema! {
    SpectraDashboardVersion {
        repository: "https://github.com/unified-field-dev/spectra-uf-app",
        table: "spectra_dashboard_version",
        version: "0.1.0",
        database: crate::dashboards::DASHBOARD_STORAGE,
        description: "One saved version of a Spectra dashboard: a copy of its panels' queries, who saved it, through which agent if any, and a one-line summary. Anyone who can open the dashboard can read its versions. Versions are never edited and expire after 180 days.",

        meta: {
            retention: "180 days",
        },

        ttl: {
            seconds: 15_552_000,
            mode: "backend_capability",
        },

        policies: {
            read: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_CHILD_RESOURCE] },
            create: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_CHILD_RESOURCE] },
            update: { always_allow: [SYSTEM_ONLY], allow: [BLOCK_ALL] },
            delete: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_CHILD_RESOURCE] },
        },

        fields: [
            id: { r#type: FieldType::String, primary_key: true, required: true },
            dashboard: { r#type: FieldType::Record("spectra_dashboard"), required: true },
            number: { r#type: FieldType::Integer, required: true },
            author: { r#type: FieldType::Record("user"), required: false },
            via: { r#type: FieldType::String, required: false },
            summary: { r#type: FieldType::String, required: true },
            title: { r#type: FieldType::String, required: true },
            description: { r#type: FieldType::String, required: false },
            body: { r#type: FieldType::Json, required: true },
            created_at: { r#type: FieldType::DateTime, required: true },
        ],

        connections: [
            dashboard: {
                table: "spectra_dashboard",
                cardinality: HasOne,
                required: true,
                on_delete: Cascade,
                model: "crate::generated::SpectraDashboard",
            },
            author: {
                table: "user",
                cardinality: HasOne,
                required: false,
                on_delete: SetNull,
                model: "lepton_identity::generated::User",
            },
        ],
    }
}
