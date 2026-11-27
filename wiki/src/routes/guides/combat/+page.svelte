<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import { num } from '#lib/format.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	// How far a weapon drawn fully in `full` ms is after `ms`: slow at first, then quicker
	const power = (ms: number, full: number) => {
		const t = Math.min(ms / full, 1);
		return (t * t + 2 * t) / 3;
	};
	const arrow = (ms: number) => Math.round(1 + 8 * power(ms, 1000));
	const axe = (ms: number) => Math.round(3 + 5 * power(ms, 600));

	const easy = (d: number) => Math.min(d, Math.floor(d / 2) + 1);
	const hard = (d: number) => Math.floor((d * 3) / 2);
</script>

<Page
	title="Combat"
	description="How fighting works in Rustcraft: melee damage and recharge, critical hits, sword sweeps, bows, crossbows, shields, armour and Protection, difficulty and the experience each creature gives."
>
	{#snippet icon()}<Slot id="Shield" big link={false} />{/snippet}

	<p>
		A blow does its full damage only once your weapon has recharged. Wait between swings, strike
		while falling for a critical hit, and let armour and a shield take what the creatures give
		back. Every weapon is listed on the <a href="/weapons">weapons page</a>, every piece of armour on
		the <a href="/armour">armour page</a>.
	</p>

	<h2>Melee</h2>
	<p>Left click a creature within reach to hit it. The damage of a blow is built in four steps:</p>
	<ol>
		<li>
			<strong>The weapon's damage</strong>: 1 bare-handed or with anything that is not a weapon,
			plus 1 per level of Sharpness.
		</li>
		<li>
			<strong>Effects</strong>: Strength adds 3 per level, Weakness takes 2 per level away, never
			below 1.
		</li>
		<li>
			<strong>The recharge</strong>: a full blow does all of it, a blow swung at once only a fifth.
			In between it grows with the square of the charge: half recharged gives 40%.
		</li>
		<li><strong>A critical hit</strong> multiplies the result by 1.5. Every blow does at least 1.</li>
	</ol>

	<h3>Weapons and recharge</h3>
	<p>
		Holding the button swings again as soon as the weapon has recharged. Switching to another item
		starts the recharge again from zero. A bare hand recharges in 0.4 s.
	</p>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Weapon</th>
					<th class="n">Damage</th>
					<th class="n">Recharge</th>
					<th class="n">Damage per second</th>
					<th class="n">Wear per blow</th>
					<th class="n">Durability</th>
				</tr>
			</thead>
			<tbody>
				{#each data.melee as w (w.item)}
					<tr>
						<td><ItemLink id={w.item} /></td>
						<td class="n">{w.damage}</td>
						<td class="n">{w.recharge / 1000} s</td>
						<td class="n">{num(w.damage / (w.recharge / 1000), 1)}</td>
						<td class="n">{w.wear || 'None'}</td>
						<td class="n">{w.durability ?? 'Never wears'}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		Swords do 4, axes 6 and the other tools 2, plus 1 for stone or bronze, 2 for iron and 3 for
		diamond; wood and gold add nothing. The pickaxe stands for the shovel and the hoe, which hit the
		same.
	</p>

	<h3>Critical hits</h3>
	<p>
		A blow at least 90% recharged while you are <strong>falling</strong> (in the air, going down,
		not flying nor swimming) is critical: <strong>×1.5 damage</strong>. Jump, then strike on the
		way down.
	</p>

	<h3>Sweeps</h3>
	<p>
		A sword blow at least 90% recharged while you stand <strong>on the ground</strong> also sweeps:
		every other creature within 2 blocks of the target, and in front of you, takes a third of the
		damage (at least 1).
	</p>

	<h3>Reach and knockback</h3>
	<ul>
		<li>You reach creatures <strong>3 blocks</strong> from your eyes, or <strong>5 with a spear</strong>.</li>
		<li>
			A full blow throws the creature back at 6 blocks a second, a hurried one at half that. The mace
			throws 2.2 times as far.
		</li>
		<li>A creature you hit cannot be hurt again for 0.5 s, and an animal you hit runs away for 5 s.</li>
		<li>
			The Stumpling's bark parries blows from the front. An axe knocks its guard aside for 4 seconds,
			and axes deal it 1.5 times their damage.
		</li>
	</ul>

	<h3>Weapon wear</h3>
	<p>
		Each blow that lands wears a sword or the mace by 1, and any other tool by 2: an axe or a pickaxe
		used as a weapon wears out twice as fast. The spear and the throwing axe never wear. Repair worn
		weapons at the <ItemLink id="Anvil" /> (see <a href="/enchanting">enchanting and the anvil</a>).
	</p>

	<h2>Ranged weapons</h2>
	<p>Hold the use button (right click) to draw, release it to shoot.</p>

	<h3>Bow</h3>
	<p>
		A <ItemLink id="Bow" /> is fully drawn after 1 second; released before 0.15 s it shoots nothing.
		The arrow flies at up to 42 blocks a second and falls in a curve. Each Bow Power level adds 1
		damage. Each shot uses one <ItemLink id="Arrow" /> and wears the bow by 1. While you draw, you walk
		at 40% of your speed.
	</p>
	<table>
		<thead><tr><th class="n">Draw</th><th class="n">Arrow damage</th></tr></thead>
		<tbody>
			{#each [150, 250, 500, 750, 1000] as ms (ms)}
				<tr><td class="n">{ms / 1000} s</td><td class="n">{arrow(ms)}</td></tr>
			{/each}
		</tbody>
	</table>

	<h3>Crossbow</h3>
	<p>
		A <ItemLink id="Crossbow" /> takes 1.25 s to load and shoots only once fully loaded. Its
		<ItemLink id="Bolt" /> always does <strong>11 damage</strong> and goes through 2 creatures before stopping
		in a third.
	</p>

	<h3>Throwing axe</h3>
	<p>
		The <ItemLink id="ThrowingAxe" /> is swung back for up to 0.6 s and thrown: 4 damage on a quick throw,
		8 on a full one. It falls to the ground as itself, to be picked up again. In melee it hits for 5 at
		the pace of an axe.
	</p>

	<h3>Splash potions</h3>
	<p>
		<ItemLink id="SplashPoisonPotion" />, <ItemLink id="SplashSlownessPotion" /> and
		<ItemLink id="SplashWeaknessPotion" /> are thrown like the throwing axe (0.6 s for a full throw)
		and burst where they land. They do no damage themselves: everyone within 4 blocks gets the effect,
		in full at the centre and half of it at the edge. See <a href="/alchemy">alchemy</a>.
	</p>
	<p>
		Arrows, bolts and thrown axes hit players as well as creatures, and a shield stops them. In
		creative, shooting takes no ammunition.
	</p>

	<h2>Shield</h2>
	<p>
		Hold the use button to raise a <ItemLink id="Shield" />. It takes 0.25 s to come up, and while it
		is up you walk at half speed and cannot sprint.
	</p>
	<ul>
		<li>It stops every blow and shot from the <strong>front half</strong> of you, measured flat on the ground.</li>
		<li>It wears by the damage it stops (at least 1 per hit): 336 points in all.</li>
		<li>A Roller's charge or the Ember Mother's blow knocks it down for 5 seconds.</li>
		<li>
			It works from <strong>any hotbar slot</strong> while your hand holds a sword, an axe, a spear, the mace
			or nothing, so you can fight and block at once.
		</li>
		<li>It cannot be raised while you ride, draw a bow, or hold food.</li>
	</ul>

	<h2>Armour</h2>
	<p>
		Each point of armour absorbs <strong>4%</strong> of a hit. Each level of Protection on any piece
		adds 1% more. Armour points count up to 20, and the total never goes past 90%. What is absorbed
		below a whole point is carried over to the next hit, so the reduction is exact in the long run.
	</p>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Material</th>
					<th class="n">Helmet</th>
					<th class="n">Chestplate</th>
					<th class="n">Leggings</th>
					<th class="n">Boots</th>
					<th class="n">Full suit</th>
					<th class="n">Absorbed</th>
				</tr>
			</thead>
			<tbody>
				{#each data.armour as a (a.material)}
					<tr>
						<td>{a.material}</td>
						{#each a.pieces as p (p.item)}
							<td class="n"><ItemLink id={p.item} label={String(p.points)} /></td>
						{/each}
						<td class="n">{a.total}</td>
						<td class="n">{a.total * 4} %</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<p>
		A full diamond suit with Protection III on all four pieces absorbs 76 + 12 = 88% of a hit.
	</p>

	<h3>Durability</h3>
	<p>Each hit armour reduces wears every piece you wear by 1.</p>
	<div class="wide">
		<table>
			<thead>
				<tr>
					<th>Material</th>
					<th class="n">Helmet</th>
					<th class="n">Chestplate</th>
					<th class="n">Leggings</th>
					<th class="n">Boots</th>
				</tr>
			</thead>
			<tbody>
				{#each data.armour as a (a.material)}
					<tr>
						<td>{a.material}</td>
						{#each a.pieces as p (p.item)}
							<td class="n">{p.durability}</td>
						{/each}
					</tr>
				{/each}
			</tbody>
		</table>
	</div>

	<h3>What armour does not reduce</h3>
	<p>
		Armour absorbs the blows and throws of creatures, arrows, fire and lava. It does
		<strong>nothing</strong> against falls, the void, drowning, starvation or poison. See
		<a href="/guides/health">health and hunger</a>.
	</p>

	<h2>Hostile creatures and difficulty</h2>
	<p>
		The difficulty of the world changes how hard hostiles strike. On <strong>Peaceful</strong> there are
		none. <strong>Easy</strong> roughly halves a blow (half, plus 1), <strong>Hard</strong> adds half
		again. Below ground, a hostile born in the Depths is 1.5 times as strong and one born in the Furnace
		twice as strong, before the difficulty is applied (see <a href="/guides/depths">the depths</a>).
	</p>
	<table>
		<thead>
			<tr>
				<th>Creature</th>
				<th class="n">Easy</th>
				<th class="n">Normal</th>
				<th class="n">Hard</th>
			</tr>
		</thead>
		<tbody>
			{#each data.strikes as [mob, d] (mob)}
				<tr>
					<td><ItemLink id={mob} /></td>
					<td class="n">{easy(d)}</td>
					<td class="n">{d}</td>
					<td class="n">{hard(d)}</td>
				</tr>
			{/each}
		</tbody>
	</table>
	<p>
		Damage of one blow at the surface, in points of health (2 per heart), before armour. Every
		creature is described on the <a href="/creatures">creatures page</a>.
	</p>

	<h2>Experience</h2>
	<p>
		A creature you kill leaves an orb of experience. Only hostiles, the Mossback and fish give any:
		other animals and villagers give none. Hostiles born in the Depths give 1.5 times as much, in the
		Furnace twice as much.
	</p>
	<table>
		<thead><tr><th>Creature</th><th class="n">Experience</th></tr></thead>
		<tbody>
			{#each data.experience as [mob, xp] (mob)}
				<tr><td><ItemLink id={mob} /></td><td class="n">{xp}</td></tr>
			{/each}
		</tbody>
	</table>
	<p>
		Spend experience at the <ItemLink id="EngravingTable" /> and the <ItemLink id="Anvil" />.
	</p>

	<h2>Riding into battle</h2>
	<p>
		While you ride a <ItemLink id="Horse" />, a donkey or a mule, your steed takes half of every blow
		a creature aims at you, rounded up, and you take the rest: a blow of 3 hurts the steed for 2 and you
		for 1. A shield cannot be raised in the saddle. See <a href="/guides/animals">taming, breeding and riding</a>.
	</p>
</Page>
