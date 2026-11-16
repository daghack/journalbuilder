// door_heath: the month the three watchers stand in one line over the
// door (watch), as two sightings of them a year apart.

fn chain(o) {
	#{
		locks: #{
			watch: #{ type: "star_alignment", options: #{ sightings: 2 } },
		},
		connect: [],
		fixed: #{
			"watch.alignment": symbols(13, "LC"),
		},
		illustrations: #{},
	}
}
