// star_alignment: the month some wandering stars next stand in one line,
// hidden in sightings of where each stood on earlier nights. Each star
// circles on an orbit of its own, as many houses round as its period, a
// house a month, and all are in their first house together only at the
// alignment: to find it is to find the month that leaves the right
// remainder for every star at once (the Chinese remainder theorem).
//
// The calendar has thirteen months to the year, so a date is a number in
// base 13: alignment is the year's digits and then the month's, 0 to 12
// (year 9, month 4 is 9 4; years past 12 take more digits). Thirteen
// shares no factor with any shorter period, so a star seen once a year,
// in the same month, is in a different house every time until it has
// been in them all: no star keeps step with the calendar, whatever its
// period, short of a period that's a multiple of 13, which is refused.
//
// periods is each star's, three to five of them; sightings how many
// sightings there are, each apart months after the last (13, a year);
// the seed picks how long before the alignment the last one is, less
// than the time the stars take to come round together again. The
// periods needn't be coprime: the sightings are of stars that do align.
//
// sightings, the output, is #{ periods, sightings }: each sighting its
// date, as alignment's is written, and the house each star stood in, 0
// its first. Unlocking reckons the alignment from the last sighting and
// checks the earlier ones agree.

import "lib/crt" as crt;

fn options() {
	#{ periods: array([5, 7, 8]), sightings: integer(2), apart: integer(13), seed: integer(0) }
}

fn check(o) {
	if o.periods.len() < 3 || o.periods.len() > 5 {
		throw `there must be 3 to 5 stars, a period each, got ${o.periods.len()}`;
	}
	for p in o.periods {
		if type_of(p) != "i64" || p < 2 || p > 24 {
			throw `a period must be a whole number from 2 to 24, got ${p}`;
		}
		if p % 13 == 0 {
			throw `a period of ${p} keeps step with the 13-month year: the star would be in the same house every year`;
		}
	}
	if o.sightings < 1 || o.sightings > 12 {
		throw `there must be 1 to 12 sightings, got ${o.sightings}`;
	}
	if o.apart < 1 {
		throw `sightings must be 1 month apart or more, got ${o.apart}`;
	}
}

fn inputs(o) {
	#{ alignment: symbol_type(13) }
}

fn outputs(o) {
	#{ sightings: "Sightings" }
}

// A date's months since year 0, month 0: its digits as a number in base
// 13. Throws unless it's written one way: a year and a month, the year
// without a 0 in front.
fn months(date) {
	if date.len() < 2 {
		throw `a date is a year and a month, two digits or more, got ${date.len()}`;
	}
	if date.len() > 2 && date[0] == 0 {
		throw "a date's year is written without a 0 in front";
	}
	if date.len() > 12 {
		throw `a date of ${date.len()} digits is too far off to reckon`;
	}
	date.reduce(|n, d| n * 13 + d, 0)
}

// A month count as a date: its year's digits, then its month.
fn date(months) {
	let digits = [months % 13];
	let year = months / 13;
	digits.insert(0, year % 13);
	while year >= 13 {
		year /= 13;
		digits.insert(0, year % 13);
	}
	digits
}

// The house each star is in, `since` months after an alignment.
fn houses(periods, since) {
	periods.map(|p| ((since % p) + p) % p)
}

fn lock(o, i) {
	let when = months(i.alignment);
	let round = crt::lcm_of(o.periods);
	let span = (o.sightings - 1) * o.apart;
	if when <= span {
		throw `the alignment is too soon for ${o.sightings} sightings ${o.apart} months apart: month ${span + 1} of the count at the earliest, got ${when}`;
	}
	// The last sighting: from 1 month before to just short of a whole
	// round, and no earlier than leaves the first sighting a date.
	let most = min(round - 1, when - span);
	let before = 1 + min((new_random(o.seed).next() * most).to_int(), most - 1);
	let sightings = [];
	for n in 0..o.sightings {
		let since = -before - (o.sightings - 1 - n) * o.apart;
		sightings.push(#{ date: date(when + since), houses: houses(o.periods, since) });
	}
	#{ sightings: #{ periods: o.periods, sightings: sightings } }
}

fn unlock(o, out) {
	let seen = out.sightings;
	if type_of(seen) != "map" || !("periods" in seen) || !("sightings" in seen) || seen.sightings.is_empty() {
		throw "the sightings should be #{ periods, sightings }, with a sighting at least";
	}
	let periods = seen.periods;
	let last = seen.sightings[seen.sightings.len() - 1];
	let at = months(last.date);
	// Each star has as far to go as brings it back to house 0.
	let wait = crt::solve(last.houses.map(|h| -h), periods);
	let when = at + (if wait.value == 0 { wait.modulus } else { wait.value });
	for (sighting, n) in seen.sightings {
		let should = houses(periods, months(sighting.date) - when);
		if sighting.houses != should {
			throw `sighting ${n + 1} has the stars in houses ${sighting.houses}, but by the last sighting they'd have been in ${should}`;
		}
	}
	#{ alignment: date(when) }
}

fn examples() {
	[
		#{
			// Year 9, month 4, three stars seen twice, a year apart.
			options: #{},
			inputs: #{ alignment: [9, 4] },
		},
		#{
			// Five stars, three sightings, a year past 12.
			options: #{ periods: [5, 7, 8, 9, 11], sightings: 3, seed: 6 },
			inputs: #{ alignment: [1, 2, 0, 7] },
		},
		#{
			// Periods that share factors, one sighting.
			options: #{ periods: [4, 6, 9], sightings: 1, seed: 2 },
			inputs: #{ alignment: [3, 12] },
		},
		#{
			// Sightings a month apart.
			options: #{ periods: [3, 5, 11, 7], sightings: 4, apart: 1, seed: 11 },
			inputs: #{ alignment: [0, 9] },
		},
		#{
			options: #{},
			inputs: #{ alignment: [0, 12] },
			error: "the alignment is too soon for 2 sightings 13 months apart",
		},
		#{
			options: #{},
			inputs: #{ alignment: [0, 4, 2] },
			error: "a date's year is written without a 0 in front",
		},
		#{
			options: #{ periods: [5, 7, 13] },
			error: "a period of 13 keeps step with the 13-month year",
		},
		#{
			options: #{ periods: [5, 7] },
			error: "there must be 3 to 5 stars",
		},
	]
}
