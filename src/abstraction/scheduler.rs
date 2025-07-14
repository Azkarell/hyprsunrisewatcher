use std::process::Command;

use chrono::{Days, prelude::*};
use sunrise::{Coordinates, SolarDay, SolarEvent};

pub struct Scheduler {
    on_sunrise: Vec<Action>,
    on_sunset: Vec<Action>,
    on_dusk: Vec<Action>,
    on_dawn: Vec<Action>,
    location_info: LocationInfo,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ActionTrigger {
    Sunrise,
    Sunset,
    Dusk,
    Dawn,
}

impl Scheduler {
    pub fn new<L: Into<LocationInfo>>(location_info: L) -> Self {
        Self {
            on_dusk: vec![],
            on_sunrise: vec![],
            on_sunset: vec![],
            on_dawn: vec![],
            location_info: location_info.into(),
        }
    }

    pub fn add_action(&mut self, location_info: ActionTrigger, action: Action) {
        match location_info {
            ActionTrigger::Sunrise => self.on_sunrise.push(action),
            ActionTrigger::Sunset => self.on_sunset.push(action),
            ActionTrigger::Dusk => self.on_dusk.push(action),
            ActionTrigger::Dawn => self.on_dawn.push(action),
        }
    }

    pub fn estimated_next_event_at(&self, date: DateTime<Utc>) -> DateTime<Utc> {
        self.location_info.interval_at(date).end
    }
}

pub struct Action {
    command: std::process::Command,
}

impl From<Command> for Action {
    fn from(value: Command) -> Self {
        Self::new(value)
    }
}
impl Action {
    pub fn new(command: Command) -> Self {
        Self { command }
    }
}

pub struct LocationInfo {
    coords: Coordinates,
}

impl From<Coordinates> for LocationInfo {
    fn from(value: Coordinates) -> Self {
        Self::new(value)
    }
}
pub struct Interval {
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

impl LocationInfo {
    pub fn new(coords: Coordinates) -> Self {
        Self { coords }
    }

    pub fn interval_at(&self, date: DateTime<Utc>) -> Interval {
        Interval::new(self.coords, date)
    }
}

#[cfg(test)]
mod test {
    use chrono::{DateTime, Utc};
    use sunrise::Coordinates;

    use crate::abstraction::scheduler::ActionTrigger;

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
