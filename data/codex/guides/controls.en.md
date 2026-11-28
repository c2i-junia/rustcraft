# Controls

Rustcraft is played with the keyboard and the mouse. Every key below is a default: all of them
but Esc can be changed in the settings, and each action can have two keys.

## Mouse

| Button | What it does |
|---|---|
| Left button | Break the block you aim at (hold it down), or hit a creature. Held, it swings again as soon as the weapon has recharged. |
| Right button | Use: place a block, open a chest, furnace or [[CraftingTable]], eat, talk to a villager, draw a [[Bow]], raise a [[Shield]], mount a horse or a boat, sit on a chair. |
| Left Shift + right button | Place a block against a chest, furnace or other block you could open, instead of opening it. On your own [[mob:Donkey]] or [[mob:Mule]], open the chest on its back. |
| Wheel | Choose the next or previous hotbar slot. It goes round from the last slot to the first. |

## Keys

### Movement

| Action | Default key |
|---|---|
| Move forward | W or Up arrow |
| Move backward | S or Down arrow |
| Move left | A or Left arrow |
| Move right | D or Right arrow |
| Jump | Space |
| Sprint | Left Ctrl |
| Toggle flying | F |
| Fly up | Space |
| Sneak / fly down | Left Shift |

### Interface

| Action | Default key |
|---|---|
| Pause | Esc |
| Inventory | E |
| Drop item | Q |
| Chat | T |
| FPS counter | F8 |
| Map | M |
| Zoom in the map | = |
| Zoom out the map | - |

### View

| Action | Default key |
|---|---|
| Camera view | F5 |
| Zoom | C |
| Lower render distance | O |
| Raise render distance | P |

### Debug

| Action | Default key |
|---|---|
| Debug overlay | F3 |
| Chunk borders | F4 |
| Block wireframe | F6 |
| Raycast debug | F7 |
| Reload chunks | R |

### What some of them do

- **Sprint**: press it while walking forward; the sprint lasts while you keep moving forward.
  You cannot sprint with 6 food or less.
- **Sneak** (Left Shift): you walk slowly and cannot fall off an edge. The same key swims down
  in water, flies down in Creative, gets you off a horse or a boat, and stands you up from a
  chair.
- **Toggle flying** (F) only works in [[guide:game-modes|Creative]]. Space both jumps and flies
  up: the two actions share the key on purpose.
- **Drop item** (Q) throws one item of what you hold; with Ctrl, the whole stack.
- **Camera view** (F5) switches between first person and third person.
- **Zoom** (C) zooms in while you hold it.
- **Chat** (T) is also where commands are typed, after a /. While you type one, Tab takes the
  suggestion shown and the Up and Down arrows choose another. See [[guide:commands|Commands]].
- **Pause** (Esc) opens the game menu: settings, your progress and achievements, saving and
  leaving.

## Hotbar

The hotbar is the row of 9 slots at the bottom of the screen. What the selected slot holds is
what you use. The number keys 1 to 9 pick a slot, and the mouse wheel steps through them. The
name of the item shows over the hotbar for a moment when you change slot. The number keys are
fixed: they cannot be rebound.

Your inventory has 36 slots in all: the 9 of the hotbar and 27 more above them.

## In the inventory

| Gesture | What it does |
|---|---|
| Left click | Pick up or put down a whole stack. |
| Right click | Pick up half a stack (rounded up), or put down one item of the stack you hold. |
| Drag a held stack over slots | With the left button, share it out evenly; with the right one, put one item in each slot. |
| Double click | Gather every stack of that item onto the cursor. With Shift, move them all. |
| Shift + click | Move the stack to the other part of the inventory, or onto the armour slot it fits. On a crafting result, craft as many as you can. |
| 1-9 over a slot | Swap it with that hotbar slot. Over a crafting result, craft into that slot. |
| Q over a slot | Throw out one item; Ctrl + Q the whole stack. |
| Click outside the panel holding a stack | With the left button, throw the stack out; with the right one, one item. |

**Inventory tweaks**, on by default, add two gestures: Shift + drag moves every stack you sweep
over, and turning the wheel over a stack moves its items one at a time. They can be switched off
on the Controls tab.

The item browser beside the inventory has its own keys, over an item: R shows its recipes, U its
uses, A bookmarks it, and Ctrl + click fills the crafting grid with its recipe. On a narrow
window the browser is a drawer that B opens and closes.

## Changing a key

Open **Settings** from the main menu or from the game menu (Esc), and go to the **Controls**
tab. Actions are listed in four groups, Movement, Interface, View and Debug, each with a
**Primary** and a **Secondary** key.

1. Click a key slot, or select it and press Enter or Space. It shows "Press a key...".
2. Press the new key. Esc cancels. Shift, Ctrl and Alt can be bound too.
3. To clear a slot, right-click it or press Delete or Backspace on it.

Changes are saved at once. A key bound to two actions is allowed, but both rows show it in red
with a warning, and so does a key that also selects a hotbar slot. The search field at the top
(Ctrl + F) finds actions by name, group or key. **Reset to defaults**, beside Back, gives every
action its default keys again after you confirm.

The same tab holds the **Mouse sensitivity** slider and the **Inventory tweaks** switch. Mouse
buttons cannot be rebound.

## See also

- [[guide:first-day|Your first day]]
- [[guide:combat|Combat]]: blows, recharge, critical hits and shields.
- [[guide:commands|Commands]]
