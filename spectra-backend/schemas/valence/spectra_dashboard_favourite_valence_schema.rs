use crate::dashboards::policies::{DASHBOARD_CHILD_RESOURCE, FAVOURITE_SELF};
use valence::prelude::*;
use valence::privacy_policies::common::SYSTEM_ONLY;

valence_schema! {
    SpectraDashboardFavourite {
        repository: "https://github.com/unified-field-dev/spectra-uf-app",
        table: "spectra_dashboard_favourite",
        version: "0.1.0",
        database: crate::dashboards::DASHBOARD_STORAGE,
        description: "A Spectra dashboard one person starred, so it sorts to the top of their dashboards list. Only that person can see or change it; the dashboard's owner can remove it by deleting the dashboard.",

        policies: {
            read: { always_allow: [SYSTEM_ONLY], allow: [FAVOURITE_SELF] },
            create: { always_allow: [SYSTEM_ONLY], allow: [FAVOURITE_SELF] },
            update: { always_allow: [SYSTEM_ONLY], allow: [FAVOURITE_SELF] },
            delete: { always_allow: [SYSTEM_ONLY], allow: [FAVOURITE_SELF, DASHBOARD_CHILD_RESOURCE] },
        },

        fields: [
            id: { r#type: FieldType::String, primary_key: true, required: true },
            user: { r#type: FieldType::Record("user"), required: true },
            dashboard: { r#type: FieldType::Record("spectra_dashboard"), required: true },
            created_at: { r#type: FieldType::DateTime, required: true },
        ],

        connections: [
            user: {
                table: "user",
                cardinality: HasOne,
                required: true,
                on_delete: Cascade,
                model: "lepton_identity::generated::User",
            },
            dashboard: {
                table: "spectra_dashboard",
                cardinality: HasOne,
                required: true,
                on_delete: Cascade,
                model: "crate::generated::SpectraDashboard",
            },
        ],
    }
}
