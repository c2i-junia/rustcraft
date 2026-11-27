// What the tables cannot say about an item: a few sentences of how players use it, shown at the
// top of its page. Keyed by item id; HTML allowed, links to other pages welcome. Every sentence
// is a fact of the game's code: keep them with it.
export const ITEM_NOTES: Record<string, string> = {
	CraftingTable:
		'<p>Place it and use it for the 3 × 3 grid, which most recipes need; your inventory only has a 2 × 2 one. Having one earns <em>Room to Work</em>. See <a href="/crafting">crafting</a>.</p>',
	Furnace:
		'<p>Smelts ores into ingots, cooks food and bakes sand into glass, one item every 10 seconds. It keeps working while you are away, as long as its chunk is loaded. Breaking it drops what is inside. See <a href="/smelting">smelting</a>.</p>',
	Torch:
		'<p>The first defence against the night: hostiles never spawn within 8 blocks of a light. Light your base and your mine.</p>',
	Bedroll:
		'<p>Use it to set your respawn point. At night, when every player online lies down, the night is skipped. If your bedroll is broken, you come back at the world spawn. Using one earns <em>Somewhere to Come Back To</em>.</p>',
	Chest:
		'<p>Holds 27 slots. Breaking it drops what is inside. Put one on a tamed donkey or mule to make it a pack animal.</p>',
	Barrel: '<p>Holds 27 slots, like a chest.</p>',
	Compass:
		'<p>Its needle points home: to your bedroll, or to the world spawn if you have none.</p>',
	DepthsCompass:
		'<p>Its needle points to the lair of the Ember Mother, at the bottom of the world. It spins in a world without one, and when you stand at the heart of the lair. Getting one earns <em>It Points Down</em>. See <a href="/guides/ember-mother">the Ember Mother</a>.</p>',
	PortalGun:
		'<p>The attack button opens your blue portal, the use button your orange one, on the face of a block up to 32 blocks away. A portal is 2 blocks wide and 3 high: its 6 wall cells must be solid full blocks (not glass or leaves) and the 6 in front of it empty. Walk into one and you come out of the other; you can see through them. A portal closes if its wall changes.</p>',
	Compost:
		'<p>Use it on a crop to grow it a stage, on a berry bush to ripen it, or on a sapling for a one in two chance to grow it into a tree at once. See <a href="/guides/farming">farming</a>.</p>',
	Lead:
		'<p>Use it on a land animal to lead it. The lead has 4 blocks of slack and snaps beyond 12; use it on a fence post to tie the animal there.</p>',
	Saddle:
		'<p>Put it on a horse, donkey or mule you tamed, then use the animal to ride it. See <a href="/guides/animals">riding</a>.</p>',
	Boat: '<p>Put it on water and use it: 8 blocks a second on water, 1 on land, with a seat for a passenger in front.</p>',
	EmberHeart:
		'<p>What the Ember Mother leaves when she falls: a block that glows with a light of 12. Only one is ever dropped per world.</p>',
	Coin: '<p>The money of the villagers: sell them crops, coal, iron, fibre, feathers or leather for coins, and buy food, tools, armour and torches with them. See <a href="/guides/trading">trading</a>.</p>',
	TreasureMap:
		'<p>Found in the chests of shipwrecks. While you hold it, your maps mark the spot of a buried chest with a red X, 24 to 72 blocks from the wreck. Dig 2 blocks down there. See <a href="/guides/structures">structures</a>.</p>',
	EmptyMap:
		'<p>Use it to start a map of the 128 × 128 blocks around you; it fills in as you walk, within 40 blocks of you. Copy a map by crafting it with an empty one.</p>',
	FilledMap:
		'<p>Shows 128 × 128 blocks, a pixel each, and fills in within 40 blocks of whoever holds it. Hang it in a map frame, or copy it with an empty map.</p>',
	MilkBucket: '<p>Drink it to clear every status effect; the bucket is left. Get it by using a bucket on a cow.</p>',
	Bucket:
		'<p>Picks up a source of water or lava, and milks a cow. Only source blocks can be picked up: water that flows cannot.</p>',
	LavaBucket:
		'<p>The best fuel of all: one burns for 1000 seconds, 100 items, and leaves its bucket.</p>',
	GlassVial: '<p>Fill it at a water source for a water vial, the start of every potion. See <a href="/alchemy">alchemy</a>.</p>',
	Shield:
		'<p>Hold the use button to raise it: it stops the blows and arrows that come from the front, and you walk at half speed. An axe or a Roller knocks it down for five seconds. It works from any hotbar slot while your hand holds a weapon or nothing. See <a href="/guides/combat">combat</a>.</p>',
	Campfire:
		'<p>A light that keeps hostiles away and that rain puts out. Relight it with a torch, flint, or flint and steel.</p>',
	RainCistern: '<p>Fills in the rain. Empty it with a bucket for a water bucket.</p>',
	Obsidian: '<p>Made where a lava source meets water. Only a diamond pickaxe harvests it.</p>',
	Gravel: '<p>Breaking it gives flint one time in ten.</p>',
	TallGrass:
		'<p>Breaking it may give fibre, rye, a carrot or a potato: the start of every farm. See <a href="/guides/farming">farming</a>.</p>',
	Diamond:
		'<p>Found deep down: diamond ore lies from y −58 to 16 in a new world, most of it below −42. Mine it with an iron pickaxe or better. See <a href="/guides/ores">ores</a>.</p>',
	IronIngot:
		'<p>Smelt iron ore for it; iron ore lies from y −24 to 72 in a new world and needs a stone or bronze pickaxe. Getting one earns <em>Worth the Climb Down</em>.</p>',
	BronzeIngot:
		'<p>Made from copper and tin. Bronze tools sit between stone and iron, and bronze armour between leather and iron: a good step before you find iron.</p>'
};
