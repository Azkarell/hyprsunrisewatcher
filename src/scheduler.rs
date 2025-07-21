use std::fmt::{Debug, Display};

use chrono::{Days, prelude::*};
use serde::{Deserialize, Serialize};
use sunrise::{Coordinates, SolarDay, SolarEvent};

use crate::{
    config::{Actions, Configuration, ManualTimeStamp},
    info::EventInfo,
};

pub struct Scheduler<T: Trigger> {
    actions: Actions,
    trigger: T,
}

pub struct TriggerSource {
    event_source: Box<dyn EventSource>,
}

impl TriggerSource {
    pub fn from_config(config: &Configuration) -> crate::error::Result<Self> {
        if let Some(auto) = &config.automatic {
            Ok(Self {
                event_source: Box::new(Scheduler::automatic(
                    (auto.latitude, auto.longitude),
                    config.actions.clone(),
                )),
            })
        } else if let Some(manual) = &config.manual {
            Ok(TriggerSource {
                event_source: Box::new(Scheduler::manual(
                    manual.time_stamps.clone(),
                    config.actions.clone(),
                )),
            })
        } else {
            Err(crate::error::Error::InvalidConfiguration.into())
        }
    }
}

impl EventSource for TriggerSource {
    fn next_event_at(&self, date: DateTime<Utc>) -> Option<EventInfo> {
        self.event_source.next_event_at(date)
    }
}

impl<T: Trigger> EventSource for Scheduler<T> {
    fn next_event_at(&self, date: DateTime<Utc>) -> Option<EventInfo> {
        let trigger = self.trigger.next_action_at(date);
        trigger.map(|(action, at)| EventInfo {
            at,
            trigger: action,
            action: self.get_action(action),
        })
    }
}
pub trait EventSource {
    fn next_event_at(&self, date: DateTime<Utc>) -> Option<EventInfo>;
}

pub trait Trigger {
    fn next_action_at(&self, date: DateTime<Utc>) -> Option<(ActionTrigger, DateTime<Utc>)>;
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionTrigger {
    Sunrise,
    Sunset,
    Dusk,
    Dawn,
}
impl Display for ActionTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self, f)
    }
}

impl ActionTrigger {
    pub fn next(self) -> Self {
        match self {
            ActionTrigger::Sunrise => ActionTrigger::Sunset,
            ActionTrigger::Sunset => ActionTrigger::Dusk,
            ActionTrigger::Dusk => ActionTrigger::Dawn,
            ActionTrigger::Dawn => ActionTrigger::Sunrise,
        }
    }
}

impl Scheduler<LocationInfo> {
    pub fn automatic<L: Into<LocationInfo>>(trigger: L, actions: Actions) -> Self {
        Self {
            actions,
            trigger: trigger.into(),
        }
    }
}

impl Trigger for Vec<ManualTimeStamp> {
    fn next_action_at(&self, date: DateTime<Utc>) -> Option<(ActionTrigger, DateTime<Utc>)> {
        let min = self.iter().min_by(move |a, b| {
            (a.trigger_time - date.naive_local().time())
                .cmp(&(b.trigger_time - date.naive_local().time()))
        });
        min.map(|m| (m.action, Utc::now().with_time(m.trigger_time).unwrap()))
    }
}
impl Scheduler<Vec<ManualTimeStamp>> {
    pub fn manual(time_stamps: Vec<ManualTimeStamp>, actions: Actions) -> Self {
        Self {
            actions,
            trigger: time_stamps,
        }
    }
}

impl<T: Trigger> Scheduler<T> {
    pub fn get_action(&self, trigger: ActionTrigger) -> Option<String> {
        self.actions.get(trigger)
    }
}

pub struct LocationInfo {
    coords: Coordinates,
}
impl From<(f64, f64)> for LocationInfo {
    fn from(value: (f64, f64)) -> Self {
        Coordinates::new(value.0, value.1).unwrap().into()
    }
}

impl Trigger for LocationInfo {
    fn next_action_at(&self, date: DateTime<Utc>) -> Option<(ActionTrigger, DateTime<Utc>)> {
        let interval = self.interval_at(date);

        Some((interval.event.next(), interval.end))
    }
}
impl LocationInfo {
    pub fn new(coords: Coordinates) -> Self {
        Self { coords }
    }

    pub fn interval_at(&self, date: DateTime<Utc>) -> Interval {
        Interval::new(self.coords, date)
    }
}
impl From<Coordinates> for LocationInfo {
    fn from(value: Coordinates) -> Self {
        Self::new(value)
    }
}
pub struct Interval {
    #[allow(dead_code)]
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    event: ActionTrigger,
}

impl Interval {
    fn new(coords: Coordinates, date: DateTime<Utc>) -> Self {
        let now = date;
        let relevant_days = [
            SolarDay::new(
                coords,
                now.checked_sub_days(Days::new(1)).unwrap().date_naive(),
            ),
            SolarDay::new(coords, now.date_naive()),
            SolarDay::new(
                coords,
                now.checked_add_days(Days::new(1)).unwrap().date_naive(),
            ),
        ];

        let today_dawn = relevant_days[1].event_time(SolarEvent::Dawn(sunrise::DawnType::Civil));
        let yesterday_dusk =
            relevant_days[0].event_time(SolarEvent::Dusk(sunrise::DawnType::Civil));
        if yesterday_dusk < now && now < today_dawn {
            return Self {
                start: yesterday_dusk,
                end: today_dawn,
                event: ActionTrigger::Dusk,
            };
        }
        let today_sunrise = relevant_days[1].event_time(SolarEvent::Sunrise);
        if today_dawn <= now && now < today_sunrise {
            return Self {
                start: today_dawn,
                end: today_sunrise,
                event: ActionTrigger::Dawn,
            };
        }
        let today_sunset = relevant_days[1].event_time(SolarEvent::Sunset);
        if today_sunrise <= now && now < today_sunset {
            return Self {
                start: today_sunrise,
                end: today_sunset,
                event: ActionTrigger::Sunrise,
            };
        }
        let today_dusk = relevant_days[1].event_time(SolarEvent::Dusk(sunrise::DawnType::Civil));
        if today_sunset <= now && now < today_dusk {
            return Self {
                start: today_sunset,
                end: today_dusk,
                event: ActionTrigger::Sunset,
            };
        }
        let tomorrow_dawn = relevant_days[2].event_time(SolarEvent::Dawn(sunrise::DawnType::Civil));
        Self {
            start: today_dusk,
            end: tomorrow_dawn,
            event: ActionTrigger::Dusk,
        }
    }

    pub fn current_event(&self) -> ActionTrigger {
        self.event
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Utc};
    use sunrise::Coordinates;

    use crate::scheduler::ActionTrigger;

    use super::Interval;
    fn test_date_sunrise() -> DateTime<Utc> {
        DateTime::from_timestamp(1752414761, 0).unwrap()
    }

    fn test_date_dusk() -> DateTime<Utc> {
        DateTime::from_timestamp(1752364594, 0).unwrap()
    }
    fn test_date_dawn() -> DateTime<Utc> {
        DateTime::from_timestamp(1752364594 + 3 * 60 * 60, 0).unwrap()
    }
    fn test_date_sunset() -> DateTime<Utc> {
        DateTime::from_timestamp(1752414761 + 6 * 60 * 60, 0).unwrap()
    }
    fn test_date_00() -> DateTime<Utc> {
        DateTime::from_timestamp(1752357600, 0).unwrap()
    }

    fn test_date_23_59_59() -> DateTime<Utc> {
        DateTime::from_timestamp(1752443999, 0).unwrap()
    }
    #[test]
    fn interval_at_sunrise_works() {
        let coords = Coordinates::new(49.598121, 11.003653).unwrap();

        let interval = Interval::new(coords, test_date_sunrise());

        assert_eq!(interval.current_event(), ActionTrigger::Sunrise)
    }

    #[test]
    fn interval_at_dusk_works() {
        let coords = Coordinates::new(49.598121, 11.003653).unwrap();

        let interval = Interval::new(coords, test_date_dusk());

        assert_eq!(interval.current_event(), ActionTrigger::Dusk)
    }

    #[test]
    fn interval_at_dawn_works() {
        let coords = Coordinates::new(49.598121, 11.003653).unwrap();

        let interval = Interval::new(coords, test_date_dawn());

        assert_eq!(interval.current_event(), ActionTrigger::Dawn)
    }

    #[test]
    fn interval_at_sunset_works() {
        let coords = Coordinates::new(49.598121, 11.003653).unwrap();

        let interval = Interval::new(coords, test_date_sunset());

        assert_eq!(interval.current_event(), ActionTrigger::Sunset)
    }

    #[test]
    fn interval_at_00_works() {
        let coords = Coordinates::new(49.598121, 11.003653).unwrap();

        let interval = Interval::new(coords, test_date_00());

        assert_eq!(interval.current_event(), ActionTrigger::Dusk)
    }

    #[test]
    fn interval_at_23_59_59_works() {
        let coords = Coordinates::new(49.598121, 11.003653).unwrap();

        let interval = Interval::new(coords, test_date_23_59_59());

        assert_eq!(interval.current_event(), ActionTrigger::Dusk)
    }
}
