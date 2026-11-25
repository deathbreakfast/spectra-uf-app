use crate::dashboards::policies::{DASHBOARD_OWNER, DASHBOARD_RESOURCE};
use valence::prelude::*;
use valence::privacy_policies::common::SYSTEM_ONLY;

valence_schema! {
    SpectraDashboard {
        repository: "https://github.com/unified-field-dev/spectra-uf-app",
        table: "spectra_dashboard",
        version: "0.1.0",
        database: crate::dashboards::DASHBOARD_STORAGE,
        description: "A saved Spectra dashboard: its title, owner and the queries behind its panels (never query results). The owner and the people it is shared with through its Gauge permissions can open it; editors can save it; only the owner can delete it. When the owner shares it, the row keeps a copy of who it's shared with, so everyone who can open it sees who else has access.",

        policies: {
            read: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_RESOURCE] },
            create: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_OWNER] },
            update: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_RESOURCE] },
            delete: { always_allow: [SYSTEM_ONLY], allow: [DASHBOARD_RESOURCE] },
        },

        fields: [
            id: { r#type: FieldType::String, primary_key: true, required: true },
            title: { r#type: FieldType::String, required: true },
            description: { r#type: FieldType::String, required: false },
            owner: { r#type: FieldType::Record("user"), required: true },
            body: { r#type: FieldType::Json, required: true },
            shared_access: { r#type: FieldType::Json, required: false },
            version: { r#type: FieldType::Integer, required: true, default: 1 },
            created_at: { r#type: FieldType::DateTime, required: true },
            updated_at: { r#type: FieldType::DateTime, required: true },
        ],

        connections: [
            owner: {
                table: "user",
                cardinality: HasOne,
                required: true,
                on_delete: Cascade,
                model: "lepton_identity::generated::User",
            },
        ],
    }
}
