<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import Slot from '#lib/components/Slot.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<Page
	title="Villagers and trade"
	description="Trading with Rustcraft villagers: every offer of the farmer, the smith and the cartographer, coins, daily stock and restock at sunrise, the dungeon directions, and how villagers live."
>
	{#snippet icon()}<Slot id="Coin" big link={false} />{/snippet}

	<p>
		Every village has villagers, and each one is a farmer, a smith or a cartographer. They buy what you
		have plenty of for <ItemLink id="Coin" label="Coins" />, and sell food, iron tools, armour, torches, and the way
		to the nearest dungeon.
	</p>

	<h2>How to trade</h2>
	<ul>
		<li>Use a villager (right click) to open its offers.</li>
		<li>
			A trade takes what it costs from your inventory and gives you the goods. It is refused if you
			cannot pay.
		</li>
		<li>The trade window closes if you move more than <strong>5 blocks</strong> away.</li>
		<li>Your first trade earns the achievement <a href="/achievements">A Fair Price</a>.</li>
	</ul>

	<h3>Coins</h3>
	<p>
		Coins are the only money, and nothing crafts them. Earn them by selling to villagers, or find them:
		shipwreck chests hold 2 to 9 half the time, buried treasure chests 8 to 24, and a fishing treasure
		can be 4 coins. See <a href="/guides/structures">structures</a> and
		<a href="/guides/fishing">fishing</a>.
	</p>

	<h3>Stock and restock</h3>
	<p>
		Each offer has a stock: how many times a villager makes it in a day. Once it is used up, the offer
		waits. Every villager restocks all its offers <strong>at sunrise</strong>. Stock is counted per
		villager, so two smiths sell twice as many swords.
	</p>

	<h2>The offers</h2>
	{#each data.villagers as v (v.id)}
		<h3><ItemLink id={v.id} /></h3>
		<table>
			<thead>
				<tr><th>You give</th><th>You get</th><th class="n">Per day</th></tr>
			</thead>
			<tbody>
				{#each v.trades as t, i (i)}
					<tr>
						<td><ItemLink id={t.cost.item} count={t.cost.count} /></td>
						<td>
							{#if 'item' in t.goods}
								<ItemLink id={t.goods.item} count={t.goods.count} />
							{:else}
								Directions to the nearest dungeon
							{/if}
						</td>
						<td class="n">{t.stock}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/each}

	<h3>Directions to a dungeon</h3>
	<p>
		The cartographer's last offer gives no item: it tells you in the chat where the nearest dungeon is,
		of any kind (ruin, crypt or sanctuary), searched within 2048 blocks. The answer names the kind, the
		distance, one of eight compass points, and for crypts and sanctuaries how many blocks up or down. For
		example: <em>"The nearest dungeon is a crypt, 100 blocks to the north, 40 blocks down"</em>.
	</p>
	<p>
		In a world without dungeons the reply is <em>"No dungeon is known in this world"</em>. Read more
		about dungeons in <a href="/guides/structures">structures</a>.
	</p>

	<h2>Villagers</h2>
	<ul>
		<li>
			By day a villager walks about its home and rests between walks. <strong>At night it goes home and
			stays there.</strong> It walks through doors, and stands still while it trades.
		</li>
		<li>
			Nothing can hurt a villager: blows, arrows and splash potions leave it be. Hostiles ignore
			villagers and hunt only players.
		</li>
	</ul>

	<h3>Professions</h3>
	<p>
		Each small or big house of a village gets one villager, born inside it. Professions go in turn in
		the order the houses are built: farmer, smith, cartographer, then farmer again. A village with three
		houses or more has all three. Other buildings, like the forge, the granary or the fields, have no
		villager.
	</p>
</Page>
