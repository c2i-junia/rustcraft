<script lang="ts">
	import { Field, FieldLabel } from '@ui-sdk/svelte/field';
	import {
		Select,
		SelectContent,
		SelectItem,
		SelectItemIndicator,
		SelectTrigger,
		SelectValue
	} from '@ui-sdk/svelte/select';

	interface Props {
		label: string;
		items: { value: string; label: string }[];
		value: string;
		onchange: (value: string) => void;
	}

	let { label, items, value, onchange }: Props = $props();
</script>

<Field class="fixed">
	<FieldLabel>{label}</FieldLabel>
	<Select {items} {value} onValueChange={(next) => onchange(String(next ?? items[0].value))}>
		<SelectTrigger>
			<SelectValue />
		</SelectTrigger>
		<SelectContent>
			{#each items as item (item.value)}
				<SelectItem value={item.value}>
					{item.label}
					<SelectItemIndicator />
				</SelectItem>
			{/each}
		</SelectContent>
	</Select>
</Field>
