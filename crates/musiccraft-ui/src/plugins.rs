//! Plugin pickers and the generic plugin parameter panel.

use crate::hint::Hint;
use egui::RichText;
use musiccraft_commands::Invocation;
use musiccraft_project::PluginRef;

use crate::{Action, UiState, theme};

/// An installed plugin, as the app's plugin host reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginEntry {
    /// Plugin id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Maker.
    pub vendor: String,
    /// `true` for instruments, `false` for effects.
    pub instrument: bool,
    /// Plugin format as shown in the browser ("CLAP", "VST3"), or empty.
    pub format: String,
}

/// One parameter of a running plugin, with its current value.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginParamView {
    /// Parameter id.
    pub id: u32,
    /// Display name.
    pub name: String,
    /// Group path, possibly empty.
    pub module: String,
    /// Lowest value.
    pub min: f64,
    /// Highest value.
    pub max: f64,
    /// Default value.
    pub default: f64,
    /// Whether only whole numbers are valid.
    pub stepped: bool,
    /// Whether the host may not change it.
    pub read_only: bool,
    /// Current value.
    pub value: f64,
    /// Current value as the plugin formats it.
    pub text: String,
}

/// A running plugin instance the UI is showing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LivePluginView {
    /// Instance number.
    pub instance: u32,
    /// Visible parameters.
    pub params: Vec<PluginParamView>,
    /// Whether it has its own window.
    pub has_editor: bool,
    /// Whether that window is open.
    pub editor_open: bool,
}

/// Plugin information for one frame. Read-only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Plugins {
    /// Whether this build can host plugins at all.
    pub available: bool,
    /// The plugin folders were not scanned at start and no rescan ran yet.
    pub unscanned: bool,
    /// Installed plugins.
    pub catalog: Vec<PluginEntry>,
    /// Running instances the UI asked about (the selected channel and effect).
    pub live: Vec<LivePluginView>,
    /// Instance numbers of every running plugin.
    pub running: Vec<u32>,
}

impl Plugins {
    /// The view of a running instance, if the app provided one.
    #[must_use]
    pub fn view(&self, instance: u32) -> Option<&LivePluginView> {
        self.live.iter().find(|v| v.instance == instance)
    }

    /// Installed instruments or effects, by name.
    pub fn of_kind(&self, instrument: bool) -> impl Iterator<Item = &PluginEntry> {
        self.catalog
            .iter()
            .filter(move |p| p.instrument == instrument)
    }
}

/// Where a plugin sits, in command numbering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// Channel number (1-based).
    Channel(usize),
    /// Mixer track (0 = master) and slot number (1-based).
    Effect(usize, usize),
}

impl Target {
    fn set_param(self, id: u32, value: &str) -> Invocation {
        match self {
            Self::Channel(c) => Invocation::new(
                "channel.set_plugin_param",
                [c.to_string(), id.to_string(), value.to_owned()],
            ),
            Self::Effect(t, s) => Invocation::new(
                "mixer.set_plugin_param",
                [
                    t.to_string(),
                    s.to_string(),
                    id.to_string(),
                    value.to_owned(),
                ],
            ),
        }
    }
}

/// Menu entries that add an installed plugin; `add` builds the invocation from
/// the plugin's id and name. Returns `true` if one was chosen.
pub(crate) fn add_menu(
    ui: &mut egui::Ui,
    plugins: &Plugins,
    instrument: bool,
    actions: &mut Vec<Action>,
    add: impl Fn(&PluginEntry) -> Invocation,
) -> bool {
    let mut chosen = false;
    let mut any = false;
    egui::ScrollArea::vertical()
        .id_salt(("plugin-menu", instrument))
        .max_height(320.0)
        .show(ui, |ui| {
            for entry in plugins.of_kind(instrument) {
                any = true;
                if ui
                    .button(&entry.name)
                    .hint(format!("{} · {}", entry.vendor, entry.id))
                    .clicked()
                {
                    actions.push(Action::Run(add(entry)));
                    chosen = true;
                    ui.close();
                }
            }
        });
    if !any {
        let text = if plugins.unscanned {
            "Plugins not scanned yet"
        } else if plugins.available {
            "No CLAP plugins found"
        } else {
            "Plugin hosting is not available"
        };
        ui.label(RichText::new(text).small().color(theme::TEXT_DIM));
    }
    if plugins.available && ui.small_button("Rescan plugin folders").clicked() {
        actions.push(Action::RescanPlugins);
        ui.close();
    }
    chosen
}

/// The parameter panel for a plugin: header, editor button and one slider per
/// parameter.
pub(crate) fn panel(
    ui: &mut egui::Ui,
    state: &mut UiState,
    plugins: &Plugins,
    plugin: &PluginRef,
    target: Target,
    actions: &mut Vec<Action>,
) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(&plugin.name).color(theme::ACCENT));
        ui.label(RichText::new(&plugin.id).small().color(theme::TEXT_DIM));
    });
    state.plugin_layout = (None, Vec::new());
    let Some(view) = plugins.view(plugin.instance) else {
        let installed = plugins.catalog.iter().any(|p| p.id == plugin.id);
        let text = if !plugins.available {
            "Plugin hosting is not available in this build."
        } else if installed {
            "This plugin failed to start; see the status bar."
        } else {
            "This plugin is not installed. Its settings are kept in the project."
        };
        ui.label(RichText::new(text).color(theme::HIGHLIGHT));
        return;
    };
    ui.horizontal(|ui| {
        if view.has_editor {
            let label = if view.editor_open {
                "Close plugin window"
            } else {
                "Open plugin window"
            };
            let button = ui.button(label);
            state.plugin_layout.0 = Some(button.rect);
            if button.clicked() {
                actions.push(Action::PluginEditor {
                    instance: plugin.instance,
                    open: !view.editor_open,
                });
            }
        }
        ui.add(
            egui::TextEdit::singleline(&mut state.plugin_filter)
                .hint_text("Filter parameters")
                .desired_width(140.0),
        );
    });
    let filter = state.plugin_filter.to_lowercase();
    egui::ScrollArea::vertical()
        .id_salt(("plugin-params", plugin.instance))
        .max_height(280.0)
        .show(ui, |ui| {
            egui::Grid::new(("plugin-param-grid", plugin.instance))
                .num_columns(3)
                .striped(true)
                .show(ui, |ui| {
                    let shown = view.params.iter().filter(|p| {
                        filter.is_empty()
                            || p.name.to_lowercase().contains(&filter)
                            || p.module.to_lowercase().contains(&filter)
                    });
                    for param in shown {
                        param_row(ui, state, param, plugin, target, actions);
                    }
                });
            if view.params.is_empty() {
                ui.label(
                    RichText::new("This plugin exposes no parameters.")
                        .small()
                        .color(theme::TEXT_DIM),
                );
            }
        });
}

fn param_row(
    ui: &mut egui::Ui,
    state: &mut UiState,
    param: &PluginParamView,
    plugin: &PluginRef,
    target: Target,
    actions: &mut Vec<Action>,
) {
    let stored = plugin.params.contains_key(&param.id);
    let name = RichText::new(&param.name).color(if stored { theme::TEXT } else { theme::TEXT_DIM });
    ui.label(name)
        .hint(format!("id {} {}", param.id, param.module));
    let mut value = param.value;
    let slider = egui::Slider::new(&mut value, param.min..=param.max.max(param.min))
        .show_value(false)
        .step_by(if param.stepped { 1.0 } else { 0.0 });
    let response = crate::widgets::add_enabled(ui, !param.read_only, slider);
    state.plugin_layout.1.push((param.id, response.rect));
    // A slider jumps on press, before egui reports a drag; the gesture starts
    // with the press.
    if response.is_pointer_button_down_on() && ui.input(|i| i.pointer.any_pressed()) {
        state.begin_gesture();
    }
    let reset = crate::widgets::wants_reset(&response);
    if response.changed() && value != param.value && !reset {
        actions.push(Action::Coalesced(
            target.set_param(param.id, &value.to_string()),
            state.gesture(),
        ));
    }
    if reset {
        actions.push(Action::Run(
            target.set_param(param.id, &param.default.to_string()),
        ));
    }
    response.context_menu(|ui| {
        if ui.button("Reset to default").clicked() {
            actions.push(Action::Run(
                target.set_param(param.id, &param.default.to_string()),
            ));
            ui.close();
        }
        if stored && ui.button("Forget stored value").clicked() {
            actions.push(Action::Run(target.set_param(param.id, "unset")));
            ui.close();
        }
    });
    ui.label(RichText::new(&param.text).monospace().small());
    ui.end_row();
}
