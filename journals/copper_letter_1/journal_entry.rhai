let maze_page = new_page();

let canvas = new_canvas();

let paragraphs = [
	"Us. I like Us very much for our collaboration. Us. What a delight it is to write it upon the page. Us. We. Us. How pleasant indeed. Alas, but there is so little time left for Us, for when I have dreamed, I have dreamed that we are at the End of Us, one way or another. At least for the most part—God does not intend I remain in Cenotaph much longer. Do not concern yourself overly with this, perhaps you will prove lucky and miss me but briefly.",

	"I dreamed too of your dance. Isn't that peculiar? In the dream, you were encircled about by an audience watching You and not Me. This pleased me immensely, for this is as it is meant to be. I dreamed that I was placed before the many people of Cenotaph and the Razor of Truth carved from me and set to display something I did not yet desire to be revealed. And yet, as it is said, such things are far from our control; there are secrets which the spirits themselves will steal if they lay too long stagnant.",

	"From the beginning you spoke of power and its games, the way you played them with the eyes. You even accused me of holding stares as a play at domination. Sometimes, when I am thinking of how best to be Us, I think of that conversation and how much it spoke of you. Few see the Game that exists, and fewer still understand its workings well enough to know what it takes to be a true player. Even before nightly visitation, I heard rumors of your Onion Dance. As the details of the spectacle have filtered to me, I have been impressed by how well you managed both the role of the Onion and the Farmer.",

	"Home has been on the mind and the story of your Onion Dance has put me in mind of another from my youth— mentor's favorite, a parable called the Emperor's Rabbit—specifically the beginning bit where the Apprentices are tasked with the making of stew, before the cannibalism and regicide. It being a Lazarine tale, perhaps it is that you are familiar? If not, I would love to learn the source of your tale.",

	"I confess that I, too, had alternate plans. Where you imagined some sort of hunt, my lines of thinking lay more direct. I was once called a straight, black line in my approaches—perhaps you will agree. First, I conceived I would steal your glasses and become the other half of Us, then that I would steal your corpse—you would inevitably be left in such a state at least once—and I would properly dine. None of these felt like Us.",

	"You have seen, and as a result you have made my time in Cenotaph approach enjoyable. As my other games come to their end, I promise that my games with you will not. You will be able to keep up with them, I have no doubt—you've proven adequately up to the task so far. I even admired the variety you managed to make out of a single secret way, I almost missed one of the many before the possibility irritated at me enough to thoroughly review. Well done. Your most banal, Loche."
];

let maze_canvases = [];
let origin = new_vec(0.0, 0.0);
let style = new_strokestyle(1.5, "darkred");
let copper_font = new_fontset(["copper_upper", "loche_upper"], ["copper_lower", "loche_lower"]);
let brush_style = new_brush("Rough Ink", 0.25, 0.5, 1.0);

for (paragraph, index) in paragraphs {
	let maze_path = left_handed_maze_path(index, 10, 10);
	maze_path.scale_by(40.0);
	let maze_canvas = maze_path.fit_msg_to(style, copper_font, paragraph);
	let page = new_page();
	page.add_canvas(maze_canvas, brush_style);
	journal_entry.add_page(page);
}

