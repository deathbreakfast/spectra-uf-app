//! Row geometry for the callsite graph.
//!
//! Emit sites are listed top to bottom grouped by table, so each table node sits beside its
//! own block of sites and no edge crosses another.

use spectra_backend::{CallsiteGraph, EmitSite};

/// Height of one emit-site row, in CSS pixels.
pub const ROW_H: f64 = 56.0;

/// One table node spanning the rows of its emit sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableBlock {
    pub name: String,
    /// Index of the first site row in the block.
    pub first_row: usize,
    /// Number of site rows (always at least one).
    pub rows: usize,
}

/// One edge from a site row to the centre of its table block, in CSS pixels from the top.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edge {
    pub site_y: f64,
    pub table_y: f64,
}

/// Sites ordered for drawing plus the table blocks and edges derived from them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GraphLayout {
    pub sites: Vec<EmitSite>,
    pub blocks: Vec<TableBlock>,
    pub edges: Vec<Edge>,
    pub height: f64,
}

#[allow(clippy::cast_precision_loss, reason = "row counts are far below 2^52")]
fn row_y(row: f64) -> f64 {
    row.mul_add(ROW_H, ROW_H / 2.0)
}

/// Orders sites by table, file, then line, and computes blocks and edges.
#[must_use]
#[allow(clippy::cast_precision_loss, reason = "row counts are far below 2^52")]
pub fn layout_graph(graph: &CallsiteGraph) -> GraphLayout {
    let mut sites = graph.sites.clone();
    sites.sort_by(|a, b| (&a.table, &a.file, a.line).cmp(&(&b.table, &b.file, b.line)));

    let mut blocks: Vec<TableBlock> = Vec::new();
    for (row, site) in sites.iter().enumerate() {
        match blocks.last_mut() {
            Some(block) if block.name == site.table => block.rows += 1,
            _ => blocks.push(TableBlock {
                name: site.table.clone(),
                first_row: row,
                rows: 1,
            }),
        }
    }

    let mut edges = Vec::with_capacity(sites.len());
    for block in &blocks {
        let first = block.first_row as f64;
        let table_y = f64::midpoint(row_y(first), row_y(first + block.rows as f64 - 1.0));
        for offset in 0..block.rows {
            edges.push(Edge {
                site_y: row_y(first + offset as f64),
                table_y,
            });
        }
    }

    let height = sites.len() as f64 * ROW_H;
    GraphLayout {
        sites,
        blocks,
        edges,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn site(file: &str, line: u32, table: &str) -> EmitSite {
        EmitSite {
            file: file.into(),
            line,
            table: table.into(),
            count: 1,
            last_seen: Utc::now(),
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn graph(sites: Vec<EmitSite>) -> CallsiteGraph {
        CallsiteGraph {
            sites,
            ..CallsiteGraph::default()
        }
    }

    #[test]
    fn empty_graph_has_no_rows() {
        let layout = layout_graph(&graph(Vec::new()));
        assert_eq!(layout.blocks, Vec::new());
        assert_eq!(layout.edges, Vec::new());
        assert!(close(layout.height, 0.0));
    }

    #[test]
    fn sites_group_under_their_table_and_edges_meet_at_block_centre() {
        let layout = layout_graph(&graph(vec![
            site("b.rs", 1, "t2"),
            site("a.rs", 9, "t1"),
            site("a.rs", 3, "t1"),
            site("c.rs", 2, "t1"),
        ]));
        let order: Vec<_> = layout.sites.iter().map(|s| (&*s.table, s.line)).collect();
        assert_eq!(order, vec![("t1", 3), ("t1", 9), ("t1", 2), ("t2", 1)]);
        assert_eq!(layout.blocks.len(), 2);
        assert_eq!((layout.blocks[0].first_row, layout.blocks[0].rows), (0, 3));
        assert_eq!((layout.blocks[1].first_row, layout.blocks[1].rows), (3, 1));
        // Block of three: centre is the middle row's centre.
        assert!(close(layout.edges[0].table_y, ROW_H * 1.5));
        assert!(close(layout.edges[2].table_y, layout.edges[0].table_y));
        // Single-row block: edge is flat.
        assert!(close(layout.edges[3].site_y, layout.edges[3].table_y));
        assert!(close(layout.height, ROW_H * 4.0));
    }
}
