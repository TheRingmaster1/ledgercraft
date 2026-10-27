//! Recurring transactions: Lists > Recurring Transactions, and the Make
//! Recurring window a form's Make Recurring… button opens.

use eframe::egui::{self, RichText, Ui};
use ledgercraft_core::{Date, Money};
use ledgercraft_engine::Command;
use ledgercraft_ledger::{
    Company, Frequency, Recurring, RecurringId, RecurringMode, RecurringSpec, TxnTemplate,
};

use crate::widgets::{self, field, money_label};
use crate::{theme, Action, FormKind, Reset};

/// A recurring transaction's settings as typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpecInput {
    pub name: String,
    pub frequency: Frequency,
    pub every: String,
    pub next: String,
    pub end: String,
    pub mode: RecurringMode,
}

impl SpecInput {
    fn new(name: String, next: Date) -> Self {
        Self {
            name,
            frequency: Frequency::Monthly,
            every: "1".into(),
            next: next.to_string(),
            end: String::new(),
            mode: RecurringMode::Remind,
        }
    }

    fn from_recurring(r: &Recurring) -> Self {
        let spec = r.spec();
        Self {
            name: spec.name,
            frequency: spec.frequency,
            every: spec.every.to_string(),
            next: spec.next.to_string(),
            end: spec.end.map_or_else(String::new, |d| d.to_string()),
            mode: spec.mode,
        }
    }

    fn build(&self) -> Result<RecurringSpec, String> {
        let every = self
            .every
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or("How often: enter a whole number, 1 or more.")?;
        let next = Date::parse(&self.next).map_err(|e| format!("Next date: {e}"))?;
        let end = if self.end.trim().is_empty() {
            None
        } else {
            Some(Date::parse(&self.end).map_err(|e| format!("End date: {e}"))?)
        };
        Ok(RecurringSpec {
            name: self.name.trim().to_string(),
            frequency: self.frequency,
            every,
            next,
            end,
            mode: self.mode,
        })
    }
}

/// The Make Recurring window: the transaction from the form, and its schedule.
pub(crate) struct NewRecurring {
    kind: FormKind,
    template: TxnTemplate,
    input: SpecInput,
    error: Option<String>,
}

/// What the Make Recurring window asks for.
pub(crate) enum RecurringOutcome {
    /// Boxed: a command is large.
    Save(Box<Command>),
    Cancel,
}

#[derive(Default)]
pub(crate) struct RecurringState {
    pub new_recurring: Option<NewRecurring>,
    /// The one whose settings are being changed.
    pub editing: Option<(RecurringId, SpecInput)>,
    pub show_inactive: bool,
}

impl RecurringState {
    pub(crate) fn reset(&mut self, what: Reset) {
        if what == Reset::EditedRecurring {
            self.editing = None;
        }
    }

    /// Opens the Make Recurring window for what a form would record.
    pub(crate) fn make_recurring(&mut self, c: &Company, kind: FormKind, cmd: Command) {
        let Command::TxnPost {
            date,
            memo,
            body,
            class,
            ..
        } = cmd
        else {
            return;
        };
        let name = match body.party() {
            Some(p) => format!("{} {}", c.party_name(p), body.label()),
            None if !memo.trim().is_empty() => memo.trim().to_string(),
            None => body.label().to_string(),
        };
        self.new_recurring = Some(NewRecurring {
            kind,
            input: SpecInput::new(name, date.add_months(1)),
            template: TxnTemplate {
                date,
                memo,
                body,
                class,
            },
            error: None,
        });
    }

    pub(crate) fn set_error(&mut self, e: String) {
        if let Some(m) = self.new_recurring.as_mut() {
            m.error = Some(e);
        }
    }
}

/// Name, how often, next and end dates, and what happens when it is due.
fn spec_fields(ui: &mut Ui, salt: &str, s: &mut SpecInput) {
    egui::Grid::new(("recurring_fields", salt))
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            widgets::hint_label(ui, "Name");
            field(ui, &mut s.name, 240.0, "e.g. Shop rent");
            ui.end_row();

            widgets::hint_label(ui, "How often");
            ui.horizontal(|ui| {
                ui.label("Every");
                field(ui, &mut s.every, 40.0, "1");
                let n = s.every.trim().parse::<u32>().unwrap_or(1);
                egui::ComboBox::from_id_salt(("recurring_frequency", salt))
                    .selected_text(s.frequency.unit(n))
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        for f in Frequency::ALL {
                            ui.selectable_value(&mut s.frequency, f, f.unit(n));
                        }
                    });
            });
            ui.end_row();

            widgets::hint_label(ui, "Next date");
            field(ui, &mut s.next, 110.0, "YYYY-MM-DD");
            ui.end_row();

            widgets::hint_label(ui, "End date");
            field(ui, &mut s.end, 110.0, "none")
                .on_hover_text("Leave empty to keep it going until you pause or delete it");
            ui.end_row();

            widgets::hint_label(ui, "When due");
            ui.vertical(|ui| {
                ui.radio_value(&mut s.mode, RecurringMode::Remind, "Remind me")
                    .on_hover_text(
                        "List it as due on Home and Lists > Recurring Transactions; you open \
                         it, check it and save it",
                    );
                ui.radio_value(
                    &mut s.mode,
                    RecurringMode::Automatic,
                    "Enter it automatically",
                )
                .on_hover_text(
                    "Entered for you, on its date, when the company is opened on or after it",
                );
            });
            ui.end_row();
        });
}

/// What a recurring transaction is: "Check · Landlord · $1,200.00".
fn describe(c: &Company, template: &TxnTemplate) -> String {
    let mut parts = vec![template.body.label().to_string()];
    if let Some(p) = template.body.party() {
        parts.push(c.party_name(p));
    }
    if let Some(total) = template.body.total() {
        parts.push(crate::currency::format(total));
    }
    parts.join(" · ")
}

pub(crate) fn make_recurring_window(
    ctx: &egui::Context,
    m: &mut NewRecurring,
    c: &Company,
) -> Option<RecurringOutcome> {
    let mut outcome = None;
    let mut open = true;
    egui::Window::new("Make Recurring")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_max_width(460.0);
            ui.label(RichText::new(describe(c, &m.template)).strong());
            ui.label(
                RichText::new(format!(
                    "The {} as it is on the form now is saved with a schedule. Each time it \
                     is due it is entered with that day's date (due dates and sales tax \
                     follow). The form itself is not saved by this.",
                    m.kind.title().to_lowercase()
                ))
                .color(theme::MUTED),
            );
            ui.add_space(8.0);
            spec_fields(ui, "new", &mut m.input);
            if let Some(e) = &m.error {
                ui.add_space(4.0);
                ui.label(RichText::new(e.as_str()).color(theme::NEGATIVE));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(RichText::new("Save Schedule").strong()).clicked() {
                    match m.input.build() {
                        Ok(spec) => {
                            outcome = Some(RecurringOutcome::Save(Box::new(
                                Command::RecurringCreate {
                                    spec,
                                    template: m.template.clone(),
                                },
                            )));
                        }
                        Err(e) => m.error = Some(e),
                    }
                }
                if ui.button("Cancel").clicked() {
                    outcome = Some(RecurringOutcome::Cancel);
                }
            });
        });
    if !open {
        outcome = Some(RecurringOutcome::Cancel);
    }
    outcome
}

/// The settings panel above the list, for the one being changed.
fn edit_panel(ui: &mut Ui, c: &Company, s: &mut RecurringState, out: &mut Vec<Action>) {
    let Some((id, input)) = s.editing.as_mut() else {
        return;
    };
    let id = *id;
    let Some(r) = c.recurring(id) else {
        s.editing = None;
        return;
    };
    let mut close = false;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(RichText::new(format!("Edit: {}", r.name)).strong());
        ui.label(RichText::new(describe(c, &r.template)).color(theme::MUTED));
        ui.label(
            RichText::new(
                "To change what it enters, enter it once, change it in its form, then \
                 Make Recurring… again and delete this one.",
            )
            .color(theme::MUTED),
        );
        ui.add_space(4.0);
        spec_fields(ui, "edit", input);
        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if ui.button(RichText::new("Save Changes").strong()).clicked() {
                match input.build() {
                    Ok(spec) => out.push(Action::Run {
                        cmd: Command::RecurringUpdate { id, spec },
                        reset: Reset::EditedRecurring,
                    }),
                    Err(e) => out.push(Action::Error(e)),
                }
            }
            if ui.button("Cancel").clicked() {
                close = true;
            }
            if ui
                .button("Delete")
                .on_hover_text("Forget it. Transactions it already entered stay. Edit > Undo puts it back.")
                .clicked()
            {
                out.push(Action::Run {
                    cmd: Command::RecurringDelete { id },
                    reset: Reset::EditedRecurring,
                });
            }
        });
    });
    if close {
        s.editing = None;
    }
    ui.add_space(6.0);
}

/// Lists > Recurring Transactions.
pub(crate) fn page(ui: &mut Ui, c: &Company, s: &mut RecurringState, out: &mut Vec<Action>) {
    ui.label(
        RichText::new("Recurring Transactions")
            .heading()
            .color(theme::ACCENT),
    );
    ui.add_space(6.0);
    ui.label(
        RichText::new(
            "Fill in an invoice, sales receipt, bill, check, deposit or journal entry, then \
             click Make Recurring… on its form to save it with a schedule. Reminders are listed \
             here and on Home when due; automatic ones are entered when the company is \
             opened on or after their date.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(6.0);
    if c.recurring.is_empty() {
        ui.label(RichText::new("No recurring transactions yet.").color(theme::MUTED));
        return;
    }
    ui.checkbox(&mut s.show_inactive, "Show paused and ended");
    edit_panel(ui, c, s, out);
    let today = Date::today();
    let mut list: Vec<&Recurring> = c
        .recurring
        .iter()
        .filter(|r| s.show_inactive || (r.active && r.next().is_some()))
        .collect();
    list.sort_by_key(|r| (r.next().is_none(), r.next(), r.name.to_lowercase()));
    let mut edit = None;
    egui::Grid::new("recurring")
        .striped(true)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for h in ["Name", "Type", "Name on it", "Amount", "How often", "Next", "", ""] {
                widgets::column_header(ui, h);
            }
            ui.end_row();
            for r in list {
                if r.active {
                    ui.label(r.name.as_str());
                } else {
                    ui.label(RichText::new(format!("{} (paused)", r.name)).color(theme::MUTED));
                }
                ui.label(r.template.body.label());
                ui.label(
                    r.template
                        .body
                        .party()
                        .map(|p| c.party_name(p))
                        .unwrap_or_default(),
                );
                money_label(ui, r.amount().unwrap_or(Money::ZERO), false);
                ui.label(r.schedule());
                match r.next() {
                    Some(next) if r.is_due(today) => {
                        ui.label(RichText::new(format!("{next} · due")).color(theme::WARNING));
                    }
                    Some(next) => {
                        ui.label(next.to_string());
                    }
                    None => {
                        ui.label(RichText::new("Ended").color(theme::MUTED));
                    }
                }
                let mode = match r.mode {
                    RecurringMode::Remind => "Reminder",
                    RecurringMode::Automatic => "Automatic",
                };
                ui.label(RichText::new(mode).color(theme::MUTED));
                ui.horizontal(|ui| {
                    let open = r.active && r.next().is_some();
                    if ui
                        .add_enabled(open, egui::Button::new("Enter…").small())
                        .on_hover_text("Open the next one in its form to check it, then Save")
                        .clicked()
                    {
                        out.push(Action::EnterRecurring(r.id));
                    }
                    if ui
                        .add_enabled(open, egui::Button::new("Skip").small())
                        .on_hover_text("Move to the next date without entering this one")
                        .clicked()
                    {
                        out.push(Action::Run {
                            cmd: Command::RecurringSkip { id: r.id },
                            reset: Reset::Nothing,
                        });
                    }
                    if ui.small_button("Edit").on_hover_text("Name and schedule").clicked() {
                        edit = Some((r.id, SpecInput::from_recurring(r)));
                    }
                    let (label, hover) = if r.active {
                        ("Pause", "Stop it being due or entered until you resume it")
                    } else {
                        ("Resume", "Due and entered on its schedule again")
                    };
                    if ui.small_button(label).on_hover_text(hover).clicked() {
                        out.push(Action::Run {
                            cmd: Command::RecurringSetActive {
                                id: r.id,
                                active: !r.active,
                            },
                            reset: Reset::Nothing,
                        });
                    }
                });
                ui.end_row();
            }
        });
    if edit.is_some() {
        s.editing = edit;
    }
}

/// Home: a line when reminders are due, linking to the list.
pub(crate) fn due_banner(ui: &mut Ui, c: &Company, out: &mut Vec<Action>) {
    let due = c.recurring_due(Date::today()).len();
    if due == 0 {
        return;
    }
    let text = if due == 1 {
        "1 recurring transaction is due: enter or skip it".to_string()
    } else {
        format!("{due} recurring transactions are due: enter or skip them")
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new("●").color(theme::WARNING));
        if ui.link(text).clicked() {
            out.push(Action::Go(crate::Page::Recurring));
        }
    });
    ui.add_space(6.0);
}

/// Above a form holding a recurring transaction's next occurrence: what
/// saving does. True when Stop was clicked (the form becomes an ordinary
/// new transaction).
pub(crate) fn entering_banner(ui: &mut Ui, c: &Company, id: RecurringId) -> bool {
    let Some(r) = c.recurring(id) else {
        return true;
    };
    let mut stop = false;
    egui::Frame::group(ui.style())
        .fill(theme::ACCENT_SOFT)
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                let next = r.next().map_or_else(String::new, |d| format!(" due {d}"));
                ui.label(format!(
                    "Entering recurring \u{201c}{}\u{201d}{next}. Check it, then Save: {} \
                     moves on to its next date.",
                    r.name, r.name
                ));
                if ui
                    .small_button("Stop")
                    .on_hover_text("Keep the form as an ordinary new transaction")
                    .clicked()
                {
                    stop = true;
                }
            });
        });
    ui.add_space(6.0);
    stop
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_settings_build_a_spec() {
        let mut s = SpecInput::new("Rent".into(), Date::parse("2026-11-01").unwrap());
        s.every = "2".into();
        s.end = "2027-12-31".into();
        let spec = s.build().unwrap();
        assert_eq!(spec.every, 2);
        assert_eq!(spec.end, Date::parse("2027-12-31").ok());
        s.every = "0".into();
        assert!(s.build().is_err());
        s.every = "1".into();
        s.next = "someday".into();
        assert!(s.build().is_err());
    }

    #[test]
    fn the_page_and_window_draw_without_actions() {
        let c = ledgercraft_engine::sample_company().unwrap();
        let mut s = RecurringState {
            show_inactive: true,
            ..RecurringState::default()
        };
        let first = c.recurring.first().unwrap();
        s.editing = Some((first.id, SpecInput::from_recurring(first)));
        let post = Command::TxnPost {
            date: first.template.date,
            number: String::new(),
            memo: first.template.memo.clone(),
            body: first.template.body.clone(),
            class: None,
        };
        s.make_recurring(&c, FormKind::Check, post);
        assert!(s.new_recurring.is_some());
        let ctx = egui::Context::default();
        crate::theme::apply(&ctx);
        let mut out = Vec::new();
        crate::theme::test_frame(&ctx, |ui| page(ui, &c, &mut s, &mut out));
        assert!(out.is_empty());
        let mut outcome = None;
        let mut m = s.new_recurring.take().unwrap();
        crate::theme::test_frame(&ctx, |ui| outcome = make_recurring_window(ui.ctx(), &mut m, &c));
        assert!(outcome.is_none());
    }
}
