// What a link to an item or a creature needs to draw: its name, its page and its icon.
import { page } from '$app/state';

export type Ref = {
	kind: 'item' | 'mob';
	id: string;
	href: string;
	name: string;
	/// Path under /textures/, or null for an item with no texture.
	icon: string | null;
	/// The texture of the top of a block drawn as a cube.
	top: string | null;
	/// The colour the game tints a grey texture with, by biome.
	tint: string | null;
	cube: boolean;
	/// The colour of the liquid of a vial.
	potion: string | null;
	/// A creature's portrait, rendered from its model: a URL, drawn smooth.
	picture?: string;
};

/// The ref of an item id, or of a creature as `mob:Sheep`, from the root layout's data.
export function ref(id: string): Ref | undefined {
	return (page.data.refs as Record<string, Ref> | undefined)?.[id];
}
