//! Measures published cloud and V2 executor plugin catalogs, excluding cached reads.

use maplecode_core_plugins::PluginCatalog;
use maplecode_extension_api::ExtensionMetrics;
use maplecode_extension_api::record_catalog_entries;

pub(crate) fn record_catalog(
    metrics: Option<&dyn ExtensionMetrics>,
    stage: &'static str,
    catalog: &PluginCatalog,
) {
    record_catalog_entries(
        metrics,
        "maplecode.plugins.catalog.entries",
        stage,
        catalog.entries.len(),
        catalog.is_partial,
        catalog.warnings.len(),
    );
}
