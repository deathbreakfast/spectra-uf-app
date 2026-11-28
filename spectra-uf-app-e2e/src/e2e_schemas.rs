//! Lab-only Spectra schemas the dashboards stories need beyond the platform smoke tables.
//!
//! Linked with `mod` so `inventory` registers them in [`spectra::spectra_core::SchemaRegistry`].

use spectra::spectra_schema;

/// Checkout orders with one personal field, so table panels have something to mask.
pub const E2E_ORDERS_TABLE: &str = "e2e_checkout_orders";

spectra_schema! {
    E2eCheckoutOrder {
        store: "default",
        table: "e2e_checkout_orders",
        version: "0.1.0",
        description: "Checkout orders for the dashboards stories",
        fields: [
            order_id: {
                r#type: String,
                classification: { pii: false, safe_for_console: true },
            },
            status: {
                r#type: String,
                classification: { pii: false, safe_for_console: true },
            },
            region: {
                r#type: String,
                classification: { pii: false, safe_for_console: true },
            },
            customer_email: {
                r#type: String,
                classification: { pii: true, safe_for_console: false },
            },
        ],
    }
}
