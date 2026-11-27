<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Weapons"
	description="Every weapon of Rustcraft: damage, recharge and damage per second, and the bow, crossbow, shield and spear."
	kicker="A bare hand does 1 damage. A blow struck before the weapon has recharged does less."
>
	{#snippet icon()}<Slot id="IronSword" big link={false} />{/snippet}

	<p>
		A weapon hits hardest once it has recharged: a blow struck too soon does as little as a fifth of its
		damage. Swords are quick, axes and the mace slow and heavy. Hold the attack button to strike again the
		moment the weapon is ready. See <a href="/guides/combat">combat</a> for criticals, sweeps and armour.
	</p>

	<h2>Melee weapons</h2>
	<p>Sorted by damage per second when you wait for each full recharge.</p>
	<div class="wide">
		<table>
			<thead>
				<tr><th>Weapon</th><th class="n">Damage</th><th class="n">Recharge</th><th class="n">Per second</th><th class="n">Durability</th></tr>
			</thead>
			<tbody>
				{#each data.weapons as w (w.id)}
					<tr>
						<td><ItemLink id={w.id} /></td>
						<td class="n">{w.damage}</td>
						<td class="n">{w.recharge} s</td>
						<td class="n">{(w.damage / w.recharge).toFixed(1)}</td>
						<td class="n">{w.durability ?? '–'}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
	<ul>
		<li>Swords sweep: a full blow on the ground also hits the creatures within 2 blocks of the target, in front of you, for a third of the damage.</li>
		<li>The <ItemLink id="Spear" /> reaches 5 blocks instead of 3.</li>
		<li>The <ItemLink id="Mace" /> knocks creatures back more than twice as far.</li>
		<li>Axes do half again their damage to Stumplings, and knock their guard aside for four seconds.</li>
	</ul>

	<h2>Ranged weapons</h2>
	<dl class="ranged">
		<dt><ItemLink id="Bow" /></dt>
		<dd>
			Hold to draw, up to one second, then release. A full draw does 9 damage, a quick one as little as 2;
			<a href="/enchanting">Bow Power</a> adds 1 per level. Each shot uses an <ItemLink id="Arrow" />, and
			you walk slower while drawing.
		</dd>
		<dt><ItemLink id="Crossbow" /></dt>
		<dd>Loads in 1.25 s and shoots a <ItemLink id="Bolt" /> for 11 damage, which pierces two creatures.</dd>
		<dt><ItemLink id="ThrowingAxe" /></dt>
		<dd>Wind up for 0.6 s and throw it for 4 to 8 damage; pick it up where it lands.</dd>
		<dt><ItemLink id="Shield" /></dt>
		<dd>
			Raised with the use button, it stops the blows and arrows that come from the front. It wears by the
			damage it stops, and a Roller's charge or the Ember Mother's blow knocks it down for five seconds.
		</dd>
	</dl>
</Page>

<style>
	.ranged {
		max-width: 72ch;
	}
	.ranged dt {
		margin-top: 12px;
		font-weight: 600;
	}
	.ranged dd {
		margin: 4px 0 0;
	}
</style>
