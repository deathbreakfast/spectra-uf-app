//! The Variables dialog on the edit page: one row per variable with its name, the field its
//! values come from, a default and the panels that use it.

use std::collections::{BTreeMap, BTreeSet};

use leptos::prelude::*;
use orbital::components::{Caption1, DialogSize, EmptyState, SpacingSize};
use orbital::primitives::{
    Body1, Button, ButtonAppearance, ButtonSize, Dialog, DialogActions, DialogBody, DialogContent,
    DialogSurface, DialogTitle, Flex, Input, InputBind, InputPrefix, MessageBar, MessageBarIntent,
    MessageBarLayout, Select, SelectBind, Table, TableBody, TableCell, TableHeader,
    TableHeaderCell, TableRow,
};
use spectra_backend::dashboards::view::{missing_variable_fields, panel_source_name};
use spectra_backend::dashboards::{Dashboard, Panel, Variable};

/// `"one"` to `"ten"`, digits after that.
pub(super) fn count_word(n: usize) -> String {
    const WORDS: [&str; 11] = [
        "no", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    WORDS
        .get(n)
        .map_or_else(|| n.to_string(), |w| (*w).to_string())
}

/// `s` with its first letter in upper case.
pub(super) fn capitalise(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().chain(chars).collect()
    })
}

/// Why the variables can't be kept as they are, or `None` when they can.
pub(super) fn variable_problem(vars: &[Variable]) -> Option<String> {
    let mut seen = BTreeSet::new();
    for var in vars {
        let name = var.name.as_str();
        let valid = name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if name.is_empty() {
            return Some("Every variable needs a name.".to_string());
        }
        if !valid {
            return Some(format!(
                "${name} can only use letters, digits and underscores, and can't start with a digit."
            ));
        }
        if !seen.insert(name) {
            return Some(format!("${name} is used twice."));
        }
        if var.field.trim().is_empty() {
            return Some(format!("Pick the field ${name} takes its values from."));
        }
    }
    None
}

/// Panels a variable on `field` changes (by title) and how many query panels ignore it
/// because their source has no such field. Text panels count as neither.
pub(super) fn coverage(
    panels: &[Panel],
    field: &str,
    fields: &BTreeMap<String, Vec<String>>,
) -> (Vec<String>, usize) {
    let probe = [Variable {
        name: "v".into(),
        field: field.into(),
        default: None,
    }];
    let mut used_by = Vec::new();
    let mut ignored = 0;
    for panel in panels {
        let Some(source) = panel_source_name(panel) else {
            continue;
        };
        let known = fields.get(source).map(Vec::as_slice);
        if missing_variable_fields(panel, &probe, known).is_empty() {
            used_by.push(panel.title.clone());
        } else {
            ignored += 1;
        }
    }
    (used_by, ignored)
}

/// The note for a variable some panels ignore.
pub(super) fn ignored_text(name: &str, field: &str, ignored: usize) -> String {
    if ignored == 1 {
        format!(
            "One panel has no {field} field, so ${name} doesn't change it. It says so under its title."
        )
    } else {
        format!(
            "{} panels have no {field} field, so ${name} doesn't change them. Each says so under its title.",
            capitalise(&count_word(ignored))
        )
    }
}

/// Every field a variable could take its values from: the fields of the dashboard's sources.
pub(super) fn field_options(
    panels: &[Panel],
    fields: &BTreeMap<String, Vec<String>>,
) -> Vec<String> {
    let sources: BTreeSet<&str> = panels.iter().filter_map(panel_source_name).collect();
    let mut options: BTreeSet<String> = sources
        .into_iter()
        .filter_map(|s| fields.get(s))
        .flatten()
        .filter(|f| f.as_str() != "ts")
        .cloned()
        .collect();
    options.retain(|f| !f.is_empty());
    options.into_iter().collect()
}

#[derive(Clone, Copy)]
struct VarRow {
    key: usize,
    name: RwSignal<String>,
    field: RwSignal<String>,
    default: RwSignal<String>,
}

impl VarRow {
    fn new(key: usize, var: &Variable) -> Self {
        Self {
            key,
            name: RwSignal::new(var.name.clone()),
            field: RwSignal::new(var.field.clone()),
            default: RwSignal::new(var.default.clone().unwrap_or_default()),
        }
    }

    fn variable(&self) -> Variable {
        let default = self.default.get().trim().to_string();
        Variable {
            name: self.name.get().trim().to_string(),
            field: self.field.get(),
            default: (!default.is_empty()).then_some(default),
        }
    }
}

/// The Variables dialog. Done writes the rows into the draft; Cancel leaves it alone.
#[component]
pub(super) fn VariablesDialog(
    /// Whether the dialog is open.
    open: RwSignal<bool>,
    /// The dashboard being edited.
    draft: RwSignal<Dashboard>,
    /// Field names of each known source, by source name.
    source_fields: StoredValue<BTreeMap<String, Vec<String>>>,
) -> impl IntoView {
    let rows = RwSignal::new(Vec::<VarRow>::new());
    let next_key = StoredValue::new(0usize);
    Effect::new(move |_| {
        if open.get() {
            let vars = draft.with_untracked(|d| d.variables.clone());
            next_key.set_value(vars.len());
            rows.set(
                vars.iter()
                    .enumerate()
                    .map(|(i, v)| VarRow::new(i, v))
                    .collect(),
            );
        }
    });
    let panels = Memo::new(move |_| {
        draft.with(|d| {
            d.sections
                .iter()
                .flat_map(|s| s.panels.iter().cloned())
                .collect::<Vec<_>>()
        })
    });
    let options =
        Memo::new(move |_| panels.with(|p| source_fields.with_value(|f| field_options(p, f))));
    let current =
        Memo::new(move |_| rows.with(|r| r.iter().map(VarRow::variable).collect::<Vec<_>>()));
    let problem = Memo::new(move |_| current.with(|v| variable_problem(v)));
    let notes = Memo::new(move |_| {
        current.with(|vars| {
            panels.with(|p| {
                source_fields.with_value(|f| {
                    vars.iter()
                        .filter(|v| !v.name.is_empty() && !v.field.is_empty())
                        .filter_map(|v| {
                            let (_, ignored) = coverage(p, &v.field, f);
                            (ignored > 0).then(|| ignored_text(&v.name, &v.field, ignored))
                        })
                        .collect::<Vec<_>>()
                })
            })
        })
    });

    let add = Callback::new(move |_| {
        let key = next_key.get_value();
        next_key.set_value(key + 1);
        let field = options
            .with_untracked(|o| o.first().cloned())
            .unwrap_or_default();
        rows.update(|r| {
            r.push(VarRow::new(
                key,
                &Variable {
                    name: String::new(),
                    field,
                    default: None,
                },
            ));
        });
    });
    let done = Callback::new(move |_| {
        if problem.get_untracked().is_some() {
            return;
        }
        let vars = current.get_untracked();
        draft.update(|d| {
            if d.variables != vars {
                d.variables = vars;
            }
        });
        open.set(false);
    });

    view! {
        <Dialog open=open>
            <DialogSurface size=DialogSize::Large>
                <div data-testid="spectra-edit-variables-dialog">
                    <DialogBody>
                        <DialogTitle>"Variables"</DialogTitle>
                        <DialogContent>
                            <Flex vertical=true gap=SpacingSize::Size120.flex_gap()>
                                <Body1>
                                    "A variable becomes a picker at the top of the dashboard. Each panel whose source has the variable's field filters on the value picked."
                                </Body1>
                                <Show
                                    when=move || rows.with(|r| !r.is_empty())
                                    fallback=|| view! {
                                        <EmptyState
                                            message="No variables yet"
                                            description="Add one to give the dashboard a picker."
                                        />
                                    }
                                >
                                    <Table>
                                        <TableHeader>
                                            <TableRow>
                                                <TableHeaderCell>"Name"</TableHeaderCell>
                                                <TableHeaderCell>"Values from"</TableHeaderCell>
                                                <TableHeaderCell>"Default"</TableHeaderCell>
                                                <TableHeaderCell>"Used by"</TableHeaderCell>
                                                <TableHeaderCell>""</TableHeaderCell>
                                            </TableRow>
                                        </TableHeader>
                                        <TableBody>
                                            <For each=move || rows.get() key=|row| row.key let:row>
                                                <VariableRow
                                                    row=row
                                                    panels=panels
                                                    options=options
                                                    source_fields=source_fields
                                                    on_remove=Callback::new(move |key: usize| {
                                                        rows.update(|r| r.retain(|row| row.key != key));
                                                    })
                                                />
                                            </For>
                                        </TableBody>
                                    </Table>
                                </Show>
                                {move || notes.get().into_iter().map(|note| view! {
                                    <div data-testid="spectra-edit-variables-ignored">
                                        <MessageBar intent=MessageBarIntent::Info layout=MessageBarLayout::Multiline>
                                            {note}
                                        </MessageBar>
                                    </div>
                                }).collect_view()}
                                {move || problem.get().map(|message| view! {
                                    <div data-testid="spectra-edit-variables-problem">
                                        <MessageBar intent=MessageBarIntent::Warning>{message}</MessageBar>
                                    </div>
                                })}
                                <span data-testid="spectra-edit-variables-add">
                                    <Button appearance=ButtonAppearance::Subtle icon=icondata::AiPlusOutlined on_click=add>
                                        "Add variable"
                                    </Button>
                                </span>
                            </Flex>
                        </DialogContent>
                        <DialogActions>
                            <Button appearance=ButtonAppearance::Subtle on_click=Callback::new(move |_| open.set(false))>
                                "Cancel"
                            </Button>
                            <span data-testid="spectra-edit-variables-done">
                                <Button
                                    appearance=ButtonAppearance::Primary
                                    disabled=Signal::derive(move || problem.get().is_some())
                                    on_click=done
                                >
                                    "Done"
                                </Button>
                            </span>
                        </DialogActions>
                    </DialogBody>
                </div>
            </DialogSurface>
        </Dialog>
    }
}

#[component]
fn VariableRow(
    row: VarRow,
    panels: Memo<Vec<Panel>>,
    options: Memo<Vec<String>>,
    source_fields: StoredValue<BTreeMap<String, Vec<String>>>,
    on_remove: Callback<usize>,
) -> impl IntoView {
    let used_by = move || {
        let field = row.field.get();
        let (used, _) = panels.with(|p| source_fields.with_value(|f| coverage(p, &field, f)));
        if used.is_empty() {
            "No panels".to_string()
        } else {
            super::detail::join_names(&used)
        }
    };
    let field_choices = move || {
        let mut choices = options.get();
        let field = row.field.get_untracked();
        if !field.is_empty() && !choices.contains(&field) {
            choices.insert(0, field);
        }
        choices
    };
    let label = move || {
        let name = row.name.get();
        if name.is_empty() {
            "variable".to_string()
        } else {
            format!("${name}")
        }
    };
    view! {
        <TableRow>
            <TableCell>
                <span data-testid="spectra-edit-variable-name">
                    <Input bind=InputBind::new(row.name) attr:aria-label="Variable name">
                        <InputPrefix slot>"$"</InputPrefix>
                    </Input>
                </span>
            </TableCell>
            <TableCell>
                <span data-testid="spectra-edit-variable-field">
                    <Select bind=SelectBind::new(row.field) attr:aria-label=move || format!("Field for {}", label())>
                        {move || field_choices().into_iter().map(|f| {
                            let label = f.clone();
                            view! { <option value=f>{label}</option> }
                        }).collect_view()}
                    </Select>
                </span>
            </TableCell>
            <TableCell>
                <span data-testid="spectra-edit-variable-default">
                    <Input bind=InputBind::new(row.default) attr:aria-label=move || format!("Default for {}", label())>
                    </Input>
                </span>
            </TableCell>
            <TableCell>
                <span data-testid="spectra-edit-variable-used-by">
                    <Caption1>{used_by}</Caption1>
                </span>
            </TableCell>
            <TableCell>
                <span data-testid="spectra-edit-variable-remove">
                    <Button
                        appearance=ButtonAppearance::Subtle
                        size=ButtonSize::Small
                        icon=icondata::AiDeleteOutlined
                        attr:aria-label=move || format!("Remove {}", label())
                        on_click=Callback::new(move |_| on_remove.run(row.key))
                    />
                </span>
            </TableCell>
        </TableRow>
    }
}

#[cfg(test)]
mod tests {
    use spectra_backend::dashboards::{ChartKind, Layout, PanelSource};

    use super::*;

    fn var(name: &str, field: &str) -> Variable {
        Variable {
            name: name.into(),
            field: field.into(),
            default: None,
        }
    }

    fn panel(title: &str, source: PanelSource) -> Panel {
        Panel {
            id: title.into(),
            title: title.into(),
            source,
            chart: ChartKind::TimeSeries,
            measure: None,
            bucket: None,
            group_by: Vec::new(),
            filters: Vec::new(),
            table: None,
            layout: Layout {
                col: 0,
                span: 6,
                row: 0,
                height: 2,
            },
            keep_window: None,
        }
    }

    fn table(name: &str) -> PanelSource {
        PanelSource::EventTable { table: name.into() }
    }

    #[test]
    fn variable_names_must_be_unique_identifiers() {
        assert_eq!(variable_problem(&[var("host", "host")]), None);
        assert!(variable_problem(&[var("", "host")]).is_some());
        assert!(variable_problem(&[var("9lives", "host")]).is_some());
        assert!(variable_problem(&[var("a-b", "host")]).is_some());
        assert_eq!(
            variable_problem(&[var("host", "host"), var("host", "region")]),
            Some("$host is used twice.".into())
        );
        assert!(variable_problem(&[var("host", "")]).is_some());
    }

    #[test]
    fn coverage_counts_panels_without_the_field() {
        let fields = BTreeMap::from([
            (
                "checkout".to_string(),
                vec!["host".to_string(), "status".to_string()],
            ),
            ("refunds".to_string(), vec!["reason".to_string()]),
        ]);
        let panels = vec![
            panel("Checkouts", table("checkout")),
            panel("Refunds", table("refunds")),
            panel("Unknown", table("not_registered")),
            panel(
                "Notes",
                PanelSource::Text {
                    markdown: String::new(),
                },
            ),
        ];
        let (used, ignored) = coverage(&panels, "host", &fields);
        assert_eq!(used, ["Checkouts", "Unknown"]);
        assert_eq!(ignored, 1);
    }

    #[test]
    fn ignored_text_reads_naturally() {
        assert_eq!(
            ignored_text("host", "host", 1),
            "One panel has no host field, so $host doesn't change it. It says so under its title."
        );
        assert_eq!(
            ignored_text("host", "host", 5),
            "Five panels have no host field, so $host doesn't change them. Each says so under its title."
        );
        assert!(ignored_text("host", "host", 12).starts_with("12 panels"));
    }

    #[test]
    fn field_options_come_from_the_dashboard_sources() {
        let fields = BTreeMap::from([
            (
                "checkout".to_string(),
                vec!["ts".to_string(), "host".to_string()],
            ),
            (
                "refunds".to_string(),
                vec!["reason".to_string(), "host".to_string()],
            ),
            ("unused".to_string(), vec!["other".to_string()]),
        ]);
        let panels = vec![panel("a", table("checkout")), panel("b", table("refunds"))];
        assert_eq!(field_options(&panels, &fields), ["host", "reason"]);
    }
}
