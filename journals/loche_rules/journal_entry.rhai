// Loche's rules booklet: Loche of the Collectors' skill masteries and rites
// of Lazarolth, as a little book in Loche's keeping. The rules are as given;
// Loche's own asides close each entry, in Loche's hand (loche_lower).
//
//   01-02 masteries  Hook, Ventriloquist, Assassinate; Light Step, Fervor
//   03-05 rites      Phantasmagoric Manifestation, Grey Feast, Leech Regret
//   06 at a glance  calls, Stealth and Tracking, states, what each rite needs
//
// Render it as a book: journalbuild loche_rules --parchment (or --pdf).

// ---- the book's look ----

const W = 1200.0;
const H = 1600.0;
const LEFT = 100.0;
const RIGHT = 1100.0;
const TOP = 210.0;

const INK = "#2b1d12";
const RED = "#9b2318";
const GREEN = "#3d6b2a";
const BARK = "#6b4423";

const PEN = new_brush("Ink Pen", 0.4, 0.8, 1.6);
const TEXT = new_style(INK, PEN);
const RED_TEXT = new_style(RED, PEN);
const CAPTION = new_style(RED, new_brush("Ink Pen", 0.3, 0.7, 1.2));
const LINE = new_style(INK, new_brush("Ink Pen", 0.3, 0.7, 2.0));
const HEADING = new_style(RED, new_brush("Calligraphy", 0.4, 0.8, 1.5, #{ pressure_profile: "constant" }), #{ width: 1.2 });
// Loche's own hand, for the asides.
const HAND = new_style(RED, new_brush("Calligraphy", 0.25, 0.6, 1.4));

const PLAIN = font("plain");
const LOCHE = font("loche_upper");
const LOCHE_HAND = font("loche_lower");

// What each kind of text in an entry looks like.
const ROLES = #{
	head: #{ font: LOCHE, style: HEADING, size: 48 },
	headrule: #{ style: CAPTION },
	rule: #{ style: CAPTION },
	title: #{ font: LOCHE, style: HEADING, size: 56 },
	lore: #{ font: PLAIN, style: CAPTION, size: 27, line_height: 1.3 },
	flavour: #{ font: PLAIN, style: TEXT, size: 25, align: "justify", line_height: 1.3 },
	stats: #{ font: PLAIN, style: CAPTION, size: 27 },
	sub: #{ font: LOCHE, style: HEADING, size: 38 },
	body: #{ font: PLAIN, style: TEXT, size: 27, align: "justify", line_height: 1.3, paragraph_gap: 0.6 },
	step: #{ font: PLAIN, style: TEXT, size: 25, align: "justify", line_height: 1.3 },
	liturgy: #{ font: PLAIN, style: RED_TEXT, size: 25, align: "justify", line_height: 1.3 },
	"call": #{ font: PLAIN, style: RED_TEXT, size: 27 },
	note: #{ font: LOCHE_HAND, style: HAND, size: 38, line_height: 1.4, align: "right" },
	folio: #{ font: PLAIN, style: CAPTION, size: 24 },
	figure: #{ style: LINE },
};

// ---- an entry page ----
//
// Every flow is named `{entry}{step}_{role}`, so sorted they come in order:
// the running head (000_head), each entry's parts with a rule between
// entries, art if there is any, and the folio (999_folio).

fn booklet_page(n, info) {
	let parts = [];
	let last = "";
	for key in info.flows.keys() {
		let role = key.sub_string(4);
		if role == "head" || role == "folio" { continue; }
		let entry = key.sub_string(0, 1);
		if last != "" && entry != last {
			parts.push(rule(#{ height: 14, margin: [14, 300, 6, 300] }));
		}
		last = entry;
		let style = #{ role: role };
		if role == "liturgy" { style.margin = [4, 50]; }
		if role == "sub" { style.margin = [10, 0, 0, 0]; }
		parts.push(flow(key, style));
	}
	if info.art.contains("figure") {
		parts.push(figure("figure", #{ grow: 1, margin: [20, 0, 0, 0] }));
	}
	column([
		flow("000_head", #{ role: "head", height: 70 }),
		rule(#{ role: "headrule", height: 10 }),
		column(parts, #{ grow: 1, gap: 8, padding: [30, 0, 0, 0] }),
		flow("999_folio", #{ role: "folio", height: 30, align: "center" }),
	], #{ padding: [55, 100, 50, 100], gap: 6 })
}

let book = new_book();

fn padded(n) {
	if n < 10 { `0${n}` } else { `${n}` }
}

// A page of entries, each a list of [role, text], its text scaled by `scale`.
fn entries(section, list, art) {
	this.entries(section, list, art, 1.0);
}

fn entries(section, list, art, scale) {
	let roles = global::ROLES;
	for name in roles.keys() {
		if roles[name].contains("size") && name != "head" && name != "folio" { roles[name].size *= scale; }
	}
	let flows = #{ "000_head": section, "999_folio": `${this.next}` };
	for (parts, e) in list {
		for (part, i) in parts {
			flows[`${e + 1}${padded(i)}_${part[0]}`] = part[1];
		}
	}
	let pages = new_layout("booklet", Fn("booklet_page")).typeset(#{
		width: global::W, height: global::H, flows: flows, roles: roles, art: art, max_pages: 1,
	});
	this.add(section, pages[0]);
}

// ---- drawings ----

// The eye of Saqir: an eye in a ring of short rays.
fn eye_in(b) {
	let r = b.height.min(b.width) * 0.4;
	let c = new_canvas().add(font("symbols").glyph("basic_eye", #{ scale: r * 1.2, position: b.center }), global::HEADING);
	for i in 0..16 {
		let d = vec2(0, -1).rotated(i * PI() / 8.0);
		c.add(new_line(b.center + d * (r * 1.05), b.center + d * (r * (if i % 2 == 0 { 1.35 } else { 1.2 }))), global::CAPTION);
	}
	c
}

// ---- masteries ----

book.entries("masteries", [
	[
		["title", "Hook"],
		["lore", "You can land devastating blows only a fellow master could defend against."],
		["body", "You may expend your unused \"Quick\" call for the use of a \"Power Strike\" call instead."],
		["call", "Call: \"Power Strike\" — in place of an unused \"Quick\"."],
		["note", "Strike once, and let it be the last thing they learn."],
	],
	[
		["title", "Ventriloquist"],
		["lore", "Throwing your voice around the room, you may create an eerie effect by speaking from stealth while remaining concealed."],
		["body", "You may speak while in the Stealth state without breaking it. The location of your whispers and speech are not able to be identified. Though you may still eventually be located with Tracking. Use the call \"Unlocatable\" before speaking to indicate that they cannot find you."],
		["call", "Call: \"Unlocatable\" — before speaking from Stealth."],
		["note", "Let them search the dark for a mouth. There is none to find."],
	],
	[
		["title", "Assassinate"],
		["lore", "A deadly killer, you can strike your target so swiftly they never see it coming."],
		["body", "You may attack from a Stealthed position of no more than two arms' reach away. This attack cannot be parried or dodged normally, although divine parries and dodges will work, and is considered a torso hit unless otherwise specified. You do not automatically re-enter Stealth, and will need to find new appropriate conditions to be Stealthed again."],
		["note", "Two arms' reach, no further. And after, the shadow must be found anew."],
	],
], #{});

book.entries("masteries", [
	[
		["title", "Light Step"],
		["lore", "No one hears you come and go, and you leave no footprints in the dust."],
		["body", "You do not need to respond to Tracking calls from humans or Malefic; Tracking calls with the Divine or Arcane qualifier must still be followed."],
		["note", "Men and devils lose my trail. Gods and the arcane do not."],
	],
	[
		["title", "Fervor"],
		["lore", "You become inspired by the tenets of your faith, riding the wave of your zeal to accomplish great feats."],
		["body", "With study and recitation of a piece of scripture for no less than 5 minutes, the user gains the Hopeful state."],
		["note", "Hold the Hunger. Hunt and Harvest come before Feast."],
	],
], #{});

// ---- rites of Lazarolth ----

book.entries("rites", [[
	["title", "Phantasmagoric Manifestation"],
	["lore", "Haunt the dreams of your victims and learn their deep fears"],
	["flavour", "Mankind's chains are manifold — placed there by tradition, by the power of the divine, by their own weaknesses and fears. But never are they closer to their true selves than when they fall into the realm of dreams."],
	["stats", "Constellation: Saqir     Orthodoxy: 3"],
	["sub", "Observances"],
	["step", "Offering — A bed of coal or charcoal is set upon the ground or within a brazier and set ablaze."],
	["step", "Liturgy —"],
	["liturgy", "\"When we close our eyes, what occurs to us? When we sleep, are we truly alive? For sleep is only a taste of death, the separation of the soul from the prison it holds so dear, if only for a moment. It is in this freedom that we become our truest selves. Lazarolth, Lady of Putrescence and Whisper, I seek truth in the taste of death. Bring me to one who dreams that I might know their heart, and allow me to walk the veil that I might see what is beyond.\""],
	["step", "Offering — An object tied to the target is cast into the fire. This may be a portion of their physical body such as hair, nails, or blood, or something close to their heart such as a written secret of theirs, a journal, their Touchstone Item or something they have made. The object is consumed by the flames as all in attendance wait in silence."],
	["step", "Act — When the fire dies, the ashes or other remnants of the burned item are collected and rubbed into the eyes of the Cultist."],
	["step", "Act — The Cultist then proceeds to sleep."],
	["sub", "Mechanical Effect"],
	["body", "The target has a dream of the Cultist's choice the next time they go to sleep. This dream will manifest either as a scene with the target and the cultist in the morning or as a written scene. The Cultist manifests in these dreams as a hooded specter with a hidden face, though the Cultist must bring any of their own Touchstone Items if they have one with them as they watch the target. The Cultist will awaken with intimate knowledge of the target's Devotion. A seer who experiences such a dream may be able to tell that it is implanted rather than natural."],
	["note", "They will see a hooded thing with no face. Let them wonder whose."],
]], #{}, 0.88);

book.entries("rites", [[
	["title", "Grey Feast"],
	["lore", "Consume another for their knowledge"],
	["flavour", "Secrets are the real currency of the world. Knowing what another would rather you not know is worth more than all the riches you can hold in your hands. Knowing what even the divine would hide from you? That is worth the world itself."],
	["stats", "Constellation: Saqir     Orthodoxy: 1"],
	["sub", "Observances"],
	["step", "Liturgy —"],
	["liturgy", "\"Mind, the seat of the soul of the self. If the heart is the castle of the intellect, then the brain is its library and throne, its memories and secrets hidden in corners of shadow. Lazarolth, Lady of Whispers, this grey feast I give to you. Reveal to me the secrets of the library, that we may share them one to another.\""],
	["step", "Focus — A Ceremonial Athame is used to crack open the skull of a human corpse."],
	["step", "Act — The brain of the corpse is consumed, either with utensils or with the Cultist's bare hands. This takes at least five minutes."],
	["sub", "Mechanical Effect"],
	["body", "After consuming the brain of the target, the Cultist gains intimate knowledge of them. They receive a skill from the user to add to their self teach list as well as all of the following: their true name, their highest skill, their most valuable Flaw, their most expensive Perk, how much Depravity they have and what sins the Depravity is tied to.\n\nIf the target had any Insanities, the Cultist gains the most severe one at a mild level, if the Cultist already has that insanity their own insanity is increased by 1. This does not remove the insanity from the target. Phlegmatic essence cannot be harvested from the target after this Rite has been performed on them. Further, corpses that have undergone this Rite cannot be Reanimated or Desecrated by a Lazarine.\n\nRemoves the Miserable state."],
	["note", "Eat slowly. Their madness is part of the meal."],
]], #{ figure: Fn("eye_in") });

book.entries("rites", [[
	["title", "Leech Regret"],
	["lore", "Temporarily suppress insanities through consuming a living tool of healing."],
	["flavour", "The world is not governed by reason; it is ruled by instinct and emotion. But not so with you. You shall starve your sentiments, regrets, and emotional reactions until they wither and die. Your actions shall be governed by intellect and reason, and thus shall you defy creation's hold on you."],
	["stats", "Constellation: Jamus     Orthodoxy: 2"],
	["sub", "Observances"],
	["step", "Offering — A human tongue is sliced into three strips of equal size, placed in a vessel of swamp water, then raised above the head of the Cultist before it is brought to chest level."],
	["step", "Offering — An ingestible Irritant Clarifier potion is poured into the vessel."],
	["step", "Focus — The vessel is placed upon a Shrine of Lazarolth and the Cultist watches as the tongue slivers begin their metamorphosis into leeches."],
	["step", "Liturgy —"],
	["liturgy", "\"So rare is the chance to abandon one's regrets, to cast them into the void that they might never return. Rare, too, is a truthful tongue. Some have deemed these impossibilities. See now how this organ shows its real visage: a parasite aching to be fed. Lazarolth, Blooddrinker, give these leeches a feast of sorrow. Let them draw from us the retributions with which we castigate ourselves. Let them purify us, that we no longer wear such shackles.\""],
	["sub", "Mechanical Effect"],
	["body", "This rite creates three Soul Leeches. Anyone who is Faith in Higher Power may swallow whole one of these live leeches, and all Insanities they are infected with are lowered to Mild intensity until the next Dawn. If the individual who swallows one is not a follower of Lazarolth, they also suffer a -1 Faith penalty for the same period."],
	["note", "Three leeches. Choose the mouths with care — the faithless pay in faith."],
]], #{});

// ---- at a glance ----

book.entries("at a glance", [
	[
		["title", "Calls"],
		["body", "\"Parry\"\n\"Dodge\"\n\"Quick\"\n\"Steady\"\n\"Tracking\"\n\"Power Strike\" — Hook: spend an unused \"Quick\" call for it.\n\"Unlocatable\" — Ventriloquist: call it before speaking from Stealth."],
	],
	[
		["title", "Stealth and Tracking"],
		["body", "Ventriloquist — speak in the Stealth state without breaking it.\nAssassinate — attack from Stealth within two arms' reach. No normal parry or dodge (divine ones still work); a torso hit unless I say otherwise. It leaves Stealth.\nLight Step — ignore Tracking calls from humans or Malefic. Divine or Arcane Tracking must be followed."],
	],
	[
		["title", "States"],
		["body", "Hopeful — Fervor: 5 minutes' study and recitation of scripture.\nMiserable — removed by the Grey Feast."],
	],
	[
		["title", "What Each Rite Needs"],
		["body", "Phantasmagoric Manifestation (Saqir, Orthodoxy 3) — coal or charcoal, on the ground or in a brazier; an object tied to the target; my Touchstone Item, if I have one; then sleep.\nGrey Feast (Saqir, Orthodoxy 1) — a human corpse; a Ceremonial Athame; five minutes to eat.\nLeech Regret (Jamus, Orthodoxy 2) — a human tongue; a vessel of swamp water; an ingestible Irritant Clarifier potion; a Shrine of Lazarolth. Makes three Soul Leeches; they last until the next Dawn."],
		["note", "Burn this page before it is found."],
	],
], #{});

journal_entry.add(book.pages);

if missing_glyphs().len() > 0 {
	warn("characters with no glyph: " + missing_glyphs());
}
