//! Local times for the report (D-024). Stored timestamps are UTC (D-009); captions must show the
//! device's local time *for that instant*, so each one is converted with the zone's own rules
//! (Portugal leaves DST on 2026-10-25: a photo from the 20th and one from the 30th differ by an
//! hour in offset). The WebView sends the IANA zone name (`Intl.DateTimeFormat().resolvedOptions()
//! .timeZone`); `jiff` reads Android's system tz database and bundles one only on Windows. An
//! unknown name falls back to the fixed offset the WebView also sends.
use jiff::tz::{Offset, TimeZone};
use jiff::Timestamp;

pub struct Clock {
    tz: TimeZone,
}

impl Clock {
    /// `zone`: IANA name, e.g. `Europe/Lisbon`; `utc_offset_min`: the device's current offset,
    /// minutes east of UTC, used when the zone is unknown here.
    pub fn new(zone: &str, utc_offset_min: i32) -> Clock {
        let tz = match TimeZone::get(zone) {
            Ok(tz) => tz,
            Err(e) => {
                log::warn!("time zone {zone:?} unavailable ({e}); using the fixed offset {utc_offset_min} min");
                Clock::fixed(utc_offset_min).tz
            }
        };
        Clock { tz }
    }

    pub fn fixed(utc_offset_min: i32) -> Clock {
        let secs = utc_offset_min.clamp(-18 * 60, 18 * 60) * 60;
        Clock { tz: TimeZone::fixed(Offset::from_seconds(secs).expect("offset within ±18 h")) }
    }

    /// `dd.mm.yyyy` of a stored UTC timestamp, in local time; the input itself when unparseable.
    pub fn date(&self, iso: &str) -> String {
        self.format(iso, "%d.%m.%Y")
    }

    /// `dd.mm.yyyy hh:mm` (photo captions, as in the client's mockup).
    pub fn date_time(&self, iso: &str) -> String {
        self.format(iso, "%d.%m.%Y %H:%M")
    }

    /// Today's local date, `dd.mm.yyyy`.
    pub fn today(&self) -> String {
        Timestamp::now().to_zoned(self.tz.clone()).strftime("%d.%m.%Y").to_string()
    }

    fn format(&self, iso: &str, fmt: &str) -> String {
        match iso.parse::<Timestamp>() {
            Ok(ts) => ts.to_zoned(self.tz.clone()).strftime(fmt).to_string(),
            Err(e) => {
                log::warn!("timestamp {iso:?} not RFC 3339 ({e}); shown as stored");
                iso.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_instant_uses_its_own_offset_across_the_dst_change() {
        let lisbon = Clock::new("Europe/Lisbon", 60);
        // Portugal: UTC+1 until 2026-10-25 01:00 UTC, UTC+0 afterwards.
        assert_eq!(lisbon.date_time("2026-10-20T10:00:00.000Z"), "20.10.2026 11:00");
        assert_eq!(lisbon.date_time("2026-10-30T10:00:00.000Z"), "30.10.2026 10:00");
        assert_eq!(lisbon.date_time("2026-10-25T00:30:00.000Z"), "25.10.2026 01:30");
        assert_eq!(lisbon.date_time("2026-10-25T01:30:00.000Z"), "25.10.2026 01:30", "the repeated hour");
        // Midnight crossing changes the date too.
        assert_eq!(lisbon.date("2026-07-01T23:30:00.000Z"), "02.07.2026");
        let azores = Clock::new("Atlantic/Azores", 0);
        assert_eq!(azores.date_time("2026-10-20T10:00:00.000Z"), "20.10.2026 10:00");
    }

    #[test]
    fn unknown_zone_falls_back_to_the_fixed_offset() {
        let c = Clock::new("Mars/Olympus_Mons", 60);
        assert_eq!(c.date_time("2026-10-30T10:00:00.000Z"), "30.10.2026 11:00", "fixed, not DST-aware");
        let c = Clock::new("", -210);
        assert_eq!(c.date_time("2026-10-30T10:00:00.000Z"), "30.10.2026 06:30");
        assert_eq!(Clock::fixed(0).date_time("not a date"), "not a date");
        assert_eq!(Clock::fixed(0).today().len(), "12.10.2026".len());
    }
}
