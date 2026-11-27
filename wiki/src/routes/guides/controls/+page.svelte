<script lang="ts">
	import Page from '#lib/components/Page.svelte';
	import ItemLink from '#lib/components/ItemLink.svelte';

	// The default bindings of the game (client/src/input/keyboard.rs), grouped and named as the
	// Controls tab of the settings lists them
	const BINDINGS: { group: string; rows: [string, string[]][] }[] = [
		{
			group: 'Movement',
			rows: [
				['Move forward', ['W', '↑']],
				['Move backward', ['S', '↓']],
				['Move left', ['A', '←']],
				['Move right', ['D', '→']],
				['Jump', ['Space']],
				['Sprint', ['Left Ctrl']],
				['Toggle flying', ['F']],
				['Fly up', ['Space']],
				['Sneak / fly down', ['Left Shift']]
			]
		},
		{
			group: 'Interface',
			rows: [
				['Pause', ['Esc']],
				['Inventory', ['E']],
				['Drop item', ['Q']],
				['Chat', ['T']],
				['FPS counter', ['F8']],
				['Map', ['M']],
				['Zoom in the map', ['=']],
				['Zoom out the map', ['-']]
			]
		},
		{
			group: 'View',
			rows: [
				['Camera view', ['F5']],
				['Zoom', ['C']],
				['Lower render distance', ['O']],
				['Raise render distance', ['P']]
			]
		},
		{
			group: 'Debug',
			rows: [
				['Debug overlay', ['F3']],
				['Chunk borders', ['F4']],
				['Block wireframe', ['F6']],
				['Raycast debug', ['F7']],
				['Reload chunks', ['R']]
			]
		}
	];
</script>

<Page
	title="Controls"
	description="Every default key and mouse action of Rustcraft, the hotbar and inventory shortcuts, and how to rebind keys in the Controls settings."
>
	<p>
		Rustcraft is played with the keyboard and the mouse. Every key below is a default: all of them
		but <kbd>Esc</kbd> can be changed in the settings, and each action can have two keys.
	</p>

	<h2>Mouse</h2>
	<table>
		<thead><tr><th>Button</th><th>What it does</th></tr></thead>
		<tbody>
			<tr>
				<td>Left button</td>
				<td>Break the block you aim at (hold it down), or hit a creature. Held, it swings again as soon as the weapon has recharged.</td>
			</tr>
			<tr>
				<td>Right button</td>
				<td>
					Use: place a block, open a chest, furnace or <ItemLink id="CraftingTable" />, eat, talk to a
					villager, draw a <ItemLink id="Bow" />, raise a <ItemLink id="Shield" />, mount a horse or a boat, sit on a chair.
				</td>
			</tr>
			<tr>
				<td><kbd>Left Shift</kbd> + right button</td>
				<td>
					Place a block against a chest, furnace or other block you could open, instead of opening it.
					On your own <ItemLink id="Donkey" /> or <ItemLink id="Mule" />, open the chest on its back.
				</td>
			</tr>
			<tr><td>Wheel</td><td>Choose the next or previous hotbar slot. It goes round from the last slot to the first.</td></tr>
		</tbody>
	</table>

	<h2>Keys</h2>
	{#each BINDINGS as { group, rows } (group)}
		<h3>{group}</h3>
		<table>
			<thead><tr><th>Action</th><th>Default key</th></tr></thead>
			<tbody>
				{#each rows as [action, keys] (action)}
					<tr>
						<td>{action}</td>
						<td>
							{#each keys as key, i (key)}{#if i > 0}{' or '}{/if}<kbd>{key}</kbd>{/each}
						</td>
					</tr>
				{/each}
			</tbody>
		</table>
	{/each}

	<h3>What some of them do</h3>
	<ul>
		<li>
			<strong>Sprint</strong>: press it while walking forward; the sprint lasts while you keep moving
			forward. You cannot sprint with 6 food or less.
		</li>
		<li>
			<strong>Sneak</strong> (<kbd>Left Shift</kbd>): you walk slowly and cannot fall off an edge. The
			same key swims down in water, flies down in Creative, gets you off a horse or a boat, and stands
			you up from a chair.
		</li>
		<li>
			<strong>Toggle flying</strong> (<kbd>F</kbd>) only works in <a href="/guides/game-modes">Creative</a>.
			<kbd>Space</kbd> both jumps and flies up: the two actions share the key on purpose.
		</li>
		<li><strong>Drop item</strong> (<kbd>Q</kbd>) throws one item of what you hold; with <kbd>Ctrl</kbd>, the whole stack.</li>
		<li><strong>Camera view</strong> (<kbd>F5</kbd>) switches between first person and third person.</li>
		<li><strong>Zoom</strong> (<kbd>C</kbd>) zooms in while you hold it.</li>
		<li>
			<strong>Chat</strong> (<kbd>T</kbd>) is also where commands are typed, after a <kbd>/</kbd>. While
			you type one, <kbd>Tab</kbd> takes the suggestion shown and <kbd>↑</kbd> <kbd>↓</kbd> choose
			another. See <a href="/guides/commands">Commands</a>.
		</li>
		<li><strong>Pause</strong> (<kbd>Esc</kbd>) opens the game menu: settings, your progress and achievements, saving and leaving.</li>
	</ul>

	<h2>Hotbar</h2>
	<p>
		The hotbar is the row of 9 slots at the bottom of the screen. What the selected slot holds is what you use. The number keys <kbd>1</kbd> to
		<kbd>9</kbd> pick a slot, and the mouse wheel steps through them. The name of the item shows over
		the hotbar for a moment when you change slot. The number keys are fixed: they cannot be rebound.
	</p>
	<p>Your inventory has 36 slots in all: the 9 of the hotbar and 27 more above them.</p>

	<h2>In the inventory</h2>
	<table>
		<thead><tr><th>Gesture</th><th>What it does</th></tr></thead>
		<tbody>
			<tr><td>Left click</td><td>Pick up or put down a whole stack.</td></tr>
			<tr><td>Right click</td><td>Pick up half a stack (rounded up), or put down one item of the stack you hold.</td></tr>
			<tr><td>Drag a held stack over slots</td><td>With the left button, share it out evenly; with the right one, put one item in each slot.</td></tr>
			<tr><td>Double click</td><td>Gather every stack of that item onto the cursor. With <kbd>Shift</kbd>, move them all.</td></tr>
			<tr><td><kbd>Shift</kbd> + click</td><td>Move the stack to the other part of the inventory, or onto the armour slot it fits. On a crafting result, craft as many as you can.</td></tr>
			<tr><td><kbd>1</kbd>–<kbd>9</kbd> over a slot</td><td>Swap it with that hotbar slot. Over a crafting result, craft into that slot.</td></tr>
			<tr><td><kbd>Q</kbd> over a slot</td><td>Throw out one item; <kbd>Ctrl</kbd> + <kbd>Q</kbd> the whole stack.</td></tr>
			<tr><td>Click outside the panel holding a stack</td><td>Throw the stack out.</td></tr>
		</tbody>
	</table>
	<p>
		<strong>Inventory tweaks</strong>, on by default, add two gestures: <kbd>Shift</kbd> + drag moves
		every stack you sweep over, and turning the wheel over a stack moves its items one at a time. They
		can be switched off on the Controls tab.
	</p>
	<p>
		The item browser beside the inventory has its own keys, over an item: <kbd>R</kbd> shows its
		recipes, <kbd>U</kbd> its uses, <kbd>A</kbd> bookmarks it, and <kbd>Ctrl</kbd> + click fills the
		crafting grid with its recipe. On a narrow window the browser is a drawer that <kbd>B</kbd> opens and
		closes.
	</p>

	<h2>Changing a key</h2>
	<p>
		Open <strong>Settings</strong> from the main menu or from the game menu (<kbd>Esc</kbd>), and
		go to the <strong>Controls</strong> tab. Actions are listed in four groups, Movement, Interface,
		View and Debug, each with a <strong>Primary</strong> and a <strong>Secondary</strong> key.
	</p>
	<ol>
		<li>Click a key slot, or select it and press <kbd>Enter</kbd> or <kbd>Space</kbd>. It shows "Press a key...".</li>
		<li>Press the new key. <kbd>Esc</kbd> cancels. <kbd>Shift</kbd>, <kbd>Ctrl</kbd> and <kbd>Alt</kbd> can be bound too.</li>
		<li>To clear a slot, right-click it or press <kbd>Delete</kbd> or <kbd>Backspace</kbd> on it.</li>
	</ol>
	<p>
		Changes are saved at once. A key bound to two actions is allowed, but both rows show it in red
		with a warning, and so does a key that also selects a hotbar slot. The search field at the top
		(<kbd>Ctrl</kbd> + <kbd>F</kbd>) finds actions by name, group or key. <strong>Reset to
			defaults</strong>, beside Back, gives every action its default keys again after you confirm.
	</p>
	<p>
		The same tab holds the <strong>Mouse sensitivity</strong> slider and the
		<strong>Inventory tweaks</strong> switch. Mouse buttons cannot be rebound.
	</p>

	<h2>See also</h2>
	<ul>
		<li><a href="/guides/first-day">Your first day</a></li>
		<li><a href="/guides/combat">Combat</a>: blows, recharge, critical hits and shields.</li>
		<li><a href="/guides/commands">Commands</a></li>
	</ul>
</Page>
