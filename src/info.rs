use std::fmt::Display;

use chrono::{DateTime, Local, Utc};
use serde::Serialize;

use crate::{config::Configuration, context::Context, scheduler::ActionTrigger};

#[derive(Serialize, PartialEq, Eq, Debug)]
pub struct EventInfo {
    pub at: DateTime<Utc>,
    pub trigger: ActionTrigger,
    pub action: Option<String>,
}

#[derive(Serialize)]
pub struct Info<'a> {
    pub next_event: Option<EventInfo>,
    pub configuration: &'a Configuration,
}

impl Display for EventInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("At: ")?;
        self.at.with_timezone(&Local).fmt(f)?;

        f.write_str("Action: ")?;
        if let Some(a) = &self.action {
            a.fmt(f)?;
        }
        f.write_str("Trigger: ")?;
        self.trigger.fmt(f)?;
        Ok(())
    }
}

pub struct InfoGatherer {
    pub next_event_at: Option<EventInfo>,
}
impl InfoGatherer {
    pub fn print(self, context: Context) -> crate::error::Result<()> {
        let info = Info {
            next_event: self.next_event_at,
            configuration: &context.config,
        };
        println!("{info}");
        Ok(())
    }

    pub fn new(next_event_at: Option<EventInfo>) -> Self {
        Self { next_event_at }
    }
}

impl<'a> Display for Info<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(ev) = &self.next_event {
            f.write_str("Event info: ")?;
            ev.fmt(f)?;
            f.write_str("\n")?;
        } else {
            f.write_str("No pending event\n")?;
        }
        self.configuration.fmt(f)?;
        Ok(())
    }
}
